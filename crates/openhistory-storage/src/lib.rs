//! Storage contracts and a deterministic in-memory backend for core and UI tests.
//!
//! The production `SQLCipher` backend is introduced behind this contract so no unencrypted fallback
//! can be selected accidentally.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use chrono::{DateTime, Duration, FixedOffset, Utc};
use openhistory_domain::{EntityKey, EventEnvelope, SemanticPayload, normalize_event_order};
use openhistory_segmentation::TaskSegment;
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

const DATABASE_KEY_BYTES: usize = 32;
const DATABASE_MIGRATIONS: &str = r"
BEGIN IMMEDIATE;
CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS raw_events (
    event_id TEXT PRIMARY KEY,
    occurred_at TEXT NOT NULL,
    monotonic_ticks INTEGER NOT NULL,
    event_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS task_segments (
    segment_id TEXT PRIMARY KEY,
    started_at TEXT NOT NULL,
    ended_at TEXT NOT NULL,
    segment_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS opted_in_repositories (
    root_path TEXT PRIMARY KEY,
    added_at TEXT NOT NULL
);
INSERT OR IGNORE INTO schema_migrations(version, applied_at)
VALUES (1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
COMMIT;
";

/// Storage operation failure without key or content disclosure.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum StorageError {
    /// Requested encrypted backend is not initialized.
    #[error("encrypted storage is not initialized")]
    NotInitialized,
    /// Transaction was aborted and all changes rolled back.
    #[error("transaction was rolled back")]
    RolledBack,
    /// The supplied key is missing or not a 256-bit hexadecimal value.
    #[error("database key is invalid")]
    InvalidKey,
    /// The existing database could not be decrypted using the supplied key.
    #[error("database key was rejected")]
    DatabaseKeyRejected,
    /// `SQLCipher` support is unavailable in the linked `SQLite` library.
    #[error("SQLCipher is unavailable")]
    SqlCipherUnavailable,
    /// A database operation failed without exposing statement or secret details.
    #[error("encrypted database operation failed")]
    Database,
    /// The operating-system credential store could not be accessed.
    #[error("credential store is unavailable")]
    CredentialStore,
}

/// A validated 256-bit `SQLCipher` key that erases its allocation on drop.
pub struct DatabaseKey(Zeroizing<String>);

impl DatabaseKey {
    /// Generates a fresh key using the operating system randomness used by UUID v4.
    #[must_use]
    pub fn generate() -> Self {
        let value = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        Self(Zeroizing::new(value))
    }

    /// Validates a lower- or upper-case hexadecimal key.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::InvalidKey`] unless `value` contains exactly 32 bytes encoded as
    /// hexadecimal.
    pub fn parse(value: &str) -> Result<Self, StorageError> {
        if value.len() != DATABASE_KEY_BYTES * 2
            || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(StorageError::InvalidKey);
        }
        Ok(Self(Zeroizing::new(value.to_ascii_lowercase())))
    }

    fn sqlcipher_literal(&self) -> Zeroizing<String> {
        Zeroizing::new(format!("x'{}'", self.0.as_str()))
    }

    fn expose_for_store(&self) -> &str {
        self.0.as_str()
    }
}

impl Drop for DatabaseKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// Minimal secure-store boundary used to retrieve the database key.
pub trait DatabaseKeyProvider {
    /// Loads the key, returning `None` only when it has never been created.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::CredentialStore`] when the secure store is inaccessible.
    fn load(&self) -> Result<Option<DatabaseKey>, StorageError>;

    /// Persists a newly generated key.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::CredentialStore`] when the secure store rejects the write.
    fn save(&self, key: &DatabaseKey) -> Result<(), StorageError>;
}

/// Keychain/Credential Manager key provider for supported desktop targets.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub struct OsDatabaseKeyProvider {
    account: String,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl OsDatabaseKeyProvider {
    /// Creates a provider scoped to one local profile identifier.
    #[must_use]
    pub fn new(account: impl Into<String>) -> Self {
        Self {
            account: account.into(),
        }
    }

    fn entry(&self) -> Result<keyring::Entry, StorageError> {
        keyring::Entry::new("app.openhistory.database", &self.account)
            .map_err(|_| StorageError::CredentialStore)
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl DatabaseKeyProvider for OsDatabaseKeyProvider {
    fn load(&self) -> Result<Option<DatabaseKey>, StorageError> {
        match self.entry()?.get_password() {
            Ok(value) => DatabaseKey::parse(&value).map(Some),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(StorageError::CredentialStore),
        }
    }

    fn save(&self, key: &DatabaseKey) -> Result<(), StorageError> {
        self.entry()?
            .set_password(key.expose_for_store())
            .map_err(|_| StorageError::CredentialStore)
    }
}

/// An initialized `SQLCipher` connection. There is deliberately no plaintext fallback.
pub struct EncryptedDatabase {
    connection: Connection,
    path: PathBuf,
}

impl EncryptedDatabase {
    /// Loads or creates a key, opens `SQLCipher`, enables WAL, and applies migrations.
    ///
    /// # Errors
    ///
    /// Returns a non-sensitive [`StorageError`] when key retrieval, decryption, or migration fails.
    pub fn open_or_create(
        path: impl AsRef<Path>,
        provider: &impl DatabaseKeyProvider,
    ) -> Result<Self, StorageError> {
        let key = if let Some(existing) = provider.load()? {
            existing
        } else {
            let generated = DatabaseKey::generate();
            provider.save(&generated)?;
            generated
        };
        Self::open(path, &key)
    }

    /// Opens a `SQLCipher` database using an explicit validated key.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::DatabaseKeyRejected`] for an existing database encrypted with a
    /// different key, and never retries with plaintext `SQLite`.
    pub fn open(path: impl AsRef<Path>, key: &DatabaseKey) -> Result<Self, StorageError> {
        let path = path.as_ref();
        let existing = path.metadata().is_ok_and(|metadata| metadata.len() > 0);
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )
        .map_err(|_| StorageError::Database)?;

        let key_literal = key.sqlcipher_literal();
        connection
            .pragma_update(None, "key", key_literal.as_str())
            .map_err(|_| StorageError::DatabaseKeyRejected)?;

        let cipher_version = connection
            .query_row("PRAGMA cipher_version", [], |row| row.get::<_, String>(0))
            .map_err(|_| StorageError::SqlCipherUnavailable)?;
        if cipher_version.trim().is_empty() {
            return Err(StorageError::SqlCipherUnavailable);
        }

        if existing {
            connection
                .query_row("SELECT count(*) FROM sqlite_master", [], |row| {
                    row.get::<_, i64>(0)
                })
                .map_err(|_| StorageError::DatabaseKeyRejected)?;
        }

        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(|_| StorageError::Database)?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(|_| StorageError::Database)?;
        connection
            .execute_batch(DATABASE_MIGRATIONS)
            .map_err(|_| StorageError::Database)?;

        Ok(Self {
            connection,
            path: path.to_path_buf(),
        })
    }

    /// Returns the database path without exposing key material.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the latest applied schema migration.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Database`] when the encrypted schema cannot be read.
    pub fn schema_version(&self) -> Result<u32, StorageError> {
        self.connection
            .query_row("SELECT max(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .map_err(|_| StorageError::Database)
    }

    /// Returns the configured journal mode for verification and diagnostics.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Database`] when the pragma cannot be read.
    pub fn journal_mode(&self) -> Result<String, StorageError> {
        self.connection
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .map_err(|_| StorageError::Database)
    }
}

impl HistoryRepository for EncryptedDatabase {
    fn insert_event(&mut self, event: EventEnvelope) -> Result<(), StorageError> {
        let event_json = serde_json::to_string(&event).map_err(|_| StorageError::Database)?;
        self.connection
            .execute(
                "INSERT OR IGNORE INTO raw_events(event_id, occurred_at, monotonic_ticks, event_json) VALUES (?1, ?2, ?3, ?4)",
                (
                    event.event_id.to_string(),
                    event.occurred_at.with_timezone(&Utc).to_rfc3339(),
                    i64::try_from(event.monotonic_ticks).unwrap_or(i64::MAX),
                    event_json,
                ),
            )
            .map_err(|_| StorageError::Database)?;
        Ok(())
    }

    fn upsert_segment(&mut self, segment: TaskSegment) -> Result<(), StorageError> {
        let segment_json = serde_json::to_string(&segment).map_err(|_| StorageError::Database)?;
        self.connection
            .execute(
                "INSERT INTO task_segments(segment_id, started_at, ended_at, segment_json) VALUES (?1, ?2, ?3, ?4) \
                 ON CONFLICT(segment_id) DO UPDATE SET started_at=excluded.started_at, ended_at=excluded.ended_at, segment_json=excluded.segment_json",
                (
                    segment.segment_id,
                    segment.started_at.to_rfc3339(),
                    segment.ended_at.to_rfc3339(),
                    segment_json,
                ),
            )
            .map_err(|_| StorageError::Database)?;
        Ok(())
    }

    fn events(&self) -> Vec<EventEnvelope> {
        let Ok(mut statement) = self.connection.prepare(
            "SELECT event_json FROM raw_events ORDER BY occurred_at, monotonic_ticks, event_id",
        ) else {
            return Vec::new();
        };
        let Ok(rows) = statement.query_map([], |row| row.get::<_, String>(0)) else {
            return Vec::new();
        };
        let events = rows
            .filter_map(Result::ok)
            .filter_map(|value| serde_json::from_str(&value).ok())
            .collect::<Vec<_>>();
        normalize_event_order(&events)
    }

    fn events_since(&self, start: DateTime<FixedOffset>) -> Vec<EventEnvelope> {
        let Ok(mut statement) = self.connection.prepare(
            "SELECT event_json FROM raw_events WHERE occurred_at >= ?1 \
             ORDER BY occurred_at, monotonic_ticks, event_id",
        ) else {
            return Vec::new();
        };
        let Ok(rows) = statement.query_map([start.with_timezone(&Utc).to_rfc3339()], |row| {
            row.get::<_, String>(0)
        }) else {
            return Vec::new();
        };
        let events = rows
            .filter_map(Result::ok)
            .filter_map(|value| serde_json::from_str(&value).ok())
            .collect::<Vec<_>>();
        normalize_event_order(&events)
    }

    fn sweep_retention(
        &mut self,
        now: DateTime<FixedOffset>,
        policy: RetentionPolicy,
    ) -> Result<DeletionReport, StorageError> {
        let cutoff = match policy {
            RetentionPolicy::NoRawHistory => now,
            RetentionPolicy::Hours(hours) => now - Duration::hours(i64::from(hours)),
        };
        let transaction = self
            .connection
            .transaction()
            .map_err(|_| StorageError::Database)?;
        let raw_events = transaction
            .execute(
                "DELETE FROM raw_events WHERE occurred_at < ?1",
                [cutoff.with_timezone(&Utc).to_rfc3339()],
            )
            .map_err(|_| StorageError::RolledBack)?;
        transaction.commit().map_err(|_| StorageError::RolledBack)?;
        Ok(DeletionReport {
            raw_events,
            segments: 0,
        })
    }

    fn delete_events_since(
        &mut self,
        start: Option<DateTime<FixedOffset>>,
    ) -> Result<DeletionReport, StorageError> {
        let transaction = self
            .connection
            .transaction()
            .map_err(|_| StorageError::Database)?;
        let (raw_events, segments) = if let Some(start) = start {
            let bound = start.with_timezone(&Utc).to_rfc3339();
            let raw_events = transaction
                .execute(
                    "DELETE FROM raw_events WHERE occurred_at >= ?1",
                    [bound.clone()],
                )
                .map_err(|_| StorageError::RolledBack)?;
            let segments = transaction
                .execute("DELETE FROM task_segments WHERE ended_at >= ?1", [bound])
                .map_err(|_| StorageError::RolledBack)?;
            (raw_events, segments)
        } else {
            let raw_events = transaction
                .execute("DELETE FROM raw_events", [])
                .map_err(|_| StorageError::RolledBack)?;
            let segments = transaction
                .execute("DELETE FROM task_segments", [])
                .map_err(|_| StorageError::RolledBack)?;
            (raw_events, segments)
        };
        transaction.commit().map_err(|_| StorageError::RolledBack)?;
        Ok(DeletionReport {
            raw_events,
            segments,
        })
    }

    fn add_repository(
        &mut self,
        root_path: &str,
        added_at: DateTime<FixedOffset>,
    ) -> Result<(), StorageError> {
        let Some(normalized) = normalized_repository_root(root_path) else {
            return Ok(());
        };
        self.connection
            .execute(
                "INSERT OR IGNORE INTO opted_in_repositories(root_path, added_at) VALUES (?1, ?2)",
                (normalized, added_at.with_timezone(&Utc).to_rfc3339()),
            )
            .map_err(|_| StorageError::Database)?;
        Ok(())
    }

    fn remove_repository(&mut self, root_path: &str) -> Result<(), StorageError> {
        let Some(normalized) = normalized_repository_root(root_path) else {
            return Ok(());
        };
        self.connection
            .execute(
                "DELETE FROM opted_in_repositories WHERE root_path = ?1",
                [normalized],
            )
            .map_err(|_| StorageError::Database)?;
        Ok(())
    }

    fn opted_in_repositories(&self) -> Vec<String> {
        let Ok(mut statement) = self
            .connection
            .prepare("SELECT root_path FROM opted_in_repositories ORDER BY root_path")
        else {
            return Vec::new();
        };
        let Ok(rows) = statement.query_map([], |row| row.get::<_, String>(0)) else {
            return Vec::new();
        };
        rows.filter_map(Result::ok).collect()
    }

    fn delete_repository_evidence(
        &mut self,
        root_path: &str,
    ) -> Result<DeletionReport, StorageError> {
        let Some(normalized_root) = normalized_repository_root(root_path) else {
            return Ok(DeletionReport::default());
        };

        let matching_ids: Vec<String> = {
            let mut statement = self
                .connection
                .prepare("SELECT event_id, event_json FROM raw_events")
                .map_err(|_| StorageError::Database)?;
            let rows = statement
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(|_| StorageError::Database)?;
            rows.filter_map(Result::ok)
                .filter_map(|(event_id, event_json)| {
                    let event: EventEnvelope = serde_json::from_str(&event_json).ok()?;
                    commit_matches_repository(&event, &normalized_root).then_some(event_id)
                })
                .collect()
        };

        let transaction = self
            .connection
            .transaction()
            .map_err(|_| StorageError::Database)?;
        for event_id in &matching_ids {
            transaction
                .execute("DELETE FROM raw_events WHERE event_id = ?1", [event_id])
                .map_err(|_| StorageError::RolledBack)?;
        }
        transaction.commit().map_err(|_| StorageError::RolledBack)?;

        Ok(DeletionReport {
            raw_events: matching_ids.len(),
            segments: 0,
        })
    }
}
/// Supported raw-event retention settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetentionPolicy {
    /// Delete raw events immediately after segment finalization.
    NoRawHistory,
    /// Retain raw events for a bounded number of hours.
    Hours(u16),
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self::Hours(48)
    }
}

/// Deletion report safe to display in the UI.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DeletionReport {
    /// Number of raw events deleted.
    pub raw_events: usize,
    /// Number of derived segments deleted.
    pub segments: usize,
}

