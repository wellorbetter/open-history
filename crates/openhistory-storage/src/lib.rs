//! Storage contracts and a deterministic in-memory backend for core and UI tests.
//!
//! The production `SQLCipher` backend is introduced behind this contract so no unencrypted fallback
//! can be selected accidentally.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use chrono::{DateTime, Duration, FixedOffset};
use openhistory_domain::{EventEnvelope, normalize_event_order};
use openhistory_segmentation::TaskSegment;
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

const DATABASE_KEY_BYTES: usize = 32;
const DATABASE_MIGRATIONS: &str = r#"
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
INSERT OR IGNORE INTO schema_migrations(version, applied_at)
VALUES (1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
COMMIT;
"#;

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
    /// SQLCipher support is unavailable in the linked SQLite library.
    #[error("SQLCipher is unavailable")]
    SqlCipherUnavailable,
    /// A database operation failed without exposing statement or secret details.
    #[error("encrypted database operation failed")]
    Database,
    /// The operating-system credential store could not be accessed.
    #[error("credential store is unavailable")]
    CredentialStore,
}

/// A validated 256-bit SQLCipher key that erases its allocation on drop.
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
    pub fn parse(value: String) -> Result<Self, StorageError> {
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
            Ok(value) => DatabaseKey::parse(value).map(Some),
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

/// An initialized SQLCipher connection. There is deliberately no plaintext fallback.
pub struct EncryptedDatabase {
    connection: Connection,
    path: PathBuf,
}

impl EncryptedDatabase {
    /// Loads or creates a key, opens SQLCipher, enables WAL, and applies migrations.
    ///
    /// # Errors
    ///
    /// Returns a non-sensitive [`StorageError`] when key retrieval, decryption, or migration fails.
    pub fn open_or_create(
        path: impl AsRef<Path>,
        provider: &impl DatabaseKeyProvider,
    ) -> Result<Self, StorageError> {
        let key = match provider.load()? {
            Some(existing) => existing,
            None => {
                let generated = DatabaseKey::generate();
                provider.save(&generated)?;
                generated
            }
        };
        Self::open(path, &key)
    }

    /// Opens a SQLCipher database using an explicit validated key.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::DatabaseKeyRejected`] for an existing database encrypted with a
    /// different key, and never retries with plaintext SQLite.
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
}

/// In-memory backend for synthetic journeys. It is never used as a production fallback.
#[derive(Default)]
pub struct MemoryHistoryRepository {
    events: BTreeMap<String, EventEnvelope>,
    segments: BTreeMap<String, TaskSegment>,
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
                .clone()
                .map(DatabaseKey::parse)
                .transpose()
        }

        fn save(&self, key: &DatabaseKey) -> Result<(), StorageError> {
            self.value
                .replace(Some(key.expose_for_store().to_owned()));
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
    fn database_key_validation_fails_closed() {
        assert!(matches!(
            DatabaseKey::parse("not-a-key".to_owned()),
            Err(StorageError::InvalidKey)
        ));
        assert!(DatabaseKey::parse("ab".repeat(DATABASE_KEY_BYTES)).is_ok());
    }
}
