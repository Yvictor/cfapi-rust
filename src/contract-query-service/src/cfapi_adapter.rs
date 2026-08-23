use cfapi::{
    api::{PrepareQueryXrefError, QueryXrefSendError, CFAPI},
    binding::{MessageEvent, MessageEvent_Types},
    event_reader::{EventReader, EventReaderSerConfig},
    message_event::MessageEventHandlerExt,
    value::CFValue,
};
use contract_query_service::{
    query::{CallbackClassification, PendingRegistry, QueryError, QueryTag},
    service::{CfapiCommandOwner, OwnerExecuteError, OwnerQuery},
    OwnedQueryXrefRow, OwnedToken, OwnedTokenValue,
};
use rust_decimal::Decimal;
use std::{
    num::NonZeroI64,
    sync::{
        mpsc::{self, Receiver, SyncSender},
        Arc,
    },
    thread::{self, JoinHandle},
};
use time::OffsetDateTime;

/// Safe owner for a CFAPI instance that is created and retained on one native
/// owner thread. CFAPI itself contains `Rc` callback owners and cannot safely be
/// moved between threads, so construction happens inside the spawned thread.
pub struct CfapiAdapter {
    commands: Option<mpsc::Sender<ThreadCommand>>,
    owner_thread: Option<JoinHandle<()>>,
}

impl CfapiAdapter {
    pub fn spawn<F>(factory: F) -> Result<Self, QueryError>
    where
        F: FnOnce() -> CFAPI + Send + 'static,
    {
        let (commands, receiver) = mpsc::channel();
        let owner_thread = thread::Builder::new()
            .name("cfapi-query-xref-native-owner".to_owned())
            .spawn(move || run_cfapi_owner(factory(), receiver))
            .map_err(|error| {
                QueryError::ProtocolViolation(format!(
                    "failed to start native CFAPI owner thread: {error}"
                ))
            })?;
        Ok(Self {
            commands: Some(commands),
            owner_thread: Some(owner_thread),
        })
    }
}

impl Drop for CfapiAdapter {
    fn drop(&mut self) {
        self.commands.take();
        if let Some(owner_thread) = self.owner_thread.take() {
            let _ = owner_thread.join();
        }
    }
}

impl CfapiCommandOwner for CfapiAdapter {
    fn execute(
        &mut self,
        query: &OwnerQuery,
        bind: &mut dyn FnMut(QueryTag) -> Result<(), QueryError>,
    ) -> Result<QueryTag, OwnerExecuteError> {
        let (prepared_tx, prepared_rx) = mpsc::sync_channel(1);
        self.commands
            .as_ref()
            .ok_or(QueryError::SessionUnavailable)?
            .send(ThreadCommand {
                query: ThreadQuery::from(query),
                prepared_tx,
            })
            .map_err(|_| QueryError::SessionUnavailable)?;

        let prepared = prepared_rx
            .recv()
            .map_err(|_| QueryError::SessionUnavailable)??;
        let tag = prepared.tag;
        if let Err(error) = bind(tag) {
            let _ = prepared.continue_tx.send(Continuation::Abort);
            return Err(OwnerExecuteError::Query(error));
        }
        prepared
            .continue_tx
            .send(Continuation::Send)
            .map_err(|_| QueryError::SessionUnavailable)?;
        match prepared
            .result_rx
            .recv()
            .map_err(|_| QueryError::SessionUnavailable)?
        {
            Ok(returned_tag) => Ok(returned_tag),
            Err(QueryXrefSendError::SendQueueFull { prepared_tag }) => {
                Err(OwnerExecuteError::SendQueueFull { tag: prepared_tag })
            }
            Err(QueryXrefSendError::TagMismatch {
                prepared_tag,
                returned_tag,
            }) => Err(OwnerExecuteError::Query(QueryError::ProtocolViolation(
                format!("CFAPI returned query tag {returned_tag} for prepared tag {prepared_tag}"),
            ))),
        }
    }
}

struct ThreadCommand {
    query: ThreadQuery,
    prepared_tx: SyncSender<Result<PreparedPhase, OwnerExecuteError>>,
}

struct ThreadQuery {
    source_id: String,
    symbol: Option<String>,
}

impl From<&OwnerQuery> for ThreadQuery {
    fn from(query: &OwnerQuery) -> Self {
        match query {
            OwnerQuery::Exact { source_id, symbol } => Self {
                source_id: source_id.to_string(),
                symbol: Some(symbol.clone()),
            },
            OwnerQuery::WholeSource { source_id } => Self {
                source_id: source_id.to_string(),
                symbol: None,
            },
        }
    }
}

struct PreparedPhase {
    tag: QueryTag,
    continue_tx: SyncSender<Continuation>,
    result_rx: Receiver<Result<QueryTag, QueryXrefSendError>>,
}

enum Continuation {
    Send,
    Abort,
}

