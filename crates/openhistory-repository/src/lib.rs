//! Metadata-only Git repository evidence.
//!
//! Reads commit identifiers, subjects, branch, and changed-path statistics for one opted-in
//! repository. Never reads working-tree content, diff hunks, staged content, or remote
//! credentials: only `git log` metadata leaves this module.

use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::{DateTime, FixedOffset};
use openhistory_domain::{
    AdapterKind, ApplicationIdentity, CaptureQuality, EventEnvelope, SemanticPayload,
    SourceIdentity,
};
use thiserror::Error;

/// A single commit's metadata, read without touching file contents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitRecord {
    /// Full commit hash.
    pub commit_id: String,
    /// Commit timestamp as recorded by Git.
    pub committed_at: DateTime<FixedOffset>,
    /// Subject line, truncated by Git's own `%s` formatting rules.
    pub subject: Option<String>,
    /// Paths changed by this commit, before exclusion filtering.
    pub changed_paths: Vec<String>,
}

/// Failure reading repository metadata.
#[derive(Debug, Error)]
pub enum RepositoryError {
    /// `git` could not be launched.
    #[error("git could not be launched: {0}")]
    Launch(std::io::Error),
    /// `git` exited with a non-zero status.
    #[error("git exited with a failure status")]
    CommandFailed,
    /// Output was not valid UTF-8 or did not match the expected format.
    #[error("git output could not be parsed")]
    UnparsableOutput,
}

/// Reads metadata-only evidence from one Git repository root.
pub struct RepositoryReader {
    root: PathBuf,
}

/// Bound on how many commits an unanchored read returns.
///
/// An unbounded first read of a large repository would scan its entire history; this keeps the
/// first read proportional to recent activity, matching what a report over recent weeks needs.
const INITIAL_READ_LIMIT: usize = 200;

/// Separates fields within one `git log` record. Chosen because it cannot appear in a subject line
/// or path, unlike a comma or pipe.
const FIELD_SEPARATOR: &str = "\u{1f}";
/// Separates records. Chosen for the same reason as [`FIELD_SEPARATOR`].
const RECORD_SEPARATOR: &str = "\u{1e}";

impl RepositoryReader {
    /// Opens a reader for the repository rooted at `root`.
    ///
    /// Does not validate that `root` is a Git repository; that surfaces as a
    /// [`RepositoryError::CommandFailed`] on the first read.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Returns the current branch name, or `None` in a detached-head state.
    ///
    /// # Errors
    ///
    /// Returns [`RepositoryError::Launch`] when `git` cannot run.
    pub fn current_branch(&self) -> Result<Option<String>, RepositoryError> {
        let output = self.run(&["rev-parse", "--abbrev-ref", "HEAD"])?;
        let Some(output) = output else {
            return Ok(None);
        };
        let name = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        Ok((name != "HEAD" && !name.is_empty()).then_some(name))
    }

    /// Reads commits more recent than `since`, oldest first.
    ///
    /// When `since` is `None`, returns at most [`INITIAL_READ_LIMIT`] of the most recent commits
    /// rather than the full history, so a first read stays bounded regardless of repository size.
    ///
    /// # Errors
    ///
    /// Returns [`RepositoryError::CommandFailed`] when the path is not a Git repository or the
    /// commit named by `since` is no longer reachable (for example after a rebase or force-push);
    /// callers should retry with `since: None` to recover.
    pub fn commits_since(&self, since: Option<&str>) -> Result<Vec<CommitRecord>, RepositoryError> {
        // The record separator is a *prefix*: `--name-only` inserts an implicit newline after the
        // format text before listing changed paths, so a trailing separator would land before
        // those paths instead of after them, splitting each record apart from its own file list.
        let format = format!("{RECORD_SEPARATOR}%H{FIELD_SEPARATOR}%cI{FIELD_SEPARATOR}%s");
        let range = since.map_or_else(String::new, |commit| format!("{commit}..HEAD"));
        let mut args = vec![
            "log".to_owned(),
            "--name-only".to_owned(),
            format!("--pretty=format:{format}"),
        ];
        if since.is_none() {
            args.push(format!("-n{INITIAL_READ_LIMIT}"));
        }
        if !range.is_empty() {
            args.push(range);
        }
        let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
        let Some(output) = self.run(&borrowed)? else {
            return Err(RepositoryError::CommandFailed);
        };
        let text = String::from_utf8_lossy(&output.stdout);
        let mut commits = parse_log(&text)?;
        commits.reverse();
        Ok(commits)
    }

    fn run(&self, args: &[&str]) -> Result<Option<std::process::Output>, RepositoryError> {
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(args)
            .output()
            .map_err(RepositoryError::Launch)?;
        if output.status.success() {
            Ok(Some(output))
        } else {
            Ok(None)
        }
    }
}

/// Parses `git log --name-only --pretty=format:<record>` output built with [`FIELD_SEPARATOR`] and
/// [`RECORD_SEPARATOR`].
fn parse_log(text: &str) -> Result<Vec<CommitRecord>, RepositoryError> {
    text.split(RECORD_SEPARATOR)
        .map(str::trim)
        .filter(|record| !record.is_empty())
        .map(parse_record)
        .collect()
}