/// Minimal repository operations used by the deterministic core.
pub trait HistoryRepository {
    /// Persists an already allowed and minimized event.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when encrypted storage is unavailable or the transaction fails.
    fn insert_event(&mut self, event: EventEnvelope) -> Result<(), StorageError>;
    /// Replaces a derived projection while preserving raw events.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when encrypted storage is unavailable or the transaction fails.
    fn upsert_segment(&mut self, segment: TaskSegment) -> Result<(), StorageError>;
    /// Returns allowed events ordered by stable event key.
    fn events(&self) -> Vec<EventEnvelope>;
    /// Returns canonical events at or after `start`, in the same deterministic order as
    /// [`Self::events`]. Reading a bounded range keeps a day view from paying for the whole
    /// retained history on every refresh.
    fn events_since(&self, start: DateTime<FixedOffset>) -> Vec<EventEnvelope>;
    /// Deletes expired raw events and returns counts only.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when cleanup cannot complete transactionally.
    fn sweep_retention(
        &mut self,
        now: DateTime<FixedOffset>,
        policy: RetentionPolicy,
    ) -> Result<DeletionReport, StorageError>;

    /// Deletes everything recorded at or after `start`, or the entire history when `start` is
    /// `None`. This is the user-initiated counterpart to [`Self::sweep_retention`], which expires
    /// the oldest records instead: a person asking to forget the last hour means the newest ones.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the deletion cannot complete transactionally.
    fn delete_events_since(
        &mut self,
        start: Option<DateTime<FixedOffset>>,
    ) -> Result<DeletionReport, StorageError>;

