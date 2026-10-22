use super::*;
use crate::agent_plugins::{self as spec, HostContext, JsHost, PluginError, PluginSource};
use serde::de::DeserializeOwned;

fn error(message: impl ToString) -> PluginError {
    PluginError {
        message: message.to_string(),
        stack: None,
    }
}
fn decode<T: DeserializeOwned>(value: serde_json::Value) -> Result<T, PluginError> {
    serde_json::from_value(value).map_err(error)
}

pub(super) fn resolve_roots(
    skills: &spec::SkillsSpec,
    plugin: &PluginSource,
    context: &HostContext,
) -> Result<Vec<spec::RootSpec>, PluginError> {
    resolve_roots_with(skills, context, |hook| {
        JsHost::new(context.clone()).call(plugin, hook, None)
    })
}

fn resolve_roots_with(
    skills: &spec::SkillsSpec,
    context: &HostContext,
    mut call: impl FnMut(&spec::HookPath) -> Result<serde_json::Value, PluginError>,
) -> Result<Vec<spec::RootSpec>, PluginError> {
    match &skills.roots {
        spec::RootsValue::Decl(decl) => Ok(spec::expand_roots(decl, context)),
        spec::RootsValue::Hook(hook) => decode(call(hook)?),
    }
}

pub(crate) fn estimate(
    agent: &AgentId,
    skills: &spec::SkillsSpec,
    plugin: &PluginSource,
    context: &DiscoveryContext,
) -> Result<AgentContextEstimate, PluginError> {
    estimate_catalog(agent, skills, plugin, context).map(|(estimate, _)| estimate)
}

fn estimate_catalog(
    agent: &AgentId,
    skills: &spec::SkillsSpec,
    plugin: &PluginSource,
    context: &DiscoveryContext,
) -> Result<(AgentContextEstimate, String), PluginError> {
    estimate_catalog_view(agent, skills, plugin, context, SkillView::All)
}

pub(crate) fn estimate_view(
    agent: &AgentId,
    skills: &spec::SkillsSpec,
    plugin: &PluginSource,
    context: &DiscoveryContext,
    view: SkillView,
) -> Result<AgentContextEstimate, PluginError> {
    estimate_catalog_view(agent, skills, plugin, context, view).map(|(estimate, _)| estimate)
}

