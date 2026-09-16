//! Consent-gated semantic platform collection.
//!
//! macOS uses safe `AXUIElement`/`AXObserver` wrappers. Windows keeps the same contract while its
//! native UI Automation implementation is developed. No adapter captures pixels, audio, or raw
//! keyboard input.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use openhistory_adapters::{ActivityAdapter, AdapterError, EventSender};
use openhistory_domain::{
    AdapterKind, ApplicationIdentity, CaptureQuality, EntityKind, EventEnvelope, SemanticPayload,
    SourceIdentity,
};
use openhistory_entities::{ProjectRegistry, Resolver, WindowObservation};
use openhistory_privacy::{CandidateContext, PolicyDecision, PrivacyPolicy};

#[cfg(target_os = "macos")]
mod macos;

const MAX_APPLICATION_CHARS: usize = 96;
const MAX_WINDOW_CHARS: usize = 160;
const MAX_ROLE_CHARS: usize = 64;
const MAX_CONTEXT_CHARS: usize = 96;

/// Explicit collection gate shared by both operating systems.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CollectionGate {
    /// User completed the in-product consent flow.
    pub consented: bool,
    /// Platform permission probe currently succeeds.
    pub platform_permission: bool,
}

/// User-selected maximum semantic detail. Higher levels include the lower levels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum CaptureDetail {
    /// Foreground application identity only.
    Application,
    /// Application and active-window metadata.
    #[default]
    Window,
    /// Bounded accessibility roles and semantic action categories.
    Semantic,
}

/// A short-lived platform observation before policy evaluation and canonical event creation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlatformSnapshot {
    /// Foreground process identifier.
    pub process_id: Option<u32>,
    /// Application label exposed by the accessibility service.
    pub application: Option<String>,
    /// Active window title.
    pub window_title: Option<String>,
    /// Document identifier as exposed by the platform. Resolved into a project entity and then
    /// discarded before the canonical event is constructed; the raw value never reaches storage.
    pub document: Option<String>,
    /// Focused accessibility role.
    pub role: Option<String>,
    /// Focused accessibility subrole, used to suppress secure text fields.
    pub subrole: Option<String>,
    /// Bounded semantic label. Raw values and selected text are never read.
    pub context: Option<String>,
}

/// Converts a transition into minimized canonical events after privacy policy evaluation.
///
/// Excluded snapshots are dropped before an [`EventEnvelope`] is constructed. `first_tick` is the
/// first monotonic value assigned to this transition. `resolver` turns `current.document` into a
/// project entity identifier immediately, so the raw path exists only for the duration of this
/// call and is never included in the returned events.
#[must_use]
pub fn canonical_events(
    previous: Option<&PlatformSnapshot>,
    current: &PlatformSnapshot,
    detail: CaptureDetail,
    policy: &PrivacyPolicy,
    resolver: &Resolver,
    occurred_at: DateTime<FixedOffset>,
    first_tick: u64,
) -> Vec<EventEnvelope> {
    if policy.evaluate(CandidateContext {
        application: current.application.as_deref(),
        application_id: None,
        window_title: current.window_title.as_deref(),
        url: None,
        private_context: false,
    }) != PolicyDecision::Allow
    {
        return Vec::new();
    }

    let app_changed = previous.is_none_or(|value| {
        value.process_id != current.process_id || value.application != current.application
    });
    let window_changed = previous.is_none_or(|value| {
        value.window_title != current.window_title || value.document != current.document
    });
    let control_changed = previous.is_none_or(|value| {
        value.role != current.role
            || value.subrole != current.subrole
            || value.context != current.context
    });

    let source = source_identity(current);
    let mut next_tick = first_tick;
    let mut events = Vec::with_capacity(3);
    let mut push = |quality, payload| {
        events.push(EventEnvelope::new(
            occurred_at,
            next_tick,
            source.clone(),
            quality,
            payload,
        ));
        next_tick = next_tick.saturating_add(1);
    };

    if app_changed {
        push(
            CaptureQuality::ApplicationOnly,
            SemanticPayload::ApplicationActivated,
        );
    }
    if detail >= CaptureDetail::Window && window_changed {
        let project_id = resolve_project_id(resolver, current, &source);
        push(
            CaptureQuality::Window,
            SemanticPayload::WindowChanged {
                window_title: minimized(current.window_title.as_deref(), MAX_WINDOW_CHARS),
                project_id,
            },
        );
    }
    if detail >= CaptureDetail::Semantic
        && control_changed
        && let Some(role) = minimized(current.role.as_deref(), MAX_ROLE_CHARS)
    {
        let context = if current.subrole.as_deref() == Some("AXSecureTextField") {
            None
        } else {
            minimized(current.context.as_deref(), MAX_CONTEXT_CHARS)
        };
        push(
            CaptureQuality::Semantic,
            SemanticPayload::ControlAction {
                role,
                action: "focus".to_owned(),
                context,
            },
        );
    }
    events
}

