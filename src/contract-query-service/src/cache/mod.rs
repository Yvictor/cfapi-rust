use crate::{ContractKey, ContractView};
use parking_lot::{Mutex, RwLock};
use std::collections::{HashMap, HashSet};
use std::mem::size_of;
use std::sync::Arc;
use thiserror::Error;
use ulid::Ulid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheLimits {
    pub max_records: usize,
    pub max_generation_bytes: usize,
    pub max_overlays: usize,
}

impl Default for CacheLimits {
    fn default() -> Self {
        Self {
            max_records: 100_000,
            max_generation_bytes: 256 * 1024 * 1024,
            max_overlays: 10_000,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Generation {
    id: Ulid,
    source_id: u16,
    complete: bool,
    rows: HashMap<ContractKey, Arc<ContractView>>,
    exchange_code_index: HashMap<(String, String), Vec<ContractKey>>,
    estimated_bytes: usize,
}

impl Generation {
    fn empty(source_id: u16) -> Self {
        Self::build(Ulid::nil(), source_id, false, HashMap::new())
    }

    fn build(
        id: Ulid,
        source_id: u16,
        complete: bool,
        rows: HashMap<ContractKey, Arc<ContractView>>,
    ) -> Self {
        let mut exchange_code_index: HashMap<(String, String), Vec<ContractKey>> = HashMap::new();
        for (key, row) in &rows {
            if let Some(exchange) = &row.metadata.exchange {
                exchange_code_index
                    .entry((exchange.as_str().to_owned(), key.symbol.clone()))
                    .or_default()
                    .push(key.clone());
            }
        }
        for keys in exchange_code_index.values_mut() {
            keys.sort_by(|left, right| {
                left.symbol
                    .cmp(&right.symbol)
                    .then(left.source_id.cmp(&right.source_id))
            });
        }
        let estimated_bytes = estimate_generation_bytes(&rows, &exchange_code_index);
        Self {
            id,
            source_id,
            complete,
            rows,
            exchange_code_index,
            estimated_bytes,
        }
    }

    pub fn id(&self) -> Ulid {
        self.id
    }

    pub fn source_id(&self) -> u16 {
        self.source_id
    }

    pub fn is_complete(&self) -> bool {
        self.complete
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn estimated_bytes(&self) -> usize {
        self.estimated_bytes
    }

    pub fn get(&self, symbol: &str) -> Option<Arc<ContractView>> {
        self.rows
            .get(&ContractKey {
                source_id: self.source_id,
                symbol: symbol.to_owned(),
            })
            .cloned()
    }

    fn indexed_keys(&self, exchange: &str, code: &str) -> &[ContractKey] {
        self.exchange_code_index
            .get(&(exchange.to_owned(), code.to_owned()))
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug)]
enum OverlayValue {
    Upsert(Arc<ContractView>),
    Delete,
}

#[derive(Clone, Debug)]
struct OverlayEntry {
    revision: u64,
    value: OverlayValue,
}

#[derive(Debug)]
struct MutationState {
    revision: u64,
    overlays: HashMap<ContractKey, OverlayEntry>,
    negatives: HashMap<ContractKey, u64>,
    exact_inflight: HashSet<ContractKey>,
}

pub struct SourceCache {
    source_id: u16,
    limits: CacheLimits,
    published: RwLock<Arc<Generation>>,
    mutation: Mutex<MutationState>,
}

impl SourceCache {
    pub fn new(source_id: u16, limits: CacheLimits) -> Self {
        Self {
            source_id,
            limits,
            published: RwLock::new(Arc::new(Generation::empty(source_id))),
            mutation: Mutex::new(MutationState {
                revision: 0,
                overlays: HashMap::new(),
                negatives: HashMap::new(),
                exact_inflight: HashSet::new(),
            }),
        }
    }

    pub fn snapshot(&self) -> Arc<Generation> {
        Arc::clone(&self.published.read())
    }

    pub fn get(&self, symbol: &str) -> Option<Arc<ContractView>> {
        let key = self.key(symbol);
        let mutation = self.mutation.lock();
        match mutation.overlays.get(&key).map(|entry| &entry.value) {
            Some(OverlayValue::Upsert(row)) => Some(Arc::clone(row)),
            Some(OverlayValue::Delete) => None,
            None => self.published.read().rows.get(&key).cloned(),
        }
    }

    pub fn by_exchange_code(&self, exchange: &str, code: &str) -> Vec<Arc<ContractView>> {
        let exchange = exchange.to_ascii_uppercase();
        let mutation = self.mutation.lock();
        let published = self.published.read();
        let mut rows = Vec::new();

        for key in published.indexed_keys(&exchange, code) {
            if !mutation.overlays.contains_key(key) {
                if let Some(row) = published.rows.get(key) {
                    rows.push(Arc::clone(row));
                }
            }
        }
        for (key, entry) in &mutation.overlays {
            let OverlayValue::Upsert(row) = &entry.value else {
                continue;
            };
            if key.symbol == code
                && row
                    .metadata
                    .exchange
                    .as_ref()
                    .is_some_and(|mic| mic.as_str() == exchange)
            {
                rows.push(Arc::clone(row));
            }
        }
        rows.sort_by(|left, right| {
            left.metadata
                .key
                .symbol
                .cmp(&right.metadata.key.symbol)
                .then(
                    left.metadata
                        .key
                        .source_id
                        .cmp(&right.metadata.key.source_id),
                )
        });
        rows
    }

    pub fn exact_upsert(&self, row: Arc<ContractView>) -> Result<u64, CacheError> {
        self.validate_row(&row)?;
        let key = row.metadata.key.clone();
        let mut mutation = self.mutation.lock();
        ensure_overlay_capacity(&mutation, &key, self.limits.max_overlays)?;
        mutation.revision += 1;
        let revision = mutation.revision;
        mutation.overlays.insert(
            key.clone(),
            OverlayEntry {
                revision,
                value: OverlayValue::Upsert(row),
            },
        );
        mutation.negatives.remove(&key);
        Ok(revision)
    }

    pub fn exact_delete(&self, symbol: &str, negative_until: u64) -> Result<u64, CacheError> {
        let key = self.key(symbol);
        let mut mutation = self.mutation.lock();
        ensure_overlay_capacity(&mutation, &key, self.limits.max_overlays)?;
        mutation.revision += 1;
        let revision = mutation.revision;
        mutation.overlays.insert(
            key.clone(),
            OverlayEntry {
                revision,
                value: OverlayValue::Delete,
            },
        );
        mutation.negatives.insert(key, negative_until);
        Ok(revision)
    }

    pub fn is_negative(&self, symbol: &str, now: u64) -> bool {
        self.mutation
            .lock()
            .negatives
            .get(&self.key(symbol))
            .is_some_and(|expires_at| now < *expires_at)
    }

    pub fn begin_sync(&self) -> StagingGeneration {
        let sync_start_revision = self.mutation.lock().revision;
        StagingGeneration {
            source_id: self.source_id,
            sync_start_revision,
            limits: self.limits,
            rows: HashMap::new(),
            estimated_row_bytes: 0,
        }
    }

    pub fn commit_sync(&self, staging: StagingGeneration) -> Result<Arc<Generation>, CacheError> {
        if staging.source_id != self.source_id {
            return Err(CacheError::WrongSource {
                expected: self.source_id,
                actual: staging.source_id,
            });
        }

        let mut mutation = self.mutation.lock();
        let mut rows = staging.rows;
        for (key, entry) in &mutation.overlays {
            if entry.revision <= staging.sync_start_revision {
                continue;
            }
            match &entry.value {
                OverlayValue::Upsert(row) => {
                    rows.insert(key.clone(), Arc::clone(row));
                }
                OverlayValue::Delete => {
                    rows.remove(key);
                }
            }
        }
        validate_record_limit(rows.len(), self.limits.max_records)?;

        let mut generation = Generation::build(Ulid::nil(), self.source_id, true, rows);
        if generation.estimated_bytes > self.limits.max_generation_bytes {
            return Err(CacheError::GenerationByteLimit {
                limit: self.limits.max_generation_bytes,
                actual: generation.estimated_bytes,
            });
        }
        generation.id = Ulid::new();
        let generation = Arc::new(generation);

        *self.published.write() = Arc::clone(&generation);
        mutation.overlays.clear();
        mutation
            .negatives
            .retain(|key, _| !generation.rows.contains_key(key));
        Ok(generation)
    }

    pub fn try_begin_exact(&self, key: &ContractKey) -> bool {
        if key.source_id != self.source_id {
            return false;
        }
        self.mutation.lock().exact_inflight.insert(key.clone())
    }

    pub fn finish_exact(&self, key: &ContractKey) {
        self.mutation.lock().exact_inflight.remove(key);
    }

    fn key(&self, symbol: &str) -> ContractKey {
        ContractKey {
            source_id: self.source_id,
            symbol: symbol.to_owned(),
        }
    }

    fn validate_row(&self, row: &ContractView) -> Result<(), CacheError> {
        let actual = row.metadata.key.source_id;
        if actual != self.source_id {
            return Err(CacheError::WrongSource {
                expected: self.source_id,
                actual,
            });
        }
        Ok(())
    }
}

pub struct StagingGeneration {
    source_id: u16,
    sync_start_revision: u64,
    limits: CacheLimits,
    rows: HashMap<ContractKey, Arc<ContractView>>,
    estimated_row_bytes: usize,
}

impl StagingGeneration {
    pub fn push(&mut self, row: Arc<ContractView>) -> Result<(), StageError> {
        let actual = row.metadata.key.source_id;
        if actual != self.source_id {
            return Err(StageError::WrongSource {
                expected: self.source_id,
                actual,
            });
        }
        let key = row.metadata.key.clone();
        let old_size = self.rows.get(&key).map_or(0, |old| estimate_row_bytes(old));
        let next_bytes = self
            .estimated_row_bytes
            .saturating_sub(old_size)
            .saturating_add(estimate_row_bytes(&row));
        let next_records = self.rows.len() + usize::from(!self.rows.contains_key(&key));
        if next_records > self.limits.max_records {
            return Err(StageError::RecordLimit {
                limit: self.limits.max_records,
            });
        }
        if next_bytes > self.limits.max_generation_bytes {
            return Err(StageError::ByteLimit {
                limit: self.limits.max_generation_bytes,
                actual: next_bytes,
            });
        }
        self.rows.insert(key, row);
        self.estimated_row_bytes = next_bytes;
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FreshnessPolicy {
    pub fresh_for: u64,
    pub max_stale_for: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Consistency {
    CachePreferred,
    FreshRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefreshFailure {
    SessionUnavailable,
    Timeout,
    Backpressure,
    RetryableTransport,
    RefreshFailed,
    PermissionDenied,
    UnsupportedSource,
    NotFound,
    ProtocolViolation,
    ValidationFailed,
}

impl RefreshFailure {
    fn blocks_cached_data(self) -> bool {
        matches!(
            self,
            Self::PermissionDenied | Self::UnsupportedSource | Self::NotFound
        )
    }

    fn permits_stale(self) -> bool {
        matches!(
            self,
            Self::SessionUnavailable
                | Self::Timeout
                | Self::Backpressure
                | Self::RetryableTransport
                | Self::RefreshFailed
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FreshnessDecision {
    Fresh,
    Stale(RefreshFailure),
    Unavailable,
}

pub fn decide_freshness(
    age: u64,
    consistency: Consistency,
    refresh_failure: Option<RefreshFailure>,
    policy: FreshnessPolicy,
) -> FreshnessDecision {
    if refresh_failure.is_some_and(RefreshFailure::blocks_cached_data) {
        return FreshnessDecision::Unavailable;
    }
    if age <= policy.fresh_for {
        return FreshnessDecision::Fresh;
    }
    let Some(failure) = refresh_failure else {
        return FreshnessDecision::Unavailable;
    };
    if consistency == Consistency::CachePreferred
        && age <= policy.max_stale_for
        && failure.permits_stale()
    {
        FreshnessDecision::Stale(failure)
    } else {
        FreshnessDecision::Unavailable
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum StageError {
    #[error("contract belongs to source {actual}, expected {expected}")]
    WrongSource { expected: u16, actual: u16 },
    #[error("staging record limit {limit} exceeded")]
    RecordLimit { limit: usize },
    #[error("staging byte limit {limit} exceeded by {actual} bytes")]
    ByteLimit { limit: usize, actual: usize },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CacheError {
    #[error("contract belongs to source {actual}, expected {expected}")]
    WrongSource { expected: u16, actual: u16 },
    #[error("exact overlay limit {limit} reached")]
    OverlayLimit { limit: usize },
    #[error("generation record limit {limit} exceeded by {actual} records")]
    GenerationRecordLimit { limit: usize, actual: usize },
    #[error("generation byte limit {limit} exceeded by {actual} bytes")]
    GenerationByteLimit { limit: usize, actual: usize },
}

fn ensure_overlay_capacity(
    mutation: &MutationState,
    key: &ContractKey,
    limit: usize,
) -> Result<(), CacheError> {
    if !mutation.overlays.contains_key(key) && mutation.overlays.len() >= limit {
        Err(CacheError::OverlayLimit { limit })
    } else {
        Ok(())
    }
}

fn validate_record_limit(actual: usize, limit: usize) -> Result<(), CacheError> {
    if actual > limit {
        Err(CacheError::GenerationRecordLimit { limit, actual })
    } else {
        Ok(())
    }
}

fn estimate_generation_bytes(
    rows: &HashMap<ContractKey, Arc<ContractView>>,
    index: &HashMap<(String, String), Vec<ContractKey>>,
) -> usize {
    let row_bytes: usize = rows.values().map(|row| estimate_row_bytes(row)).sum();
    let index_bytes: usize = index
        .iter()
        .map(|((exchange, code), keys)| {
            size_of::<(String, String)>()
                + exchange.len()
                + code.len()
                + keys
                    .iter()
                    .map(|key| size_of::<ContractKey>() + key.symbol.len())
                    .sum::<usize>()
        })
        .sum();
    row_bytes.saturating_add(index_bytes)
}

fn estimate_row_bytes(row: &ContractView) -> usize {
    let metadata = &row.metadata;
    size_of::<ContractView>()
        .saturating_add(metadata.key.symbol.len())
        .saturating_add(metadata.name.as_ref().map_or(0, String::len))
        .saturating_add(
            metadata
                .contract_size
                .as_ref()
                .map_or(0, |size| size.raw.len()),
        )
        .saturating_add(
            metadata
                .tick_size
                .as_ref()
                .map_or(0, |schedule| schedule.raw.len()),
        )
        .saturating_add(row.gics.as_ref().map_or(0, |gics| {
            gics.category.len() + gics.sector.len() + gics.industry.len()
        }))
}
