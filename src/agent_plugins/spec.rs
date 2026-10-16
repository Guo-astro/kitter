use serde::{Deserialize, Serialize};
use std::path::{Component, PathBuf};

use super::{HostContext, expand_path};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "HookMarker", into = "HookMarker")]
pub struct HookPath(pub String);

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HookMarker {
    #[serde(rename = "$hook")]
    path: String,
}
impl From<HookMarker> for HookPath {
    fn from(value: HookMarker) -> Self {
        Self(value.path)
    }
}
impl From<HookPath> for HookMarker {
    fn from(value: HookPath) -> Self {
        Self { path: value.0 }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PathTemplate {
    Template(String),
    Hook(HookPath),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IconSpec {
    Single(String),
    Themed(ThemeIcon),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeIcon {
    pub light: String,
    pub dark: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentSpec {
    pub api_version: u32,
    pub id: String,
    pub name: String,
    pub icon: Option<IconSpec>,
    #[serde(default)]
    pub global_only: bool,
    #[serde(default)]
    pub reads: Vec<String>,
    pub install: Option<InstallSpec>,
    pub skills: Option<SkillsSpec>,
}
impl AgentSpec {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if !valid_agent_id(&self.id) {
            return Err("invalid or reserved agent id".into());
        }
        if let Some(install) = &self.install
            && std::path::Path::new(&install.project)
                .components()
                .any(|c| matches!(c, Component::Prefix(_) | Component::RootDir))
        {
            return Err("install.project must be a relative path".into());
        }
        if self.api_version != 1 {
            return Err("apiVersion must be 1".into());
        }
        let svg = |s: &str| {
            let s = s.trim();
            if s.starts_with("<svg") && s.len() <= 64 * 1024 {
                Ok(())
            } else {
                Err("inline icon must start with <svg and be at most 64 KiB".to_owned())
            }
        };
        match &self.icon {
            Some(IconSpec::Single(s)) if !s.starts_with("builtin:") => svg(s)?,
            Some(IconSpec::Themed(icon)) => {
                svg(&icon.light)?;
                svg(&icon.dark)?;
            }
            _ => {}
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallSpec {
    pub project: String,
    pub global: PathTemplate,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillsSpec {
    pub roots: RootsValue,
    pub scan: Option<ScanSpec>,
    pub metadata: Option<Metadata>,
    pub validate: Option<Validation>,
    pub id: Option<SkillId>,
    pub visibility: Option<VisibilityRule>,
    pub prompt_path: Option<PromptPath>,
    pub collision: Option<Collision>,
    pub builtins: Option<HookPath>,
    pub filter: Option<HookPath>,
    pub resolve: Option<HookPath>,
    pub catalog: Option<CatalogSpec>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RootsValue {
    Decl(RootsDecl),
    Hook(HookPath),
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RootsDecl {
    #[serde(default)]
    pub project: Vec<String>,
    #[serde(default)]
    pub user: Vec<String>,
    pub env_paths: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ScanSpec {
    Profile(ScanProfile),
    Bounded(RecursiveScan),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecursiveScan {
    pub recursive: ScanLimits,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScanLimits {
    pub max_depth: Option<usize>,
    pub max_directories: Option<usize>,
    pub max_entries: Option<usize>,
}

macro_rules! values {
    ($name:ident { $($value:ident),* $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub enum $name { $($value),* }
    };
}
values!(ScanProfile {
    Recursive,
    DirectChildren,
    ProviderChildren,
    DimRecursive,
    PiIgnored
});
values!(Metadata {
    Strict,
    BodyFallback,
    Provider
});
values!(Validation { Agentskills, None });
values!(SkillId { Name, Path });
values!(VisibilityRule {
    Frontmatter,
    Automatic
});
values!(PromptPath { None, File });
values!(Collision {
    KeepAll,
    FirstWins,
    LastWins
});
values!(Scope {
    Local,
    Repository,
    User,
    System
});
values!(Layout {
    Default,
    RootMarkdown,
    FlatMarkdown,
    DirectChildren
});
values!(Visibility {
    Automatic,
    NameOnly,
    ManualOnly,
    Conditional
});
values!(Source {
    Filesystem,
    Builtin,
    Plugin
});
values!(CatalogFormat { Xml });

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CatalogSpec {
    Xml(XmlCatalog),
    Custom(RenderCatalog),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct XmlCatalog {
    pub format: CatalogFormat,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenderCatalog {
    pub render: HookPath,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginIdentity {
    pub id: String,
    pub display_name: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RootSpec {
    pub path: PathBuf,
    pub scope: Scope,
    pub layout: Option<Layout>,
    pub exact_file: Option<PathBuf>,
    pub follow_symlinks: Option<bool>,
    pub plugin: Option<PluginIdentity>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryRoot {
    pub path: PathBuf,
    pub scope: Scope,
    pub plugin: Option<PluginIdentity>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillEntry {
    pub key: usize,
    pub id: String,
    pub name: String,
    pub description: String,
    pub when_to_use: Option<String>,
    pub has_explicit_description: bool,
    pub file: String,
    pub dir: String,
    pub root: Option<EntryRoot>,
    pub scope: Scope,
    pub frontmatter: serde_json::Map<String, serde_json::Value>,
    pub visibility: Visibility,
    pub prompt_path: Option<String>,
    pub source: Source,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillEntryInput {
    pub id: String,
    pub name: String,
    pub description: String,
    pub when_to_use: Option<String>,
    pub visibility: Option<Visibility>,
    pub scope: Option<Scope>,
}

pub fn expand_roots(decl: &RootsDecl, ctx: &HostContext) -> Vec<RootSpec> {
    let root = |path, scope, layout| RootSpec {
        path,
        scope,
        layout,
        exact_file: None,
        follow_symlinks: None,
        plugin: None,
    };
    let mut roots = Vec::new();
    for dir in ctx.walk_up() {
        for relative in &decl.project {
            roots.push(root(dir.join(relative), Scope::Repository, None));
        }
    }
    for template in &decl.user {
        roots.push(root(
            expand_path(template, &ctx.home, &ctx.home),
            Scope::User,
            None,
        ));
    }
    if let Some(name) = &decl.env_paths {
        roots.extend(
            ctx.env_paths(name)
                .into_iter()
                .map(|path| root(path, Scope::User, Some(Layout::RootMarkdown))),
        );
    }
    roots
}

pub(crate) fn valid_agent_id(value: &str) -> bool {
    let id = value.as_bytes();
    (1..=32).contains(&id.len())
        && id[0].is_ascii_lowercase()
        && id
            .iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
        && value != "universal"
}
