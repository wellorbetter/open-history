//! Typed entities resolved from captured evidence.
//!
//! An entity is the identity that segmentation, search, digests, and correlation match on. Its
//! identifier is derived from a durable property — a repository path, a document path, a canonical
//! host, a calendar entry, an agent thread — so that presentation changes such as a reformatted
//! window title never produce a new identity for the same underlying thing.
//!
//! Derivation is pure: identical input yields byte-equivalent entities and identifiers in every
//! process, which is what makes segment projections replayable.

use serde::{Deserialize, Serialize};

use crate::AdapterKind;

/// Separator placed between an entity key's discriminant and its canonical value.
///
/// `U+001F` (unit separator) cannot appear in a path, host, or platform identifier, so no two
/// distinct keys can produce the same canonical form.
const KEY_SEPARATOR: char = '\u{1f}';

/// Hexadecimal characters retained from the derivation hash, giving a 128-bit identifier.
const IDENTIFIER_LENGTH: usize = 32;

/// Category of a resolved entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    /// A codebase or workspace, usually a repository root.
    Project,
    /// A single document or source file.
    File,
    /// A network host reached through a browser.
    Host,
    /// A calendar meeting.
    Meeting,
    /// One conversation thread with a local coding agent.
    AgentThread,
}

/// Strength of the evidence an entity was resolved from.
///
/// Variants are ordered from weakest to strongest so that consumers can apply a minimum threshold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityConfidence {
    /// Read out of presentation text such as a window title, with no durable property to confirm it.
    TitleDerived,
    /// Read from a durable property exposed by one source.
    Observed,
    /// Read from durable properties that agree across more than one source.
    Corroborated,
}

/// Durable property an entity identifier is derived from.
///
/// Every constructor normalizes its input and rejects values that carry no identity, so an entity
/// is never built from a blank or whitespace-only observation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "key", content = "value", rename_all = "snake_case")]
pub enum EntityKey {
    /// Filesystem path of a repository or workspace root.
    RepositoryPath(String),
    /// Filesystem path of a document.
    DocumentPath(String),
    /// Canonical host name.
    Host(String),
    /// Identifier issued by the platform calendar store.
    CalendarEntry(String),
    /// Identifier issued by a local coding agent for one session.
    AgentThread(String),
    /// Project name read from presentation text only.
    TitleDerivedProject(String),
}

impl EntityKey {
    /// Builds a repository key, or `None` when the path carries no identity.
    #[must_use]
    pub fn repository_path(path: &str) -> Option<Self> {
        normalize_path(path).map(Self::RepositoryPath)
    }

    /// Builds a document key, or `None` when the path carries no identity.
    #[must_use]
    pub fn document_path(path: &str) -> Option<Self> {
        normalize_path(path).map(Self::DocumentPath)
    }

    /// Builds a host key, or `None` when the host carries no identity.
    #[must_use]
    pub fn host(host: &str) -> Option<Self> {
        normalize_host(host).map(Self::Host)
    }

    /// Builds a calendar key, or `None` when the identifier is blank.
    #[must_use]
    pub fn calendar_entry(identifier: &str) -> Option<Self> {
        normalize_opaque(identifier).map(Self::CalendarEntry)
    }

    /// Builds an agent thread key, or `None` when the identifier is blank.
    #[must_use]
    pub fn agent_thread(identifier: &str) -> Option<Self> {
        normalize_opaque(identifier).map(Self::AgentThread)
    }

    /// Builds a title-derived project key, or `None` when the name is blank.
    #[must_use]
    pub fn title_derived_project(name: &str) -> Option<Self> {
        normalize_opaque(name).map(Self::TitleDerivedProject)
    }

    /// Returns the entity category this key identifies.
    #[must_use]
    pub const fn kind(&self) -> EntityKind {
        match self {
            Self::RepositoryPath(_) | Self::TitleDerivedProject(_) => EntityKind::Project,
            Self::DocumentPath(_) => EntityKind::File,
            Self::Host(_) => EntityKind::Host,
            Self::CalendarEntry(_) => EntityKind::Meeting,
            Self::AgentThread(_) => EntityKind::AgentThread,
        }
    }

    /// Returns the confidence this key supports on its own.
    ///
    /// Only a key read from presentation text is reduced; every other variant names a durable
    /// property. Corroboration across sources is applied by the resolver, not by the key.
    #[must_use]
    pub const fn confidence(&self) -> EntityConfidence {
        match self {
            Self::TitleDerivedProject(_) => EntityConfidence::TitleDerived,
            _ => EntityConfidence::Observed,
        }
    }

    /// Returns the normalized value without its discriminant.
    #[must_use]
    pub fn value(&self) -> &str {
        match self {
            Self::RepositoryPath(value)
            | Self::DocumentPath(value)
            | Self::Host(value)
            | Self::CalendarEntry(value)
            | Self::AgentThread(value)
            | Self::TitleDerivedProject(value) => value,
        }
    }

