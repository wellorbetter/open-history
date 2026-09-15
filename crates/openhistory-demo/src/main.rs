//! End-to-end demonstration of the work-evidence-and-reporting pipeline on synthetic data.
//!
//! This is not a product surface. It exists to prove, by actually running it, that entity
//! resolution, Git evidence, agent-session evidence, and range digests compose into a report a
//! human could plausibly use for a status update — the scenario the whole change is built around.
//! Every name, message, and timestamp below is invented for this demonstration.

use std::fs;
use std::process::Command;

use chrono::{DateTime, FixedOffset, NaiveDate, TimeZone};
use openhistory_agent_sessions::{derive, session_event};
use openhistory_digest::{DateRange, EvidenceItem, EvidenceKind, aggregate_range, render_markdown};
use openhistory_domain::{AdapterKind, EntityKind, EntityProvenance, SemanticPayload};
use openhistory_entities::{ProjectRegistry, Resolver, WindowObservation};
use openhistory_repository::{RepositoryReader, apply_path_exclusions, commit_event};

const OFFSET_HOURS: i32 = 8; // Matches the +08:00 timestamps used elsewhere in this repository.

fn main() {
    let workspace = tempfile::tempdir().expect("create demo workspace");
    let repo_root = workspace.path().join("open-history");
    fs::create_dir_all(&repo_root).unwrap();

    setup_repository(&repo_root);
    let session_path = write_session_log(workspace.path());
    let project = resolve_project_entity(&repo_root, workspace.path());
    let commit_events = read_repository_evidence(&repo_root);
    let (evidence, session_evt) = derive_session_evidence(&session_path, &repo_root);
    let items = build_evidence(&project, &commit_events, &session_evt, &evidence);
    render_and_report(&items);
}

fn setup_repository(repo_root: &std::path::Path) {
    println!("== 1. Setting up a synthetic Git repository ==");
    init_repo(repo_root);
    commit_file(
        repo_root,
        "crates/openhistory-entities/src/lib.rs",
        "// resolver v1",
        "feat(entities): add deterministic resolver",
        at("2026-09-14", 11, 20),
    );
    commit_file(
        repo_root,
        "crates/openhistory-digest/src/lib.rs",
        "// digest v1",
        "feat(digest): aggregate evidence into work items",
        at("2026-09-15", 15, 5),
    );
    commit_file(
        repo_root,
        ".env",
        "SECRET=do-not-report-this",
        "chore: local env for testing",
        at("2026-09-15", 15, 40),
    );
    println!("  created 3 commits in {}", repo_root.display());
}

fn write_session_log(workspace: &std::path::Path) -> std::path::PathBuf {
    println!("\n== 2. Writing a synthetic Codex session log ==");
    let session_path = workspace.join("session-01JD3K7M4QWERTY.jsonl");
    fs::write(&session_path, SYNTHETIC_SESSION).unwrap();
    println!("  wrote {}", session_path.display());
    session_path
}

fn resolve_project_entity(
    repo_root: &std::path::Path,
    workspace: &std::path::Path,
) -> openhistory_domain::Entity {
    println!("\n== 3. Resolving window-activity entities ==");
    let registry = ProjectRegistry::new([repo_root.to_string_lossy().into_owned()]);
    let resolver = Resolver::new(registry, workspace.to_str().map(str::to_owned));
    let provenance = EntityProvenance {
        adapter: AdapterKind::MacOsAccessibility,
        source_id: "macos-demo".into(),
    };
    let editor_document = repo_root
        .join("crates/openhistory-entities/src/lib.rs")
        .to_string_lossy()
        .into_owned();
    let editor_resolution = resolver.resolve(
        &WindowObservation {
            application: Some("Visual Studio Code"),
            application_id: Some("com.microsoft.VSCode"),
            window_title: Some("lib.rs — open-history"),
            document_path: Some(&editor_document),
            url: None,
        },
        &provenance,
    );
    let project = editor_resolution
        .entity(EntityKind::Project)
        .expect("editor session resolves a project entity")
        .clone();
    println!(
        "  editor observation resolved project id={} label={:?}",
        project.id, project.label
    );
    project
}

fn read_repository_evidence(repo_root: &std::path::Path) -> Vec<openhistory_domain::EventEnvelope> {
    println!("\n== 4. Reading Git evidence (metadata only) ==");
    let reader = RepositoryReader::new(repo_root);
    let branch = reader.current_branch().unwrap();
    let commits = reader.commits_since(None).unwrap();
    let path_exclusions = vec![".env".to_owned()];
    let mut commit_events = Vec::new();
    for (index, commit) in commits.iter().enumerate() {
        let filtered = apply_path_exclusions(commit.changed_paths.clone(), &path_exclusions);
        commit_events.push(commit_event(
            repo_root,
            branch.as_deref(),
            commit,
            filtered,
            "git:open-history",
            u64::try_from(index).unwrap(),
        ));
    }
    println!(
        "  read {} commits on branch {:?}; .env changes excluded before persistence",
        commit_events.len(),
        branch
    );
    commit_events
}

