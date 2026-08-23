use crate::domain::{OwnedQueryXrefRow, OwnedTokenValue};
use parking_lot::Mutex;
use std::{
    collections::{HashMap, VecDeque},
    num::NonZeroI64,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

pub type RequestId = u64;
pub type QueryTag = NonZeroI64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryKind {
    Exact,
    WholeSource,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum QueryError {
    #[error("local query concurrency limit reached")]
    LocalBackpressure,
    #[error("CFAPI send queue is full")]
    SendQueueFull,
    #[error("CFAPI session is unavailable")]
    SessionUnavailable,
    #[error("query timed out")]
    Timeout,
    #[error("query was cancelled")]
    Cancelled,
    #[error("contract was not found")]
    NotFound,
    #[error("source permission was denied")]
    PermissionDenied,
    #[error("CFAPI status {code}: {message}")]
    CfapiStatus { code: i32, message: String },
    #[error("query response queue is full")]
    ResponseBackpressure,
    #[error("query generation limit reached")]
    GenerationLimit,
    #[error("CFAPI QueryXref protocol violation: {0}")]
    ProtocolViolation(String),
    #[error("query tag is active or quarantined")]
    TagCollision,
    #[error("query coordinator is shutting down")]
    ShuttingDown,
}

#[derive(Clone, Debug)]
pub struct RegistryLimits {
    pub max_exact: usize,
    pub max_exact_per_source: usize,
    pub max_whole_source: usize,
    pub item_queue_capacity: usize,
    pub max_records: usize,
    pub max_owned_bytes: usize,
    pub tombstone_capacity: usize,
    pub tombstone_ttl: Duration,
}

impl Default for RegistryLimits {
    fn default() -> Self {
        Self {
            max_exact: 256,
            max_exact_per_source: 32,
            max_whole_source: 2,
            item_queue_capacity: 4_096,
            max_records: 100_000,
            max_owned_bytes: 256 * 1024 * 1024,
            tombstone_capacity: 65_536,
            tombstone_ttl: Duration::from_secs(360),
        }
    }
}

pub struct ExactQuery {
    pub request_id: RequestId,
    receiver: oneshot::Receiver<Result<OwnedQueryXrefRow, QueryError>>,
}

impl ExactQuery {
    pub async fn receive(self) -> Result<OwnedQueryXrefRow, QueryError> {
        self.receiver.await.unwrap_or(Err(QueryError::Cancelled))
    }
}

pub struct WholeSourceQuery {
    pub request_id: RequestId,
    pub items: mpsc::Receiver<OwnedQueryXrefRow>,
    completion: oneshot::Receiver<Result<(), QueryError>>,
}

impl WholeSourceQuery {
    pub async fn completion(self) -> Result<(), QueryError> {
        self.completion.await.unwrap_or(Err(QueryError::Cancelled))
    }

    pub async fn drain(mut self) -> Result<Vec<OwnedQueryXrefRow>, QueryError> {
        let mut rows = Vec::new();
        while let Some(row) = self.items.recv().await {
            rows.push(row);
        }
        self.completion
            .await
            .unwrap_or(Err(QueryError::Cancelled))?;
        Ok(rows)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SendDisposition {
    Sent,
    AlreadyTerminal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallbackClassification {
    Delivered,
    Late,
    Duplicate,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PendingState {
    Queued,
    Bound(QueryTag),
    Sent(QueryTag),
}

enum ResponseSink {
    Exact(Mutex<Option<oneshot::Sender<Result<OwnedQueryXrefRow, QueryError>>>>),
    Whole {
        items: mpsc::Sender<OwnedQueryXrefRow>,
        completion: Mutex<Option<oneshot::Sender<Result<(), QueryError>>>>,
    },
}

struct PendingEntry {
    request_id: RequestId,
    source_id: u16,
    kind: QueryKind,
    state: Mutex<PendingState>,
    response: ResponseSink,
    records: AtomicUsize,
    owned_bytes: AtomicUsize,
    delivery: Mutex<WholeDeliveryState>,
}

#[derive(Default)]
struct WholeDeliveryState {
    active_parts: usize,
    terminal: Option<Result<(), QueryError>>,
}

impl PendingEntry {
    fn send_exact(&self, result: Result<OwnedQueryXrefRow, QueryError>) {
        if let ResponseSink::Exact(sender) = &self.response {
            if let Some(sender) = sender.lock().take() {
                let _ = sender.send(result);
            }
        }
    }

    fn deliver_whole(&self, result: Result<(), QueryError>) {
        if let ResponseSink::Whole { completion, .. } = &self.response {
            if let Some(sender) = completion.lock().take() {
                let _ = sender.send(result);
            }
        }
    }

    fn schedule_whole_terminal(&self, result: Result<(), QueryError>) {
        let ready = {
            let mut delivery = self.delivery.lock();
            merge_terminal(&mut delivery.terminal, result);
            (delivery.active_parts == 0)
                .then(|| delivery.terminal.take())
                .flatten()
        };
        if let Some(result) = ready {
            self.deliver_whole(result);
        }
    }

    fn finish_part(&self, error: Option<QueryError>) {
        let ready = {
            let mut delivery = self.delivery.lock();
            debug_assert!(delivery.active_parts > 0);
            delivery.active_parts -= 1;
            if let Some(error) = error {
                merge_terminal(&mut delivery.terminal, Err(error));
            }
            (delivery.active_parts == 0)
                .then(|| delivery.terminal.take())
                .flatten()
        };
        if let Some(result) = ready {
            self.deliver_whole(result);
        }
    }

    fn fail(&self, error: QueryError) {
        match self.kind {
            QueryKind::Exact => self.send_exact(Err(error)),
            QueryKind::WholeSource => self.schedule_whole_terminal(Err(error)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TombstoneReason {
    Late,
    Completed,
}

struct Tombstone {
    tag: QueryTag,
    reason: TombstoneReason,
    expires_at: Instant,
}

#[derive(Default)]
struct RegistryState {
    by_request: HashMap<RequestId, Arc<PendingEntry>>,
    by_tag: HashMap<QueryTag, Arc<PendingEntry>>,
    tombstones: VecDeque<Tombstone>,
    shutting_down: bool,
}

pub struct PendingRegistry {
    limits: RegistryLimits,
    state: Mutex<RegistryState>,
}

impl PendingRegistry {
    pub fn new(limits: RegistryLimits) -> Self {
        assert!(limits.item_queue_capacity > 0);
        assert!(limits.tombstone_capacity > 0);
        Self {
            limits,
            state: Mutex::new(RegistryState::default()),
        }
    }

    pub fn register_exact(
        &self,
        request_id: RequestId,
        source_id: u16,
    ) -> Result<ExactQuery, QueryError> {
        let (sender, receiver) = oneshot::channel();
        let entry = Arc::new(PendingEntry {
            request_id,
            source_id,
            kind: QueryKind::Exact,
            state: Mutex::new(PendingState::Queued),
            response: ResponseSink::Exact(Mutex::new(Some(sender))),
            records: AtomicUsize::new(0),
            owned_bytes: AtomicUsize::new(0),
            delivery: Mutex::new(WholeDeliveryState::default()),
        });
        let mut state = self.state.lock();
        self.prepare_state(&mut state)?;
        let exact_count = state
            .by_request
            .values()
            .filter(|item| item.kind == QueryKind::Exact)
            .count();
        let source_count = state
            .by_request
            .values()
            .filter(|item| item.kind == QueryKind::Exact && item.source_id == source_id)
            .count();
        if exact_count >= self.limits.max_exact
            || source_count >= self.limits.max_exact_per_source
            || state.by_request.contains_key(&request_id)
        {
            return Err(QueryError::LocalBackpressure);
        }
        state.by_request.insert(request_id, entry);
        Ok(ExactQuery {
            request_id,
            receiver,
        })
    }

    pub fn register_whole_source(
        &self,
        request_id: RequestId,
        source_id: u16,
    ) -> Result<WholeSourceQuery, QueryError> {
        let (item_sender, items) = mpsc::channel(self.limits.item_queue_capacity);
        let (completion_sender, completion) = oneshot::channel();
        let entry = Arc::new(PendingEntry {
            request_id,
            source_id,
            kind: QueryKind::WholeSource,
            state: Mutex::new(PendingState::Queued),
            response: ResponseSink::Whole {
                items: item_sender,
                completion: Mutex::new(Some(completion_sender)),
            },
            records: AtomicUsize::new(0),
            owned_bytes: AtomicUsize::new(0),
            delivery: Mutex::new(WholeDeliveryState::default()),
        });
        let mut state = self.state.lock();
        self.prepare_state(&mut state)?;
        let whole_count = state
            .by_request
            .values()
            .filter(|item| item.kind == QueryKind::WholeSource)
            .count();
        let source_active = state
            .by_request
            .values()
            .any(|item| item.kind == QueryKind::WholeSource && item.source_id == source_id);
        if whole_count >= self.limits.max_whole_source
            || source_active
            || state.by_request.contains_key(&request_id)
        {
            return Err(QueryError::LocalBackpressure);
        }
        state.by_request.insert(request_id, entry);
        Ok(WholeSourceQuery {
            request_id,
            items,
            completion,
        })
    }

    pub fn bind(&self, request_id: RequestId, tag: QueryTag) -> Result<(), QueryError> {
        let mut state = self.state.lock();
        self.prune_tombstones(&mut state, Instant::now());
        if state.by_tag.contains_key(&tag) || state.tombstones.iter().any(|item| item.tag == tag) {
            return Err(QueryError::TagCollision);
        }
        let entry = {
            let entry = state
                .by_request
                .get(&request_id)
                .ok_or(QueryError::Cancelled)?;
            let mut pending_state = entry.state.lock();
            if *pending_state != PendingState::Queued {
                return Err(QueryError::ProtocolViolation(
                    "request was bound more than once".to_owned(),
                ));
            }
            *pending_state = PendingState::Bound(tag);
            Arc::clone(entry)
        };
        state.by_tag.insert(tag, entry);
        Ok(())
    }

    pub fn mark_sent(
        &self,
        request_id: RequestId,
        tag: QueryTag,
    ) -> Result<SendDisposition, QueryError> {
        let mut state = self.state.lock();
        if let Some(entry) = state.by_request.get(&request_id) {
            let mut pending_state = entry.state.lock();
            if *pending_state != PendingState::Bound(tag) {
                return Err(QueryError::ProtocolViolation(
                    "sent tag does not match bound tag".to_owned(),
                ));
            }
            *pending_state = PendingState::Sent(tag);
            return Ok(SendDisposition::Sent);
        }
        self.prune_tombstones(&mut state, Instant::now());
        if state.tombstones.iter().any(|item| item.tag == tag) {
            Ok(SendDisposition::AlreadyTerminal)
        } else {
            Err(QueryError::ProtocolViolation(
                "sent request is no longer registered".to_owned(),
            ))
        }
    }

    pub fn rollback_send_queue_full(&self, request_id: RequestId, tag: QueryTag) -> bool {
        self.terminal_by_request(request_id, Some(tag), QueryError::SendQueueFull)
    }

    pub fn cancel(&self, request_id: RequestId) -> bool {
        self.terminal_by_request(request_id, None, QueryError::Cancelled)
    }

    pub fn timeout(&self, request_id: RequestId) -> bool {
        self.terminal_by_request(request_id, None, QueryError::Timeout)
    }

    pub fn source_for_active_tag(&self, tag: QueryTag) -> Result<u16, CallbackClassification> {
        let mut state = self.state.lock();
        self.prune_tombstones(&mut state, Instant::now());
        state
            .by_tag
            .get(&tag)
            .map(|entry| entry.source_id)
            .ok_or_else(|| self.classify_locked(&state, tag))
    }

    pub fn on_image_part(&self, tag: QueryTag, row: OwnedQueryXrefRow) -> CallbackClassification {
        let entry = match self.lookup_part(tag) {
            Ok(entry) => entry,
            Err(classification) => return classification,
        };
        if entry.kind != QueryKind::WholeSource {
            self.terminal_by_tag(
                tag,
                QueryError::ProtocolViolation("exact query received IMAGE_PART".to_owned()),
            );
            return CallbackClassification::Delivered;
        }
        if !self.reserve_row(&entry, &row) {
            let terminal_claimed = self.terminal_by_tag(tag, QueryError::GenerationLimit);
            entry.finish_part((!terminal_claimed).then_some(QueryError::GenerationLimit));
            return CallbackClassification::Delivered;
        }
        let result = match &entry.response {
            ResponseSink::Whole { items, .. } => items.try_send(row),
            ResponseSink::Exact(_) => unreachable!(),
        };
        if result.is_err() {
            let terminal_claimed = self.terminal_by_tag(tag, QueryError::ResponseBackpressure);
            entry.finish_part((!terminal_claimed).then_some(QueryError::ResponseBackpressure));
        } else {
            entry.finish_part(None);
        }
        CallbackClassification::Delivered
    }

    pub fn on_image_complete(
        &self,
        tag: QueryTag,
        final_row: Option<OwnedQueryXrefRow>,
    ) -> CallbackClassification {
        let entry = match self.claim_terminal(tag, TombstoneReason::Completed) {
            Ok(entry) => entry,
            Err(classification) => return classification,
        };
        match entry.kind {
            QueryKind::Exact => match final_row {
                Some(row) => entry.send_exact(Ok(row)),
                None => entry.send_exact(Err(QueryError::ProtocolViolation(
                    "exact IMAGE_COMPLETE did not contain a row".to_owned(),
                ))),
            },
            QueryKind::WholeSource => {
                if let Some(row) = final_row {
                    if !self.reserve_row(&entry, &row) {
                        entry.schedule_whole_terminal(Err(QueryError::GenerationLimit));
                        return CallbackClassification::Delivered;
                    }
                    if let ResponseSink::Whole { items, .. } = &entry.response {
                        if items.try_send(row).is_err() {
                            entry.schedule_whole_terminal(Err(QueryError::ResponseBackpressure));
                            return CallbackClassification::Delivered;
                        }
                    }
                }
                entry.schedule_whole_terminal(Ok(()));
            }
        }
        CallbackClassification::Delivered
    }

    pub fn on_status(
        &self,
        tag: QueryTag,
        code: i32,
        message: impl Into<String>,
    ) -> CallbackClassification {
        let error = match code {
            14 => QueryError::NotFound,
            -12 => QueryError::PermissionDenied,
            code => QueryError::CfapiStatus {
                code,
                message: message.into(),
            },
        };
        if self.terminal_by_tag(tag, error) {
            CallbackClassification::Delivered
        } else {
            self.classify_missing(tag)
        }
    }

    pub fn on_unexpected_event(
        &self,
        tag: QueryTag,
        event: &'static str,
    ) -> CallbackClassification {
        if self.terminal_by_tag(
            tag,
            QueryError::ProtocolViolation(format!("unexpected {event} event")),
        ) {
            CallbackClassification::Delivered
        } else {
            self.classify_missing(tag)
        }
    }

    pub fn fail_source(&self, source_id: u16, error: QueryError) -> usize {
        self.fail_matching(error, |entry| entry.source_id == source_id)
    }

    pub fn fail_all(&self, error: QueryError) -> usize {
        self.fail_matching(error, |_| true)
    }

    pub fn shutdown(&self) -> usize {
        {
            let mut state = self.state.lock();
            state.shutting_down = true;
        }
        self.fail_all(QueryError::ShuttingDown)
    }

    pub fn pending_count(&self) -> usize {
        self.state.lock().by_request.len()
    }

    fn prepare_state(&self, state: &mut RegistryState) -> Result<(), QueryError> {
        self.prune_tombstones(state, Instant::now());
        if state.shutting_down {
            Err(QueryError::ShuttingDown)
        } else {
            Ok(())
        }
    }

    fn lookup_part(&self, tag: QueryTag) -> Result<Arc<PendingEntry>, CallbackClassification> {
        let mut state = self.state.lock();
        self.prune_tombstones(&mut state, Instant::now());
        let entry = state
            .by_tag
            .get(&tag)
            .cloned()
            .ok_or_else(|| self.classify_locked(&state, tag))?;
        if entry.kind == QueryKind::WholeSource {
            entry.delivery.lock().active_parts += 1;
        }
        Ok(entry)
    }

    fn claim_terminal(
        &self,
        tag: QueryTag,
        reason: TombstoneReason,
    ) -> Result<Arc<PendingEntry>, CallbackClassification> {
        let mut state = self.state.lock();
        self.prune_tombstones(&mut state, Instant::now());
        let Some(entry) = state.by_tag.remove(&tag) else {
            return Err(self.classify_locked(&state, tag));
        };
        state.by_request.remove(&entry.request_id);
        self.add_tombstone(&mut state, tag, reason);
        Ok(entry)
    }

    fn terminal_by_tag(&self, tag: QueryTag, error: QueryError) -> bool {
        let reason = tombstone_reason(&error);
        let entry = self.claim_terminal(tag, reason).ok();
        if let Some(entry) = entry {
            entry.fail(error);
            true
        } else {
            false
        }
    }

    fn terminal_by_request(
        &self,
        request_id: RequestId,
        expected_tag: Option<QueryTag>,
        error: QueryError,
    ) -> bool {
        let entry = {
            let mut state = self.state.lock();
            let Some(entry) = state.by_request.get(&request_id).cloned() else {
                return false;
            };
            let bound_tag = match *entry.state.lock() {
                PendingState::Queued => None,
                PendingState::Bound(tag) | PendingState::Sent(tag) => Some(tag),
            };
            if expected_tag.is_some() && expected_tag != bound_tag {
                return false;
            }
            state.by_request.remove(&request_id);
            if let Some(tag) = bound_tag {
                state.by_tag.remove(&tag);
                self.add_tombstone(&mut state, tag, tombstone_reason(&error));
            }
            entry
        };
        entry.fail(error);
        true
    }

    fn fail_matching<F>(&self, error: QueryError, predicate: F) -> usize
    where
        F: Fn(&PendingEntry) -> bool,
    {
        let entries = {
            let mut state = self.state.lock();
            let ids: Vec<_> = state
                .by_request
                .values()
                .filter(|entry| predicate(entry))
                .map(|entry| entry.request_id)
                .collect();
            ids.into_iter()
                .filter_map(|id| {
                    let entry = state.by_request.remove(&id)?;
                    let tag = match *entry.state.lock() {
                        PendingState::Queued => None,
                        PendingState::Bound(tag) | PendingState::Sent(tag) => Some(tag),
                    };
                    if let Some(tag) = tag {
                        state.by_tag.remove(&tag);
                        self.add_tombstone(&mut state, tag, tombstone_reason(&error));
                    }
                    Some(entry)
                })
                .collect::<Vec<_>>()
        };
        let count = entries.len();
        for entry in entries {
            entry.fail(error.clone());
        }
        count
    }

    fn reserve_row(&self, entry: &PendingEntry, row: &OwnedQueryXrefRow) -> bool {
        let records = entry.records.fetch_add(1, Ordering::Relaxed) + 1;
        let row_bytes = owned_row_bytes(row);
        let bytes = entry.owned_bytes.fetch_add(row_bytes, Ordering::Relaxed) + row_bytes;
        records <= self.limits.max_records && bytes <= self.limits.max_owned_bytes
    }

    fn classify_missing(&self, tag: QueryTag) -> CallbackClassification {
        let mut state = self.state.lock();
        self.prune_tombstones(&mut state, Instant::now());
        self.classify_locked(&state, tag)
    }

    fn classify_locked(&self, state: &RegistryState, tag: QueryTag) -> CallbackClassification {
        match state.tombstones.iter().find(|item| item.tag == tag) {
            Some(Tombstone {
                reason: TombstoneReason::Late,
                ..
            }) => CallbackClassification::Late,
            Some(_) => CallbackClassification::Duplicate,
            None => CallbackClassification::Unknown,
        }
    }

    fn add_tombstone(&self, state: &mut RegistryState, tag: QueryTag, reason: TombstoneReason) {
        while state.tombstones.len() >= self.limits.tombstone_capacity {
            state.tombstones.pop_front();
        }
        state.tombstones.push_back(Tombstone {
            tag,
            reason,
            expires_at: Instant::now() + self.limits.tombstone_ttl,
        });
    }

    fn prune_tombstones(&self, state: &mut RegistryState, now: Instant) {
        state.tombstones.retain(|item| item.expires_at > now);
    }
}

fn tombstone_reason(error: &QueryError) -> TombstoneReason {
    match error {
        QueryError::SendQueueFull
        | QueryError::SessionUnavailable
        | QueryError::Timeout
        | QueryError::Cancelled
        | QueryError::ResponseBackpressure
        | QueryError::GenerationLimit
        | QueryError::ShuttingDown => TombstoneReason::Late,
        QueryError::LocalBackpressure
        | QueryError::NotFound
        | QueryError::PermissionDenied
        | QueryError::CfapiStatus { .. }
        | QueryError::ProtocolViolation(_)
        | QueryError::TagCollision => TombstoneReason::Completed,
    }
}

fn merge_terminal(current: &mut Option<Result<(), QueryError>>, incoming: Result<(), QueryError>) {
    if current.is_none() || matches!((&*current, &incoming), (Some(Ok(())), Err(_))) {
        *current = Some(incoming);
    }
}

fn owned_row_bytes(row: &OwnedQueryXrefRow) -> usize {
    row.symbol.len()
        + row
            .tokens
            .iter()
            .map(|token| {
                std::mem::size_of_val(token)
                    + match &token.value {
                        OwnedTokenValue::String(value) => value.len(),
                        OwnedTokenValue::Integer(_) | OwnedTokenValue::Decimal(_) => 0,
                    }
            })
            .sum::<usize>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_flight_part_error_overrides_deferred_terminal_success() {
        let (items, _receiver) = mpsc::channel(1);
        let (completion, mut result) = oneshot::channel();
        let entry = PendingEntry {
            request_id: 1,
            source_id: 533,
            kind: QueryKind::WholeSource,
            state: Mutex::new(PendingState::Sent(QueryTag::new(1).unwrap())),
            response: ResponseSink::Whole {
                items,
                completion: Mutex::new(Some(completion)),
            },
            records: AtomicUsize::new(0),
            owned_bytes: AtomicUsize::new(0),
            delivery: Mutex::new(WholeDeliveryState {
                active_parts: 1,
                terminal: None,
            }),
        };

        entry.schedule_whole_terminal(Ok(()));
        assert_eq!(result.try_recv(), Err(oneshot::error::TryRecvError::Empty));

        entry.finish_part(Some(QueryError::ResponseBackpressure));
        assert_eq!(result.try_recv(), Ok(Err(QueryError::ResponseBackpressure)));
    }
}