    /// Returns the string the identifier is derived from.
    #[must_use]
    fn canonical_form(&self) -> String {
        let discriminant = match self {
            Self::RepositoryPath(_) => "repository_path",
            Self::DocumentPath(_) => "document_path",
            Self::Host(_) => "host",
            Self::CalendarEntry(_) => "calendar_entry",
            Self::AgentThread(_) => "agent_thread",
            Self::TitleDerivedProject(_) => "title_derived_project",
        };
        format!("{discriminant}{KEY_SEPARATOR}{}", self.value())
    }
}

/// Stable identifier for one entity.
///
/// Derived only from an [`EntityKey`], so it survives restarts, reindexing, and any change in how
/// the underlying thing is displayed.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EntityId(String);

impl EntityId {
    /// Derives the identifier for a key.
    #[must_use]
    pub fn derive(key: &EntityKey) -> Self {
        let digest = blake3::hash(key.canonical_form().as_bytes());
        let mut hex = digest.to_hex().to_string();
        hex.truncate(IDENTIFIER_LENGTH);
        Self(hex)
    }

    /// Returns the identifier as a string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for EntityId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// The source that contributed evidence for an entity.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EntityProvenance {
    /// Adapter family that supplied the evidence.
    pub adapter: AdapterKind,
    /// Stable local identity of the adapter instance.
    pub source_id: String,
}

/// An entity resolved from captured evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entity {
    /// Identifier derived from [`Entity::key`].
    pub id: EntityId,
    /// Category of the entity.
    pub kind: EntityKind,
    /// Durable property the identity rests on.
    pub key: EntityKey,
    /// Human-readable label when one was observed. Never invented.
    pub label: Option<String>,
    /// Strength of the evidence behind this entity.
    pub confidence: EntityConfidence,
    /// Sources that contributed evidence, ordered deterministically and free of duplicates.
    pub provenance: Vec<EntityProvenance>,
}

impl Entity {
    /// Builds an entity from a key and the source that observed it.
    #[must_use]
    pub fn new(key: EntityKey, label: Option<String>, provenance: EntityProvenance) -> Self {
        Self {
            id: EntityId::derive(&key),
            kind: key.kind(),
            confidence: key.confidence(),
            key,
            label,
            provenance: vec![provenance],
        }
    }
}

/// Normalizes a filesystem path, returning `None` when nothing identifying remains.
///
/// Trailing separators are removed so that `/a/b` and `/a/b/` are one identity. Case is preserved:
/// case folding would merge distinct paths on case-sensitive filesystems.
fn normalize_path(path: &str) -> Option<String> {
    let trimmed = path.trim();
    let without_trailing = trimmed.trim_end_matches(['/', '\\']);
    let candidate = if without_trailing.is_empty() {
        trimmed
    } else {
        without_trailing
    };
    (!candidate.is_empty() && candidate != "/" && candidate != "\\").then(|| candidate.to_owned())
}

/// Normalizes a host, returning `None` when nothing identifying remains.
fn normalize_host(host: &str) -> Option<String> {
    let lowered = host.trim().trim_end_matches('.').to_ascii_lowercase();
    let canonical = lowered.strip_prefix("www.").unwrap_or(&lowered);
    (!canonical.is_empty()).then(|| canonical.to_owned())
}