    /// Opts a repository root into Git evidence collection.
    ///
    /// A path that carries no identity (blank, or the filesystem root) is silently ignored,
    /// matching the resolver's own normalization: there is no repository to observe.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the write cannot complete.
    fn add_repository(
        &mut self,
        root_path: &str,
        added_at: DateTime<FixedOffset>,
    ) -> Result<(), StorageError>;

    /// Withdraws a previously opted-in repository. Removing a root that was never added is not an
    /// error.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the write cannot complete.
    fn remove_repository(&mut self, root_path: &str) -> Result<(), StorageError>;

    /// Returns every opted-in repository root, normalized and in a stable order.
    fn opted_in_repositories(&self) -> Vec<String>;

    /// Deletes every stored event whose evidence came from `root_path`, regardless of whether the
    /// repository is still opted in.
    ///
    /// This is what makes exclusion retroactive: opting a repository out stops future collection
    /// through [`HistoryRepository::remove_repository`], and this removes what was already
    /// stored. The two are separate calls so a caller can withdraw access without necessarily
    /// discarding history, or vice versa.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when cleanup cannot complete transactionally.
    fn delete_repository_evidence(
        &mut self,
        root_path: &str,
    ) -> Result<DeletionReport, StorageError>;
}

/// Normalizes a repository root using the same rule the entity resolver's project registry
/// applies, so a path stored here and a path checked there always agree on identity.
fn normalized_repository_root(root_path: &str) -> Option<String> {
    EntityKey::repository_path(root_path).map(|key| key.value().to_owned())
}