fn run_cfapi_owner(mut api: CFAPI, commands: Receiver<ThreadCommand>) {
    while let Ok(command) = commands.recv() {
        let prepared = match api
            .prepare_query_xref(&command.query.source_id, command.query.symbol.as_deref())
        {
            Ok(prepared) => prepared,
            Err(PrepareQueryXrefError::ZeroTag) => {
                let _ = command.prepared_tx.send(Err(OwnerExecuteError::Query(
                    QueryError::ProtocolViolation(
                        "CFAPI generated a zero QueryXref tag".to_owned(),
                    ),
                )));
                continue;
            }
        };
        let tag = prepared.tag();
        let (continue_tx, continue_rx) = mpsc::sync_channel(1);
        let (result_tx, result_rx) = mpsc::sync_channel(1);
        if command
            .prepared_tx
            .send(Ok(PreparedPhase {
                tag,
                continue_tx,
                result_rx,
            }))
            .is_err()
        {
            continue;
        }
        match continue_rx.recv() {
            Ok(Continuation::Send) => {
                let _ = result_tx.send(prepared.send());
            }
            Ok(Continuation::Abort) | Err(_) => drop(prepared),
        }
    }
}

#[derive(Clone)]
pub struct QueryXrefEventBridge {
    registry: Arc<PendingRegistry>,
}

impl QueryXrefEventBridge {
    pub fn new(registry: Arc<PendingRegistry>) -> Self {
        Self { registry }
    }

    pub fn handle_event(&self, event: &MessageEvent) -> CallbackClassification {
        let Some(tag) = NonZeroI64::new(event.getTag()) else {
            return CallbackClassification::Unknown;
        };
        match event.getType() as MessageEvent_Types {
            MessageEvent_Types::IMAGE_PART => match self.registry.source_for_active_tag(tag) {
                Ok(source_id) => match owned_row_from_event(event, source_id) {
                    Ok(Some(row)) => self.registry.on_image_part(tag, row),
                    Ok(None) | Err(_) => self
                        .registry
                        .on_unexpected_event(tag, "invalid QueryXref IMAGE_PART"),
                },
                Err(classification) => classification,
            },
            MessageEvent_Types::IMAGE_COMPLETE => match self.registry.source_for_active_tag(tag) {
                Ok(source_id) => match owned_row_from_event(event, source_id) {
                    Ok(row) => self.registry.on_image_complete(tag, row),
                    Err(_) => self
                        .registry
                        .on_unexpected_event(tag, "invalid QueryXref IMAGE_COMPLETE"),
                },
                Err(classification) => classification,
            },
            MessageEvent_Types::STATUS => self.registry.on_status(
                tag,
                i32::from(event.getStatusCode()),
                event.getStatusString().to_string(),
            ),
            MessageEvent_Types::REFRESH => self.registry.on_unexpected_event(tag, "REFRESH"),
            MessageEvent_Types::UPDATE => self.registry.on_unexpected_event(tag, "UPDATE"),
        }
    }
}

impl MessageEventHandlerExt for QueryXrefEventBridge {
    fn on_message_event(&self, event: &MessageEvent) {
        self.handle_event(event);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventConversionError {
    InvalidSource(i32),
    InvalidTokenNumber(i32),
    NonFiniteDecimal { token: u16 },
}

pub fn owned_token_from_cf_value(
    number: i32,
    value: CFValue,
) -> Result<Option<OwnedToken>, EventConversionError> {
    let number =
        u16::try_from(number).map_err(|_| EventConversionError::InvalidTokenNumber(number))?;
    let value = match value {
        CFValue::String(value) => OwnedTokenValue::String(value),
        CFValue::Int(value) => OwnedTokenValue::Integer(value),
        CFValue::Double(value) | CFValue::Datetime(value) => {
            let value = Decimal::from_f64_retain(value)
                .ok_or(EventConversionError::NonFiniteDecimal { token: number })?;
            OwnedTokenValue::Decimal(value)
        }
        CFValue::Unknown => return Ok(None),
    };
    Ok(Some(OwnedToken { number, value }))
}

pub fn build_owned_query_xref_row(
    source_id: i32,
    symbol: String,
    values: impl IntoIterator<Item = (i32, CFValue)>,
    observed_at: OffsetDateTime,
) -> Result<Option<OwnedQueryXrefRow>, EventConversionError> {
    let source_id =
        u16::try_from(source_id).map_err(|_| EventConversionError::InvalidSource(source_id))?;
    let mut tokens = Vec::new();
    for (number, value) in values {
        if let Some(token) = owned_token_from_cf_value(number, value)? {
            tokens.push(token);
        }
    }
    if tokens.is_empty() {
        return Ok(None);
    }
    Ok(Some(OwnedQueryXrefRow {
        source_id,
        symbol,
        tokens,
        observed_at,
    }))
}

fn owned_row_from_event(
    event: &MessageEvent,
    source_id: u16,
) -> Result<Option<OwnedQueryXrefRow>, EventConversionError> {
    let config = EventReaderSerConfig::default();
    let mut reader = EventReader::new(event, &config);
    let values = std::iter::from_fn(|| reader.next_with_token_number());
    build_owned_query_xref_row(
        i32::from(source_id),
        event.getSymbol().to_string(),
        values,
        OffsetDateTime::now_utc(),
    )
}
