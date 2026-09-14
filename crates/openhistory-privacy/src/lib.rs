//! Pre-persistence privacy policy evaluation.

use serde::{Deserialize, Serialize};
use url::Url;

/// User-configurable exclusions applied before durable writes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivacyPolicy {
    /// Case-insensitive application names or platform identifiers.
    pub applications: Vec<String>,
    /// Case-insensitive window-title fragments.
    pub window_patterns: Vec<String>,
    /// Exact hosts or parent domains.
    pub websites: Vec<String>,
}
/// Minimal candidate context. It must be dropped immediately after evaluation.
#[derive(Clone, Copy, Debug, Default)]
pub struct CandidateContext<'a> {
    /// Application display name.
    pub application: Option<&'a str>,
    /// Bundle identifier, package family, or executable identity.
    pub application_id: Option<&'a str>,
    /// Window title exposed by the platform.
    pub window_title: Option<&'a str>,
    /// Browser URL from an approved adapter.
    pub url: Option<&'a str>,
    /// Positive private-mode signal from the browser adapter.
    pub private_context: bool,
}

/// Non-identifying policy outcome safe for diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyDecision {
    /// Candidate can continue to minimization and persistence.
    Allow,
    /// Candidate must be dropped before event creation.
    Exclude(ExclusionReason),
}

/// Category-only reason that never repeats excluded content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExclusionReason {
    /// Application matched policy.
    Application,
    /// Window title matched policy.
    Window,
    /// Website host matched policy.
    Website,
    /// Adapter positively identified a private browser context.
    PrivateContext,
    /// URL was malformed and therefore could not be safely evaluated.
    MalformedUrl,
}

impl PrivacyPolicy {
    /// Evaluates a candidate without retaining or returning identifying input.
    #[must_use]
    pub fn evaluate(&self, candidate: CandidateContext<'_>) -> PolicyDecision {
        if candidate.private_context {
            return PolicyDecision::Exclude(ExclusionReason::PrivateContext);
        }

        if matches_any(candidate.application, &self.applications, false) {
            return PolicyDecision::Exclude(ExclusionReason::Application);
        }

        if matches_any(candidate.application_id, &self.applications, false) {
            return PolicyDecision::Exclude(ExclusionReason::Application);
        }

        if matches_any(candidate.window_title, &self.window_patterns, true) {
            return PolicyDecision::Exclude(ExclusionReason::Window);
        }

        if let Some(raw_url) = candidate.url {
            let Ok(url) = Url::parse(raw_url) else {
                return PolicyDecision::Exclude(ExclusionReason::MalformedUrl);
            };
            let Some(host) = url.host_str() else {
                return PolicyDecision::Exclude(ExclusionReason::MalformedUrl);
            };
            let host = host.trim_end_matches('.').to_ascii_lowercase();
            if self.websites.iter().any(|configured| {
                let configured = configured.trim_start_matches('.').to_ascii_lowercase();
                !configured.is_empty()
                    && (host == configured || host.ends_with(&format!(".{configured}")))
            }) {
                return PolicyDecision::Exclude(ExclusionReason::Website);
            }
        }

        PolicyDecision::Allow
    }

    /// Builds and forwards an event only after the candidate passes policy evaluation.
    ///
    /// The builder is intentionally lazy so excluded identifying content does not need to enter a
    /// canonical event, storage mock, or downstream diagnostic layer.
    ///
    /// # Errors
    ///
    /// Returns the forwarding callback's error for allowed candidates. Excluded candidates never
    /// call either callback and return their category-only decision.
    pub fn forward_if_allowed<T, E>(
        &self,
        candidate: CandidateContext<'_>,
        build: impl FnOnce() -> T,
        forward: impl FnOnce(T) -> Result<(), E>,
    ) -> Result<PolicyDecision, E> {
        let decision = self.evaluate(candidate);
        if decision == PolicyDecision::Allow {
            forward(build())?;
        }
        Ok(decision)
    }
}

fn matches_any(value: Option<&str>, patterns: &[String], contains: bool) -> bool {
    value.is_some_and(|value| {
        let value = value.to_ascii_lowercase();
        patterns.iter().any(|pattern| {
            let pattern = pattern.trim().to_ascii_lowercase();
            if pattern.is_empty() {
                return false;
            }
            if contains {
                value.contains(&pattern)
            } else {
                value == pattern
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> PrivacyPolicy {
        PrivacyPolicy {
            applications: vec!["1Password".into()],
            window_patterns: vec!["secret".into()],
            websites: vec!["bank.example".into()],
        }
    }

    #[test]
    fn application_window_and_subdomain_match_before_persistence() {
        assert_eq!(
            policy().evaluate(CandidateContext {
                application: Some("1password"),
                ..CandidateContext::default()
            }),
            PolicyDecision::Exclude(ExclusionReason::Application)
        );
        assert_eq!(
            policy().evaluate(CandidateContext {
                window_title: Some("My SECRET document"),
                ..CandidateContext::default()
            }),
            PolicyDecision::Exclude(ExclusionReason::Window)
        );
        assert_eq!(
            policy().evaluate(CandidateContext {
                url: Some("https://login.bank.example/path?token=never-retain"),
                ..CandidateContext::default()
            }),
            PolicyDecision::Exclude(ExclusionReason::Website)
        );
    }

    #[test]
    fn private_context_wins_without_inspecting_content() {
        assert_eq!(
            policy().evaluate(CandidateContext {
                application: Some("Browser"),
                application_id: None,
                window_title: Some("unknown"),
                url: None,
                private_context: true,
            }),
            PolicyDecision::Exclude(ExclusionReason::PrivateContext)
        );
    }

    #[test]
    fn platform_application_id_is_excluded() {
        assert_eq!(
            policy().evaluate(CandidateContext {
                application_id: Some("1PASSWORD"),
                ..CandidateContext::default()
            }),
            PolicyDecision::Exclude(ExclusionReason::Application)
        );
    }

    #[test]
    fn malformed_browser_url_fails_closed() {
        assert_eq!(
            policy().evaluate(CandidateContext {
                url: Some("not a url"),
                ..CandidateContext::default()
            }),
            PolicyDecision::Exclude(ExclusionReason::MalformedUrl)
        );
    }
    #[test]
    fn excluded_content_never_reaches_builder_storage_or_diagnostics() {
        let sensitive = "bank-token-never-forward";
        let mut built = false;
        let mut stored = Vec::new();
        let decision = policy()
            .forward_if_allowed(
                CandidateContext {
                    window_title: Some(sensitive),
                    url: Some("https://login.bank.example/private"),
                    ..CandidateContext::default()
                },
                || {
                    built = true;
                    sensitive.to_owned()
                },
                |value| {
                    stored.push(value);
                    Ok::<(), ()>(())
                },
            )
            .unwrap();

        assert!(!built);
        assert!(stored.is_empty());
        let diagnostic = format!("{decision:?}");
        assert_eq!(diagnostic, "Exclude(Window)");
        assert!(!diagnostic.contains(sensitive));
    }

    #[test]
    fn empty_patterns_do_not_exclude_everything() {
        let empty = PrivacyPolicy {
            applications: vec![String::new()],
            window_patterns: vec!["  ".into()],
            websites: vec![String::new()],
        };
        assert_eq!(
            empty.evaluate(CandidateContext {
                application: Some("Editor"),
                window_title: Some("OpenHistory"),
                url: Some("https://example.com"),
                private_context: false,
                ..CandidateContext::default()
            }),
            PolicyDecision::Allow
        );
    }
}
