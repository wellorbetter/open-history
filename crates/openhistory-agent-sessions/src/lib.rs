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
    AdapterKind, AgentSessionState, ApplicationIdentity, CaptureQuality, EventEnvelope,
    SemanticPayload, SourceIdentity,
};
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
                    if evidence.intent.is_none() {
                        evidence.intent = Some(text.clone());
                    }
                    evidence.latest_request = Some(text);
                }
                RecordUpdate::TaskStarted => evidence.state = AgentSessionState::Active,
                RecordUpdate::TaskCompleted(result) => {
                    evidence.state = AgentSessionState::Idle;
                    if let Some(result) = result {
                        evidence.result = Some(result);
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

/// Builds a canonical event from derived evidence.
#[must_use]
pub fn session_event(
    thread_id: &str,
    evidence: &DerivedEvidence,
    source_id: &str,
    monotonic_ticks: u64,
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

        let event = session_event("thread-1", &evidence, "codex:thread-1", 0);

        match event.payload {
            SemanticPayload::AgentSessionUpdate {
                thread_id,
                intent,
                result,
                state,
                ..
            } => {
                assert_eq!(thread_id, "thread-1");
                assert_eq!(intent.as_deref(), Some("add a session list"));
                assert_eq!(result.as_deref(), Some("finished the read-only list."));
                assert_eq!(state, AgentSessionState::Idle);
            }
            other => panic!("expected AgentSessionUpdate, got {other:?}"),
        }
    }
}