/// Returns true when `event` is a `RepositoryCommit` sourced from `normalized_root`.
///
/// Comparison happens on the already-normalized root so that trailing-separator or whitespace
/// differences between how a path was stored and how it is being removed can never cause evidence
/// to survive its own repository's removal.
fn commit_matches_repository(event: &EventEnvelope, normalized_root: &str) -> bool {
    match &event.payload {
        SemanticPayload::RepositoryCommit {
            repository_path, ..
        } => normalized_repository_root(repository_path).as_deref() == Some(normalized_root),
        _ => false,
    }
}

/// In-memory backend for synthetic journeys. It is never used as a production fallback.
#[derive(Default)]
pub struct MemoryHistoryRepository {
    events: BTreeMap<String, EventEnvelope>,
    segments: BTreeMap<String, TaskSegment>,
    repositories: std::collections::BTreeSet<String>,
}

impl HistoryRepository for MemoryHistoryRepository {
    fn insert_event(&mut self, event: EventEnvelope) -> Result<(), StorageError> {
        self.events.insert(event.event_id.to_string(), event);
        Ok(())
    }

    fn upsert_segment(&mut self, segment: TaskSegment) -> Result<(), StorageError> {
        self.segments.insert(segment.segment_id.clone(), segment);
        Ok(())
    }

