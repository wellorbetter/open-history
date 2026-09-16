//! Deterministic resolution of captured observations into typed entities.
//!
//! The resolver runs at the capture boundary, immediately after the privacy evaluator and before
//! the raw document path is discarded. It converts what an adapter observed into
//! [`openhistory_domain::Entity`] values whose identifiers every other source can reproduce from
//! the same durable property.
//!
//! Resolution never guesses. An observation that matches no rule yields no entity and is reported
//! as unresolved, so that a downstream consumer can tell "nothing identifiable was observed" from
//! "an identity was established".

use openhistory_domain::{Entity, EntityKey, EntityKind, EntityProvenance};
use url::Url;

/// Separators an editor or terminal places between the parts of a window title.
const TITLE_SEPARATORS: [&str; 3] = [" — ", " – ", " - "];

/// Category of application an observation came from.
///
/// The category selects which rules may run, so that a title pattern that is meaningful for an
/// editor is not applied to an unrelated application.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationClass {
    /// Code or text editor.
    Editor,
    /// Terminal emulator.
    Terminal,
    /// Web browser.
    Browser,
    /// Document or note application.
    Document,
    /// Application with no known title convention.
    Unknown,
}

impl ApplicationClass {
    /// Classifies an application from its platform identifier, falling back to its display name.
    ///
    /// The platform identifier is preferred because display names are localized and user-editable.
    #[must_use]
    pub fn classify(display_name: Option<&str>, platform_id: Option<&str>) -> Self {
        if let Some(class) = platform_id.and_then(Self::from_platform_id) {
            return class;
        }
        display_name
            .and_then(Self::from_display_name)
            .unwrap_or(Self::Unknown)
    }

    /// Matches a bundle or package identifier against known applications.
    fn from_platform_id(platform_id: &str) -> Option<Self> {
        let lowered = platform_id.to_ascii_lowercase();
        let table = [
            ("com.microsoft.vscode", Self::Editor),
            ("com.todesktop.230313mzl4w4u92", Self::Editor),
            ("com.apple.dt.xcode", Self::Editor),
            ("com.jetbrains.intellij", Self::Editor),
            ("dev.zed.zed", Self::Editor),
            ("com.sublimetext.4", Self::Editor),
            ("com.apple.terminal", Self::Terminal),
            ("com.googlecode.iterm2", Self::Terminal),
            ("dev.warp.warp-stable", Self::Terminal),
            ("net.kovidgoyal.kitty", Self::Terminal),
            ("com.mitchellh.ghostty", Self::Terminal),
            ("com.apple.safari", Self::Browser),
            ("com.google.chrome", Self::Browser),
            ("org.mozilla.firefox", Self::Browser),
            ("company.thebrowser.browser", Self::Browser),
            ("com.microsoft.edgemac", Self::Browser),
            ("com.apple.notes", Self::Document),
            ("com.apple.preview", Self::Document),
            ("com.apple.textedit", Self::Document),
            ("md.obsidian", Self::Document),
        ];
        table
            .into_iter()
            .find_map(|(identifier, class)| (lowered == identifier).then_some(class))
    }

    /// Matches a display name against known applications.
    fn from_display_name(display_name: &str) -> Option<Self> {
        let lowered = display_name.to_ascii_lowercase();
        let table = [
            ("visual studio code", Self::Editor),
            ("code", Self::Editor),
            ("cursor", Self::Editor),
            ("xcode", Self::Editor),
            ("intellij idea", Self::Editor),
            ("zed", Self::Editor),
            ("sublime text", Self::Editor),
            ("terminal", Self::Terminal),
            ("iterm2", Self::Terminal),
            ("warp", Self::Terminal),
            ("kitty", Self::Terminal),
            ("alacritty", Self::Terminal),
            ("ghostty", Self::Terminal),
            ("wezterm", Self::Terminal),
            ("safari", Self::Browser),
            ("google chrome", Self::Browser),
            ("firefox", Self::Browser),
            ("arc", Self::Browser),
            ("microsoft edge", Self::Browser),
            ("brave browser", Self::Browser),
            ("notes", Self::Document),
            ("preview", Self::Document),
            ("textedit", Self::Document),
            ("obsidian", Self::Document),
        ];
        table
            .into_iter()
            .find_map(|(name, class)| (lowered == name).then_some(class))
    }
}

