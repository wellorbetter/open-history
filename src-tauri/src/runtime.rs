//! Consent-driven native collection and encrypted event persistence.

use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use openhistory_adapters::{ActivityAdapter, AdapterError, bounded_event_channel};
use openhistory_entities::ProjectRegistry;
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
        adapter.set_project_registry(
            ProjectRegistry::new(database.opted_in_repositories()),
            std::env::var("HOME").ok(),
        );
        Ok(Self {
            adapter: AsyncMutex::new(adapter),
            database: Arc::new(Mutex::new(database)),
            writer: AsyncMutex::new(None),
            dashboard,
        })
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
        {
            let mut database = self.database.lock().map_err(|_| StorageError::Database)?;
            database.add_repository(root_path, now)?;
        }
        self.refresh_project_registry().await;
        Ok(())
    }

    /// Withdraws a repository from Git evidence collection and project resolution.
    ///
    /// # Errors
    ///
    /// Returns a storage error if the removal cannot be persisted.
    pub async fn remove_repository(&self, root_path: &str) -> Result<(), StorageError> {
        {
            let mut database = self.database.lock().map_err(|_| StorageError::Database)?;
            database.remove_repository(root_path)?;
        }
        self.refresh_project_registry().await;
        Ok(())
    }

    /// Returns every opted-in repository root.
    ///
    /// # Errors
    ///
    /// Returns a storage error if the encrypted database cannot be read.
    pub fn opted_in_repositories(&self) -> Result<Vec<String>, StorageError> {
        let database = self.database.lock().map_err(|_| StorageError::Database)?;
        Ok(database.opted_in_repositories())
    }

    async fn refresh_project_registry(&self) {
        let roots = self
            .database
            .lock()
            .map_or_else(|_| Vec::new(), |database| database.opted_in_repositories());
        self.adapter
            .lock()
            .await
            .set_project_registry(ProjectRegistry::new(roots), std::env::var("HOME").ok());
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