    fn events(&self) -> Vec<EventEnvelope> {
        normalize_event_order(&self.events.values().cloned().collect::<Vec<_>>())
    }

    fn events_since(&self, start: DateTime<FixedOffset>) -> Vec<EventEnvelope> {
        let events = self
            .events
            .values()
            .filter(|event| event.occurred_at >= start)
            .cloned()
            .collect::<Vec<_>>();
        normalize_event_order(&events)
    }

    fn sweep_retention(
        &mut self,
        now: DateTime<FixedOffset>,
        policy: RetentionPolicy,
    ) -> Result<DeletionReport, StorageError> {
        let cutoff = match policy {
            RetentionPolicy::NoRawHistory => now,
            RetentionPolicy::Hours(hours) => now - Duration::hours(i64::from(hours)),
        };
        let before = self.events.len();
        self.events.retain(|_, event| event.occurred_at >= cutoff);
        Ok(DeletionReport {
            raw_events: before - self.events.len(),
            segments: 0,
        })
    }

    fn delete_events_since(
        &mut self,
        start: Option<DateTime<FixedOffset>>,
    ) -> Result<DeletionReport, StorageError> {
        let events_before = self.events.len();
        let segments_before = self.segments.len();
        if let Some(start) = start {
            self.events.retain(|_, event| event.occurred_at < start);
            self.segments.retain(|_, segment| segment.ended_at < start);
        } else {
            self.events.clear();
            self.segments.clear();
        }
        Ok(DeletionReport {
            raw_events: events_before - self.events.len(),
            segments: segments_before - self.segments.len(),
        })
    }

    fn add_repository(
        &mut self,
        root_path: &str,
        _added_at: DateTime<FixedOffset>,
    ) -> Result<(), StorageError> {
        if let Some(normalized) = normalized_repository_root(root_path) {
            self.repositories.insert(normalized);
        }
        Ok(())
    }

    fn remove_repository(&mut self, root_path: &str) -> Result<(), StorageError> {
        if let Some(normalized) = normalized_repository_root(root_path) {
            self.repositories.remove(&normalized);
        }
        Ok(())
    }

    fn opted_in_repositories(&self) -> Vec<String> {
        self.repositories.iter().cloned().collect()
    }