fn source_identity(snapshot: &PlatformSnapshot) -> SourceIdentity {
    SourceIdentity {
        source_id: snapshot.process_id.map_or_else(
            || "macos-accessibility:unknown".to_owned(),
            |pid| format!("macos-accessibility:{pid}"),
        ),
        adapter: AdapterKind::MacOsAccessibility,
        application: ApplicationIdentity {
            display_name: minimized(snapshot.application.as_deref(), MAX_APPLICATION_CHARS),
            platform_id: None,
            process_id: snapshot.process_id,
        },
    }
}

fn minimized(value: Option<&str>, maximum: usize) -> Option<String> {
    let normalized = value?
        .chars()
        .filter(|character| !character.is_control())
        .collect::<String>();
    let collapsed = normalized.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return None;
    }
    Some(collapsed.chars().take(maximum).collect())
}

/// Resolves the project entity for a snapshot, returning its identifier as a string.
///
/// This is the only place the raw `current.document` value is read: it is passed to the resolver
/// and then dropped, never reaching the returned string.
fn resolve_project_id(
    resolver: &Resolver,
    current: &PlatformSnapshot,
    source: &SourceIdentity,
) -> Option<String> {
    let provenance = openhistory_domain::EntityProvenance {
        adapter: source.adapter,
        source_id: source.source_id.clone(),
    };
    let resolution = resolver.resolve(
        &WindowObservation {
            application: current.application.as_deref(),
            application_id: source.application.platform_id.as_deref(),
            window_title: current.window_title.as_deref(),
            document_path: current.document.as_deref(),
            url: None,
        },
        &provenance,
    );
    resolution
        .entity(EntityKind::Project)
        .map(|entity| entity.id.to_string())
}

impl CollectionGate {
    /// Collection can run only while both controls are positive.
    #[must_use]
    pub const fn is_open(self) -> bool {
        self.consented && self.platform_permission
    }
}

/// Target-selected adapter facade. Native observer implementations replace the fixture pump.
pub struct PlatformAdapter {
    gate: CollectionGate,
    detail: CaptureDetail,
    policy: PrivacyPolicy,
    resolver: Resolver,
    running: Arc<AtomicBool>,
    #[cfg(target_os = "macos")]
    stop: Option<tokio::sync::watch::Sender<bool>>,
    #[cfg(target_os = "macos")]
    task: Option<tokio::task::JoinHandle<()>>,
}

impl Default for PlatformAdapter {
    fn default() -> Self {
        Self {
            gate: CollectionGate::default(),
            detail: CaptureDetail::default(),
            policy: PrivacyPolicy::default(),
            resolver: Resolver::new(ProjectRegistry::default(), None),
            running: Arc::new(AtomicBool::new(false)),
            #[cfg(target_os = "macos")]
            stop: None,
            #[cfg(target_os = "macos")]
            task: None,
        }
    }
}

impl PlatformAdapter {
    /// Updates consent and current permission atomically from the core's perspective.
    pub fn set_gate(&mut self, gate: CollectionGate) {
        self.gate = gate;
        if !gate.is_open() {
            self.running.store(false, Ordering::Release);
            #[cfg(target_os = "macos")]
            if let Some(stop) = &self.stop {
                let _ = stop.send(true);
            }
        }
    }

    /// Selects the maximum user-consented capture detail.
    pub const fn set_capture_detail(&mut self, detail: CaptureDetail) {
        self.detail = detail;
    }

    /// Replaces the exclusion policy used before event construction.
    pub fn set_privacy_policy(&mut self, policy: PrivacyPolicy) {
        self.policy = policy;
    }

    /// Replaces the opted-in repository roots used to resolve project entities.
    ///
    /// A document outside every registered root resolves to no project, matching the
    /// repository-opt-in requirement: adding a repository here is what turns its paths into
    /// reportable project identity.
    pub fn set_project_registry(
        &mut self,
        registry: ProjectRegistry,
        home_directory: Option<String>,
    ) {
        self.resolver = Resolver::new(registry, home_directory);
    }

    /// Reports visible adapter health without source content.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }
}

#[async_trait]
impl ActivityAdapter for PlatformAdapter {
    fn id(&self) -> &'static str {
        if cfg!(target_os = "macos") {
            "macos-accessibility"
        } else if cfg!(target_os = "windows") {
            "windows-automation"
        } else {
            "unsupported-platform"
        }
    }

    async fn start(&mut self, sender: EventSender) -> Result<(), AdapterError> {
        if !self.gate.is_open() {
            self.running.store(false, Ordering::Release);
            return Err(AdapterError::PermissionRequired);
        }

        #[cfg(target_os = "macos")]
        {
            if !macos::permission_granted() {
                self.running.store(false, Ordering::Release);
                return Err(AdapterError::PermissionRequired);
            }
            if let Some(task) = self.task.take() {
                task.abort();
            }
            let (stop, receiver) = tokio::sync::watch::channel(false);
            let running = Arc::clone(&self.running);
            running.store(true, Ordering::Release);
            self.stop = Some(stop);
            self.task = Some(tokio::spawn(macos::run_collector(
                sender,
                receiver,
                self.detail,
                self.policy.clone(),
                self.resolver.clone(),
                running,
            )));
            return Ok(());
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = sender;
            self.running.store(false, Ordering::Release);
            return Err(AdapterError::Unavailable);
        }
        #[allow(unreachable_code)]
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), AdapterError> {
        self.running.store(false, Ordering::Release);
        #[cfg(target_os = "macos")]
        {
            if let Some(stop) = self.stop.take() {
                let _ = stop.send(true);
            }
            if let Some(task) = self.task.take() {
                let _ = task.await;
            }
        }
        Ok(())
    }
}