/// Repository roots the user has opted in to observing.
///
/// Membership is what turns an observed path into a project identity: a document outside every
/// registered root resolves to a file with no project, which is what repository opt-in requires.
#[derive(Clone, Debug, Default)]
pub struct ProjectRegistry {
    /// Normalized roots ordered longest first so that a nested repository wins over its parent.
    roots: Vec<String>,
}

impl ProjectRegistry {
    /// Builds a registry from opted-in repository root paths, discarding paths with no identity.
    #[must_use]
    pub fn new<I, S>(roots: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut normalized: Vec<String> = roots
            .into_iter()
            .filter_map(|root| {
                EntityKey::repository_path(root.as_ref()).map(|key| key.value().to_owned())
            })
            .collect();
        normalized.sort_unstable();
        normalized.dedup();
        normalized.sort_by(|left, right| {
            right
                .len()
                .cmp(&left.len())
                .then_with(|| left.as_str().cmp(right.as_str()))
        });
        Self { roots: normalized }
    }

    /// Returns the registered root containing `path`, preferring the most specific one.
    #[must_use]
    pub fn containing(&self, path: &str) -> Option<&str> {
        let candidate = EntityKey::document_path(path)?;
        self.roots
            .iter()
            .find(|root| is_within(root, candidate.value()))
            .map(String::as_str)
    }

    /// Returns true when no repository has been opted in.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }
}

/// One observation offered to the resolver.
///
/// Every field is optional because platform accessibility data is routinely incomplete, and an
/// absent field must stay absent rather than be filled with a placeholder.
#[derive(Clone, Copy, Debug, Default)]
pub struct WindowObservation<'a> {
    /// Application display name when the platform exposed one.
    pub application: Option<&'a str>,
    /// Bundle or package identifier when the platform exposed one.
    pub application_id: Option<&'a str>,
    /// Window title after minimization.
    pub window_title: Option<&'a str>,
    /// Document path exposed by the focused window, before it is discarded.
    pub document_path: Option<&'a str>,
    /// Normalized URL supplied by an approved browser adapter.
    pub url: Option<&'a str>,
}

/// Entities resolved from one observation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Resolution {
    /// Resolved entities, ordered deterministically by kind and identifier.
    pub entities: Vec<Entity>,
}

impl Resolution {
    /// Returns true when the observation established no identity.
    #[must_use]
    pub fn is_unresolved(&self) -> bool {
        self.entities.is_empty()
    }

    /// Returns the first entity of a kind, if one was resolved.
    #[must_use]
    pub fn entity(&self, kind: EntityKind) -> Option<&Entity> {
        self.entities.iter().find(|entity| entity.kind == kind)
    }
}

/// Applies deterministic rules to observations.
#[derive(Clone, Debug, Default)]
pub struct Resolver {
    /// Opted-in repository roots used to turn a path into a project.
    registry: ProjectRegistry,
    /// Home directory used to expand a leading `~` in terminal titles.
    ///
    /// Supplied rather than read from the environment so that resolution stays a pure function of
    /// its inputs and remains reproducible in tests and on another machine.
    home_directory: Option<String>,
}

impl Resolver {
    /// Builds a resolver over the opted-in repositories.
    #[must_use]
    pub fn new(registry: ProjectRegistry, home_directory: Option<String>) -> Self {
        Self {
            registry,
            home_directory,
        }
    }