    fn delete_repository_evidence(
        &mut self,
        root_path: &str,
    ) -> Result<DeletionReport, StorageError> {
        let Some(normalized_root) = normalized_repository_root(root_path) else {
            return Ok(DeletionReport::default());
        };
        let before = self.events.len();
        self.events
            .retain(|_, event| !commit_matches_repository(event, &normalized_root));
        Ok(DeletionReport {
            raw_events: before - self.events.len(),
            segments: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use chrono::DateTime;
    use openhistory_domain::{
        AdapterKind, ApplicationIdentity, CaptureQuality, EventEnvelope, SemanticPayload,
        SourceIdentity,
    };

    use super::*;

    #[derive(Default)]
    struct MemoryKeyProvider {
        value: RefCell<Option<String>>,
    }

    impl DatabaseKeyProvider for MemoryKeyProvider {
        fn load(&self) -> Result<Option<DatabaseKey>, StorageError> {
            self.value
                .borrow()
                .as_deref()
                .map(DatabaseKey::parse)
                .transpose()
        }

        fn save(&self, key: &DatabaseKey) -> Result<(), StorageError> {
            self.value.replace(Some(key.expose_for_store().to_owned()));
            Ok(())
        }
    }

    fn event(timestamp: &str, ticks: u64) -> EventEnvelope {
        EventEnvelope::new(
            DateTime::parse_from_rfc3339(timestamp).unwrap(),
            ticks,
            SourceIdentity {
                source_id: "fixture".into(),
                adapter: AdapterKind::Synthetic,
                application: ApplicationIdentity {
                    display_name: Some("Editor".into()),
                    platform_id: None,
                    process_id: None,
                },
            },
            CaptureQuality::Window,
            SemanticPayload::ApplicationActivated,
        )
    }

    fn commit_event(timestamp: &str, ticks: u64, repository_path: &str) -> EventEnvelope {
        EventEnvelope::new(
            DateTime::parse_from_rfc3339(timestamp).unwrap(),
            ticks,
            SourceIdentity {
                source_id: "git:fixture".into(),
                adapter: AdapterKind::Import,
                application: ApplicationIdentity {
                    display_name: Some("Git".into()),
                    platform_id: None,
                    process_id: None,
                },
            },
            CaptureQuality::Semantic,
            SemanticPayload::RepositoryCommit {
                repository_path: repository_path.to_owned(),
                commit_id: format!("commit-{ticks}"),
                branch: Some("main".into()),
                subject: Some("fixture commit".into()),
                changed_paths: vec!["src/lib.rs".into()],
            },
        )
    }

    #[test]
    fn default_retention_deletes_only_older_raw_events() {
        let mut repository = MemoryHistoryRepository::default();
        repository
            .insert_event(event("2026-09-06T15:59:59+08:00", 1))
            .unwrap();
        repository
            .insert_event(event("2026-09-06T16:00:00+08:00", 2))
            .unwrap();
        let now = DateTime::parse_from_rfc3339("2026-09-08T16:00:00+08:00").unwrap();
        let report = repository
            .sweep_retention(now, RetentionPolicy::default())
            .unwrap();
        assert_eq!(report.raw_events, 1);
        assert_eq!(repository.events().len(), 1);
    }

    #[test]
    fn a_bounded_read_returns_the_same_events_the_full_read_would_from_that_point() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.sqlite3");
        let provider = MemoryKeyProvider::default();
        let mut database = EncryptedDatabase::open_or_create(&path, &provider).unwrap();
        let mut memory = MemoryHistoryRepository::default();
        for (moment, tick) in [
            ("2026-09-16T23:59:59+08:00", 1),
            ("2026-09-17T00:00:00+08:00", 2),
            ("2026-09-17T09:30:00+08:00", 3),
        ] {
            let entry = event(moment, tick);
            database.insert_event(entry.clone()).unwrap();
            memory.insert_event(entry).unwrap();
        }

        let start = DateTime::parse_from_rfc3339("2026-09-17T00:00:00+08:00").unwrap();
        let bounded = database.events_since(start);
        assert_eq!(
            bounded.len(),
            2,
            "the previous day is excluded at the query"
        );
        assert!(bounded.iter().all(|event| event.occurred_at >= start));
        assert_eq!(
            bounded,
            database
                .events()
                .into_iter()
                .filter(|event| event.occurred_at >= start)
                .collect::<Vec<_>>(),
            "a bounded read must not reorder or drop anything a full read would keep"
        );
        // Both backends answer the same question the same way.
        assert_eq!(memory.events_since(start), bounded);
    }

    #[test]
    fn deleting_recent_history_removes_the_newest_records_and_keeps_the_rest() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.sqlite3");
        let provider = MemoryKeyProvider::default();
        let mut database = EncryptedDatabase::open_or_create(&path, &provider).unwrap();
        for (moment, tick) in [
            ("2026-09-17T08:00:00+08:00", 1),
            ("2026-09-17T09:55:00+08:00", 2),
            ("2026-09-17T09:59:00+08:00", 3),
        ] {
            database.insert_event(event(moment, tick)).unwrap();
        }

        // "Forget the last ten minutes" means the newest records, the opposite end from retention.
        let start = DateTime::parse_from_rfc3339("2026-09-17T09:50:00+08:00").unwrap();
        let report = database.delete_events_since(Some(start)).unwrap();
        assert_eq!(report.raw_events, 2);
        let remaining = database.events();
        assert_eq!(remaining.len(), 1);
        assert!(remaining[0].occurred_at < start);

        // Deleting everything leaves nothing behind, not even the older survivor.
        let report = database.delete_events_since(None).unwrap();
        assert_eq!(report.raw_events, 1);
        assert!(database.events().is_empty());
    }

    #[test]
    fn deleting_all_history_clears_derived_segments_too() {
        let mut repository = MemoryHistoryRepository::default();
        repository
            .insert_event(event("2026-09-17T08:00:00+08:00", 1))
            .unwrap();
        repository
            .upsert_segment(TaskSegment {
                segment_id: "segment-1".to_owned(),
                started_at: DateTime::parse_from_rfc3339("2026-09-17T08:00:00+08:00").unwrap(),
                ended_at: DateTime::parse_from_rfc3339("2026-09-17T08:20:00+08:00").unwrap(),
                event_ids: Vec::new(),
                applications: Vec::new(),
                titles: Vec::new(),
                project_id: None,
                confidence: openhistory_segmentation::SegmentConfidence::Medium,
                observed_seconds: 20 * 60,
                open_ended: false,
            })
            .unwrap();

        let report = repository.delete_events_since(None).unwrap();
        assert_eq!(report.raw_events, 1);
        assert_eq!(report.segments, 1, "a derived projection is history too");
        assert!(repository.events().is_empty());
    }

    #[test]
    fn encrypted_database_reopens_with_stored_key_and_wal() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.sqlite3");
        let provider = MemoryKeyProvider::default();

        {
            let database = EncryptedDatabase::open_or_create(&path, &provider).unwrap();
            assert_eq!(database.schema_version().unwrap(), 1);
            assert_eq!(database.journal_mode().unwrap().to_ascii_lowercase(), "wal");
            assert_eq!(database.path(), path);
        }

        let reopened = EncryptedDatabase::open_or_create(&path, &provider).unwrap();
        assert_eq!(reopened.schema_version().unwrap(), 1);
    }