fn parse_record(record: &str) -> Result<CommitRecord, RepositoryError> {
    let mut lines = record.splitn(2, '\n');
    let header = lines.next().ok_or(RepositoryError::UnparsableOutput)?;
    let mut fields = header.split(FIELD_SEPARATOR);
    let commit_id = fields
        .next()
        .filter(|value| !value.is_empty())
        .ok_or(RepositoryError::UnparsableOutput)?
        .to_owned();
    let committed_at = fields
        .next()
        .ok_or(RepositoryError::UnparsableOutput)
        .and_then(|value| {
            DateTime::parse_from_rfc3339(value).map_err(|_| RepositoryError::UnparsableOutput)
        })?;
    let subject = fields
        .next()
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let changed_paths = lines
        .next()
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();

    Ok(CommitRecord {
        commit_id,
        committed_at,
        subject,
        changed_paths,
    })
}

/// Removes paths matching an exclusion pattern.
///
/// A pattern matches a path when the path contains it as a substring; this keeps configuration
/// simple (`node_modules/`, `.env`) while still catching nested occurrences.
#[must_use]
pub fn apply_path_exclusions(paths: Vec<String>, patterns: &[String]) -> Vec<String> {
    if patterns.is_empty() {
        return paths;
    }
    paths
        .into_iter()
        .filter(|path| {
            !patterns
                .iter()
                .any(|pattern| path.contains(pattern.as_str()))
        })
        .collect()
}

