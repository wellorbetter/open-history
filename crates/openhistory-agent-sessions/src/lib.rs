//! Incremental evidence derivation from local AI coding-agent session records.
//!
//! Targets the Codex rollout JSONL format: one JSON object per line, each carrying a `timestamp`
//! and a `type`-tagged `payload`. Two dialects are recognized — `response_item` (current) and
//! `event_msg` (legacy) — because both are observed in real local session stores.
//!
//! Reading is append-only and incremental: [`derive`] takes the byte offset it previously stopped
//! at and returns the offset to resume from, so a long-running session is re-read only where it
//! grew. A record this parser does not recognize is skipped with a diagnostic rather than treated
//! as a hard failure, so one malformed line or an unfamiliar upstream version degrades this source
//! without stopping capture.

use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};

use chrono::{DateTime, Utc};
use openhistory_domain::{
    AdapterKind, AgentSessionState, ApplicationIdentity, CaptureQuality, EntityId, EntityKey,
    EventEnvelope, SemanticPayload, SourceIdentity,
};
use openhistory_entities::ProjectRegistry;
use serde::Deserialize;
use thiserror::Error;

/// Failure reading a session record source.
#[derive(Debug, Error)]
pub enum SessionSourceError {
    /// The record source could not be opened or read.
    #[error("session record source could not be read: {0}")]
    Io(std::io::Error),
}

/// Diagnostic for one skipped or degraded record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionDiagnostic {
    /// A line was not valid JSON.
    MalformedJson {
        /// Line number within the file, starting at 1.
        line: usize,
    },
    /// A line was valid JSON but matched no recognized record shape.
    UnrecognizedRecord {
        /// Line number within the file, starting at 1.
        line: usize,
    },
}

/// Accumulated intent, latest activity, and result for one session, plus where reading stopped.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DerivedEvidence {
    /// First user request observed in the session.
    pub intent: Option<String>,
    /// Most recent user request observed in the session.
    pub latest_request: Option<String>,
    /// Most recent authoritative agent result, taken only from a completed turn.
    pub result: Option<String>,
    /// Current turn state.
    pub state: AgentSessionState,
    /// Timestamp of the last record folded into this evidence.
    pub updated_at: Option<DateTime<Utc>>,
    /// Byte offset to resume reading from on the next incremental read.
    pub read_offset: u64,
    /// Total lines folded so far, used to keep diagnostic line numbers correct across resumes.
    pub lines_read: usize,
    /// Records that did not parse or were not recognized, most recent last.
    pub diagnostics: Vec<SessionDiagnostic>,
}

/// Reads new records appended to `source` since `previous`, folding them into evidence.
///
/// When `previous` is `None`, or when the source is shorter than the previous read offset (the
/// record was truncated or rewritten), reading starts from the beginning. Otherwise reading resumes
/// at the previous offset, so a growing session is parsed incrementally rather than from scratch.
///
/// # Errors
///
/// Returns [`SessionSourceError`] when the source cannot be seeked or read. A malformed or
/// unrecognized individual line is not an error: it is recorded in
/// [`DerivedEvidence::diagnostics`] and reading continues.
pub fn derive<R: Read + Seek>(
    mut source: R,
    previous: Option<&DerivedEvidence>,
) -> Result<DerivedEvidence, SessionSourceError> {
    let length = source
        .seek(SeekFrom::End(0))
        .map_err(SessionSourceError::Io)?;
    let mut evidence = previous.cloned().unwrap_or_default();
    let resume_from = if evidence.read_offset > length {
        0
    } else {
        evidence.read_offset
    };
    if resume_from == 0 {
        evidence = DerivedEvidence::default();
    }

    source
        .seek(SeekFrom::Start(resume_from))
        .map_err(SessionSourceError::Io)?;
    let mut reader = BufReader::new(source);
    let mut line_number = if resume_from == 0 {
        0
    } else {
        evidence.lines_read
    };
    let mut line = String::new();
    loop {
        line.clear();
        let bytes_read = reader
            .read_line(&mut line)
            .map_err(SessionSourceError::Io)?;
        if bytes_read == 0 {
            break;
        }
        line_number += 1;
        let trimmed = line.trim_end_matches(['\n', '\r']);
        if !trimmed.is_empty() {
            fold_line(&mut evidence, trimmed, line_number);
        }
    }
    evidence.read_offset = length;
    evidence.lines_read = line_number;
    Ok(evidence)
}