impl Drop for PlatformAdapter {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

/// Returns the current platform permission probe without requesting a new permission.
#[cfg(target_os = "macos")]
#[must_use]
pub fn platform_permission_granted() -> bool {
    macos::permission_granted()
}

/// Windows UI Automation does not use the macOS-style trust prompt.
#[cfg(target_os = "windows")]
#[must_use]
pub const fn platform_permission_granted() -> bool {
    true
}

/// Unsupported targets cannot collect semantic platform events.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[must_use]
pub const fn platform_permission_granted() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use openhistory_adapters::bounded_event_channel;

    use super::*;

    #[tokio::test]
    async fn consent_and_permission_are_both_required_and_revocable() {
        let (sender, _receiver) = bounded_event_channel(4);
        let mut adapter = PlatformAdapter::default();
        assert_eq!(
            adapter.start(sender.clone()).await,
            Err(AdapterError::PermissionRequired)
        );
        adapter.set_gate(CollectionGate {
            consented: true,
            platform_permission: true,
        });
        assert!(adapter.gate.is_open());
        adapter.set_gate(CollectionGate {
            consented: true,
            platform_permission: false,
        });
        assert!(!adapter.is_running());
    }

    fn snapshot(app: &str, window: &str) -> PlatformSnapshot {
        PlatformSnapshot {
            process_id: Some(42),
            application: Some(app.to_owned()),
            window_title: Some(window.to_owned()),
            document: Some(format!("/private/path/{window}")),
            role: Some("AXTextArea".to_owned()),
            subrole: None,
            context: Some("Editor pane".to_owned()),
        }
    }

    #[test]
    fn capture_detail_is_explicit_and_project_identity_is_resolved_not_hashed() {
        let current = snapshot("Editor", "OpenHistory");
        let at = chrono::DateTime::parse_from_rfc3339("2026-09-08T16:50:00+08:00").unwrap();
        // Registering the document's parent directory as an opted-in repository is what lets the
        // resolver turn the raw path into a project identity at all.
        let resolver = Resolver::new(ProjectRegistry::new(["/private/path"]), None);
        let window = canonical_events(
            None,
            &current,
            CaptureDetail::Window,
            &PrivacyPolicy::default(),
            &resolver,
            at,
            10,
        );
        assert_eq!(window.len(), 2);
        assert_eq!(window[0].quality, CaptureQuality::ApplicationOnly);
        let SemanticPayload::WindowChanged { project_id, .. } = &window[1].payload else {
            panic!("expected window event");
        };
        assert_ne!(project_id.as_deref(), current.document.as_deref());
        let expected = openhistory_domain::EntityId::derive(
            &openhistory_domain::EntityKey::repository_path("/private/path").unwrap(),
        );
        assert_eq!(project_id.as_deref(), Some(expected.as_str()));

        let semantic = canonical_events(
            None,
            &current,
            CaptureDetail::Semantic,
            &PrivacyPolicy::default(),
            &resolver,
            at,
            20,
        );
        assert_eq!(semantic.len(), 3);
    }

    #[test]
    fn an_unregistered_document_resolves_no_project() {
        let current = snapshot("Editor", "OpenHistory");
        let at = chrono::DateTime::parse_from_rfc3339("2026-09-08T16:50:00+08:00").unwrap();
        let resolver = Resolver::default();
        let window = canonical_events(
            None,
            &current,
            CaptureDetail::Window,
            &PrivacyPolicy::default(),
            &resolver,
            at,
            10,
        );
        let SemanticPayload::WindowChanged { project_id, .. } = &window[1].payload else {
            panic!("expected window event");
        };
        assert_eq!(*project_id, None);
    }

    #[test]
    fn exclusions_happen_before_events_and_secure_context_is_suppressed() {
        let mut current = snapshot("Editor", "secret plan");
        let at = chrono::DateTime::parse_from_rfc3339("2026-09-08T16:50:00+08:00").unwrap();
        let policy = PrivacyPolicy {
            window_patterns: vec!["secret".to_owned()],
            ..PrivacyPolicy::default()
        };
        let resolver = Resolver::default();
        assert!(
            canonical_events(
                None,
                &current,
                CaptureDetail::Semantic,
                &policy,
                &resolver,
                at,
                0
            )
            .is_empty()
        );

        current.window_title = Some("Allowed".to_owned());
        current.subrole = Some("AXSecureTextField".to_owned());
        current.context = Some("must-never-persist".to_owned());
        let events = canonical_events(
            None,
            &current,
            CaptureDetail::Semantic,
            &PrivacyPolicy::default(),
            &resolver,
            at,
            0,
        );
        let SemanticPayload::ControlAction { context, .. } = &events[2].payload else {
            panic!("expected semantic event");
        };
        assert!(context.is_none());
    }
}