fn derive_session_evidence(
    session_path: &std::path::Path,
    repo_root: &std::path::Path,
) -> (
    openhistory_agent_sessions::DerivedEvidence,
    openhistory_domain::EventEnvelope,
) {
    println!("\n== 5. Deriving agent-session evidence ==");
    let file = fs::File::open(session_path).unwrap();
    let evidence = derive(file, None).unwrap();
    println!(
        "  intent={:?}\n  latest_request={:?}\n  result={:?}\n  state={:?}\n  diagnostics={:?}",
        evidence.intent,
        evidence.latest_request,
        evidence.result,
        evidence.state,
        evidence.diagnostics
    );
    // The session's working directory (here, the fixture repo it ran in) is what correlates it to
    // a project entity, the same opted-in-repository rule window activity resolution follows.
    let registry = ProjectRegistry::new([repo_root.to_string_lossy().into_owned()]);
    let project_id = openhistory_agent_sessions::resolve_session_project(
        &registry,
        Some(&repo_root.to_string_lossy()),
    );
    println!("  session working directory resolved project id={project_id:?}");
    let session_evt = session_event(
        "01JD3K7M4QWERTY",
        &evidence,
        "codex:01JD3K7M4QWERTY",
        0,
        project_id,
    );
    (evidence, session_evt)
}

fn build_evidence(
    project: &openhistory_domain::Entity,
    commit_events: &[openhistory_domain::EventEnvelope],
    session_evt: &openhistory_domain::EventEnvelope,
    _evidence: &openhistory_agent_sessions::DerivedEvidence,
) -> Vec<EvidenceItem> {
    println!("\n== 6. Aggregating a week-range digest ==");
    let project_label = project
        .label
        .clone()
        .or_else(|| Some("open-history".into()));
    let mut items = vec![EvidenceItem {
        project_id: project.id.clone(),
        project_label: project_label.clone(),
        occurred_at: at("2026-09-14", 9, 30),
        kind: EvidenceKind::Attention { minutes: 95 },
    }];
    for event in commit_events {
        if let SemanticPayload::RepositoryCommit {
            commit_id, subject, ..
        } = &event.payload
        {
            items.push(EvidenceItem {
                project_id: project.id.clone(),
                project_label: project_label.clone(),
                occurred_at: event.occurred_at,
                kind: EvidenceKind::Commit {
                    commit_id: commit_id.clone(),
                    subject: subject.clone(),
                },
            });
        }
    }
    if let SemanticPayload::AgentSessionUpdate {
        thread_id, result, ..
    } = &session_evt.payload
    {
        items.push(EvidenceItem {
            project_id: project.id.clone(),
            project_label: project_label.clone(),
            occurred_at: session_evt.occurred_at,
            kind: EvidenceKind::AgentSession {
                thread_id: thread_id.clone(),
                summary: result.clone(),
            },
        });
    }
    // A second, unrelated project with only reading time — proves work with no artifacts still
    // shows up in the report.
    items.push(EvidenceItem {
        project_id: openhistory_domain::EntityId::derive(
            &openhistory_domain::EntityKey::host("docs.rs").unwrap(),
        ),
        project_label: Some("Reading: docs.rs".into()),
        occurred_at: at("2026-09-16", 14, 0),
        kind: EvidenceKind::Attention { minutes: 20 },
    });
    items
}

fn render_and_report(items: &[EvidenceItem]) {
    let range = DateRange::new(
        NaiveDate::from_ymd_opt(2026, 9, 14).unwrap(),
        NaiveDate::from_ymd_opt(2026, 9, 20).unwrap(),
    );
    let work_items = aggregate_range(items, range);
    let markdown = render_markdown(range, &work_items);

    println!("\n== 7. Rendered report draft ==\n");
    println!("{markdown}");

    let output_path = std::env::temp_dir().join("openhistory-demo-report.md");
    fs::write(&output_path, &markdown).unwrap();
    println!("(also written to {})", output_path.display());
}

fn at(day: &str, hour: u32, minute: u32) -> DateTime<FixedOffset> {
    let date = NaiveDate::parse_from_str(day, "%Y-%m-%d").unwrap();
    let offset = FixedOffset::east_opt(OFFSET_HOURS * 3600).unwrap();
    offset
        .from_local_datetime(&date.and_hms_opt(hour, minute, 0).unwrap())
        .unwrap()
}

fn git(repo: &std::path::Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .status()
        .expect("git must be installed for this demo");
    assert!(status.success(), "git {args:?} failed");
}

fn init_repo(repo: &std::path::Path) {
    git(repo, &["init", "--quiet", "--initial-branch=main"]);
    git(repo, &["config", "user.email", "demo@example.com"]);
    git(repo, &["config", "user.name", "OpenHistory Demo"]);
}

fn commit_file(
    repo: &std::path::Path,
    relative_path: &str,
    contents: &str,
    subject: &str,
    when: DateTime<FixedOffset>,
) {
    let file_path = repo.join(relative_path);
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&file_path, contents).unwrap();
    git(repo, &["add", relative_path]);
    let date = when.to_rfc3339();
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["commit", "--quiet", "-m", subject])
        .env("GIT_AUTHOR_DATE", &date)
        .env("GIT_COMMITTER_DATE", &date)
        .status()
        .unwrap();
    assert!(status.success(), "git commit failed");
}

const SYNTHETIC_SESSION: &str = r#"{"timestamp":"2026-09-15T07:10:00Z","type":"event_msg","payload":{"type":"task_started","turn_id":"turn-1"}}
{"timestamp":"2026-09-15T07:10:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Add a range digest that aggregates evidence into work items"}]}}
{"timestamp":"2026-09-15T07:10:02Z","type":"response_item","payload":{"type":"agent_message","message":"Working on the digest aggregation now."}}
{"timestamp":"2026-09-15T07:32:00Z","type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-1","last_agent_message":"Added openhistory-digest with range aggregation and Markdown export; all tests pass."}}
"#;
