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
        AX_DESCRIPTION_ATTRIBUTE, AX_DOCUMENT_ATTRIBUTE, AX_ROLE_ATTRIBUTE,
        AX_SUBROLE_ATTRIBUTE, AX_TITLE_ATTRIBUTE,
    },
    ax_notification::{
        AX_APPLICATION_ACTIVATED_NOTIFICATION, AX_FOCUSED_UI_ELEMENT_CHANGED_NOTIFICATION,
        AX_FOCUSED_WINDOW_CHANGED_NOTIFICATION, AX_MAIN_WINDOW_CHANGED_NOTIFICATION,
        AX_MENU_ITEM_SELECTED_NOTIFICATION, AX_SELECTED_TEXT_CHANGED_NOTIFICATION,
        AX_TITLE_CHANGED_NOTIFICATION, AX_VALUE_CHANGED_NOTIFICATION,
    },
    is_process_trusted, system_wide,
};
use chrono::Local;
use openhistory_adapters::{AdapterError, EventSender};
use openhistory_domain::{CaptureQuality, EventEnvelope, SemanticPayload};
use openhistory_privacy::{CandidateContext, PolicyDecision, PrivacyPolicy};
use tokio::sync::watch;

use crate::{
    CaptureDetail, MAX_CONTEXT_CHARS, MAX_ROLE_CHARS, PlatformSnapshot, canonical_events,
    minimized, source_identity,
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

pub(super) fn permission_granted() -> bool {
    is_process_trusted()
}

pub(super) async fn run_collector(
    sender: EventSender,
    mut stop: watch::Receiver<bool>,
    detail: CaptureDetail,
    policy: PrivacyPolicy,
    running: Arc<AtomicBool>,
) {
    let mut previous: Option<PlatformSnapshot> = None;
    let mut ticks = 0_u64;
    let mut observed_pid = None;
    let mut stream: Option<AXNotificationStream> = None;
    let mut probe = tokio::time::interval(Duration::from_millis(750));
    probe.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            changed = stop.changed() => {
                if changed.is_err() || *stop.borrow() {
                    break;
                }
            }
            _ = probe.tick() => {
                if !permission_granted() {
                    let _ = sender.send(Err(AdapterError::PermissionRequired)).await;
                    break;
                }
                let Ok((current, application)) = snapshot() else {
                    continue;
                };
                if current.process_id != observed_pid {
                    observed_pid = current.process_id;
                    stream = application.as_ref().and_then(|element| {
                        AXNotificationStream::subscribe_many(element, NOTIFICATIONS, 64)
                            .or_else(|_| {
                                AXNotificationStream::subscribe_many(
                                    element,
                                    FALLBACK_NOTIFICATIONS,
                                    64,
                                )
                            })
                            .ok()
                    });
                }
                let events = canonical_events(
                    previous.as_ref(),
                    &current,
                    detail,
                    &policy,
                    Local::now().fixed_offset(),
                    ticks,
                );
                ticks = ticks.saturating_add(u64::try_from(events.len()).unwrap_or(u64::MAX));
                if !send_all(&sender, events).await {
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
                let events = canonical_events(
                    previous.as_ref(),
                    &current,
                    detail,
                    &policy,
                    Local::now().fixed_offset(),
                    ticks,
                );
                ticks = ticks.saturating_add(u64::try_from(events.len()).unwrap_or(u64::MAX));
                if !send_all(&sender, events).await {
                    break;
                }
                if detail >= CaptureDetail::Semantic
                    && is_semantic_action(&event.notification)
                    && let Some(action) = action_for_notification(&event.notification)
                    && let Some(value) = semantic_action(&current, action, &policy, ticks)
                {
                    ticks = ticks.saturating_add(1);
                    if sender.send(Ok(value)).await.is_err() {
                        break;
                    }
                }
                previous = Some(current);
            }
        }
    }
    running.store(false, Ordering::Release);
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