/// Minimum length a bare token must reach before it is treated as credential-shaped on its own,
/// without a `key=`/`key:` prefix or a `Bearer ` marker to go on.
const MIN_BARE_TOKEN_LENGTH: usize = 24;

/// Replaces text that looks like a credential with `[REDACTED]` before it can be persisted.
///
/// Three independent patterns are covered, each redacting only the credential-shaped portion so
/// the surrounding request or result text stays readable: a `Bearer <token>` marker, a
/// `key=value`/`key: value` pair whose key names a credential, and a bare long token that mixes
/// letters and digits (an API key pasted with no label at all). This is deliberately conservative
/// about the bare-token case — favoring a false negative over redacting an ordinary long word —
/// because the surrounding request text is exactly what a status report needs to stay readable.
#[must_use]
fn redact_credentials(text: &str) -> String {
    let with_bearer_redacted = redact_bearer_tokens(text);
    let with_pairs_redacted = redact_key_value_pairs(&with_bearer_redacted);
    redact_bare_tokens(&with_pairs_redacted)
}

fn redact_bearer_tokens(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut words = text.split(' ').peekable();
    while let Some(word) = words.next() {
        if word.eq_ignore_ascii_case("bearer")
            && let Some(&next) = words.peek()
            && !next.is_empty()
        {
            result.push_str(word);
            result.push_str(" [REDACTED]");
            words.next();
        } else {
            result.push_str(word);
        }
        if words.peek().is_some() {
            result.push(' ');
        }
    }
    result
}

/// Key names that mark the value beside them as a credential, checked case-insensitively.
const CREDENTIAL_KEY_MARKERS: [&str; 5] = ["api_key", "apikey", "token", "secret", "password"];

fn redact_key_value_pairs(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut words = text.split(' ').peekable();
    while let Some(word) = words.next() {
        let separator = word.find(['=', ':']);
        if let Some(index) = separator {
            let (key, rest) = word.split_at(index);
            let value = &rest[1..];
            let lowered_key = key.to_ascii_lowercase();
            if !value.is_empty()
                && CREDENTIAL_KEY_MARKERS
                    .iter()
                    .any(|marker| lowered_key.contains(marker))
            {
                result.push_str(key);
                result.push(rest.as_bytes()[0] as char);
                result.push_str("[REDACTED]");
                if words.peek().is_some() {
                    result.push(' ');
                }
                continue;
            }
        }
        result.push_str(word);
        if words.peek().is_some() {
            result.push(' ');
        }
    }
    result
}

fn redact_bare_tokens(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut words = text.split(' ').peekable();
    while let Some(word) = words.next() {
        if looks_like_bare_credential(word) {
            result.push_str("[REDACTED]");
        } else {
            result.push_str(word);
        }
        if words.peek().is_some() {
            result.push(' ');
        }
    }
    result
}

/// A bare token reads as a credential when it is long, made only of identifier-safe characters,
/// and mixes letters with digits — prose and file paths rarely do all three at once.
fn looks_like_bare_credential(word: &str) -> bool {
    let trimmed = word.trim_matches(|character: char| character.is_ascii_punctuation());
    if trimmed.chars().count() < MIN_BARE_TOKEN_LENGTH {
        return false;
    }
    let identifier_safe = trimmed
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '-' || character == '_');
    let has_letter = trimmed
        .chars()
        .any(|character| character.is_ascii_alphabetic());
    let has_digit = trimmed.chars().any(|character| character.is_ascii_digit());
    identifier_safe && has_letter && has_digit
}

