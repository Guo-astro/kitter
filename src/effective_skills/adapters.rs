//! The production seam for effective-Skill inspection.
//!
//! Each Agent adapter owns the static scan and metadata profile used before
//! its entry rules render the initial model catalog. Runtime loading is not
//! part of this module.

use std::collections::HashMap;

use super::additional_agents::{DimAgentAdapter, OmpAdapter};
use super::scanner::ScanProfile;
use super::{
    AgentContextEstimate, AgentId, ClaudeCodeAdapter, CodexAdapter, DiscoveryContext,
    EffectiveSkill, GrokAdapter, MetadataProfile, OpenCodePolicy, PiAdapter, SkillScope, discover,
    estimate_skills, estimate_with_policy, estimate_with_profile, normalize_grok_name,
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

impl AgentAdapter for PiAdapter {
    fn inspect(&self, context: &DiscoveryContext) -> AgentContextEstimate {
        estimate_with_profile(
            self,
            context,
            ScanProfile::PiIgnored,
            MetadataProfile::StrictFrontmatter,
        )
    }
}

impl AgentAdapter for OmpAdapter {
    fn inspect(&self, context: &DiscoveryContext) -> AgentContextEstimate {
        let skills = discover(
            self,
            context,
            ScanProfile::ProviderChildren,
            MetadataProfile::ProviderFrontmatter,
        );
        estimate_skills(
            self,
            super::additional_agents::resolve_omp_collisions(skills, context),
        )
    }
}

impl AgentAdapter for DimAgentAdapter {
    fn inspect(&self, context: &DiscoveryContext) -> AgentContextEstimate {
        let mut skills = discover(
            self,
            context,
            ScanProfile::DimRecursive,
            MetadataProfile::StrictFrontmatter,
        );
        // Apply disabled IDs after resolving name shadowing, as DimAgent does.
        skills.retain(|skill| skill.visibility != super::SkillVisibility::ManualOnly);
        estimate_skills(self, skills)
    }
}

impl AgentAdapter for GrokAdapter {
    fn inspect(&self, context: &DiscoveryContext) -> AgentContextEstimate {
        let skills = discover(
            self,
            context,
            ScanProfile::Recursive {
                max_depth: 5,
                max_directories: usize::MAX,
                max_entries: usize::MAX,
            },
            MetadataProfile::BodyFallback,
        );
        estimate_skills(self, resolve_grok_collisions(skills))
    }
}

fn resolve_grok_collisions(skills: Vec<EffectiveSkill>) -> Vec<EffectiveSkill> {
    let mut resolved: Vec<EffectiveSkill> = Vec::with_capacity(skills.len());
    let mut names: HashMap<String, (SkillScope, usize)> = HashMap::new();
    for mut skill in skills {
        let Some(&(winner_scope, winner_index)) = names.get(&skill.id) else {
            names.insert(skill.id.clone(), (skill.scope, resolved.len()));
            resolved.push(skill);
            continue;
        };
        if winner_scope != skill.scope {
            continue;
        }
        let directory_name = grok_path_identity(&skill);
        if directory_name != skill.id && !names.contains_key(&directory_name) {
            skill.id = directory_name.clone();
            skill.name = directory_name.clone();
            names.insert(directory_name, (skill.scope, resolved.len()));
            resolved.push(skill);
            continue;
        }
        if directory_name == skill.id {
            let winner_directory = grok_path_identity(&resolved[winner_index]);
            if winner_directory != skill.id && !names.contains_key(&winner_directory) {
                let contested = skill.id.clone();
                resolved[winner_index].id = winner_directory.clone();
                resolved[winner_index].name = winner_directory.clone();
                names.remove(&contested);
                names.insert(winner_directory, (winner_scope, winner_index));
                names.insert(contested, (skill.scope, resolved.len()));
                resolved.push(skill);
            }
        }
    }
    resolved
}

fn grok_path_identity(skill: &EffectiveSkill) -> String {
    let raw = if skill
        .path
        .file_name()
        .is_some_and(|name| name == "SKILL.md")
    {
        skill
            .path
            .parent()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
    } else {
        skill.path.file_stem().and_then(|name| name.to_str())
    };
    raw.map(normalize_grok_name)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| skill.id.clone())
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
        "pi" => PiAdapter.inspect(context),
        "omp" => OmpAdapter.inspect(context),
        "dimagent" => DimAgentAdapter {
            home: super::additional_agents::dim_home(context),
        }
        .inspect(context),
        "grok" => GrokAdapter.inspect(context),
        _ => return None,
    }))
}

#[cfg(test)]
pub(super) fn inspect_dimagent(
    context: &DiscoveryContext,
    home: std::path::PathBuf,
) -> AgentContextEstimate {
    DimAgentAdapter { home }.inspect(context)
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
        "pi" => &PiAdapter,
        "omp" => &OmpAdapter,
        "grok" => &GrokAdapter,
        "dimagent" => &DimAgentAdapter {
            home: Default::default(),
        },
        _ => return String::new(),
    };
    policy.render_initial_catalog(skills).text
}