    /// Resolves one observation.
    #[must_use]
    pub fn resolve(
        &self,
        observation: &WindowObservation<'_>,
        provenance: &EntityProvenance,
    ) -> Resolution {
        let class = ApplicationClass::classify(observation.application, observation.application_id);
        let mut entities = Vec::new();

        if let Some(path) = observation.document_path {
            self.push_document(&mut entities, path, provenance);
        }

        if let Some(url) = observation.url {
            push_host(&mut entities, url, provenance);
        }

        if class == ApplicationClass::Terminal
            && let Some(title) = observation.window_title
            && let Some(path) = self.terminal_path(title)
            && let Some(root) = self.registry.containing(&path)
            && let Some(key) = EntityKey::repository_path(root)
        {
            push(&mut entities, Entity::new(key, None, provenance.clone()));
        }

        if class == ApplicationClass::Editor
            && !entities
                .iter()
                .any(|entity| entity.kind == EntityKind::Project)
            && let Some(title) = observation.window_title
            && let Some(name) = trailing_segment(title)
            && let Some(key) = EntityKey::title_derived_project(name)
        {
            push(&mut entities, Entity::new(key, None, provenance.clone()));
        }

        entities.sort_by(|left, right| {
            left.kind
                .cmp(&right.kind)
                .then_with(|| left.id.cmp(&right.id))
        });
        Resolution { entities }
    }

    /// Adds the file entity for a document path, and its project when the path is opted in.
    fn push_document(&self, entities: &mut Vec<Entity>, path: &str, provenance: &EntityProvenance) {
        let Some(file_key) = EntityKey::document_path(path) else {
            return;
        };
        if let Some(root) = self.registry.containing(path)
            && let Some(project_key) = EntityKey::repository_path(root)
        {
            push(entities, Entity::new(project_key, None, provenance.clone()));
        }
        push(entities, Entity::new(file_key, None, provenance.clone()));
    }

    /// Extracts a path from a terminal title, expanding a leading `~` when a home directory is set.
    fn terminal_path(&self, title: &str) -> Option<String> {
        let token = title
            .split_whitespace()
            .find(|token| token.starts_with('/') || token.starts_with("~/"))?;
        token.strip_prefix("~/").map_or_else(
            || Some(token.to_owned()),
            |relative| {
                self.home_directory
                    .as_ref()
                    .map(|home| format!("{}/{relative}", home.trim_end_matches('/')))
            },
        )
    }
}

/// Adds the host entity for a URL that parses and exposes one.
fn push_host(entities: &mut Vec<Entity>, url: &str, provenance: &EntityProvenance) {
    let Ok(parsed) = Url::parse(url) else {
        return;
    };
    let Some(host) = parsed.host_str() else {
        return;
    };
    if let Some(key) = EntityKey::host(host) {
        push(entities, Entity::new(key, None, provenance.clone()));
    }
}

/// Adds an entity unless one with the same identifier is already present.
fn push(entities: &mut Vec<Entity>, entity: Entity) {
    if !entities.iter().any(|existing| existing.id == entity.id) {
        entities.push(entity);
    }
}

/// Returns true when `path` is `root` itself or lies beneath it.
///
/// The separator check prevents `/work/app-legacy` from matching the root `/work/app`.
fn is_within(root: &str, path: &str) -> bool {
    let Some(rest) = path.strip_prefix(root) else {
        return false;
    };
    rest.is_empty() || rest.starts_with('/') || rest.starts_with('\\')
}

/// Returns the last segment of a title that uses a known separator.
///
/// A title with no separator has no project part to read, so `None` is returned rather than
/// treating the whole title as a project name.
fn trailing_segment(title: &str) -> Option<&str> {
    let (index, separator) = TITLE_SEPARATORS
        .into_iter()
        .filter_map(|separator| title.rfind(separator).map(|index| (index, separator)))
        .max_by_key(|(index, _)| *index)?;
    let segment = title[index + separator.len()..].trim();
    (!segment.is_empty()).then_some(segment)
}

#[cfg(test)]
mod tests {
    use openhistory_domain::{AdapterKind, EntityConfidence, EntityId};

    use super::*;

