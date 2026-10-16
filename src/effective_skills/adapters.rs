//! The production seam for effective-Skill inspection.
//!
//! Each Agent adapter owns the static scan and metadata profile used before
//! its entry rules render the initial model catalog. Runtime loading is not
//! part of this module.

#[cfg(test)]
use super::EffectiveSkill;
use super::scanner::ScanProfile;
use super::{
    AgentContextEstimate, AgentId, ClaudeCodeAdapter, CodexAdapter, DiscoveryContext,
    MetadataProfile, OpenCodePolicy, estimate_with_policy, estimate_with_profile,
};
use crate::agent_plugins::{AgentRegistry, PluginDiagnostic};

trait AgentAdapter {
    fn inspect(&self, context: &DiscoveryContext) -> AgentContextEstimate;
}

macro_rules! legacy_adapter {
    ($adapter:ident, $policy:ident) => {
        struct $adapter;
        impl AgentAdapter for $adapter {
            fn inspect(&self, context: &DiscoveryContext) -> AgentContextEstimate {
                estimate_with_policy(&$policy, context)
            }
        }
    };
}

impl AgentAdapter for CodexAdapter {
    fn inspect(&self, context: &DiscoveryContext) -> AgentContextEstimate {
        estimate_with_profile(
            self,
            context,
            ScanProfile::Recursive {
                max_depth: 6,
                max_directories: 2_000,
                max_entries: 20_000,
            },
            MetadataProfile::StrictFrontmatter,
        )
    }
}

impl AgentAdapter for ClaudeCodeAdapter {
    fn inspect(&self, context: &DiscoveryContext) -> AgentContextEstimate {
        estimate_with_profile(
            self,
            context,
            ScanProfile::DirectChildren,
            MetadataProfile::BodyFallback,
        )
    }
}

legacy_adapter!(OpenCodeAdapter, OpenCodePolicy);

pub(super) fn inspect_project(
    registry: &AgentRegistry,
    context: &DiscoveryContext,
    include_global_only: bool,
) -> (Vec<AgentContextEstimate>, Vec<PluginDiagnostic>) {
    let mut estimates = Vec::new();
    let mut diagnostics = Vec::new();
    for agent in registry
        .agents()
        .iter()
        .filter(|agent| include_global_only || !agent.global_only)
    {
        match inspect_registry_agent(registry, &agent.id, context) {
            Some(Ok(estimate)) => estimates.push(estimate),
            Some(Err(diagnostic)) => diagnostics.push(diagnostic),
            None => {}
        }
    }
    (estimates, diagnostics)
}

#[cfg(test)]
pub(super) fn inspect_agent(
    agent: &AgentId,
    context: &DiscoveryContext,
) -> Option<AgentContextEstimate> {
    inspect_registry_agent(AgentRegistry::global(), agent, context)?.ok()
}

fn inspect_registry_agent(
    registry: &AgentRegistry,
    agent: &AgentId,
    context: &DiscoveryContext,
) -> Option<Result<AgentContextEstimate, PluginDiagnostic>> {
    if let Some(def) = registry.agent(agent.as_str())
        && let Some(skills) = &def.skills
    {
        return Some(
            super::engine::estimate(&def.id, skills, &def.source, context).map_err(|error| {
                PluginDiagnostic {
                    origin: def.origin.clone(),
                    id: Some(def.id.to_string()),
                    message: match error.stack {
                        Some(stack) => format!("{}\n{stack}", error.message),
                        None => error.message,
                    },
                }
            }),
        );
    }
    Some(Ok(match agent.as_str() {
        "codex" => CodexAdapter.inspect(context),
        "claude-code" => ClaudeCodeAdapter.inspect(context),
        "opencode" => OpenCodeAdapter.inspect(context),
        _ => return None,
    }))
}

#[cfg(test)]
pub(super) fn inspect_dimagent(
    context: &DiscoveryContext,
    home: std::path::PathBuf,
) -> AgentContextEstimate {
    let def = AgentRegistry::global().agent("dimagent").unwrap();
    let source = crate::agent_plugins::PluginSource {
        name: def.source.name.clone(),
        code: std::borrow::Cow::Owned(def.source.code.replace(
            "dimHome(ctx))",
            &format!("{})", serde_json::to_string(&home).unwrap()),
        )),
    };
    super::engine::estimate(&def.id, def.skills.as_ref().unwrap(), &source, context).unwrap()
}

#[cfg(test)]
pub(super) fn render_catalog(agent: &AgentId, skills: &[EffectiveSkill]) -> String {
    if let Some(def) = AgentRegistry::global().agent(agent.as_str())
        && let Some(spec) = &def.skills
    {
        if let Some(crate::agent_plugins::CatalogSpec::Custom(catalog)) = &spec.catalog {
            let mut host = crate::agent_plugins::JsHost::new(crate::agent_plugins::HostContext {
                home: Default::default(),
                cwd: Default::default(),
                repo_root: None,
                is_global: false,
            });
            let entries = skills
                .iter()
                .enumerate()
                .map(|(key, skill)| {
                    serde_json::json!({
                        "key": key, "id": skill.id, "name": skill.name,
                        "description": skill.description, "whenToUse": skill.when_to_use,
                        "file": skill.path, "dir": skill.path.parent(),
                        "scope": skill.scope, "visibility": skill.visibility,
                        "promptPath": skill.prompt_path,
                    })
                })
                .collect::<Vec<_>>();
            let rendered = host
                .call(
                    &def.source,
                    &catalog.render,
                    Some(serde_json::json!(entries)),
                )
                .unwrap();
            return rendered
                .as_str()
                .or_else(|| rendered["text"].as_str())
                .unwrap()
                .to_owned();
        }
        return super::render_name_description_catalog(skills);
    }
    let policy: &dyn super::AgentSkillPolicy = match agent.as_str() {
        "codex" => &CodexAdapter,
        "claude-code" => &ClaudeCodeAdapter,
        "opencode" => &OpenCodePolicy,
        _ => return String::new(),
    };
    policy.render_initial_catalog(skills).text
}
