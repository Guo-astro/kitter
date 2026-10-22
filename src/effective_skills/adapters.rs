//! The production seam for effective-Skill inspection: every Agent is
//! estimated by its plugin through the engine.

#[cfg(test)]
use super::EffectiveSkill;
use super::{AgentContextEstimate, AgentId, DiscoveryContext};
use crate::agent_plugins::{AgentRegistry, PluginDiagnostic};

pub(super) fn inspect_project(
    registry: &AgentRegistry,
    context: &DiscoveryContext,
    include_global_only: bool,
) -> (Vec<AgentContextEstimate>, Vec<PluginDiagnostic>) {
    inspect_project_view(
        registry,
        context,
        include_global_only,
        super::SkillView::All,
    )
}

pub(super) fn inspect_project_view(
    registry: &AgentRegistry,
    context: &DiscoveryContext,
    include_global_only: bool,
    view: super::SkillView,
) -> (Vec<AgentContextEstimate>, Vec<PluginDiagnostic>) {
    let mut estimates = Vec::new();
    let mut diagnostics = Vec::new();
    for agent in registry
        .agents()
        .iter()
        .filter(|agent| include_global_only || !agent.global_only)
    {
        match inspect_registry_agent(registry, &agent.id, context, view) {
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
    inspect_registry_agent(
        AgentRegistry::global(),
        agent,
        context,
        super::SkillView::All,
    )?
    .ok()
}

fn inspect_registry_agent(
    registry: &AgentRegistry,
    agent: &AgentId,
    context: &DiscoveryContext,
    view: super::SkillView,
) -> Option<Result<AgentContextEstimate, PluginDiagnostic>> {
    let def = registry.agent(agent.as_str())?;
    let skills = def.skills.as_ref()?;
    Some(
        super::engine::estimate_view(&def.id, skills, &def.source, context, view).map_err(
            |error| PluginDiagnostic {
                origin: def.origin.clone(),
                id: Some(def.id.to_string()),
                message: match error.stack {
                    Some(stack) => format!("{}\n{stack}", error.message),
                    None => error.message,
                },
            },
        ),
    )
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
    let def = AgentRegistry::global().agent(agent.as_str()).unwrap();
    let Some(crate::agent_plugins::CatalogSpec::Custom(catalog)) = def
        .skills
        .as_ref()
        .and_then(|skills| skills.catalog.as_ref())
    else {
        return super::render_name_description_catalog(skills);
    };
    let entries = skills
        .iter()
        .enumerate()
        .map(|(key, skill)| {
            serde_json::json!({
                "key": key, "id": skill.id, "name": skill.name,
                "description": skill.description, "whenToUse": skill.when_to_use,
                "file": skill.path, "dir": skill.path.parent(),
                "scope": skill.scope, "promptPath": skill.prompt_path,
                "visibility": super::engine::plugin_visibility(skill.visibility),
            })
        })
        .collect::<Vec<_>>();
    let mut host = crate::agent_plugins::JsHost::new(crate::agent_plugins::HostContext {
        home: Default::default(),
        cwd: Default::default(),
        repo_root: None,
        is_global: false,
    });
    let rendered = host
        .call(
            &def.source,
            &catalog.render,
            Some(serde_json::json!(entries)),
        )
        .unwrap();
    rendered
        .as_str()
        .or_else(|| rendered["text"].as_str())
        .unwrap()
        .to_owned()
}