    const REPOSITORY: &str = "/Users/dev/open-history";

    fn provenance() -> EntityProvenance {
        EntityProvenance {
            adapter: AdapterKind::MacOsAccessibility,
            source_id: "macos-1".into(),
        }
    }

    fn resolver() -> Resolver {
        Resolver::new(
            ProjectRegistry::new([REPOSITORY, "/Users/dev/timetrace"]),
            Some("/Users/dev".into()),
        )
    }

    fn project_id() -> EntityId {
        EntityId::derive(&EntityKey::repository_path(REPOSITORY).unwrap())
    }

    #[test]
    fn editor_with_a_document_resolves_file_and_project() {
        let resolution = resolver().resolve(
            &WindowObservation {
                application: Some("Visual Studio Code"),
                application_id: Some("com.microsoft.VSCode"),
                window_title: Some("storage.rs — open-history"),
                document_path: Some(
                    "/Users/dev/open-history/crates/openhistory-storage/src/lib.rs",
                ),
                url: None,
            },
            &provenance(),
        );

        let project = resolution.entity(EntityKind::Project).unwrap();
        assert_eq!(project.id, project_id());
        assert_eq!(project.confidence, EntityConfidence::Observed);
        assert!(resolution.entity(EntityKind::File).is_some());
    }

    #[test]
    fn terminal_browser_and_document_applications_each_resolve() {
        let resolver = resolver();

        let terminal = resolver.resolve(
            &WindowObservation {
                application: Some("Terminal"),
                application_id: Some("com.apple.Terminal"),
                window_title: Some("dev — ~/open-history — -zsh"),
                ..WindowObservation::default()
            },
            &provenance(),
        );
        assert_eq!(
            terminal.entity(EntityKind::Project).unwrap().id,
            project_id()
        );

        let browser = resolver.resolve(
            &WindowObservation {
                application: Some("Google Chrome"),
                application_id: Some("com.google.Chrome"),
                window_title: Some("SQLCipher — Google Chrome"),
                url: Some("https://WWW.Example.com/docs?q=1"),
                ..WindowObservation::default()
            },
            &provenance(),
        );
        let host = browser.entity(EntityKind::Host).unwrap();
        assert_eq!(host.key.value(), "example.com");

        let document = resolver.resolve(
            &WindowObservation {
                application: Some("Notes"),
                application_id: Some("com.apple.Notes"),
                window_title: Some("Weekly report"),
                document_path: Some("/Users/dev/Documents/weekly.md"),
                ..WindowObservation::default()
            },
            &provenance(),
        );
        assert!(document.entity(EntityKind::File).is_some());
        assert!(
            document.entity(EntityKind::Project).is_none(),
            "a document outside every opted-in repository has no project"
        );
    }

    #[test]
    fn unmatched_titles_are_unresolved_rather_than_guessed() {
        let unmatched = [
            WindowObservation {
                application: Some("Some Unknown App"),
                window_title: Some("Untitled"),
                ..WindowObservation::default()
            },
            WindowObservation {
                application: Some("Visual Studio Code"),
                application_id: Some("com.microsoft.VSCode"),
                window_title: Some("scratch.rs"),
                ..WindowObservation::default()
            },
            WindowObservation {
                application: Some("Terminal"),
                application_id: Some("com.apple.Terminal"),
                window_title: Some("dev — -zsh"),
                ..WindowObservation::default()
            },
            WindowObservation::default(),
        ];

        for observation in unmatched {
            let resolution = resolver().resolve(&observation, &provenance());
            assert!(
                resolution.is_unresolved(),
                "expected no entity for {observation:?}"
            );
        }
    }

    #[test]
    fn a_terminal_outside_opted_in_repositories_resolves_nothing() {
        let resolution = resolver().resolve(
            &WindowObservation {
                application: Some("Terminal"),
                application_id: Some("com.apple.Terminal"),
                window_title: Some("dev — /private/secrets — -zsh"),
                ..WindowObservation::default()
            },
            &provenance(),
        );

        assert!(resolution.is_unresolved());
    }