/// Builds a canonical event for one commit, after exclusion filtering has been applied to its
/// changed paths.
#[must_use]
pub fn commit_event(
    repository_root: &Path,
    branch: Option<&str>,
    commit: &CommitRecord,
    changed_paths: Vec<String>,
    source_id: &str,
    monotonic_ticks: u64,
) -> EventEnvelope {
    EventEnvelope::new(
        commit.committed_at,
        monotonic_ticks,
        SourceIdentity {
            source_id: source_id.to_owned(),
            adapter: AdapterKind::Import,
            application: ApplicationIdentity {
                display_name: Some("Git".into()),
                platform_id: None,
                process_id: None,
            },
        },
        CaptureQuality::Semantic,
        SemanticPayload::RepositoryCommit {
            repository_path: repository_root.to_string_lossy().into_owned(),
            commit_id: commit.commit_id.clone(),
            branch: branch.map(str::to_owned),
            subject: commit.subject.clone(),
            changed_paths,
        },
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::Command;

    use tempfile::TempDir;

    use super::*;

    fn init_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        let run = |args: &[&str]| {
            let status = Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .status()
                .unwrap();
            assert!(status.success(), "git {args:?} failed");
        };
        run(&["init", "--quiet", "--initial-branch=main"]);
        run(&["config", "user.email", "dev@example.com"]);
        run(&["config", "user.name", "Dev"]);
        dir
    }

    fn commit(dir: &TempDir, path: &str, contents: &str, subject: &str) {
        let file_path = dir.path().join(path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&file_path, contents).unwrap();
        let run = |args: &[&str]| {
            let status = Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .status()
                .unwrap();
            assert!(status.success(), "git {args:?} failed");
        };
        run(&["add", path]);
        run(&["commit", "--quiet", "-m", subject]);
    }

    #[test]
    fn reads_commits_with_subject_and_changed_paths() {
        let repo = init_repo();
        commit(&repo, "src/lib.rs", "fn main() {}", "feat: add entry point");
        commit(&repo, "README.md", "# Hello", "docs: add readme");

        let reader = RepositoryReader::new(repo.path());
        let commits = reader.commits_since(None).unwrap();

        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].subject.as_deref(), Some("feat: add entry point"));
        assert_eq!(commits[0].changed_paths, vec!["src/lib.rs"]);
        assert_eq!(commits[1].subject.as_deref(), Some("docs: add readme"));
        assert_eq!(reader.current_branch().unwrap().as_deref(), Some("main"));
    }

    #[test]
    fn incremental_read_returns_only_newer_commits() {
        let repo = init_repo();
        commit(&repo, "a.txt", "a", "first commit");
        let reader = RepositoryReader::new(repo.path());
        let first_batch = reader.commits_since(None).unwrap();
        let anchor = first_batch[0].commit_id.clone();

        commit(&repo, "b.txt", "b", "second commit");
        commit(&repo, "c.txt", "c", "third commit");

        let incremental = reader.commits_since(Some(&anchor)).unwrap();

        assert_eq!(incremental.len(), 2);
        assert_eq!(incremental[0].subject.as_deref(), Some("second commit"));
        assert_eq!(incremental[1].subject.as_deref(), Some("third commit"));
    }

    #[test]
    fn no_new_commits_since_the_anchor_yields_nothing() {
        let repo = init_repo();
        commit(&repo, "a.txt", "a", "only commit");
        let reader = RepositoryReader::new(repo.path());
        let anchor = reader.commits_since(None).unwrap()[0].commit_id.clone();

        assert!(reader.commits_since(Some(&anchor)).unwrap().is_empty());
    }

    #[test]
    fn an_unreachable_anchor_after_history_rewrite_fails_rather_than_guessing() {
        let repo = init_repo();
        commit(&repo, "a.txt", "a", "only commit");
        let reader = RepositoryReader::new(repo.path());

        let result = reader.commits_since(Some("0000000000000000000000000000000000dead"));

        assert!(matches!(result, Err(RepositoryError::CommandFailed)));
    }

    #[test]
    fn a_non_repository_path_fails_without_panicking() {
        let dir = TempDir::new().unwrap();
        let reader = RepositoryReader::new(dir.path());

        assert!(matches!(
            reader.commits_since(None),
            Err(RepositoryError::CommandFailed)
        ));
    }

    #[test]
    fn exclusion_patterns_remove_matching_paths_only() {
        let paths = vec![
            "src/lib.rs".to_owned(),
            ".env".to_owned(),
            "secrets/token.txt".to_owned(),
        ];
        let filtered = apply_path_exclusions(paths, &[".env".into(), "secrets/".into()]);
        assert_eq!(filtered, vec!["src/lib.rs".to_owned()]);
    }

    #[test]
    fn no_patterns_leaves_paths_untouched() {
        let paths = vec!["src/lib.rs".to_owned()];
        assert_eq!(apply_path_exclusions(paths.clone(), &[]), paths);
    }

    #[test]
    fn commit_event_carries_metadata_and_filtered_paths() {
        let repo = init_repo();
        commit(&repo, "src/lib.rs", "fn main() {}", "feat: add entry point");
        let reader = RepositoryReader::new(repo.path());
        let commit_record = &reader.commits_since(None).unwrap()[0];

        let event = commit_event(
            repo.path(),
            Some("main"),
            commit_record,
            vec!["src/lib.rs".into()],
            "git:test-repo",
            0,
        );

        match event.payload {
            SemanticPayload::RepositoryCommit {
                subject,
                branch,
                changed_paths,
                ..
            } => {
                assert_eq!(subject.as_deref(), Some("feat: add entry point"));
                assert_eq!(branch.as_deref(), Some("main"));
                assert_eq!(changed_paths, vec!["src/lib.rs".to_owned()]);
            }
            other => panic!("expected RepositoryCommit, got {other:?}"),
        }
    }

    fn run_git(dir: &std::path::Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed in {}", dir.display());
    }

    #[test]
    fn a_detached_head_reads_commits_the_same_as_a_branch_checkout() {
        let repo = init_repo();
        commit(&repo, "a.txt", "a", "first commit");
        commit(&repo, "b.txt", "b", "second commit");
        run_git(repo.path(), &["checkout", "--quiet", "--detach", "HEAD"]);

        let reader = RepositoryReader::new(repo.path());
        let commits = reader.commits_since(None).unwrap();

        assert_eq!(commits.len(), 2);
        assert_eq!(commits[1].subject.as_deref(), Some("second commit"));
        // A detached HEAD is not on any branch; that must not be confused with a real branch name.
        assert_eq!(reader.current_branch().unwrap(), None);
    }

    #[test]
    fn switching_branches_does_not_fabricate_or_duplicate_commits() {
        let repo = init_repo();
        commit(&repo, "a.txt", "a", "on main");
        let reader = RepositoryReader::new(repo.path());
        let anchor = reader.commits_since(None).unwrap()[0].commit_id.clone();

        run_git(repo.path(), &["checkout", "--quiet", "-b", "feature"]);
        commit(&repo, "b.txt", "b", "on feature");
        run_git(repo.path(), &["checkout", "--quiet", "main"]);

        // The feature commit is not reachable from main: reading from the same anchor on main
        // must not report it, even though it exists in the repository's object store.
        let from_main = reader.commits_since(Some(&anchor)).unwrap();
        assert!(from_main.is_empty());

        run_git(repo.path(), &["checkout", "--quiet", "feature"]);
        let from_feature = reader.commits_since(Some(&anchor)).unwrap();
        assert_eq!(from_feature.len(), 1);
        assert_eq!(from_feature[0].subject.as_deref(), Some("on feature"));
    }

    #[test]
    fn a_shallow_clone_reads_its_available_history_without_erroring() {
        let origin = init_repo();
        commit(&origin, "a.txt", "a", "first commit");
        commit(&origin, "b.txt", "b", "second commit");
        commit(&origin, "c.txt", "c", "third commit");

        let shallow = TempDir::new().unwrap();
        let status = Command::new("git")
            .args([
                "clone",
                "--quiet",
                "--depth",
                "1",
                "--branch",
                "main",
                // A plain local path clone ignores --depth; file:// makes it a real shallow clone.
                &format!("file://{}", origin.path().display()),
                &shallow.path().display().to_string(),
            ])
            .status()
            .unwrap();
        assert!(status.success(), "shallow clone failed");

        let reader = RepositoryReader::new(shallow.path());
        let commits = reader.commits_since(None).unwrap();

        // A shallow clone only has the tip commit; that boundary must read as "one commit
        // available", not as an error or as the full three-commit history.
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].subject.as_deref(), Some("third commit"));
    }
}
