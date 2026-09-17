//! Consent-driven native collection and encrypted event persistence.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use openhistory_adapters::{ActivityAdapter, AdapterError, bounded_event_channel};
use openhistory_domain::EventEnvelope;
use openhistory_entities::ProjectRegistry;
use openhistory_platform::{
    CaptureDetail, CollectionGate, PlatformAdapter, platform_permission_granted,
    request_platform_permission,
};
use openhistory_privacy::PrivacyPolicy;
use openhistory_segmentation::{SegmentationSettings, TaskSegment, segment_events};
use openhistory_storage::{
    EncryptedDatabase, HistoryRepository, OsDatabaseKeyProvider, StorageError,
};
use tauri::{AppHandle, Emitter};
use tokio::{sync::Mutex as AsyncMutex, task::JoinHandle};

use crate::dashboard::{CollectionStatus, DashboardState, HistoryScope};

/// Applications never recorded, whatever the capture detail.
///
/// Public and shared with the dashboard projection on purpose: the settings card tells the user
/// which applications are excluded, and it used to do that from its own hand-written copy of this
/// list — a copy that was already one entry out of date. One list means the card cannot claim an
/// exclusion that is not installed, or omit one that is.
pub const EXCLUDED_APPLICATIONS: [&str; 3] = ["1Password", "Keychain Access", "Passwords"];

/// Owns the privileged adapter, bounded writer, and encrypted local database.
pub struct CollectorRuntime {
    adapter: AsyncMutex<PlatformAdapter>,
    /// Encrypted storage, absent until [`CollectorRuntime::open_storage`] has opened it.
    database: Arc<Mutex<Option<EncryptedDatabase>>>,
    path: PathBuf,
    writer: AsyncMutex<Option<JoinHandle<()>>>,
    dashboard: DashboardState,
}

impl CollectorRuntime {
    /// Prepares the adapter without touching the disk or the credential store.
    ///
    /// Does no I/O on purpose. Opening encrypted storage reads a key out of the OS credential
    /// store, which can stop to ask the user for their password; doing that here — on the main
    /// thread, before any window exists — freezes the whole app behind a dialog it cannot render.
    /// The window comes up first, and [`CollectorRuntime::open_storage`] runs where waiting is
    /// harmless.
    #[must_use]
    pub fn new(path: PathBuf, dashboard: DashboardState, detail: CaptureDetail) -> Self {
        let mut adapter = PlatformAdapter::default();
        adapter.set_capture_detail(detail);
        adapter.set_privacy_policy(PrivacyPolicy {
            applications: EXCLUDED_APPLICATIONS.map(str::to_owned).to_vec(),
            ..PrivacyPolicy::default()
        });
        Self {
            adapter: AsyncMutex::new(adapter),
            database: Arc::new(Mutex::new(None)),
            path,
            writer: AsyncMutex::new(None),
            dashboard,
        }
    }

    /// Opens `SQLCipher` with a random key held by the OS credential store.
    ///
    /// Blocks for as long as the credential store takes, so it belongs off the main thread.
    ///
    /// # Errors
    ///
    /// Returns a non-sensitive storage error if encrypted storage cannot be opened.
    pub async fn open_storage(&self) -> Result<(), StorageError> {
        let provider = OsDatabaseKeyProvider::new("default-profile");
        let database = EncryptedDatabase::open_or_create(&self.path, &provider)?;
        *self.database.lock().map_err(|_| StorageError::Database)? = Some(database);
        self.refresh_project_registry().await;
        Ok(())
    }

    /// Reports whether encrypted storage is open and readable yet.
    ///
    /// Callers use this to say "still unlocking" rather than "nothing recorded", which an empty
    /// read would otherwise be mistaken for.
    #[must_use]
    pub fn storage_ready(&self) -> bool {
        self.database
            .lock()
            .is_ok_and(|database| database.is_some())
    }

    /// Runs `action` against open storage, or reports that storage is not open yet.
    fn with_database<T>(
        &self,
        action: impl FnOnce(&mut EncryptedDatabase) -> Result<T, StorageError>,
    ) -> Result<T, StorageError> {
        let mut guard = self.database.lock().map_err(|_| StorageError::Database)?;
        let database = guard.as_mut().ok_or(StorageError::NotInitialized)?;
        action(database)
    }

    /// Opts a repository into Git evidence collection and Accessibility project resolution.
    ///
    /// Takes effect for accessibility resolution the next time collection starts; already-running
    /// collection keeps its previous registry, matching how a privacy-policy change is applied.
    ///
    /// # Errors
    ///
    /// Returns a storage error if the opt-in cannot be persisted.
    pub async fn add_repository(&self, root_path: &str) -> Result<(), StorageError> {
        let now = chrono::Local::now().fixed_offset();
        self.with_database(|database| database.add_repository(root_path, now))?;
        self.refresh_project_registry().await;
        Ok(())
    }

