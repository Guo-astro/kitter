use std::{
    borrow::Cow,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock, RwLock},
};

use super::{
    AgentId, AgentSpec, HookPath, HostContext, IconSpec, InstallTarget, JsHost, PathTemplate,
    PluginSource, SkillsSpec, builtin::BUILTIN_PLUGINS, expand_path, host::SpecLoader,
    id::canonical_id, spec::valid_agent_id,
};

const UNIVERSAL_PROJECT: &str = ".agents/skills";
const UNIVERSAL_GLOBAL: &str = "~/.agents/skills";

const BUILTIN_ICONS: &[(&str, &str)] = &[
    ("amp", "icons/provider-amp.svg"),
    ("antigravity", "icons/provider-antigravity.svg"),
    ("claude", "icons/provider-claude.svg"),
    ("codex", "icons/provider-codex.svg"),
    ("copilot", "icons/provider-copilot.svg"),
    ("cursor", "icons/provider-cursor.svg"),
    ("dimagent", "icons/provider-dimagent.svg"),
    ("droid", "icons/provider-droid.svg"),
    ("gemini", "icons/provider-gemini.svg"),
    ("grok", "icons/provider-grok.svg"),
    ("hermes", "icons/provider-hermes.svg"),
    ("omp", "icons/provider-omp.svg"),
    ("openclaw", "icons/provider-openclaw.svg"),
    ("opencode", "icons/provider-opencode.svg"),
    ("pi", "icons/provider-pi.svg"),
    ("trae", "icons/provider-trae.svg"),
];

pub struct AgentRegistry {
    agents: Vec<AgentDef>,
    diagnostics: Vec<PluginDiagnostic>,
    all_agents: Vec<AgentDef>,
    generation: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PluginOrigin {
    Builtin,
    User(PathBuf),
    Override(PathBuf),
}

#[derive(Clone, Debug)]
pub struct PluginDiagnostic {
    pub origin: PluginOrigin,
    pub id: Option<String>,
    pub message: String,
}

#[derive(Clone)]
pub struct AgentDef {
    pub id: AgentId,
    pub name: String,
    pub icon: AgentIcon,
    pub global_only: bool,
    pub reads: Vec<InstallTarget>,
    pub install: Option<InstallDef>,
    pub skills: Option<SkillsSpec>,
    pub origin: PluginOrigin,
    pub source: PluginSource,
}

#[derive(Clone)]
pub enum AgentIcon {
    Builtin(&'static str),
    Inline {
        light: Arc<[u8]>,
        dark: Option<Arc<[u8]>>,
    },
    Monogram,
}

#[derive(Clone)]
pub struct InstallDef {
    pub project: String,
    pub global: PathValue,
}

#[derive(Clone)]
pub enum PathValue {
    Template(String),
    Hook(HookPath),
}

impl AgentDef {
    pub(super) fn from_spec(
        spec: AgentSpec,
        origin: PluginOrigin,
        source: PluginSource,
    ) -> Result<Self, String> {
        let icon = match spec.icon {
            Some(IconSpec::Single(s)) if s.starts_with("builtin:") => {
                let name = s.strip_prefix("builtin:").unwrap();
                AgentIcon::Builtin(
                    BUILTIN_ICONS
                        .iter()
                        .find(|(key, _)| *key == name)
                        .ok_or_else(|| format!("unknown builtin icon: {name}"))?
                        .1,
                )
            }
            Some(IconSpec::Single(s)) => AgentIcon::Inline {
                light: Arc::from(s.into_bytes()),
                dark: None,
            },
            Some(IconSpec::Themed(icon)) => AgentIcon::Inline {
                light: Arc::from(icon.light.into_bytes()),
                dark: Some(Arc::from(icon.dark.into_bytes())),
            },
            None => AgentIcon::Monogram,
        };
        Ok(Self {
            id: AgentId::from(spec.id.as_str()),
            name: spec.name,
            icon,
            global_only: spec.global_only,
            reads: spec
                .reads
                .iter()
                .map(|s| InstallTarget::from(s.as_str()))
                .collect(),
            install: spec.install.map(|install| InstallDef {
                project: install.project,
                global: match install.global {
                    PathTemplate::Template(s) => PathValue::Template(s),
                    PathTemplate::Hook(hook) => PathValue::Hook(hook),
                },
            }),
            skills: spec.skills,
            origin,
            source,
        })
    }
}

pub fn plugin_dir() -> PathBuf {
    crate::config::app_data_dir().join("agents")
}

fn user_plugin_files(dir: &Path) -> (Vec<PathBuf>, Vec<PluginDiagnostic>) {
    let mut paths = Vec::new();
    let mut diagnostics = Vec::new();
    match fs::read_dir(dir) {
        Ok(entries) => paths.extend(
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "js") && path.is_file()),
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => diagnostics.push(PluginDiagnostic {
            origin: PluginOrigin::User(dir.into()),
            id: None,
            message: error.to_string(),
        }),
    }
    paths.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    (paths, diagnostics)
}

fn current_registry() -> &'static RwLock<&'static AgentRegistry> {
    static REGISTRY: OnceLock<RwLock<&'static AgentRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        RwLock::new(Box::leak(Box::new(AgentRegistry::load_from_paths(
            BUILTIN_PLUGINS,
            &plugin_dir(),
            &super::manage::settings_path(),
        ))))
    })
}