fn fold_line(evidence: &mut DerivedEvidence, line: &str, line_number: usize) {
    let Ok(record) = serde_json::from_str::<Record>(line) else {
        evidence
            .diagnostics
            .push(SessionDiagnostic::MalformedJson { line: line_number });
        return;
    };

    match record.payload.classify() {
        RecordOutcome::Update(update) => {
            evidence.updated_at = Some(record.timestamp);
            match update {
                RecordUpdate::UserMessage(text) => {
                    let text = redact_credentials(&text);
                    if evidence.intent.is_none() {
                        evidence.intent = Some(text.clone());
                    }
                    evidence.latest_request = Some(text);
                }
                RecordUpdate::TaskStarted => evidence.state = AgentSessionState::Active,
                RecordUpdate::TaskCompleted(result) => {
                    evidence.state = AgentSessionState::Idle;
                    if let Some(result) = result {
                        evidence.result = Some(redact_credentials(&result));
                    }
                }
                RecordUpdate::TurnAborted => evidence.state = AgentSessionState::Aborted,
            }
        }
        // A recognized shape with nothing to report (a developer rule, mid-turn chatter, tool
        // output) is not a diagnostic: the record was understood, it just carries no evidence.
        RecordOutcome::RecognizedNoUpdate => {}
        RecordOutcome::Unrecognized => {
            evidence
                .diagnostics
                .push(SessionDiagnostic::UnrecognizedRecord { line: line_number });
        }
    }
}

/// One line of a rollout file.
#[derive(Deserialize)]
struct Record {
    timestamp: DateTime<Utc>,
    #[serde(flatten)]
    payload: RecordEnvelope,
}

/// The `type`-tagged envelope wrapping either dialect's payload.
#[derive(Deserialize)]
#[serde(tag = "type")]
enum RecordEnvelope {
    #[serde(rename = "response_item")]
    ResponseItem { payload: ResponseItemPayload },
    #[serde(rename = "event_msg")]
    EventMsg { payload: EventMsgPayload },
    /// A top-level `type` this parser does not recognize at all, such as a future dialect.
    #[serde(other)]
    Unrecognized,
}

/// A meaning extracted from one record, independent of which dialect produced it.
enum RecordUpdate {
    UserMessage(String),
    TaskStarted,
    TaskCompleted(Option<String>),
    TurnAborted,
}

/// What folding one record accomplished.
enum RecordOutcome {
    /// The record was understood and changes evidence.
    Update(RecordUpdate),
    /// The record's shape is known but carries nothing reportable (a developer rule, mid-turn
    /// assistant chatter, tool output). This is not a diagnostic: it was understood, not skipped.
    RecognizedNoUpdate,
    /// The record's `type` matched no shape this parser recognizes.
    Unrecognized,
}

