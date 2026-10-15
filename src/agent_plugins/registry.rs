use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use super::{AgentId, InstallTarget, expand_path, id::canonical_id};

const UNIVERSAL_PROJECT: &str = ".agents/skills";
const UNIVERSAL_GLOBAL: &str = "~/.agents/skills";

pub struct AgentRegistry {
    agents: Vec<AgentDef>,
}

pub struct AgentDef {
    pub id: AgentId,
    pub name: String,
    pub icon: AgentIcon,
    pub global_only: bool,
    pub reads: Vec<InstallTarget>,
    pub install: Option<InstallDef>,
}

pub enum AgentIcon {
    Builtin(&'static str),
    Monogram,
}

pub struct InstallDef {
    pub project: String,
    pub global: PathValue,
}

pub enum PathValue {
    Template(String),
    /// Temporary until the omp plugin owns its profile logic (T5c).
    Builtin(fn(&Path) -> PathBuf),
}

impl AgentRegistry {
    pub fn global() -> &'static Self {
        static REGISTRY: OnceLock<AgentRegistry> = OnceLock::new();
        REGISTRY.get_or_init(|| Self {
            agents: vec![
                AgentDef {
                    id: AgentId::from("codex"),
                    name: "Codex".into(),
                    icon: AgentIcon::Builtin("icons/provider-codex.svg"),
                    global_only: false,
                    reads: vec![InstallTarget::from("codex"), InstallTarget::universal()],
                    install: Some(InstallDef {
                        project: ".codex/skills".into(),
                        global: PathValue::Template("${env:CODEX_HOME:-~/.codex}/skills".into()),
                    }),
                },
                AgentDef {
                    id: AgentId::from("claude-code"),
                    name: "Claude Code".into(),
                    icon: AgentIcon::Builtin("icons/provider-claude.svg"),
                    global_only: false,
                    reads: vec![InstallTarget::from("claude-code")],
                    install: Some(InstallDef {
                        project: ".claude/skills".into(),
                        global: PathValue::Template("~/.claude/skills".into()),
                    }),
                },
                AgentDef {
                    id: AgentId::from("cursor"),
                    name: "Cursor".into(),
                    icon: AgentIcon::Builtin("icons/provider-cursor.svg"),
                    global_only: false,
                    reads: vec![
                        InstallTarget::from("cursor"),
                        InstallTarget::from("claude-code"),
                        InstallTarget::from("codex"),
                        InstallTarget::universal(),
                    ],
                    install: Some(InstallDef {
                        project: ".cursor/skills".into(),
                        global: PathValue::Template("~/.cursor/skills".into()),
                    }),
                },
                AgentDef {
                    id: AgentId::from("opencode"),
                    name: "OpenCode".into(),
                    icon: AgentIcon::Builtin("icons/provider-opencode.svg"),
                    global_only: false,
                    reads: vec![
                        InstallTarget::from("opencode"),
                        InstallTarget::from("claude-code"),
                        InstallTarget::universal(),
                    ],
                    install: Some(InstallDef {
                        project: ".opencode/skills".into(),
                        global: PathValue::Template(
                            "${env:XDG_CONFIG_HOME:-~/.config}/opencode/skills".into(),
                        ),
                    }),
                },
                AgentDef {
                    id: AgentId::from("pi"),
                    name: "Pi".into(),
                    icon: AgentIcon::Builtin("icons/provider-pi.svg"),
                    global_only: false,
                    reads: vec![InstallTarget::from("pi"), InstallTarget::universal()],
                    install: Some(InstallDef {
                        project: ".pi/skills".into(),
                        global: PathValue::Template(
                            "${env:PI_CODING_AGENT_DIR:-~/.pi/agent}/skills".into(),
                        ),
                    }),
                },
                AgentDef {
                    id: AgentId::from("omp"),
                    name: "omp".into(),
                    icon: AgentIcon::Builtin("icons/provider-omp.svg"),
                    global_only: false,
                    reads: vec![
                        InstallTarget::from("omp"),
                        InstallTarget::from("claude-code"),
                        InstallTarget::from("codex"),
                        InstallTarget::from("opencode"),
                        InstallTarget::from("antigravity"),
                        InstallTarget::from("copilot"),
                        InstallTarget::universal(),
                    ],
                    install: Some(InstallDef {
                        project: ".omp/skills".into(),
                        global: PathValue::Builtin(|home| {
                            crate::agents::omp_agent_dir(home).join("skills")
                        }),
                    }),
                },
                AgentDef {
                    id: AgentId::from("dimagent"),
                    name: "DimAgent".into(),
                    icon: AgentIcon::Builtin("icons/provider-dimagent.svg"),
                    global_only: false,
                    reads: vec![InstallTarget::universal()],
                    install: None,
                },
                AgentDef {
                    id: AgentId::from("grok"),
                    name: "Grok".into(),
                    icon: AgentIcon::Builtin("icons/provider-grok.svg"),
                    global_only: false,
                    reads: vec![
                        InstallTarget::from("grok"),
                        InstallTarget::from("claude-code"),
                        InstallTarget::from("cursor"),
                        InstallTarget::universal(),
                    ],
                    install: Some(InstallDef {
                        project: ".grok/skills".into(),
                        global: PathValue::Template("~/.grok/skills".into()),
                    }),
                },
                AgentDef {
                    id: AgentId::from("openclaw"),
                    name: "OpenClaw".into(),
                    icon: AgentIcon::Builtin("icons/provider-openclaw.svg"),
                    global_only: true,
                    reads: vec![],
                    install: None,
                },
                AgentDef {
                    id: AgentId::from("hermes"),
                    name: "Hermes".into(),
                    icon: AgentIcon::Builtin("icons/provider-hermes.svg"),
                    global_only: true,
                    reads: vec![],
                    install: None,
                },
                AgentDef {
                    id: AgentId::from("droid"),
                    name: "Droid".into(),
                    icon: AgentIcon::Builtin("icons/provider-droid.svg"),
                    global_only: false,
                    reads: vec![
                        InstallTarget::from("droid"),
                        InstallTarget::from("antigravity"),
                        InstallTarget::universal(),
                    ],
                    install: Some(InstallDef {
                        project: ".factory/skills".into(),
                        global: PathValue::Template("~/.factory/skills".into()),
                    }),
                },
                AgentDef {
                    id: AgentId::from("amp"),
                    name: "Amp".into(),
                    icon: AgentIcon::Builtin("icons/provider-amp.svg"),
                    global_only: false,
                    reads: vec![
                        InstallTarget::from("claude-code"),
                        InstallTarget::universal(),
                    ],
                    install: None,
                },
                AgentDef {
                    id: AgentId::from("antigravity"),
                    name: "Antigravity".into(),
                    icon: AgentIcon::Builtin("icons/provider-antigravity.svg"),
                    global_only: false,
                    reads: vec![
                        InstallTarget::from("antigravity"),
                        InstallTarget::universal(),
                    ],
                    install: Some(InstallDef {
                        project: ".agent/skills".into(),
                        global: PathValue::Template("~/.gemini/config/skills".into()),
                    }),
                },
                AgentDef {
                    id: AgentId::from("copilot"),
                    name: "GitHub Copilot".into(),
                    icon: AgentIcon::Builtin("icons/provider-copilot.svg"),
                    global_only: false,
                    reads: vec![
                        InstallTarget::from("copilot"),
                        InstallTarget::from("claude-code"),
                        InstallTarget::universal(),
                    ],
                    install: Some(InstallDef {
                        project: ".github/skills".into(),
                        global: PathValue::Template("~/.copilot/skills".into()),
                    }),
                },
                AgentDef {
                    id: AgentId::from("trae"),
                    name: "Trae".into(),
                    icon: AgentIcon::Builtin("icons/provider-trae.svg"),
                    global_only: false,
                    reads: vec![InstallTarget::universal()],
                    install: None,
                },
            ],
        })
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
        let install = AgentRegistry::global()
            .agent(self.as_str())?
            .install
            .as_ref()?;
        Some(match &install.global {
            PathValue::Template(template) => expand_path(template, home, home),
            PathValue::Builtin(path) => path(home),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