    /// Withdraws a repository from Git evidence collection and project resolution, and deletes
    /// evidence already stored from it.
    ///
    /// # Errors
    ///
    /// Returns a storage error if the removal cannot be persisted.
    pub async fn remove_repository(&self, root_path: &str) -> Result<(), StorageError> {
        self.with_database(|database| {
            database.remove_repository(root_path)?;
            database.delete_repository_evidence(root_path)
        })?;
        self.refresh_project_registry().await;
        Ok(())
    }

    /// Returns every opted-in repository root.
    ///
    /// # Errors
    ///
    /// Returns a storage error if the encrypted database cannot be read.
    pub fn opted_in_repositories(&self) -> Result<Vec<String>, StorageError> {
        self.with_database(|database| Ok(database.opted_in_repositories()))
    }

    async fn refresh_project_registry(&self) {
        let roots = self
            .with_database(|database| Ok(database.opted_in_repositories()))
            .unwrap_or_default();
        self.adapter
            .lock()
            .await
            .set_project_registry(ProjectRegistry::new(roots), std::env::var("HOME").ok());
    }

    /// Restores collection to the state the user last left it in.
    ///
    /// Collection is the entire point of the app, and stopping it at every launch loses exactly the
    /// day it was meant to record — silently, since nothing announces that it stopped. During
    /// development that happens on every rebuild.
    ///
    /// Permission is probed rather than requested. A launch is not a moment the user asked for
    /// anything, so it must not raise a system trust prompt; without permission the app comes up
    /// saying so and waits to be asked.
    pub async fn resume(&self, app: AppHandle) {
        let status = if platform_permission_granted() {
            match self.start(app.clone()).await {
                Ok(()) => CollectionStatus::Recording,
                Err(AdapterError::PermissionRequired) => CollectionStatus::PermissionNeeded,
                Err(_) => CollectionStatus::Error,
            }
        } else {
            CollectionStatus::PermissionNeeded
        };
        self.dashboard.set_status(status);
        let _ = app.emit("collection-status-changed", status);
    }

    /// Starts collection only as a direct result of the user's recording action.
    ///
    /// Requesting permission here (rather than merely probing it) is deliberate: this only runs
    /// when the user has just clicked to start collection, so it is the correct moment to show
    /// the system trust prompt if it hasn't been granted yet. Already-trusted processes see no
    /// prompt at all.
    ///
    /// # Errors
    ///
    /// Returns an adapter error when permission is absent or collection is unavailable.
    pub async fn start(&self, app: AppHandle) -> Result<(), AdapterError> {
        self.stop().await?;
        let permission = request_platform_permission();
        let mut adapter = self.adapter.lock().await;
        adapter.set_gate(CollectionGate {
            consented: true,
            platform_permission: permission,
        });
        let (sender, mut receiver) = bounded_event_channel(256);
        adapter.start(sender).await?;
        drop(adapter);

        let database = Arc::clone(&self.database);
        let dashboard = self.dashboard.clone();
        *self.writer.lock().await = Some(tokio::spawn(async move {
            while let Some(item) = receiver.recv().await {
                match item {
                    Ok(event) => {
                        let persisted = database.lock().map_err(|_| ()).and_then(|mut value| {
                            value
                                .as_mut()
                                .ok_or(())?
                                .insert_event(event)
                                .map_err(|_| ())
                        });
                        if persisted.is_err() {
                            dashboard.set_status(CollectionStatus::Error);
                            let _ = app.emit("collection-status-changed", CollectionStatus::Error);
                            break;
                        }
                    }
                    Err(AdapterError::PermissionRequired) => {
                        dashboard.set_status(CollectionStatus::PermissionNeeded);
                        let _ = app.emit(
                            "collection-status-changed",
                            CollectionStatus::PermissionNeeded,
                        );
                        break;
                    }
                    Err(_) => {
                        dashboard.set_status(CollectionStatus::Error);
                        let _ = app.emit("collection-status-changed", CollectionStatus::Error);
                        break;
                    }
                }
            }
        }));
        Ok(())
    }

    /// Stops the native observer and waits for the bounded writer to drain.
    ///
    /// # Errors
    ///
    /// Returns an adapter error if the observer cannot stop cleanly.
    pub async fn stop(&self) -> Result<(), AdapterError> {
        self.adapter.lock().await.stop().await?;
        if let Some(writer) = self.writer.lock().await.take() {
            let _ = writer.await;
        }
        Ok(())
    }

    /// Returns the number of encrypted canonical events for local diagnostics.
    #[must_use]
    pub fn event_count(&self) -> usize {
        self.with_database(|database| Ok(database.events().len()))
            .unwrap_or(0)
    }