/// Trims an opaque identifier, returning `None` when it is blank.
fn normalize_opaque(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provenance() -> EntityProvenance {
        EntityProvenance {
            adapter: AdapterKind::Synthetic,
            source_id: "synthetic-1".into(),
        }
    }

    #[test]
    fn identical_input_derives_identical_entities() {
        let key = EntityKey::repository_path("/Users/dev/open-history").unwrap();
        let first = Entity::new(key.clone(), Some("open-history".into()), provenance());
        let second = Entity::new(key, Some("open-history".into()), provenance());

        assert_eq!(first, second);
        assert_eq!(
            serde_json::to_string(&first).unwrap(),
            serde_json::to_string(&second).unwrap()
        );
    }

    #[test]
    fn identifier_is_a_pure_function_of_the_key() {
        // A fixed expectation fails if derivation ever becomes process-dependent, which is what
        // "byte-equivalent across restarts" requires.
        let key = EntityKey::repository_path("/Users/dev/open-history").unwrap();
        let expected = EntityId::derive(&key);

        assert_eq!(expected.as_str().len(), IDENTIFIER_LENGTH);
        assert!(expected.as_str().chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(EntityId::derive(&key), expected);
    }

    #[test]
    fn label_does_not_participate_in_identity() {
        let key = EntityKey::repository_path("/Users/dev/open-history").unwrap();
        let labeled = Entity::new(key.clone(), Some("OpenHistory".into()), provenance());
        let unlabeled = Entity::new(key, None, provenance());

        assert_eq!(labeled.id, unlabeled.id);
    }

    #[test]
    fn distinct_keys_never_collide() {
        let keys = [
            EntityKey::repository_path("/Users/dev/open-history").unwrap(),
            EntityKey::document_path("/Users/dev/open-history").unwrap(),
            EntityKey::host("open-history").unwrap(),
            EntityKey::calendar_entry("open-history").unwrap(),
            EntityKey::agent_thread("open-history").unwrap(),
            EntityKey::title_derived_project("open-history").unwrap(),
        ];

        let mut identifiers: Vec<String> = keys
            .iter()
            .map(|key| EntityId::derive(key).as_str().to_owned())
            .collect();
        identifiers.sort_unstable();
        identifiers.dedup();

        assert_eq!(identifiers.len(), keys.len());
    }

    #[test]
    fn trailing_separators_do_not_split_identity() {
        let bare = EntityKey::repository_path("/Users/dev/open-history").unwrap();
        let trailing = EntityKey::repository_path("/Users/dev/open-history/").unwrap();
        let padded = EntityKey::repository_path("  /Users/dev/open-history  ").unwrap();

        assert_eq!(EntityId::derive(&bare), EntityId::derive(&trailing));
        assert_eq!(EntityId::derive(&bare), EntityId::derive(&padded));
    }

    #[test]
    fn hosts_are_canonicalized_but_paths_keep_their_case() {
        let upper = EntityKey::host("WWW.Example.COM.").unwrap();
        let lower = EntityKey::host("example.com").unwrap();
        assert_eq!(EntityId::derive(&upper), EntityId::derive(&lower));

        let mixed_case_path = EntityKey::document_path("/Users/dev/Notes.md").unwrap();
        let lower_case_path = EntityKey::document_path("/users/dev/notes.md").unwrap();
        assert_ne!(
            EntityId::derive(&mixed_case_path),
            EntityId::derive(&lower_case_path)
        );
    }

    #[test]
    fn blank_observations_produce_no_key() {
        assert!(EntityKey::repository_path("   ").is_none());
        assert!(EntityKey::document_path("").is_none());
        assert!(EntityKey::host(" . ").is_none());
        assert!(EntityKey::calendar_entry("\t").is_none());
        assert!(EntityKey::agent_thread("").is_none());
        assert!(EntityKey::title_derived_project(" ").is_none());
        assert!(EntityKey::repository_path("/").is_none());
    }

    #[test]
    fn title_derived_keys_carry_reduced_confidence() {
        let durable = EntityKey::repository_path("/Users/dev/open-history").unwrap();
        let from_title = EntityKey::title_derived_project("open-history").unwrap();

        assert_eq!(durable.confidence(), EntityConfidence::Observed);
        assert_eq!(from_title.confidence(), EntityConfidence::TitleDerived);
        assert!(EntityConfidence::TitleDerived < EntityConfidence::Observed);
        assert!(EntityConfidence::Observed < EntityConfidence::Corroborated);
        assert_eq!(durable.kind(), from_title.kind());
    }

    #[test]
    fn identifiers_match_their_recorded_values() {
        // Pinned so that a change in derivation is caught here rather than by silently splitting
        // every stored entity's history in two. Changing these values is a data migration.
        let golden = [
            (
                EntityKey::repository_path("/Users/dev/open-history").unwrap(),
                "92574a4942f1afdfcc907caa7bc63a29",
            ),
            (
                EntityKey::document_path("/Users/dev/open-history/src/main.rs").unwrap(),
                "126530cbd631f62d5da97fb71c4356fa",
            ),
            (
                EntityKey::host("example.com").unwrap(),
                "ad31751f4a43d8ce6985d5ac03aac3d3",
            ),
            (
                EntityKey::calendar_entry("CAL-2026-09-15-001").unwrap(),
                "8c8fd8691c34bcc418727c9c8a38d3ee",
            ),
            (
                EntityKey::agent_thread("01JD3K7M4QWERTY").unwrap(),
                "56f028ee4e61db5b32f2482c0a482ee5",
            ),
            (
                EntityKey::title_derived_project("open-history").unwrap(),
                "3e716ff33de482f8100d8f7d0f3fa314",
            ),
        ];

        for (key, expected) in golden {
            assert_eq!(EntityId::derive(&key).as_str(), expected, "key: {key:?}");
        }
    }

    #[test]
    fn entities_round_trip() {
        let key = EntityKey::agent_thread("01JD3K7M4QWERTY").unwrap();
        let entity = Entity::new(key, Some("Harness evidence gate".into()), provenance());

        let encoded = serde_json::to_string(&entity).unwrap();
        let decoded: Entity = serde_json::from_str(&encoded).unwrap();

        assert_eq!(decoded, entity);
        assert_eq!(decoded.kind, EntityKind::AgentThread);
    }
}