impl AgentRegistry {
    pub fn global() -> &'static Self {
        *current_registry().read().unwrap()
    }

    pub(super) fn load_from_paths(
        builtins: &[(&str, &str)],
        dir: &Path,
        settings_path: &Path,
    ) -> Self {
        let (paths, mut diagnostics) = user_plugin_files(dir);
        let mut registry = Self::load(builtins, &paths);
        match super::manage::read_settings(settings_path) {
            Ok(settings) => registry.apply_settings(&settings),
            Err(error) => diagnostics.push(PluginDiagnostic {
                origin: PluginOrigin::User(settings_path.into()),
                id: None,
                message: error.to_string(),
            }),
        }
        diagnostics.append(&mut registry.diagnostics);
        registry.diagnostics = diagnostics;
        registry
    }

    pub(super) fn apply_settings(&mut self, settings: &super::manage::AgentSettings) {
        self.all_agents.sort_by_key(|agent| {
            settings
                .order
                .iter()
                .position(|id| id == agent.id.as_str())
                .unwrap_or(usize::MAX)
        });
        self.agents = self
            .all_agents
            .iter()
            .filter(|agent| !settings.disabled.contains(agent.id.as_str()))
            .cloned()
            .collect();
    }

    pub fn load(builtins: &[(&str, &str)], user_files: &[PathBuf]) -> Self {
        let loader = SpecLoader::new();
        let mut slots = Vec::new();
        let mut diagnostics = Vec::new();
        for &(id, code) in builtins {
            let source = PluginSource {
                name: format!("builtin:{id}.js"),
                code: Cow::Owned(code.into()),
            };
            let agent = Self::load_agent(&loader, source, PluginOrigin::Builtin, id);
            match agent {
                Ok(agent) => slots.push((id.to_string(), Some(agent))),
                Err(message) => {
                    diagnostics.push(PluginDiagnostic {
                        origin: PluginOrigin::Builtin,
                        id: Some(id.into()),
                        message,
                    });
                    slots.push((id.to_string(), None));
                }
            }
        }
        for path in user_files {
            let id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .filter(|id| valid_agent_id(id));
            let position = id.and_then(|id| slots.iter().position(|(key, _)| key == id));
            let origin = if position.is_some() {
                PluginOrigin::Override(path.clone())
            } else {
                PluginOrigin::User(path.clone())
            };
            let result = (|| {
                let id = id.ok_or_else(|| "invalid plugin filename".to_string())?;
                let source = PluginSource {
                    name: path.to_string_lossy().into_owned(),
                    code: Cow::Owned(fs::read_to_string(path).map_err(|e| e.to_string())?),
                };
                Self::load_agent(&loader, source, origin.clone(), id)
            })();
            match result {
                Ok(agent) => {
                    if let Some(position) = position {
                        slots[position].1 = Some(agent);
                    } else {
                        slots.push((agent.id.to_string(), Some(agent)));
                    }
                }
                Err(message) => {
                    if let Some(position) = position {
                        slots[position].1 = None;
                    }
                    diagnostics.push(PluginDiagnostic {
                        origin,
                        id: id.map(str::to_string),
                        message,
                    });
                }
            }
        }
        let agents: Vec<AgentDef> = slots.into_iter().filter_map(|(_, agent)| agent).collect();
        for agent in &agents {
            for target in &agent.reads {
                if !target.is_universal()
                    && !agents.iter().any(|other| {
                        other.id.as_str() == target.as_str() && other.install.is_some()
                    })
                {
                    diagnostics.push(PluginDiagnostic {
                        origin: agent.origin.clone(),
                        id: Some(agent.id.to_string()),
                        message: format!("unknown install target: {target}"),
                    });
                }
            }
        }
        Self {
            all_agents: agents.clone(),
            agents,
            diagnostics,
            generation: 0,
        }
    }

    fn load_agent(
        loader: &SpecLoader,
        source: PluginSource,
        origin: PluginOrigin,
        id: &str,
    ) -> Result<AgentDef, String> {
        let spec = loader.load(&source).map_err(|e| match e.stack {
            Some(stack) => format!("{}\n{stack}", e.message),
            None => e.message,
        })?;
        if spec.id != id {
            return Err("plugin id must match its filename".into());
        }
        AgentDef::from_spec(spec, origin, source)
    }

    /// Reloads plugins and `agents.json`, replacing what `global()` returns.
    pub fn reload() -> &'static Self {
        Self::reload_from_paths(
            current_registry(),
            BUILTIN_PLUGINS,
            &plugin_dir(),
            &super::manage::settings_path(),
        )
    }

    pub(super) fn reload_from_paths(
        current: &RwLock<&'static Self>,
        builtins: &[(&str, &str)],
        dir: &Path,
        path: &Path,
    ) -> &'static Self {
        let mut registry = Self::load_from_paths(builtins, dir, path);
        let mut current = current.write().unwrap();
        registry.generation = current.generation + 1;
        // Keep old snapshots alive so existing callers can retain static references.
        *current = Box::leak(Box::new(registry));
        *current
    }

    /// Increments on every `reload()`; lets the UI invalidate cached views.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Enabled and disabled Agents in user order, for management views.
    /// `agents()` returns only the enabled ones.
    pub fn all_agents(&self) -> &[AgentDef] {
        &self.all_agents
    }

    pub fn is_enabled(&self, id: &str) -> bool {
        self.agent(id).is_some()
    }

    pub fn diagnostics(&self) -> &[PluginDiagnostic] {
        &self.diagnostics
    }

    pub fn agents(&self) -> &[AgentDef] {
        &self.agents
    }

    pub fn agent(&self, id: &str) -> Option<&AgentDef> {
        self.agents.iter().find(|agent| agent.id.as_str() == id)
    }

    pub fn resolve(&self, input: &str) -> Option<AgentId> {
        self.agent(canonical_id(input))
            .map(|agent| agent.id.clone())
    }

    pub fn install_targets(&self) -> Vec<InstallTarget> {
        std::iter::once(InstallTarget::universal())
            .chain(
                self.agents
                    .iter()
                    .filter(|agent| agent.install.is_some())
                    .map(|agent| InstallTarget::from(agent.id.as_str())),
            )
            .collect()
    }
}