    /// Applies a new capture granularity. Collection that is already running is restarted so the
    /// change takes effect immediately rather than at the next launch.
    ///
    /// # Errors
    ///
    /// Returns an adapter error when running collection cannot be restarted.
    pub async fn set_capture_detail(
        &self,
        detail: CaptureDetail,
        app: &AppHandle,
    ) -> Result<(), AdapterError> {
        let was_running = self.adapter.lock().await.is_running();
        self.adapter.lock().await.set_capture_detail(detail);
        if was_running {
            self.start(app.clone()).await?;
        }
        Ok(())
    }

    /// Reports how much disk the encrypted history occupies, including its write-ahead log and
    /// shared-memory sidecars, which carry real pending data and can dwarf the main file.
    #[must_use]
    pub fn storage_bytes(&self) -> u64 {
        let sidecar = |suffix: &str| {
            let mut name = self.path.clone().into_os_string();
            name.push(suffix);
            std::path::PathBuf::from(name)
        };
        [self.path.clone(), sidecar("-wal"), sidecar("-shm")]
            .iter()
            .filter_map(|candidate| candidate.metadata().ok())
            .map(|metadata| metadata.len())
            .sum()
    }

    /// Deletes history the user explicitly asked to forget, newest-first by scope.
    ///
    /// # Errors
    ///
    /// Returns a storage error if the deletion cannot complete.
    pub fn delete_history(&self, scope: HistoryScope) -> Result<usize, StorageError> {
        let now = chrono::Local::now();
        let start = match scope {
            HistoryScope::Last10Minutes => Some(now.fixed_offset() - chrono::Duration::minutes(10)),
            HistoryScope::LastHour => Some(now.fixed_offset() - chrono::Duration::hours(1)),
            HistoryScope::Today => now
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .and_then(|midnight| midnight.and_local_timezone(*now.offset()).single()),
            HistoryScope::All => None,
        };
        let report = self.with_database(|database| database.delete_events_since(start))?;
        Ok(report.raw_events)
    }

    /// Deterministically segments one local day's persisted events for dashboard projection.
    ///
    /// # Errors
    ///
    /// Returns a storage error if the encrypted database cannot be read.
    pub fn segments_for(&self, day: chrono::NaiveDate) -> Result<Vec<TaskSegment>, StorageError> {
        let now = chrono::Local::now();
        // The read starts a day early on purpose. It is the local-date filter below that decides
        // what belongs to `day`; the only job of this bound is to not exclude any of it, and a
        // bound computed with today's UTC offset can land after the target day's midnight when
        // daylight saving moved in between, silently clipping its first hour.
        let Some(midnight) = day.pred_opt().unwrap_or(day).and_hms_opt(0, 0, 0) else {
            return Ok(Vec::new());
        };
        let start = midnight
            .and_local_timezone(*now.offset())
            .single()
            .unwrap_or_else(|| now.fixed_offset());
        let events: Vec<EventEnvelope> = self
            .with_database(|database| Ok(database.events_since(start)))?
            .into_iter()
            .filter(|event| event.occurred_at.date_naive() == day)
            .collect();
        Ok(segment_events(&events, SegmentationSettings::default()))
    }
}

#[cfg(test)]
mod tests {
    use super::{CollectorRuntime, StorageError};
    use crate::dashboard::DashboardState;
    use openhistory_platform::CaptureDetail;

    /// Construction must not read the credential store. It runs on the main thread before any
    /// window exists, and a keychain prompt raised from there freezes the app behind a dialog it
    /// cannot draw. The temporary directory is never created, which is the point: nothing opens it.
    #[test]
    fn a_new_runtime_opens_nothing() {
        let path = std::env::temp_dir().join("openhistory-does-not-exist/history.sqlite3");
        let runtime = CollectorRuntime::new(
            path.clone(),
            DashboardState::default(),
            CaptureDetail::default(),
        );
        assert!(!runtime.storage_ready());
        assert!(!path.exists());
    }

    /// Reads before storage opens say so, rather than returning an empty history. An empty answer
    /// would be read as "nothing was recorded", which is a claim about the day and not about the
    /// key.
    #[test]
    fn reads_before_storage_opens_report_that_it_is_not_open() {
        let runtime = CollectorRuntime::new(
            std::env::temp_dir().join("openhistory-does-not-exist/history.sqlite3"),
            DashboardState::default(),
            CaptureDetail::default(),
        );
        assert!(matches!(
            runtime.segments_for(chrono::Local::now().date_naive()),
            Err(StorageError::NotInitialized)
        ));
        assert!(matches!(
            runtime.opted_in_repositories(),
            Err(StorageError::NotInitialized)
        ));
    }
}
