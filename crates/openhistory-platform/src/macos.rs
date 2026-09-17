//! macOS Accessibility observation behind the consent gate.

use std::{
    future::pending,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use axuielement::{
    AXUIElement,
    async_api::AXNotificationStream,
    ax_attribute::{
        AX_DESCRIPTION_ATTRIBUTE, AX_DOCUMENT_ATTRIBUTE, AX_ROLE_ATTRIBUTE, AX_SUBROLE_ATTRIBUTE,
        AX_TITLE_ATTRIBUTE,
    },
    ax_notification::{
        AX_APPLICATION_ACTIVATED_NOTIFICATION, AX_FOCUSED_UI_ELEMENT_CHANGED_NOTIFICATION,
        AX_FOCUSED_WINDOW_CHANGED_NOTIFICATION, AX_MAIN_WINDOW_CHANGED_NOTIFICATION,
        AX_MENU_ITEM_SELECTED_NOTIFICATION, AX_SELECTED_TEXT_CHANGED_NOTIFICATION,
        AX_TITLE_CHANGED_NOTIFICATION, AX_VALUE_CHANGED_NOTIFICATION,
    },
    is_process_trusted, is_process_trusted_with_prompt, system_wide,
};
use chrono::{DateTime, FixedOffset, Local};
use openhistory_adapters::{AdapterError, EventSender};
use openhistory_domain::{CaptureQuality, EventEnvelope, LifecycleBoundary, SemanticPayload};
use openhistory_entities::Resolver;
use openhistory_privacy::{CandidateContext, PolicyDecision, PrivacyPolicy};
use tokio::sync::watch;

use crate::{
    CaptureDetail, MAX_CONTEXT_CHARS, MAX_ROLE_CHARS, PlatformSnapshot, canonical_events,
    minimized, presence_marker, source_identity,
};

const NOTIFICATIONS: &[&str] = &[
    AX_APPLICATION_ACTIVATED_NOTIFICATION,
    AX_FOCUSED_WINDOW_CHANGED_NOTIFICATION,
    AX_MAIN_WINDOW_CHANGED_NOTIFICATION,
    AX_FOCUSED_UI_ELEMENT_CHANGED_NOTIFICATION,
    AX_MENU_ITEM_SELECTED_NOTIFICATION,
    AX_SELECTED_TEXT_CHANGED_NOTIFICATION,
    AX_TITLE_CHANGED_NOTIFICATION,
    AX_VALUE_CHANGED_NOTIFICATION,
];

const FALLBACK_NOTIFICATIONS: &[&str] = &[
    AX_APPLICATION_ACTIVATED_NOTIFICATION,
    AX_FOCUSED_WINDOW_CHANGED_NOTIFICATION,
    AX_MAIN_WINDOW_CHANGED_NOTIFICATION,
    AX_FOCUSED_UI_ELEMENT_CHANGED_NOTIFICATION,
];

/// How often the foreground state is polled.
const PROBE_INTERVAL: Duration = Duration::from_millis(750);

/// How far the wall clock may run between two probes before the gap is read as suspension rather
/// than scheduling jitter. Generous on purpose: a false suspension would wrongly void time the user
/// really did spend, and only a suspended process can fall this far behind a 750ms timer.
const SUSPENSION_THRESHOLD: chrono::Duration = chrono::Duration::seconds(30);

pub(super) fn permission_granted() -> bool {
    is_process_trusted()
}
/// Shows the system Accessibility trust prompt when not already granted. Returns immediately,
/// without prompting, if the process is already trusted.
pub(super) fn request_permission() -> bool {
    is_process_trusted_with_prompt()
}

pub(super) async fn run_collector(
    sender: EventSender,
    mut stop: watch::Receiver<bool>,
    detail: CaptureDetail,
    policy: PrivacyPolicy,
    resolver: Resolver,
    running: Arc<AtomicBool>,
) {
    let mut recorder = Recorder {
        sender,
        detail,
        policy,
        resolver,
        ticks: 0,
    };
    let mut previous: Option<PlatformSnapshot> = None;
    let mut observed_pid = None;
    let mut stream: Option<AXNotificationStream> = None;
    let mut probe = tokio::time::interval(PROBE_INTERVAL);
    probe.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // Collection starting is itself an observation: it marks everything before this instant as time
    // this collector cannot speak for, so no earlier activity is credited with the gap.
    if !recorder.mark(LifecycleBoundary::Resume, now()).await {
        running.store(false, Ordering::Release);
        return;
    }

    // The wall clock at the previous probe. The loop wakes every 750ms, so a far larger jump means
    // the process was not running in between — the machine suspended. That is a real observation of
    // absence, available without asking the system anything.
    let mut last_probe = now();
    let mut graceful = false;

    loop {
        tokio::select! {
            changed = stop.changed() => {
                if changed.is_err() || *stop.borrow() {
                    graceful = true;
                    break;
                }
            }
            _ = probe.tick() => {
                if !permission_granted() {
                    let _ = recorder.sender.send(Err(AdapterError::PermissionRequired)).await;
                    break;
                }
                let probed_at = now();
                if suspended(last_probe, probed_at) {
                    // Bracket the stretch nobody watched: alive at the last probe, alive again now.
                    if !recorder.mark(LifecycleBoundary::Sleep, last_probe).await
                        || !recorder.mark(LifecycleBoundary::Resume, probed_at).await
                    {
                        break;
                    }
                    // Nothing observed before the suspension can be compared against what is in
                    // front of us now, so the next snapshot is treated as a fresh start.
                    previous = None;
                }
                last_probe = probed_at;
                let Ok((current, application)) = snapshot() else {
                    continue;
                };
                if current.process_id != observed_pid {
                    observed_pid = current.process_id;
                    stream = subscribe(application.as_ref());
                }
                if !recorder.changes(previous.as_ref(), &current).await {
                    break;
                }
                previous = Some(current);
            }
            event = async {
                match stream.as_ref() {
                    Some(value) => value.next().await,
                    None => pending().await,
                }
            } => {
                let Some(event) = event else {
                    stream = None;
                    continue;
                };
                let Ok((current, _)) = snapshot() else {
                    continue;
                };
                if !recorder.changes(previous.as_ref(), &current).await
                    || !recorder.semantic_action(&event.notification, &current).await
                {
                    break;
                }
                previous = Some(current);
            }
        }
    }
    // A collector that was asked to stop knows exactly when it stopped watching, so it says so and
    // the last observation's attention ends here instead of running on to whatever is recorded next.
    if graceful {
        let _ = recorder.mark(LifecycleBoundary::Shutdown, now()).await;
    }
    running.store(false, Ordering::Release);
}

fn now() -> DateTime<FixedOffset> {
    Local::now().fixed_offset()
}

/// Everything needed to turn an observation into recorded events: where they go, how much detail is
/// permitted, and the tick counter that keeps them ordered. These travel together through every
/// branch of the collector, so they are one value rather than five parameters repeated at each site.
struct Recorder {
    sender: EventSender,
    detail: CaptureDetail,
    policy: PrivacyPolicy,
    resolver: Resolver,
    ticks: u64,
}

impl Recorder {
    /// Records a presence marker, reporting whether the receiver is still listening.
    async fn mark(&mut self, boundary: LifecycleBoundary, at: DateTime<FixedOffset>) -> bool {
        self.ticks = self.ticks.saturating_add(1);
        self.sender
            .send(Ok(presence_marker(boundary, at, self.ticks)))
            .await
            .is_ok()
    }

    /// Records whatever changed between `previous` and `current`. The timer and the notification
    /// stream both arrive at "something may have changed", so they decide what that means through
    /// this one path rather than two copies of it that can drift apart.
    async fn changes(
        &mut self,
        previous: Option<&PlatformSnapshot>,
        current: &PlatformSnapshot,
    ) -> bool {
        let events = canonical_events(
            previous,
            current,
            self.detail,
            &self.policy,
            &self.resolver,
            now(),
            self.ticks,
        );
        self.ticks = self
            .ticks
            .saturating_add(u64::try_from(events.len()).unwrap_or(u64::MAX));
        send_all(&self.sender, events).await
    }

    /// Records the semantic action a notification stands for, when the configured detail level
    /// allows one and privacy policy permits it. A notification carrying no reportable action is an
    /// ordinary outcome, not a failure, so it still reports the receiver as listening.
    async fn semantic_action(&mut self, notification: &str, current: &PlatformSnapshot) -> bool {
        if self.detail < CaptureDetail::Semantic || !is_semantic_action(notification) {
            return true;
        }
        let Some(action) = action_for_notification(notification) else {
            return true;
        };
        let Some(value) = semantic_action(current, action, &self.policy, self.ticks) else {
            return true;
        };
        self.ticks = self.ticks.saturating_add(1);
        self.sender.send(Ok(value)).await.is_ok()
    }
}

/// Observes one application, falling back to the smaller notification set when the full one is
/// refused. No stream at all is a degraded but valid state: the probe timer still reports changes.
fn subscribe(application: Option<&AXUIElement>) -> Option<AXNotificationStream> {
    let element = application?;
    AXNotificationStream::subscribe_many(element, NOTIFICATIONS, 64)
        .or_else(|_| AXNotificationStream::subscribe_many(element, FALLBACK_NOTIFICATIONS, 64))
        .ok()
}

/// Whether the wall clock ran further between two probes than a running process could have fallen
/// behind a 750ms timer — which means it was not running, and the machine suspended.
///
/// Callers bracket the gap with `Sleep` at `last_probe` and `Resume` at `probed_at`: the machine went
/// down at some unknown moment after that probe, so crediting attention only up to the probe itself
/// never overstates it.
fn suspended(last_probe: DateTime<FixedOffset>, probed_at: DateTime<FixedOffset>) -> bool {
    probed_at.signed_duration_since(last_probe) > SUSPENSION_THRESHOLD
}

fn snapshot() -> Result<(PlatformSnapshot, Option<AXUIElement>), AdapterError> {
    let system = system_wide().ok_or(AdapterError::Unavailable)?;
    let application = system
        .focused_application()
        .map_err(|_| AdapterError::Unavailable)?;
    let window = system.focused_window().ok().flatten();
    let control = system.focused_ui_element().ok().flatten();
    let process_id = application
        .as_ref()
        .and_then(|value| value.pid().ok())
        .and_then(|value| u32::try_from(value).ok());

    let value = PlatformSnapshot {
        process_id,
        application: string_attribute(application.as_ref(), AX_TITLE_ATTRIBUTE),
        window_title: string_attribute(window.as_ref(), AX_TITLE_ATTRIBUTE),
        document: string_attribute(window.as_ref(), AX_DOCUMENT_ATTRIBUTE),
        role: string_attribute(control.as_ref(), AX_ROLE_ATTRIBUTE),
        subrole: string_attribute(control.as_ref(), AX_SUBROLE_ATTRIBUTE),
        context: string_attribute(control.as_ref(), AX_TITLE_ATTRIBUTE)
            .or_else(|| string_attribute(control.as_ref(), AX_DESCRIPTION_ATTRIBUTE)),
    };
    Ok((value, application))
}

fn string_attribute(element: Option<&AXUIElement>, attribute: &str) -> Option<String> {
    element?.string_attribute(attribute).ok().flatten()
}

async fn send_all(sender: &EventSender, events: Vec<EventEnvelope>) -> bool {
    for event in events {
        if sender.send(Ok(event)).await.is_err() {
            return false;
        }
    }
    true
}

fn is_semantic_action(notification: &str) -> bool {
    matches!(
        notification,
        AX_MENU_ITEM_SELECTED_NOTIFICATION
            | AX_SELECTED_TEXT_CHANGED_NOTIFICATION
            | AX_VALUE_CHANGED_NOTIFICATION
    )
}

fn action_for_notification(notification: &str) -> Option<&'static str> {
    match notification {
        AX_MENU_ITEM_SELECTED_NOTIFICATION => Some("select"),
        AX_SELECTED_TEXT_CHANGED_NOTIFICATION | AX_VALUE_CHANGED_NOTIFICATION => Some("edit"),
        _ => None,
    }
}

fn semantic_action(
    snapshot: &PlatformSnapshot,
    action: &str,
    policy: &PrivacyPolicy,
    ticks: u64,
) -> Option<EventEnvelope> {
    if policy.evaluate(CandidateContext {
        application: snapshot.application.as_deref(),
        application_id: None,
        window_title: snapshot.window_title.as_deref(),
        url: None,
        private_context: false,
    }) != PolicyDecision::Allow
    {
        return None;
    }
    let role = minimized(snapshot.role.as_deref(), MAX_ROLE_CHARS)?;
    let context = if snapshot.subrole.as_deref() == Some("AXSecureTextField") {
        None
    } else {
        minimized(snapshot.context.as_deref(), MAX_CONTEXT_CHARS)
    };
    Some(EventEnvelope::new(
        Local::now().fixed_offset(),
        ticks,
        source_identity(snapshot),
        CaptureQuality::Semantic,
        SemanticPayload::ControlAction {
            role,
            action: action.to_owned(),
            context,
        },
    ))
}
