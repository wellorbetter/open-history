//! Interpretation by a coding agent already installed and signed in on this machine.
//!
//! Everything else in this app reports only what was observed. This module is the one place that
//! asks a model what the observations meant, so it is deliberately narrow: it runs only when the
//! user asks for it, it sends only the evidence that is already on screen, and whatever comes back
//! is checked against that evidence before it is shown.
//!
//! Using a CLI the user has already authenticated is not the same as running a model locally. The
//! agent still sends the prompt to its vendor. Callers must say so where the user can read it.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use async_trait::async_trait;
use openhistory_summaries::{
    Summarizer, SummaryError, SummaryOutput, SummaryRequest, build_minimized_prompt,
    validate_output,
};

/// How long to wait for an agent before giving up. Generous, because these agents routinely take
/// tens of seconds, and a timeout here means the user gets nothing rather than something late.
const DEADLINE: Duration = Duration::from_secs(90);

/// Agent commands this app knows how to run non-interactively, in preference order.
///
/// Each entry is the executable name and the arguments that make it answer a single prompt and
/// exit. The prompt itself is appended as one final argument, never interpolated into a shell
/// string, so captured window titles cannot become commands.
const KNOWN_AGENTS: &[(&str, &[&str])] = &[("claude", &["-p"]), ("codex", &["exec"])];

/// Directories searched for an agent, on top of whatever `PATH` holds.
///
/// A desktop app launched from Finder inherits a nearly empty `PATH`, so the agent the user
/// installed into their shell profile is invisible to it. These are where those installs land.
fn extra_directories() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        directories.push(PathBuf::from(&home).join(".local/bin"));
        directories.push(PathBuf::from(&home).join(".bun/bin"));
        directories.push(PathBuf::from(&home).join(".volta/bin"));
        directories.push(PathBuf::from(&home).join(".npm-global/bin"));
    }
    directories.push(PathBuf::from("/opt/homebrew/bin"));
    directories.push(PathBuf::from("/usr/local/bin"));
    directories
}

/// A coding agent CLI on this machine, and how to make it answer one prompt.
pub struct LocalAgent {
    program: PathBuf,
    arguments: Vec<String>,
}

impl LocalAgent {
    /// Finds the first known agent installed on this machine, if any.
    #[must_use]
    pub fn detect() -> Option<Self> {
        let path = std::env::var("PATH").unwrap_or_default();
        let searched: Vec<PathBuf> = std::env::split_paths(&path)
            .chain(extra_directories())
            .collect();
        KNOWN_AGENTS.iter().find_map(|(name, arguments)| {
            searched
                .iter()
                .map(|directory| directory.join(name))
                .find(|candidate| is_executable(candidate))
                .map(|program| Self {
                    program,
                    arguments: arguments.iter().map(|value| (*value).to_owned()).collect(),
                })
        })
    }

    /// Names the agent that will answer, so the user can be told who is being asked.
    #[must_use]
    pub fn name(&self) -> String {
        self.program.file_name().map_or_else(
            || "local agent".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        )
    }
}

fn is_executable(candidate: &Path) -> bool {
    let Ok(metadata) = candidate.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    true
}

/// What an agent is allowed to return: the same shape as [`SummaryOutput`], except that an agent
/// that names no entities is treated as naming none rather than as returning nothing.
#[derive(serde::Deserialize)]
struct Draft {
    title: String,
    summary: String,
    #[serde(default)]
    entities: Vec<String>,
}

/// Pulls the JSON object out of an agent's reply.
///
/// Agents wrap answers in prose and fenced code blocks however they like, so the object is located
/// by its braces rather than by requiring the whole reply to parse.
fn parse_reply(reply: &str) -> Result<SummaryOutput, SummaryError> {
    let start = reply.find('{').ok_or(SummaryError::InvalidOutput)?;
    let end = reply.rfind('}').ok_or(SummaryError::InvalidOutput)?;
    let object = reply.get(start..=end).ok_or(SummaryError::InvalidOutput)?;
    let draft: Draft = serde_json::from_str(object).map_err(|_| SummaryError::InvalidOutput)?;
    Ok(SummaryOutput {
        title: draft.title,
        summary: draft.summary,
        entities: draft.entities,
    })
}

#[async_trait]
impl Summarizer for LocalAgent {
    async fn summarize(&self, request: &SummaryRequest) -> Result<SummaryOutput, SummaryError> {
        let prompt = build_minimized_prompt(request);
        let mut command = tokio::process::Command::new(&self.program);
        command
            .args(&self.arguments)
            .arg(&prompt)
            // These agents are themselves scripts that shell out to a runtime, so they need a
            // usable PATH even though this app was launched without one.
            .env("PATH", runnable_path())
            // Nothing is offered on stdin: an agent that decides to ask a question gets end-of-file
            // and exits, rather than waiting forever for an answer nobody can give it.
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let output = tokio::time::timeout(DEADLINE, command.output())
            .await
            .map_err(|_| SummaryError::Timeout)?
            .map_err(|_| SummaryError::Unavailable)?;
        if !output.status.success() {
            return Err(SummaryError::Unavailable);
        }
        let reply = String::from_utf8_lossy(&output.stdout);
        validate_output(request, parse_reply(&reply)?)
    }
}

/// A `PATH` a spawned agent can actually work in.
fn runnable_path() -> String {
    let inherited = std::env::var("PATH").unwrap_or_default();
    let mut seen = std::collections::BTreeSet::new();
    let directories: Vec<PathBuf> = std::env::split_paths(&inherited)
        .chain(extra_directories())
        .chain([PathBuf::from("/usr/bin"), PathBuf::from("/bin")])
        .filter(|directory| seen.insert(directory.clone()))
        .collect();
    std::env::join_paths(directories)
        .map_or(inherited, |value| value.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::{Draft, parse_reply};
    use openhistory_summaries::SummaryError;

    #[test]
    fn a_fenced_reply_still_parses() {
        let reply = "Here you go:\n```json\n{\"title\":\"Reviewing a diff\",\"summary\":\"Read a diff in Ghostty.\",\"entities\":[\"Ghostty\"]}\n```\n";
        let output = parse_reply(reply).expect("the object is findable inside the prose");
        assert_eq!(output.title, "Reviewing a diff");
        assert_eq!(output.entities, vec!["Ghostty".to_owned()]);
    }

    /// An agent that answers in prose has not answered. Guessing a title out of its text would be
    /// inventing the one thing this path exists to obtain honestly.
    #[test]
    fn a_reply_without_an_object_is_rejected() {
        assert_eq!(
            parse_reply("I could not tell what you were doing."),
            Err(SummaryError::InvalidOutput)
        );
    }

    /// Naming nothing is a legitimate answer; it just claims no entities to check.
    #[test]
    fn a_reply_naming_no_entities_is_accepted() {
        let draft: Draft =
            serde_json::from_str(r#"{"title":"A","summary":"B"}"#).expect("entities are optional");
        assert!(draft.entities.is_empty());
        assert_eq!(draft.title, "A");
        assert_eq!(draft.summary, "B");
    }
}