fn estimate_catalog_view(
    agent: &AgentId,
    skills: &spec::SkillsSpec,
    plugin: &PluginSource,
    context: &DiscoveryContext,
    view: SkillView,
) -> Result<(AgentContextEstimate, String), PluginError> {
    let host_context = HostContext {
        home: context.home.clone(),
        cwd: context.cwd.clone(),
        repo_root: context.repository_root.clone(),
        is_global: is_global_context(context),
    };
    let has_hooks = matches!(skills.roots, spec::RootsValue::Hook(_))
        || skills.builtins.is_some()
        || skills.filter.is_some()
        || skills.resolve.is_some()
        || matches!(skills.catalog, Some(spec::CatalogSpec::Custom(_)));
    let mut host = has_hooks.then(|| JsHost::new(host_context.clone()));
    let mut call = |hook: &spec::HookPath, input| host.as_mut().unwrap().call(plugin, hook, input);
    let roots = resolve_roots_with(skills, &host_context, |hook| call(hook, None))?;
    let mut result = Vec::<spec::SkillEntry>::new();
    if let Some(hook) = &skills.builtins {
        let builtins: Vec<spec::SkillEntryInput> = decode(call(hook, None)?)?;
        for input in builtins {
            result.push(spec::SkillEntry {
                key: result.len(),
                id: input.id,
                name: input.name,
                description: input.description,
                when_to_use: input.when_to_use,
                has_explicit_description: true,
                file: "<built-in>".into(),
                dir: String::new(),
                root: None,
                scope: input.scope.unwrap_or(spec::Scope::System),
                frontmatter: Default::default(),
                visibility: input.visibility.unwrap_or(spec::Visibility::Automatic),
                prompt_path: None,
                source: spec::Source::Builtin,
            });
        }
    }
    let scan = match &skills.scan {
        None | Some(spec::ScanSpec::Profile(spec::ScanProfile::Recursive)) => {
            scanner::ScanProfile::Recursive {
                max_depth: usize::MAX,
                max_directories: usize::MAX,
                max_entries: usize::MAX,
            }
        }
        Some(spec::ScanSpec::Bounded(scan)) => scanner::ScanProfile::Recursive {
            max_depth: scan.recursive.max_depth.unwrap_or(usize::MAX),
            max_directories: scan.recursive.max_directories.unwrap_or(usize::MAX),
            max_entries: scan.recursive.max_entries.unwrap_or(usize::MAX),
        },
        Some(spec::ScanSpec::Profile(profile)) => match profile {
            spec::ScanProfile::DirectChildren => scanner::ScanProfile::DirectChildren,
            spec::ScanProfile::ProviderChildren => scanner::ScanProfile::ProviderChildren,
            spec::ScanProfile::DimRecursive => scanner::ScanProfile::DimRecursive,
            spec::ScanProfile::PiIgnored => scanner::ScanProfile::PiIgnored,
            spec::ScanProfile::Recursive => unreachable!(),
        },
    };
    let metadata_profile = match skills.metadata {
        None | Some(spec::Metadata::Strict) => MetadataProfile::StrictFrontmatter,
        Some(spec::Metadata::BodyFallback) => MetadataProfile::BodyFallback,
        Some(spec::Metadata::Provider) => MetadataProfile::ProviderFrontmatter,
    };
    let mut entries = Vec::new();
    let mut canonical_paths = HashSet::new();
    let mut dim_budgets = [1_000usize; 2];
    for root_spec in roots {
        let root = skill_root(&root_spec);
        let files = if matches!(scan, scanner::ScanProfile::DimRecursive) {
            let budget = &mut dim_budgets[usize::from(matches!(
                root_spec.scope,
                spec::Scope::Local | spec::Scope::Repository
            ))];
            let files = scanner::dim_recursive(&root, *budget);
            *budget -= files.len();
            files
        } else {
            scanner::scan(&root, scan)
        };
        for skill_file in files {
            let skill_dir = skill_file.parent().unwrap_or(&root.path).to_path_buf();
            let canonical_target = if root.exact_skill_file.is_some()
                || (root.include_root_markdown && skill_dir == root.path)
            {
                &skill_file
            } else {
                &skill_dir
            };
            let canonical = fs::canonicalize(canonical_target)
                .unwrap_or_else(|_| canonical_target.to_path_buf());
            if !canonical_paths.insert(canonical) {
                continue;
            }
            let Some(metadata) = read_metadata_with_profile(&skill_file, metadata_profile) else {
                continue;
            };
            if skills.validate != Some(spec::Validation::None)
                && !compatible_skill_is_valid(&metadata)
            {
                continue;
            }
            let id = if skills.id == Some(spec::SkillId::Path) {
                path_derived_skill_id(&root, &skill_dir, &skill_file)
            } else {
                metadata.name.clone()
            };
            let visibility = if skills.visibility == Some(spec::VisibilityRule::Automatic) {
                spec::Visibility::Automatic
            } else {
                plugin_visibility(frontmatter_visibility(&metadata))
            };
            let frontmatter = fs::read_to_string(&skill_file)
                .ok()
                .and_then(|content| {
                    yaml_frontmatter(&content)
                        .and_then(|yaml| serde_yaml::from_str::<serde_json::Value>(yaml).ok())
                })
                .and_then(|value| value.as_object().cloned())
                .unwrap_or_default();
            entries.push(spec::SkillEntry {
                key: result.len() + entries.len(),
                id,
                name: metadata.name,
                description: metadata.description,
                when_to_use: metadata.when_to_use,
                has_explicit_description: metadata.has_explicit_description,
                file: skill_file.display().to_string(),
                dir: skill_dir.display().to_string(),
                root: Some(spec::EntryRoot {
                    path: root.path.clone(),
                    scope: root_spec.scope,
                    plugin: root_spec.plugin.clone(),
                }),
                scope: root_spec.scope,
                frontmatter,
                visibility,
                prompt_path: (skills.prompt_path == Some(spec::PromptPath::File))
                    .then(|| skill_file.display().to_string()),
                source: if root_spec.plugin.is_some() {
                    spec::Source::Plugin
                } else {
                    spec::Source::Filesystem
                },
            });
        }
    }
    if let Some(hook) = &skills.filter {
        entries = apply_updates(
            &entries,
            call(hook, Some(serde_json::to_value(&entries).map_err(error)?))?,
        )?;
    }
    let mut ids = result
        .iter()
        .map(|entry| entry.id.clone())
        .collect::<HashSet<_>>();
    for entry in entries {
        match skills.collision.unwrap_or(spec::Collision::FirstWins) {
            spec::Collision::KeepAll => {}
            spec::Collision::FirstWins if !ids.insert(entry.id.clone()) => continue,
            spec::Collision::FirstWins => {}
            spec::Collision::LastWins => {
                if !ids.insert(entry.id.clone())
                    && let Some(index) = result.iter().position(|skill| skill.id == entry.id)
                {
                    result.remove(index);
                }
            }
        }
        result.push(entry);
    }
    if let Some(hook) = &skills.resolve {
        result = apply_updates(
            &result,
            call(hook, Some(serde_json::to_value(&result).map_err(error)?))?,
        )?;
    }
    result.retain(|entry| match view {
        SkillView::All => true,
        SkillView::Skills => !matches!(entry.source, spec::Source::Plugin),
        SkillView::Plugins => matches!(entry.source, spec::Source::Plugin),
    });
    let visible = result
        .iter()
        .filter(|entry| {
            matches!(
                entry.visibility,
                spec::Visibility::Automatic | spec::Visibility::NameOnly
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    let (text, included) = match &skills.catalog {
        None | Some(spec::CatalogSpec::Xml(_)) => (
            render_name_description_catalog(
                &visible.iter().map(effective_skill).collect::<Vec<_>>(),
            ),
            visible.len(),
        ),
        Some(spec::CatalogSpec::Custom(catalog)) => {
            #[derive(Deserialize)]
            #[serde(untagged)]
            enum Render {
                Text(String),
                Counted { text: String, included: usize },
            }
            match decode(call(
                &catalog.render,
                Some(serde_json::to_value(&visible).map_err(error)?),
            )?)? {
                Render::Text(text) => (text, visible.len()),
                Render::Counted { text, included } => (text, included),
            }
        }
    };
    let skills = result.iter().map(effective_skill).collect::<Vec<_>>();
    Ok((
        AgentContextEstimate {
            agent: agent.clone(),
            discovered_count: skills.len(),
            model_visible_count: included,
            manual_only_count: skills
                .iter()
                .filter(|s| s.visibility == SkillVisibility::ManualOnly)
                .count(),
            name_only_count: skills
                .iter()
                .filter(|s| s.visibility == SkillVisibility::NameOnly)
                .count(),
            conditional_count: skills
                .iter()
                .filter(|s| s.visibility == SkillVisibility::Conditional)
                .count(),
            estimated_tokens: approx_token_count(&text),
            skills,
        },
        text,
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntryUpdate {
    key: usize,
    id: String,
    name: String,
    visibility: spec::Visibility,
    prompt_path: Option<String>,
}
fn apply_updates(
    entries: &[spec::SkillEntry],
    value: serde_json::Value,
) -> Result<Vec<spec::SkillEntry>, PluginError> {
    let updates: Vec<EntryUpdate> = decode(value)?;
    updates
        .into_iter()
        .map(|update| {
            let mut entry = entries
                .iter()
                .find(|entry| entry.key == update.key)
                .cloned()
                .ok_or_else(|| error(format!("unknown skill key: {}", update.key)))?;
            entry.id = update.id;
            entry.name = update.name;
            entry.visibility = update.visibility;
            entry.prompt_path = update.prompt_path;
            Ok(entry)
        })
        .collect()
}
fn skill_root(root: &spec::RootSpec) -> SkillRoot {
    let scope = skill_scope(root.scope);
    let mut result = if let Some(file) = &root.exact_file {
        SkillRoot::exact(file.clone(), scope)
    } else {
        SkillRoot::new(root.path.clone(), scope)
    };
    result = match root.layout.unwrap_or(spec::Layout::Default) {
        spec::Layout::Default => result,
        spec::Layout::RootMarkdown => result.with_root_markdown(),
        spec::Layout::FlatMarkdown => result.flat_markdown(),
        spec::Layout::DirectChildren => result.direct_children(),
    };
    if root.follow_symlinks == Some(false) {
        result = result.without_directory_symlinks();
    }
    if let Some(plugin) = &root.plugin {
        result.source =
            SkillRoot::plugin(root.path.clone(), scope, &plugin.id, &plugin.display_name).source;
    }
    result
}
fn skill_scope(scope: spec::Scope) -> SkillScope {
    match scope {
        spec::Scope::Local => SkillScope::Local,
        spec::Scope::Repository => SkillScope::Repository,
        spec::Scope::User => SkillScope::User,
        spec::Scope::System => SkillScope::System,
    }
}
pub(super) fn plugin_visibility(visibility: SkillVisibility) -> spec::Visibility {
    match visibility {
        SkillVisibility::Automatic => spec::Visibility::Automatic,
        SkillVisibility::NameOnly => spec::Visibility::NameOnly,
        SkillVisibility::ManualOnly => spec::Visibility::ManualOnly,
        SkillVisibility::Conditional => spec::Visibility::Conditional,
    }
}
fn effective_skill(entry: &spec::SkillEntry) -> EffectiveSkill {
    EffectiveSkill {
        id: entry.id.clone(),
        name: entry.name.clone(),
        description: entry.description.clone(),
        when_to_use: entry.when_to_use.clone(),
        path: entry.file.clone().into(),
        root_path: entry.root.as_ref().map(|root| root.path.clone()),
        prompt_path: entry.prompt_path.clone(),
        scope: skill_scope(entry.scope),
        visibility: match entry.visibility {
            spec::Visibility::Automatic => SkillVisibility::Automatic,
            spec::Visibility::NameOnly => SkillVisibility::NameOnly,
            spec::Visibility::ManualOnly => SkillVisibility::ManualOnly,
            spec::Visibility::Conditional => SkillVisibility::Conditional,
        },
        source: match entry.source {
            spec::Source::Filesystem => SkillSource::Filesystem,
            spec::Source::Builtin => SkillSource::Builtin,
            spec::Source::Plugin => {
                let plugin = entry.root.as_ref().unwrap().plugin.as_ref().unwrap();
                SkillSource::Plugin {
                    id: plugin.id.clone(),
                    display_name: plugin.display_name.clone(),
                }
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_plugins::load_spec;
    use std::borrow::Cow;
    use tempfile::tempdir;

    const CURSOR: &str = r#"
        import { defineAgent, standard } from 'kitter';
        export default defineAgent({ apiVersion: 1, id: 'cursor', name: 'Cursor',
            skills: standard.skills({
                project: ['.cursor/skills', '.agents/skills', '.claude/skills', '.codex/skills'],
                user: ['~/.cursor/skills', '~/.agents/skills', '~/.claude/skills', '~/.codex/skills']
            })
        });
    "#;
    const COPILOT: &str = r#"
        import { defineAgent, standard } from 'kitter';
        export default defineAgent({ apiVersion: 1, id: 'copilot', name: 'GitHub Copilot',
            skills: standard.skills({
                project: ['.github/skills', '.agents/skills', '.claude/skills'],
                user: ['~/.copilot/skills', '~/.agents/skills', '~/.claude/skills'],
                envPaths: 'COPILOT_SKILLS_DIRS'
            })
        });
    "#;
    fn source(code: &'static str) -> PluginSource {
        PluginSource {
            name: "test-agent.js".into(),
            code: Cow::Borrowed(code),
        }
    }
    #[test]
    fn cursor_and_copilot_match_legacy_on_golden_fixture() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("home");
        let repo = temp.path().join("repo");
        let project = repo.join("subdir");
        golden::fixture(&home, &repo, &project);
        for code in [CURSOR, COPILOT] {
            let plugin = source(code);
            let spec = load_spec(&plugin).unwrap();
            let agent = AgentId::from(spec.id.as_str());
            for cwd in [&home, &project] {
                let context = DiscoveryContext {
                    home: home.clone(),
                    cwd: cwd.clone(),
                    repository_root: find_repository_root(cwd),
                };
                let (actual, text) =
                    estimate_catalog(&agent, spec.skills.as_ref().unwrap(), &plugin, &context)
                        .unwrap();
                let expected = adapters::inspect_agent(&agent, &context).unwrap();
                let unused_plugin = PluginSource {
                    name: "unused.js".into(),
                    code: Cow::Borrowed("invalid JavaScript"),
                };
                assert_eq!(
                    estimate(
                        &agent,
                        spec.skills.as_ref().unwrap(),
                        &unused_plugin,
                        &context
                    )
                    .unwrap(),
                    expected
                );
                assert_eq!(actual, expected, "{} at {}", agent, cwd.display());
                let visible = expected
                    .skills
                    .iter()
                    .filter(|skill| {
                        matches!(
                            skill.visibility,
                            SkillVisibility::Automatic | SkillVisibility::NameOnly
                        )
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                assert_eq!(text, adapters::render_catalog(&agent, &visible));
            }
        }
    }

    const HOOKS: &str = r#"
        import { defineAgent, standard } from 'kitter';
        export default defineAgent({ apiVersion: 1, id: 'hooks', name: 'Hooks',
            skills: standard.skills({
                roots: ctx => standard.roots(ctx, { user: ['~/.cursor/skills'] }),
                builtins: ctx => [{ id: 'native', name: 'built-in', description: 'Built in' }],
                filter: (entries, ctx) => entries.filter(e => e.name !== 'second').map(e => ({
                    ...e, name: 'changed', description: 'ignored change', extra: true
                })),
                resolve: (entries, ctx) => entries.reverse().map(e => ({ ...e, extra: 'accepted' })),
                catalog: { render: (entries, ctx) => ({ text: entries.map(e => e.name).join('|'), included: 1 }) }
            })
        });
    "#;
    #[test]
    fn hooks_batch_updates_collision_render_and_unknown_key() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("home");
        let repo = temp.path().join("repo");
        golden::fixture(&home, &repo, &repo.join("subdir"));
        let context = DiscoveryContext {
            cwd: home.clone(),
            home,
            repository_root: None,
        };
        let plugin = source(HOOKS);
        let spec = load_spec(&plugin).unwrap();
        let skills = spec.skills.unwrap();
        let (actual, text) =
            estimate_catalog(&AgentId::from("hooks"), &skills, &plugin, &context).unwrap();
        // The builtin wins the native collision; the other entry was filtered out.
        assert_eq!(actual.skills.len(), 1);
        assert_eq!(actual.skills[0].name, "built-in");
        assert_eq!(actual.skills[0].source, SkillSource::Builtin);
        assert_eq!(actual.model_visible_count, 1);
        assert_eq!(actual.estimated_tokens, approx_token_count(&text));
        assert_eq!(text, "built-in");
        let code = HOOKS.replace(
            "id: 'native', name: 'built-in'",
            "id: 'builtin', name: 'built-in'",
        );
        let plugin = PluginSource {
            name: "distinct.js".into(),
            code: Cow::Owned(code),
        };
        let spec = load_spec(&plugin).unwrap();
        let (actual, text) = estimate_catalog(
            &AgentId::from("hooks"),
            spec.skills.as_ref().unwrap(),
            &plugin,
            &context,
        )
        .unwrap();
        assert_eq!(
            actual
                .skills
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>(),
            ["changed", "built-in"]
        );
        assert_eq!(actual.skills[0].description, "Use native");
        assert_eq!(actual.discovered_count, 2);
        assert_eq!(actual.model_visible_count, 1);
        assert_eq!(text, "changed|built-in");
        let plugin = PluginSource {
            name: "unknown.js".into(),
            code: Cow::Owned(
                HOOKS.replace("...e, name: 'changed'", "...e, key: 999, name: 'changed'"),
            ),
        };
        assert!(
            estimate(&AgentId::from("hooks"), &skills, &plugin, &context)
                .unwrap_err()
                .message
                .contains("unknown skill key")
        );
    }
    #[test]
    fn invalid_hook_results_and_exceptions_return_errors() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("home");
        let repo = temp.path().join("repo");
        golden::fixture(&home, &repo, &repo.join("subdir"));
        let context = DiscoveryContext {
            cwd: home.clone(),
            home,
            repository_root: None,
        };
        for (before, after) in [
            (
                "ctx => standard.roots(ctx, { user: ['~/.cursor/skills'] })",
                "ctx => ({ path: 'wrong shape' })",
            ),
            (
                "ctx => [{ id: 'native', name: 'built-in', description: 'Built in' }]",
                "ctx => [null]",
            ),
            ("entries.filter(e => e.name !== 'second')", "[{}]"),
            (
                "entries.reverse()",
                "[{ key: 999, id: 'unknown', name: 'unknown', visibility: 'automatic' }]",
            ),
            ("entries.reverse()", "null"),
            (
                "({ text: entries.map(e => e.name).join('|'), included: 1 })",
                "({ text: 'wrong shape', included: 'one' })",
            ),
            (
                "({ text: entries.map(e => e.name).join('|'), included: 1 })",
                "(() => { throw new Error('render failed'); })()",
            ),
        ] {
            let plugin = PluginSource {
                name: "invalid.js".into(),
                code: Cow::Owned(HOOKS.replace(before, after)),
            };
            let spec = load_spec(&plugin).unwrap();
            assert!(
                estimate(
                    &AgentId::from("hooks"),
                    spec.skills.as_ref().unwrap(),
                    &plugin,
                    &context
                )
                .is_err(),
                "{after}"
            );
        }
        let plugin = PluginSource {
            name: "string.js".into(),
            code: Cow::Owned(HOOKS.replace(
                "({ text: entries.map(e => e.name).join('|'), included: 1 })",
                "'plain text'",
            )),
        };
        let spec = load_spec(&plugin).unwrap();
        let (result, text) = estimate_catalog(
            &AgentId::from("hooks"),
            spec.skills.as_ref().unwrap(),
            &plugin,
            &context,
        )
        .unwrap();
        assert_eq!(text, "plain text");
        assert_eq!(result.model_visible_count, 1);
    }
}