    #[test]
    fn encrypted_database_rejects_missing_and_incorrect_keys() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.sqlite3");
        let key = DatabaseKey::generate();
        let database = EncryptedDatabase::open(&path, &key).unwrap();
        assert_eq!(database.schema_version().unwrap(), 1);
        drop(database);

        let unkeyed = Connection::open(&path).unwrap();
        let plaintext_read = unkeyed.query_row("SELECT count(*) FROM sqlite_master", [], |row| {
            row.get::<_, i64>(0)
        });
        assert!(plaintext_read.is_err());
        drop(unkeyed);

        let wrong_key = DatabaseKey::generate();
        assert!(matches!(
            EncryptedDatabase::open(&path, &wrong_key),
            Err(StorageError::DatabaseKeyRejected)
        ));
    }

    #[test]
    fn encrypted_repository_persists_events_and_sweeps_in_utc_order() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.sqlite3");
        let key = DatabaseKey::generate();
        let mut database = EncryptedDatabase::open(&path, &key).unwrap();
        database
            .insert_event(event("2026-09-06T15:59:59+08:00", 1))
            .unwrap();
        database
            .insert_event(event("2026-09-06T08:00:00+00:00", 2))
            .unwrap();
        assert_eq!(database.events().len(), 2);

        let report = database
            .sweep_retention(
                DateTime::parse_from_rfc3339("2026-09-08T08:00:00+00:00").unwrap(),
                RetentionPolicy::default(),
            )
            .unwrap();
        assert_eq!(report.raw_events, 1);
        assert_eq!(database.events().len(), 1);
    }

    #[test]
    fn database_key_validation_fails_closed() {
        assert!(matches!(
            DatabaseKey::parse("not-a-key"),
            Err(StorageError::InvalidKey)
        ));
        assert!(DatabaseKey::parse(&"ab".repeat(DATABASE_KEY_BYTES)).is_ok());
    }

    fn added_at() -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339("2026-09-15T09:00:00+08:00").unwrap()
    }

    #[test]
    fn memory_repository_opt_in_is_normalized_deduplicated_and_ordered() {
        let mut repository = MemoryHistoryRepository::default();
        repository
            .add_repository("/Users/dev/beta", added_at())
            .unwrap();
        repository
            .add_repository("/Users/dev/alpha/", added_at())
            .unwrap();
        repository
            .add_repository("/Users/dev/alpha", added_at())
            .unwrap();

        assert_eq!(
            repository.opted_in_repositories(),
            vec!["/Users/dev/alpha".to_owned(), "/Users/dev/beta".to_owned()]
        );
    }

    #[test]
    fn memory_repository_removal_and_blank_paths_are_handled() {
        let mut repository = MemoryHistoryRepository::default();
        repository
            .add_repository("/Users/dev/alpha", added_at())
            .unwrap();
        repository.add_repository("   ", added_at()).unwrap();
        assert_eq!(repository.opted_in_repositories().len(), 1);

        repository.remove_repository("/Users/dev/alpha").unwrap();
        assert!(repository.opted_in_repositories().is_empty());
        // Removing something never added, or a blank path, is not an error.
        repository.remove_repository("/never/added").unwrap();
        repository.remove_repository("").unwrap();
    }

    #[test]
    fn encrypted_repository_persists_opted_in_repositories_across_reopen() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.sqlite3");
        let key = DatabaseKey::generate();

        {
            let mut database = EncryptedDatabase::open(&path, &key).unwrap();
            database
                .add_repository("/Users/dev/open-history/", added_at())
                .unwrap();
            database
                .add_repository("/Users/dev/open-history", added_at())
                .unwrap();
            database
                .add_repository("/Users/dev/timetrace", added_at())
                .unwrap();
        }

        let mut reopened = EncryptedDatabase::open(&path, &key).unwrap();
        assert_eq!(
            reopened.opted_in_repositories(),
            vec![
                "/Users/dev/open-history".to_owned(),
                "/Users/dev/timetrace".to_owned()
            ]
        );

        reopened.remove_repository("/Users/dev/timetrace").unwrap();
        assert_eq!(
            reopened.opted_in_repositories(),
            vec!["/Users/dev/open-history".to_owned()]
        );
    }

    #[test]
    fn memory_repository_removal_deletes_only_that_repositorys_evidence() {
        let mut repository = MemoryHistoryRepository::default();
        repository
            .insert_event(commit_event("2026-09-14T10:00:00+08:00", 1, "/work/app"))
            .unwrap();
        repository
            .insert_event(commit_event("2026-09-14T11:00:00+08:00", 2, "/work/other"))
            .unwrap();
        repository
            .insert_event(event("2026-09-14T12:00:00+08:00", 3))
            .unwrap();

        let report = repository.delete_repository_evidence("/work/app").unwrap();

        assert_eq!(report.raw_events, 1);
        let remaining = repository.events();
        assert_eq!(remaining.len(), 2);
        assert!(!remaining.iter().any(|event| commit_matches_repository(
            event,
            &normalized_repository_root("/work/app").unwrap()
        )));
    }

    #[test]
    fn deletion_is_a_no_op_for_a_repository_with_no_stored_evidence() {
        let mut repository = MemoryHistoryRepository::default();
        repository
            .insert_event(commit_event("2026-09-14T10:00:00+08:00", 1, "/work/app"))
            .unwrap();

        let report = repository
            .delete_repository_evidence("/work/never-had-any-commits")
            .unwrap();

        assert_eq!(report.raw_events, 0);
        assert_eq!(repository.events().len(), 1);
    }

    #[test]
    fn encrypted_repository_removal_deletes_only_that_repositorys_evidence() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.sqlite3");
        let key = DatabaseKey::generate();
        let mut database = EncryptedDatabase::open(&path, &key).unwrap();
        database
            .insert_event(commit_event("2026-09-14T10:00:00+08:00", 1, "/work/app"))
            .unwrap();
        database
            .insert_event(commit_event("2026-09-14T11:00:00+08:00", 2, "/work/other"))
            .unwrap();

        let report = database.delete_repository_evidence("/work/app/").unwrap();

        assert_eq!(report.raw_events, 1);
        assert_eq!(database.events().len(), 1);
    }

    #[test]
    fn artifact_derived_evidence_is_swept_by_the_same_raw_retention_as_platform_events() {
        let mut repository = MemoryHistoryRepository::default();
        repository
            .insert_event(commit_event("2026-09-06T15:59:59+08:00", 1, "/work/app"))
            .unwrap();
        repository
            .insert_event(commit_event("2026-09-06T16:00:00+08:00", 2, "/work/app"))
            .unwrap();

        let now = DateTime::parse_from_rfc3339("2026-09-08T16:00:00+08:00").unwrap();
        let report = repository
            .sweep_retention(now, RetentionPolicy::default())
            .unwrap();

        // Retention does not special-case a payload kind: the same 48-hour cutoff that applies to
        // window-activity events applies to repository-commit events, without a parallel sweep.
        assert_eq!(report.raw_events, 1);
        assert_eq!(repository.events().len(), 1);
    }
}
