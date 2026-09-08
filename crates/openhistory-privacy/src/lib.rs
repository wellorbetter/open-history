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
    /// Application display name or platform identity.
    pub application: Option<&'a str>,
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
                host == configured || host.ends_with(&format!(".{configured}"))
            }) {
                return PolicyDecision::Exclude(ExclusionReason::Website);
            }
        }

        PolicyDecision::Allow
    }
}

fn matches_any(value: Option<&str>, patterns: &[String], contains: bool) -> bool {
    value.is_some_and(|value| {
        let value = value.to_ascii_lowercase();
        patterns.iter().any(|pattern| {
            let pattern = pattern.to_ascii_lowercase();
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
                window_title: Some("unknown"),
                url: None,
                private_context: true,
            }),
            PolicyDecision::Exclude(ExclusionReason::PrivateContext)
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
}