    #[test]
    fn an_editor_without_a_document_falls_back_to_a_title_derived_project() {
        let resolution = resolver().resolve(
            &WindowObservation {
                application: Some("Visual Studio Code"),
                application_id: Some("com.microsoft.VSCode"),
                window_title: Some("● notes.md — scratchpad"),
                ..WindowObservation::default()
            },
            &provenance(),
        );

        let project = resolution.entity(EntityKind::Project).unwrap();
        assert_eq!(project.confidence, EntityConfidence::TitleDerived);
        assert_eq!(project.key.value(), "scratchpad");
    }

    #[test]
    fn a_durable_project_suppresses_the_title_fallback() {
        let resolution = resolver().resolve(
            &WindowObservation {
                application: Some("Visual Studio Code"),
                application_id: Some("com.microsoft.VSCode"),
                window_title: Some("lib.rs — a misleading name"),
                document_path: Some("/Users/dev/open-history/crates/openhistory-domain/src/lib.rs"),
                ..WindowObservation::default()
            },
            &provenance(),
        );

        let projects: Vec<_> = resolution
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::Project)
            .collect();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].id, project_id());
        assert_eq!(projects[0].confidence, EntityConfidence::Observed);
    }

    #[test]
    fn resolution_is_deterministic_and_ordered() {
        let observation = WindowObservation {
            application: Some("Visual Studio Code"),
            application_id: Some("com.microsoft.VSCode"),
            window_title: Some("lib.rs — open-history"),
            document_path: Some("/Users/dev/open-history/crates/openhistory-domain/src/lib.rs"),
            url: Some("https://example.com/"),
        };

        let first = resolver().resolve(&observation, &provenance());
        let second = resolver().resolve(&observation, &provenance());
        assert_eq!(first, second);

        let kinds: Vec<_> = first.entities.iter().map(|entity| entity.kind).collect();
        let mut sorted = kinds.clone();
        sorted.sort_unstable();
        assert_eq!(kinds, sorted);
    }

    #[test]
    fn the_most_specific_repository_wins() {
        let registry = ProjectRegistry::new(["/work/app", "/work/app/vendor/lib"]);

        assert_eq!(
            registry.containing("/work/app/vendor/lib/src/main.rs"),
            Some("/work/app/vendor/lib")
        );
        assert_eq!(
            registry.containing("/work/app/src/main.rs"),
            Some("/work/app")
        );
    }

    #[test]
    fn a_sibling_path_does_not_match_a_registered_root() {
        let registry = ProjectRegistry::new(["/work/app"]);

        assert_eq!(registry.containing("/work/app-legacy/src/main.rs"), None);
        assert_eq!(registry.containing("/work/app"), Some("/work/app"));
    }

    #[test]
    fn a_terminal_title_without_a_home_directory_keeps_the_tilde_unexpanded() {
        let resolver = Resolver::new(ProjectRegistry::new([REPOSITORY]), None);

        let resolution = resolver.resolve(
            &WindowObservation {
                application: Some("Terminal"),
                application_id: Some("com.apple.Terminal"),
                window_title: Some("dev — ~/open-history — -zsh"),
                ..WindowObservation::default()
            },
            &provenance(),
        );

        assert!(resolution.is_unresolved());
    }

    #[test]
    fn applications_are_classified_by_identifier_before_display_name() {
        assert_eq!(
            ApplicationClass::classify(Some("Notes"), Some("com.apple.Terminal")),
            ApplicationClass::Terminal
        );
        assert_eq!(
            ApplicationClass::classify(Some("Terminal"), None),
            ApplicationClass::Terminal
        );
        assert_eq!(
            ApplicationClass::classify(Some("Unheard Of"), Some("com.example.unknown")),
            ApplicationClass::Unknown
        );
        assert_eq!(
            ApplicationClass::classify(None, None),
            ApplicationClass::Unknown
        );
    }
}