impl AgentDef {
    pub fn global_dir(&self, home: &Path) -> Option<PathBuf> {
        let install = self.install.as_ref()?;
        Some(match &install.global {
            PathValue::Template(template) => expand_path(template, home, home),
            PathValue::Hook(hook) => {
                let mut host = JsHost::new(HostContext {
                    home: home.into(),
                    cwd: home.into(),
                    repo_root: None,
                    is_global: true,
                });
                let value = host.call(&self.source, hook, None).ok()?;
                PathBuf::from(value.as_str()?)
            }
        })
    }

    pub fn supports_target(&self, target: &InstallTarget) -> bool {
        self.reads.contains(target)
    }
}

impl InstallTarget {
    pub fn project_dir(&self) -> Option<&'static str> {
        if self.is_universal() {
            return Some(UNIVERSAL_PROJECT);
        }
        AgentRegistry::global()
            .agent(self.as_str())?
            .install
            .as_ref()
            .map(|install| install.project.as_str())
    }

    pub fn global_dir(&self, home: &Path) -> Option<PathBuf> {
        if self.is_universal() {
            return Some(expand_path(UNIVERSAL_GLOBAL, home, home));
        }
        let agent = AgentRegistry::global().agent(self.as_str())?;
        agent.global_dir(home)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plugin(id: &str, fields: &str) -> String {
        format!("export default {{apiVersion:1,id:{id:?},name:'Test',{fields}}}")
    }

    #[test]
    fn builtin_plugins_load_without_diagnostics() {
        let registry = AgentRegistry::load(BUILTIN_PLUGINS, &[]);
        assert!(
            registry.diagnostics().is_empty(),
            "{:?}",
            registry.diagnostics()
        );
        assert_eq!(registry.agents().len(), 15);
        assert!(
            registry
                .agents()
                .iter()
                .all(|agent| agent.origin == PluginOrigin::Builtin)
        );
    }

    #[test]
    fn user_plugins_append_in_filename_order_without_recursing() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("z-last.js"), plugin("z-last", "")).unwrap();
        fs::write(dir.path().join("a-first.js"), plugin("a-first", "")).unwrap();
        fs::write(dir.path().join("ignored.txt"), "invalid").unwrap();
        fs::create_dir(dir.path().join("nested")).unwrap();
        fs::write(dir.path().join("nested/ignored.js"), "invalid").unwrap();
        let registry = AgentRegistry::load(BUILTIN_PLUGINS, &user_plugin_files(dir.path()).0);
        assert!(registry.diagnostics().is_empty());
        assert_eq!(
            registry.agents()[15..]
                .iter()
                .map(|a| a.id.as_str())
                .collect::<Vec<_>>(),
            ["a-first", "z-last"]
        );
        assert_eq!(
            registry.agent("a-first").unwrap().origin,
            PluginOrigin::User(dir.path().join("a-first.js"))
        );
    }

    #[test]
    fn loads_only_the_selected_user_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("selected.js");
        fs::write(&path, plugin("selected", "")).unwrap();
        fs::write(dir.path().join("ignored.js"), "invalid").unwrap();
        let registry = AgentRegistry::load(BUILTIN_PLUGINS, &[path]);
        assert!(registry.diagnostics().is_empty());
        assert_eq!(registry.agents().len(), 16);
        assert!(registry.agent("selected").is_some());
        assert!(registry.agent("ignored").is_none());
    }

    #[test]
    fn user_override_replaces_the_whole_definition_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("codex.js");
        fs::write(&path, plugin("codex", "globalOnly:true")).unwrap();
        let registry = AgentRegistry::load(BUILTIN_PLUGINS, &user_plugin_files(dir.path()).0);
        let agent = &registry.agents()[0];
        assert_eq!(agent.id.as_str(), "codex");
        assert_eq!(agent.name, "Test");
        assert_eq!(agent.origin, PluginOrigin::Override(path.clone()));
        assert!(agent.global_only);
        assert!(agent.reads.is_empty());
        assert!(agent.install.is_none());
        assert!(matches!(agent.icon, AgentIcon::Monogram));
        assert_eq!(agent.source.name, path.to_string_lossy());
        assert_eq!(registry.agents().len(), 15);
        assert_eq!(registry.agents()[1].id.as_str(), "claude-code");
    }

    #[test]
    fn invalid_overrides_never_fall_back_and_other_plugins_still_load() {
        let dir = tempfile::tempdir().unwrap();
        for code in [
            "export default { broken: ; }".to_owned(),
            plugin("other-id", ""),
            plugin("codex", "typo:true"),
            plugin("codex", "icon:'builtin:missing'"),
            plugin("codex", "install:{project:'/absolute',global:'~/.test'}"),
        ] {
            let path = dir.path().join("codex.js");
            fs::write(&path, code).unwrap();
            fs::write(dir.path().join("z-valid.js"), plugin("z-valid", "")).unwrap();
            let registry = AgentRegistry::load(BUILTIN_PLUGINS, &user_plugin_files(dir.path()).0);
            assert!(registry.agent("codex").is_none());
            assert!(registry.agent("z-valid").is_some());
            let diagnostic = registry
                .diagnostics()
                .iter()
                .find(|d| d.origin == PluginOrigin::Override(path.clone()))
                .unwrap();
            assert_eq!(diagnostic.id.as_deref(), Some("codex"));
            assert!(!diagnostic.message.is_empty());
        }
    }

    #[test]
    fn invalid_user_filenames_have_no_diagnostic_id() {
        let dir = tempfile::tempdir().unwrap();
        for filename in ["01-custom.js", "universal.js"] {
            fs::write(dir.path().join(filename), plugin("codex", "")).unwrap();
        }
        let registry = AgentRegistry::load(BUILTIN_PLUGINS, &user_plugin_files(dir.path()).0);
        assert_eq!(registry.agents().len(), 15);
        assert_eq!(registry.diagnostics().len(), 2);
        assert!(registry.diagnostics().iter().all(|d| d.id.is_none()));
    }

    #[test]
    fn unknown_reads_are_retained_and_reported_after_loading_all_plugins() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("a-first.js"),
            plugin("a-first", "reads:['missing','z-last','universal']"),
        )
        .unwrap();
        fs::write(
            dir.path().join("z-last.js"),
            plugin(
                "z-last",
                "install:{project:'.last/skills',global:'~/.last/skills'}",
            ),
        )
        .unwrap();
        let registry = AgentRegistry::load(BUILTIN_PLUGINS, &user_plugin_files(dir.path()).0);
        let agent = registry.agent("a-first").unwrap();
        assert_eq!(
            agent
                .reads
                .iter()
                .map(InstallTarget::as_str)
                .collect::<Vec<_>>(),
            ["missing", "z-last", "universal"]
        );
        assert_eq!(registry.diagnostics().len(), 1);
        assert_eq!(registry.diagnostics()[0].id.as_deref(), Some("a-first"));
        assert!(registry.diagnostics()[0].message.contains("missing"));
    }

    #[test]
    fn user_inline_icons_keep_single_and_themed_svg_bytes() {
        let dir = tempfile::tempdir().unwrap();
        for (id, icon) in [
            ("single", "'<svg/>'"),
            (
                "themed",
                "{light:'<svg fill=\"black\"/>',dark:'<svg fill=\"white\"/>'}",
            ),
        ] {
            fs::write(
                dir.path().join(format!("{id}.js")),
                plugin(id, &format!("icon:{icon}")),
            )
            .unwrap();
        }
        let registry = AgentRegistry::load(&[], &user_plugin_files(dir.path()).0);
        assert!(
            registry.diagnostics().is_empty(),
            "{:?}",
            registry.diagnostics()
        );
        assert!(matches!(&registry.agent("single").unwrap().icon,
            AgentIcon::Inline { light, dark: None } if light.as_ref() == b"<svg/>"));
        assert!(matches!(&registry.agent("themed").unwrap().icon,
            AgentIcon::Inline { light, dark: Some(dark) }
                if light.as_ref() == b"<svg fill=\"black\"/>" && dark.as_ref() == b"<svg fill=\"white\"/>"));
    }

    #[test]
    fn builtin_icons_include_every_provider_asset() {
        let icons =
            fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icons")).unwrap();
        for entry in icons {
            let filename = entry.unwrap().file_name().to_string_lossy().into_owned();
            if let Some(name) = filename
                .strip_prefix("provider-")
                .and_then(|s| s.strip_suffix(".svg"))
            {
                let name = name.strip_suffix(".dark").unwrap_or(name);
                assert!(
                    BUILTIN_ICONS.iter().any(|(key, _)| *key == name),
                    "{filename}"
                );
            }
        }
    }

    #[test]
    fn omp_global_hook_matches_the_existing_profile_path() {
        let home = tempfile::tempdir().unwrap();
        let registry = AgentRegistry::load(BUILTIN_PLUGINS, &[]);
        let agent = registry.agent("omp").unwrap();
        let PathValue::Hook(hook) = &agent.install.as_ref().unwrap().global else {
            panic!("OMP must use a hook")
        };
        let mut host = JsHost::new(HostContext {
            home: home.path().into(),
            cwd: home.path().into(),
            repo_root: None,
            is_global: true,
        });
        let value = host.call(&agent.source, hook, None).unwrap();
        assert_eq!(
            PathBuf::from(value.as_str().unwrap()),
            crate::agents::omp_agent_dir(home.path()).join("skills")
        );
        assert_eq!(
            InstallTarget::from("omp").global_dir(home.path()).unwrap(),
            crate::agents::omp_agent_dir(home.path()).join("skills")
        );
    }

    #[test]
    fn global_path_hooks_return_none_on_errors_or_non_strings() {
        // Exercise the same resolver as InstallTarget without changing the global registry.
        for expression in ["() => {throw new Error('failed')}", "() => 42"] {
            let code = plugin(
                "test",
                &format!("install:{{project:'.test',global:{expression}}}"),
            );
            let registry = AgentRegistry::load(&[("test", &code)], &[]);
            assert!(registry.diagnostics().is_empty());
            assert!(
                registry
                    .agent("test")
                    .unwrap()
                    .global_dir(Path::new("/home/test"))
                    .is_none()
            );
        }
    }

    #[test]
    fn resolves_shared_aliases_without_accepting_unknown_agents() {
        let registry = AgentRegistry::global();
        for (input, expected) in [
            ("claude", "claude-code"),
            ("claude_code", "claude-code"),
            ("open_code", "opencode"),
            ("dim_agent", "dimagent"),
            ("open_claw", "openclaw"),
        ] {
            assert_eq!(registry.resolve(input).unwrap().as_str(), expected);
            let id: AgentId = serde_json::from_value(serde_json::json!(input)).unwrap();
            assert_eq!(id.as_str(), expected);
        }
        assert!(registry.agent("claude").is_none());
        assert!(registry.resolve("missing-plugin").is_none());
        assert!(registry.agent("universal").is_none());
    }
}