impl RecordEnvelope {
    fn classify(&self) -> RecordOutcome {
        match self {
            Self::ResponseItem { payload } => payload.classify(),
            Self::EventMsg { payload } => payload.classify(),
            Self::Unrecognized => RecordOutcome::Unrecognized,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum ResponseItemPayload {
    #[serde(rename = "message")]
    Message {
        role: String,
        content: Vec<TextContent>,
    },
    #[serde(rename = "agent_message")]
    AgentMessage {},
    #[serde(rename = "function_call_output")]
    FunctionCallOutput {},
    #[serde(other)]
    Other,
}

impl ResponseItemPayload {
    fn classify(&self) -> RecordOutcome {
        match self {
            Self::Message { role, content } if role == "user" => joined_text(content)
                .map_or(RecordOutcome::RecognizedNoUpdate, |text| {
                    RecordOutcome::Update(RecordUpdate::UserMessage(text))
                }),
            // Developer rules, mid-turn assistant chatter, and tool output carry no reportable
            // evidence: the developer role is not user intent, a freeform agent_message is not yet
            // the authoritative result (see `TaskCompleted`), and tool output is private. All three
            // shapes are still recognized, so they are not diagnosed as unrecognized.
            Self::Message { .. } | Self::AgentMessage {} | Self::FunctionCallOutput {} => {
                RecordOutcome::RecognizedNoUpdate
            }
            Self::Other => RecordOutcome::Unrecognized,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum EventMsgPayload {
    #[serde(rename = "task_started")]
    TaskStarted,
    #[serde(rename = "task_complete")]
    TaskComplete {
        #[serde(default)]
        last_agent_message: Option<String>,
    },
    #[serde(rename = "turn_aborted")]
    TurnAborted,
    #[serde(rename = "user_message")]
    UserMessage {
        #[serde(default)]
        message: Option<String>,
        #[serde(default)]
        content: Vec<TextContent>,
    },
    #[serde(rename = "agent_message")]
    AgentMessage {},
    #[serde(other)]
    Other,
}

impl EventMsgPayload {
    fn classify(&self) -> RecordOutcome {
        match self {
            Self::TaskStarted => RecordOutcome::Update(RecordUpdate::TaskStarted),
            Self::TaskComplete { last_agent_message } => {
                RecordOutcome::Update(RecordUpdate::TaskCompleted(last_agent_message.clone()))
            }
            Self::TurnAborted => RecordOutcome::Update(RecordUpdate::TurnAborted),
            Self::UserMessage { message, content } => message
                .clone()
                .or_else(|| joined_text(content))
                .map_or(RecordOutcome::RecognizedNoUpdate, |text| {
                    RecordOutcome::Update(RecordUpdate::UserMessage(text))
                }),
            // An agent_message under the legacy dialect is intermediate narration, not a result;
            // only `task_complete.last_agent_message` is authoritative (see module docs). The
            // shape is still recognized.
            Self::AgentMessage {} => RecordOutcome::RecognizedNoUpdate,
            Self::Other => RecordOutcome::Unrecognized,
        }
    }
}

#[derive(Deserialize)]
struct TextContent {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: Option<String>,
}

fn joined_text(content: &[TextContent]) -> Option<String> {
    let text = content
        .iter()
        .filter(|item| item.kind == "input_text")
        .filter_map(|item| item.text.as_deref())
        .collect::<Vec<_>>()
        .join("");
    (!text.is_empty()).then_some(text)
}

/// Resolves a session's working directory against opted-in repositories.
///
/// Returns `None` when there is no working directory to go on, or when it falls outside every
/// opted-in repository — a session in an unregistered directory correlates to no project, the
/// same rule window-activity resolution follows, rather than guessing from the path alone.
#[must_use]
pub fn resolve_session_project(
    registry: &ProjectRegistry,
    working_directory: Option<&str>,
) -> Option<String> {
    let root = registry.containing(working_directory?)?;
    let key = EntityKey::repository_path(root)?;
    Some(EntityId::derive(&key).to_string())
}

/// Builds a canonical event from derived evidence.
///
/// `project_id` is resolved by the caller (typically via [`resolve_session_project`]) rather than
/// computed here, so this function stays a pure mapping from already-derived evidence to an event
/// and never needs to know how a working directory was obtained.
#[must_use]
pub fn session_event(
    thread_id: &str,
    evidence: &DerivedEvidence,
    source_id: &str,
    monotonic_ticks: u64,
    project_id: Option<String>,
) -> EventEnvelope {
    let occurred_at = evidence.updated_at.unwrap_or_else(Utc::now).fixed_offset();
    EventEnvelope::new(
        occurred_at,
        monotonic_ticks,
        SourceIdentity {
            source_id: source_id.to_owned(),
            adapter: AdapterKind::Import,
            application: ApplicationIdentity {
                display_name: Some("Coding agent".into()),
                platform_id: None,
                process_id: None,
            },
        },
        CaptureQuality::Semantic,
        SemanticPayload::AgentSessionUpdate {
            thread_id: thread_id.to_owned(),
            intent: evidence.intent.clone(),
            latest_request: evidence.latest_request.clone(),
            result: evidence.result.clone(),
            state: evidence.state,
            project_id,
        },
    )
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn lines(records: &[&str]) -> Cursor<Vec<u8>> {
        let mut text = records.join("\n");
        text.push('\n');
        Cursor::new(text.into_bytes())
    }

    const USER_TURN_1: &str = r#"{"timestamp":"2030-01-01T00:00:02Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"add a session list"}]}}"#;
    const DEVELOPER_RULE: &str = r#"{"timestamp":"2030-01-01T00:00:00Z","type":"response_item","payload":{"type":"message","role":"developer","content":[{"type":"input_text","text":"private developer rule"}]}}"#;
    const TASK_STARTED_1: &str = r#"{"timestamp":"2030-01-01T00:00:01Z","type":"event_msg","payload":{"type":"task_started","turn_id":"turn-1"}}"#;
    const AGENT_CHATTER: &str = r#"{"timestamp":"2030-01-01T00:00:03Z","type":"response_item","payload":{"type":"agent_message","message":"working on it"}}"#;
    const TOOL_OUTPUT: &str = r#"{"timestamp":"2030-01-01T00:00:04Z","type":"response_item","payload":{"type":"function_call_output","output":"private tool output"}}"#;
    const TASK_COMPLETE_1: &str = r#"{"timestamp":"2030-01-01T00:00:05Z","type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-1","last_agent_message":"finished the read-only list."}}"#;
    const TASK_STARTED_2: &str = r#"{"timestamp":"2030-01-01T00:01:00Z","type":"event_msg","payload":{"type":"task_started","turn_id":"turn-2"}}"#;
    const USER_TURN_2: &str = r#"{"timestamp":"2030-01-01T00:01:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"check the edge cases"}]}}"#;

    #[test]
    fn folds_a_complete_turn_from_the_current_dialect() {
        let source = lines(&[
            DEVELOPER_RULE,
            TASK_STARTED_1,
            USER_TURN_1,
            AGENT_CHATTER,
            TOOL_OUTPUT,
            TASK_COMPLETE_1,
        ]);

        let evidence = derive(source, None).unwrap();

        assert_eq!(evidence.intent.as_deref(), Some("add a session list"));
        assert_eq!(
            evidence.latest_request.as_deref(),
            Some("add a session list")
        );
        assert_eq!(
            evidence.result.as_deref(),
            Some("finished the read-only list.")
        );
        assert_eq!(evidence.state, AgentSessionState::Idle);
        assert!(evidence.diagnostics.is_empty());
    }

    #[test]
    fn mid_turn_agent_chatter_never_becomes_the_result() {
        let source = lines(&[TASK_STARTED_1, USER_TURN_1, AGENT_CHATTER]);

        let evidence = derive(source, None).unwrap();

        assert_eq!(evidence.result, None, "no task_complete was observed yet");
        assert_eq!(evidence.state, AgentSessionState::Active);
    }

    #[test]
    fn legacy_dialect_folds_the_same_way() {
        let source = lines(&[
            r#"{"timestamp":"2031-02-01T01:00:00Z","type":"event_msg","payload":{"type":"task_started","turn_id":"legacy-1"}}"#,
            r#"{"timestamp":"2031-02-01T01:00:01Z","type":"event_msg","payload":{"type":"user_message","message":"analyze the cache issue"}}"#,
            r#"{"timestamp":"2031-02-01T01:00:02Z","type":"event_msg","payload":{"type":"agent_message","message":"not yet a result"}}"#,
            r#"{"timestamp":"2031-02-01T01:00:03Z","type":"event_msg","payload":{"type":"task_complete","turn_id":"legacy-1","last_agent_message":"cache analysis complete."}}"#,
        ]);

        let evidence = derive(source, None).unwrap();

        assert_eq!(evidence.intent.as_deref(), Some("analyze the cache issue"));
        assert_eq!(evidence.result.as_deref(), Some("cache analysis complete."));
        assert_eq!(evidence.state, AgentSessionState::Idle);
    }

    #[test]
    fn an_aborted_turn_is_reported_as_aborted() {
        let source = lines(&[
            r#"{"timestamp":"2031-02-01T01:01:00Z","type":"event_msg","payload":{"type":"task_started","turn_id":"legacy-2"}}"#,
            r#"{"timestamp":"2031-02-01T01:01:01Z","type":"event_msg","payload":{"type":"user_message","content":[{"type":"input_text","text":"stop this experiment"}]}}"#,
            r#"{"timestamp":"2031-02-01T01:01:02Z","type":"event_msg","payload":{"type":"turn_aborted","turn_id":"legacy-2","reason":"interrupt"}}"#,
        ]);

        let evidence = derive(source, None).unwrap();

        assert_eq!(evidence.state, AgentSessionState::Aborted);
        assert_eq!(
            evidence.latest_request.as_deref(),
            Some("stop this experiment")
        );
    }

    #[test]
    fn a_malformed_line_is_skipped_without_discarding_its_siblings() {
        let mut text = format!("{TASK_STARTED_1}\nnot json at all\n{USER_TURN_1}\n");
        text.push_str(TASK_COMPLETE_1);
        text.push('\n');
        let source = Cursor::new(text.into_bytes());

        let evidence = derive(source, None).unwrap();

        assert_eq!(
            evidence.latest_request.as_deref(),
            Some("add a session list")
        );
        assert_eq!(
            evidence.diagnostics,
            vec![SessionDiagnostic::MalformedJson { line: 2 }]
        );
    }

    #[test]
    fn an_unrecognized_record_shape_is_diagnosed_not_fatal() {
        let source = lines(&[
            TASK_STARTED_1,
            r#"{"timestamp":"2030-01-01T00:00:02Z","type":"a_future_record_type","payload":{}}"#,
            TASK_COMPLETE_1,
        ]);

        let evidence = derive(source, None).unwrap();

        assert_eq!(evidence.state, AgentSessionState::Idle);
        assert_eq!(
            evidence.diagnostics,
            vec![SessionDiagnostic::UnrecognizedRecord { line: 2 }]
        );
    }

    #[test]
    fn a_growing_session_is_read_incrementally() {
        let mut text = format!("{TASK_STARTED_1}\n{USER_TURN_1}\n");
        let first_pass = derive(Cursor::new(text.clone().into_bytes()), None).unwrap();
        assert_eq!(first_pass.state, AgentSessionState::Active);

        text.push_str(TASK_COMPLETE_1);
        text.push('\n');
        text.push_str(TASK_STARTED_2);
        text.push('\n');
        text.push_str(USER_TURN_2);
        text.push('\n');

        // A tracking reader proves the incremental pass never seeks before the previous offset,
        // i.e. it genuinely resumes rather than happening to reprocess the same lines correctly.
        let mut tracked = TrackingReader::new(text.into_bytes());
        let second_pass = derive(&mut tracked, Some(&first_pass)).unwrap();

        assert!(tracked.min_seek_position >= first_pass.read_offset);
        assert_eq!(second_pass.intent.as_deref(), Some("add a session list"));
        assert_eq!(
            second_pass.latest_request.as_deref(),
            Some("check the edge cases")
        );
        assert_eq!(
            second_pass.result.as_deref(),
            Some("finished the read-only list.")
        );
        assert_eq!(second_pass.state, AgentSessionState::Active);
    }

    /// Wraps a cursor and records the lowest absolute position ever sought to, other than the
    /// length probe `derive` performs up front.
    struct TrackingReader {
        cursor: Cursor<Vec<u8>>,
        min_seek_position: u64,
        length: u64,
    }

    impl TrackingReader {
        fn new(data: Vec<u8>) -> Self {
            let length = u64::try_from(data.len()).unwrap();
            Self {
                cursor: Cursor::new(data),
                min_seek_position: length,
                length,
            }
        }
    }

    impl Read for TrackingReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.cursor.read(buf)
        }
    }

    impl Seek for TrackingReader {
        fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
            let resolved = self.cursor.seek(pos)?;
            if resolved != self.length {
                self.min_seek_position = self.min_seek_position.min(resolved);
            }
            Ok(resolved)
        }
    }

    #[test]
    fn a_truncated_or_rewritten_record_is_re_read_from_the_start() {
        let first_pass = derive(lines(&[TASK_STARTED_1, USER_TURN_1]), None).unwrap();

        let rewritten = derive(
            lines(&[r#"{"timestamp":"2030-02-01T00:00:00Z","type":"event_msg","payload":{"type":"task_started","turn_id":"new"}}"#]),
            Some(&first_pass),
        )
        .unwrap();

        assert_eq!(
            rewritten.latest_request, None,
            "prior evidence must not leak in"
        );
        assert_eq!(rewritten.state, AgentSessionState::Active);
    }

    #[test]
    fn session_event_carries_thread_identity_and_derived_fields() {
        let evidence =
            derive(lines(&[TASK_STARTED_1, USER_TURN_1, TASK_COMPLETE_1]), None).unwrap();

        let event = session_event(
            "thread-1",
            &evidence,
            "codex:thread-1",
            0,
            Some("project-id".into()),
        );

        match event.payload {
            SemanticPayload::AgentSessionUpdate {
                thread_id,
                intent,
                result,
                state,
                project_id,
                ..
            } => {
                assert_eq!(thread_id, "thread-1");
                assert_eq!(intent.as_deref(), Some("add a session list"));
                assert_eq!(result.as_deref(), Some("finished the read-only list."));
                assert_eq!(state, AgentSessionState::Idle);
                assert_eq!(project_id.as_deref(), Some("project-id"));
            }
            other => panic!("expected AgentSessionUpdate, got {other:?}"),
        }
    }

    #[test]
    fn bearer_tokens_are_redacted() {
        let redacted = redact_credentials("Authorization: Bearer sk-abcdef1234567890 please retry");
        assert!(!redacted.contains("sk-abcdef1234567890"));
        assert!(redacted.contains("Bearer [REDACTED]"));
        assert!(redacted.contains("please retry"));
    }

    #[test]
    fn key_value_credentials_are_redacted_but_the_key_name_stays_readable() {
        for pair in [
            "api_key=sk-liveabcdef1234567890",
            "token: ghp_abcdef1234567890xyz9",
        ] {
            let redacted = redact_credentials(pair);
            assert!(
                redacted.contains("[REDACTED]"),
                "expected redaction in {redacted}"
            );
            assert!(
                !redacted.contains("abcdef1234567890"),
                "leaked in {redacted}"
            );
        }
    }

    #[test]
    fn a_bare_long_alphanumeric_token_is_redacted() {
        let redacted = redact_credentials("here is the key AKIAABCDEFGHIJKLMNOP1234 for prod");
        assert!(!redacted.contains("AKIAABCDEFGHIJKLMNOP1234"));
        assert!(redacted.contains("[REDACTED]"));
        assert!(redacted.contains("here is the key"));
        assert!(redacted.contains("for prod"));
    }

    #[test]
    fn ordinary_request_text_is_never_touched() {
        let text =
            "Add a range digest that aggregates evidence into work items for the weekly report";
        assert_eq!(redact_credentials(text), text);
    }

    #[test]
    fn a_long_file_path_is_not_mistaken_for_a_credential() {
        // All-lowercase, no digits: fails the "mixes letters and digits" heuristic on purpose.
        let text = "wrote to crates/openhistory-agent-sessions/src/lib.rs successfully";
        assert_eq!(redact_credentials(text), text);
    }

    #[test]
    fn credentials_in_the_latest_request_and_result_are_redacted_before_being_stored() {
        let source = lines(&[
            r#"{"timestamp":"2030-01-01T00:00:00Z","type":"event_msg","payload":{"type":"task_started","turn_id":"turn-1"}}"#,
            r#"{"timestamp":"2030-01-01T00:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"use token=ghp_abcdef1234567890xyz to deploy"}]}}"#,
            r#"{"timestamp":"2030-01-01T00:00:02Z","type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-1","last_agent_message":"deployed using Bearer sk-abcdef1234567890"}}"#,
        ]);

        let evidence = derive(source, None).unwrap();

        assert!(
            !evidence
                .latest_request
                .as_ref()
                .unwrap()
                .contains("ghp_abcdef1234567890xyz")
        );
        assert!(
            !evidence
                .result
                .as_ref()
                .unwrap()
                .contains("sk-abcdef1234567890")
        );
    }

    #[test]
    fn a_session_in_an_opted_in_repository_resolves_that_project() {
        let registry = ProjectRegistry::new(["/work/open-history"]);
        let resolved =
            resolve_session_project(&registry, Some("/work/open-history/crates/foo")).unwrap();
        let expected = EntityId::derive(&EntityKey::repository_path("/work/open-history").unwrap());
        assert_eq!(resolved, expected.to_string());
    }

    #[test]
    fn a_session_outside_every_opted_in_repository_resolves_no_project() {
        let registry = ProjectRegistry::new(["/work/open-history"]);
        assert_eq!(
            resolve_session_project(&registry, Some("/work/unregistered")),
            None
        );
        assert_eq!(resolve_session_project(&registry, None), None);
    }

    #[test]
    fn a_session_with_no_corresponding_window_activity_is_still_represented() {
        // The session itself never observes window activity; it only derives from its own
        // records. Its canonical event still carries full evidence and a resolved project when
        // the working directory matches an opted-in repository, with no attention data implied.
        let registry = ProjectRegistry::new(["/work/open-history"]);
        let evidence =
            derive(lines(&[TASK_STARTED_1, USER_TURN_1, TASK_COMPLETE_1]), None).unwrap();
        let project_id = resolve_session_project(&registry, Some("/work/open-history"));

        let event = session_event("thread-1", &evidence, "codex:thread-1", 0, project_id);

        let SemanticPayload::AgentSessionUpdate {
            result, project_id, ..
        } = event.payload
        else {
            panic!("expected AgentSessionUpdate");
        };
        assert_eq!(result.as_deref(), Some("finished the read-only list."));
        assert!(project_id.is_some());
    }
}
