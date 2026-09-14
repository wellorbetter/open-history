//! Consent-driven native collection and encrypted event persistence.

use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use openhistory_adapters::{ActivityAdapter, AdapterError, bounded_event_channel};
use openhistory_platform::{
    CaptureDetail, CollectionGate, PlatformAdapter, platform_permission_granted,
};
use openhistory_privacy::PrivacyPolicy;
use openhistory_storage::{
    EncryptedDatabase, HistoryRepository, OsDatabaseKeyProvider, StorageError,
};
use tauri::{AppHandle, Emitter};
use tokio::{sync::Mutex as AsyncMutex, task::JoinHandle};

use crate::dashboard::{CollectionStatus, DashboardState};

/// Owns the privileged adapter, bounded writer, and encrypted local database.
pub struct CollectorRuntime {
    adapter: AsyncMutex<PlatformAdapter>,
    database: Arc<Mutex<EncryptedDatabase>>,
    writer: AsyncMutex<Option<JoinHandle<()>>>,
    dashboard: DashboardState,
}

impl CollectorRuntime {
    /// Initializes `SQLCipher` with a random key held by the OS credential store.
    ///
    /// # Errors
    ///
    /// Returns a non-sensitive storage error if encrypted storage cannot be opened.
    pub fn initialize(path: &Path, dashboard: DashboardState) -> Result<Self, StorageError> {
        let provider = OsDatabaseKeyProvider::new("default-profile");
        let database = EncryptedDatabase::open_or_create(path, &provider)?;
        let mut adapter = PlatformAdapter::default();
        adapter.set_capture_detail(CaptureDetail::Window);
        adapter.set_privacy_policy(PrivacyPolicy {
            applications: vec![
                "1Password".to_owned(),
                "Keychain Access".to_owned(),
                "Passwords".to_owned(),
            ],
            ..PrivacyPolicy::default()
        });
        Ok(Self {
            adapter: AsyncMutex::new(adapter),
            database: Arc::new(Mutex::new(database)),
            writer: AsyncMutex::new(None),
            dashboard,
        })
    }

    /// Starts collection only as a direct result of the user's recording action.
    ///
    /// # Errors
    ///
    /// Returns an adapter error when permission is absent or collection is unavailable.
    pub async fn start(&self, app: AppHandle) -> Result<(), AdapterError> {
        self.stop().await?;
        let permission = platform_permission_granted();
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
                        let persisted = database
                            .lock()
                            .map_err(|_| ())
                            .and_then(|mut value| value.insert_event(event).map_err(|_| ()));
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
        self.database
            .lock()
            .map_or(0, |database| database.events().len())
    }
}
