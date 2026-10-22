use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    ProjectSkill, ProjectSkillInstallation, SkillSummary,
    effective_skills::{
        AgentContextEstimate, AgentId, SkillSource, SkillVisibility,
        group_effective_skills_with_paths,
    },
};

#[derive(Clone)]
pub(super) struct EffectivePluginGroup {
    pub key: String,
    pub agent: AgentId,
    pub display_name: String,
    pub skills: Vec<String>,
}

pub(super) fn plugin_groups(
    estimates: &[AgentContextEstimate],
    selected_agent: Option<AgentId>,
) -> Vec<EffectivePluginGroup> {
    let mut groups = BTreeMap::<String, EffectivePluginGroup>::new();
    for estimate in estimates.iter().filter(|estimate| {
        selected_agent
            .as_ref()
            .is_none_or(|agent| agent == &estimate.agent)
    }) {
        for skill in estimate.plugin_skills() {
            let Some(id) = skill.source.plugin_id() else {
                continue;
            };
            let display_name = skill.source.plugin_display_name().unwrap_or(id);
            let key = format!("{}:{id}", estimate.agent.as_str());
            let group = groups
                .entry(key.clone())
                .or_insert_with(|| EffectivePluginGroup {
                    key,
                    agent: estimate.agent.clone(),
                    display_name: display_name.to_string(),
                    skills: Vec::new(),
                });
            if !group.skills.contains(&skill.name) {
                group.skills.push(skill.name.clone());
            }
        }
    }
    for group in groups.values_mut() {
        group.skills.sort();
    }
    groups.into_values().collect()
}

#[derive(Clone)]
pub(super) struct EffectiveSkillRow {
    pub key: String,
    pub name: String,
    pub description: String,
    pub locations: Vec<PathBuf>,
    pub built_in: bool,
    pub agents: Vec<AgentId>,
    pub manual_only: bool,
    pub direct_installations: Vec<ProjectSkillInstallation>,
    pub library_keys: Vec<String>,
}

#[derive(Default)]
struct FilePaths(HashMap<PathBuf, PathBuf>);

impl FilePaths {
    fn canonical(&mut self, path: &Path) -> PathBuf {
        self.0
            .entry(path.to_path_buf())
            .or_insert_with(|| path.canonicalize().unwrap_or_else(|_| path.to_path_buf()))
            .clone()
    }

    fn installation_key(&mut self, path: &Path) -> PathBuf {
        let parent = self.canonical(path.parent().unwrap_or_else(|| Path::new("")));
        path.file_name()
            .map(|name| parent.join(name))
            .unwrap_or(parent)
    }
}

struct InstallationIdentity {
    installation: ProjectSkillInstallation,
    key: PathBuf,
}

/// Built on a worker. UI installation badges only look up library paths;
/// they never resolve symlinks or scan the full project's installations.
#[derive(Default)]
pub(super) struct ProjectSnapshot {
    pub universal_agents: Vec<AgentId>,
    skills: Arc<[ProjectSkill]>,
    installations: HashMap<PathBuf, HashMap<String, Arc<[ProjectSkillInstallation]>>>,
    identities: HashMap<PathBuf, Vec<InstallationIdentity>>,
    library_files: HashMap<PathBuf, Vec<String>>,
}

impl std::ops::Deref for ProjectSnapshot {
    type Target = [ProjectSkill];
    fn deref(&self) -> &Self::Target {
        &self.skills
    }
}

impl ProjectSnapshot {
    pub fn new(skills: Vec<ProjectSkill>, library: &[SkillSummary]) -> Self {
        let mut paths = FilePaths::default();
        let mut identities = HashMap::<PathBuf, Vec<InstallationIdentity>>::new();
        let mut directories = HashMap::<(String, PathBuf), Vec<ProjectSkillInstallation>>::new();
        for skill in &skills {
            for installation in &skill.installations {
                directories
                    .entry((skill.name.clone(), paths.canonical(&installation.path)))
                    .or_default()
                    .push(installation.clone());
                identities
                    .entry(paths.canonical(&installation.path.join("SKILL.md")))
                    .or_default()
                    .push(InstallationIdentity {
                        key: paths.installation_key(&installation.path),
                        installation: installation.clone(),
                    });
            }
        }
        let mut installations =
            HashMap::<PathBuf, HashMap<String, Arc<[ProjectSkillInstallation]>>>::new();
        let mut library_files = HashMap::<PathBuf, Vec<String>>::new();
        for skill in library {
            if let Some(entries) =
                directories.get(&(skill.record.name.clone(), paths.canonical(&skill.path)))
            {
                installations
                    .entry(skill.path.clone())
                    .or_default()
                    .insert(skill.record.name.clone(), entries.clone().into());
            }
            library_files
                .entry(paths.canonical(&skill.path.join("SKILL.md")))
                .or_default()
                .push(super::skill_storage_name(skill).to_string());
        }
        Self {
            universal_agents: Vec::new(),
            skills: skills.into(),
            installations,
            identities,
            library_files,
        }
    }

    pub fn installations_for(&self, skill: &SkillSummary) -> &[ProjectSkillInstallation] {
        self.installations
            .get(&skill.path)
            .and_then(|by_name| by_name.get(&skill.record.name))
            .map(AsRef::as_ref)
            .unwrap_or_default()
    }
}

#[derive(Clone, Default)]
pub(super) struct ProjectView {
    pub rows: Arc<[EffectiveSkillRow]>,
    pub plugins: Arc<[EffectivePluginGroup]>,
    pub can_batch: bool,
}

#[derive(Default)]
pub(super) struct ProjectRenderSnapshot {
    pub estimates: Arc<[AgentContextEstimate]>,
    views: BTreeMap<Option<AgentId>, ProjectView>,
    library_agents: HashMap<String, Vec<AgentId>>,
}

impl ProjectRenderSnapshot {
    pub fn new(estimates: Vec<AgentContextEstimate>, project: &ProjectSnapshot) -> Self {
        let mut paths = FilePaths::default();
        let mut views = BTreeMap::new();
        // Each filtered view has its own names and visibility.
        for agent in std::iter::once(None).chain(
            estimates
                .iter()
                .map(|estimate| Some(estimate.agent.clone())),
        ) {
            let rows: Arc<[EffectiveSkillRow]> =
                skill_rows(&estimates, project, agent.clone(), &mut paths).into();
            let can_batch = rows.iter().any(|row| !row.direct_installations.is_empty());
            views.insert(
                agent.clone(),
                ProjectView {
                    rows,
                    plugins: plugin_groups(&estimates, agent).into(),
                    can_batch,
                },
            );
        }
        let mut file_agents = HashMap::<PathBuf, Vec<AgentId>>::new();
        for estimate in &estimates {
            for skill in &estimate.skills {
                if matches!(skill.source, SkillSource::Builtin) {
                    continue;
                }
                let agents = file_agents.entry(paths.canonical(&skill.path)).or_default();
                if !agents.contains(&estimate.agent) {
                    agents.push(estimate.agent.clone());
                }
            }
        }
        let mut library_agents = HashMap::new();
        for (file, keys) in &project.library_files {
            for key in keys {
                library_agents.insert(
                    key.clone(),
                    file_agents.get(file).cloned().unwrap_or_default(),
                );
            }
        }
        Self {
            estimates: estimates.into(),
            views,
            library_agents,
        }
    }

    pub fn agents_for_skill(&self, skill: &SkillSummary) -> &[AgentId] {
        self.library_agents
            .get(super::skill_storage_name(skill))
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub fn view(&self, agent: Option<AgentId>) -> ProjectView {
        self.views.get(&agent).cloned().unwrap_or_default()
    }
}

fn skill_rows(
    estimates: &[AgentContextEstimate],
    project: &ProjectSnapshot,
    selected_agent: Option<AgentId>,
    paths: &mut FilePaths,
) -> Vec<EffectiveSkillRow> {
    let groups = group_effective_skills_with_paths(
        estimates
            .iter()
            .filter(|estimate| {
                selected_agent
                    .as_ref()
                    .is_none_or(|agent| agent == &estimate.agent)
            })
            .flat_map(|estimate| {
                estimate
                    .skills
                    .iter()
                    .filter(|skill| !skill.is_plugin())
                    .map(|skill| (estimate.agent.clone(), skill))
            }),
        |path| paths.canonical(path),
    );
    let mut rows = Vec::with_capacity(groups.len());
    for group in groups {
        let Some((_, first)) = group.entries().first() else {
            continue;
        };
        let key = if matches!(first.source, SkillSource::Filesystem) {
            format!("file:{}", paths.canonical(&first.path).display())
        } else {
            format!("builtin:{}:{}", group.entries()[0].0, first.id)
        };
        let mut row = EffectiveSkillRow {
            key,
            name: first.name.clone(),
            description: first.description.clone(),
            locations: Vec::new(),
            built_in: false,
            agents: Vec::new(),
            manual_only: true,
            direct_installations: Vec::new(),
            library_keys: Vec::new(),
        };
        let mut seen = HashSet::new();
        for (agent, skill) in group.entries() {
            if row.description.is_empty() && !skill.description.is_empty() {
                row.description = skill.description.clone();
            }
            if let Some(root) = &skill.root_path
                && !row.locations.contains(root)
            {
                row.locations.push(root.clone());
            }
            row.built_in |= matches!(skill.source, SkillSource::Builtin);
            if !row.agents.contains(agent) {
                row.agents.push(agent.clone());
            }
            row.manual_only &= skill.visibility == SkillVisibility::ManualOnly;
            let file = paths.canonical(&skill.path);
            if let Some(installations) = project.identities.get(&file) {
                for identity in installations {
                    if seen.insert(&identity.key) {
                        row.direct_installations.push(identity.installation.clone());
                    }
                }
                if let Some(keys) = project.library_files.get(&file) {
                    for key in keys {
                        if !row.library_keys.contains(key) {
                            row.library_keys.push(key.clone());
                        }
                    }
                }
            }
        }
        row.locations
            .sort_by_cached_key(|path| super::display_path(path));
        row.agents.sort_by_key(|agent| {
            crate::agent_plugins::AgentRegistry::global()
                .agents()
                .iter()
                .position(|item| item.id.as_str() == agent.as_str())
                .unwrap_or(usize::MAX)
        });
        rows.push(row);
    }
    rows.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.key.cmp(&right.key))
    });
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        InstallTarget, SkillOrigin, SkillRecord,
        effective_skills::{EffectiveSkill, SkillScope},
    };

    fn skill(name: &str, path: PathBuf) -> EffectiveSkill {
        EffectiveSkill {
            id: name.into(),
            name: name.into(),
            description: "Fixture".into(),
            when_to_use: None,
            path,
            root_path: None,
            prompt_path: None,
            scope: SkillScope::Local,
            visibility: SkillVisibility::Automatic,
            source: SkillSource::Filesystem,
        }
    }
    fn estimate(agent: AgentId, skills: Vec<EffectiveSkill>) -> AgentContextEstimate {
        AgentContextEstimate {
            agent,
            discovered_count: skills.len(),
            model_visible_count: skills.len(),
            manual_only_count: 0,
            name_only_count: 0,
            conditional_count: 0,
            estimated_tokens: 0,
            skills,
        }
    }
    fn summary(path: PathBuf) -> SkillSummary {
        SkillSummary {
            version: None,
            path,
            installed_projects: 0,
            manual_only: false,
            record: SkillRecord {
                name: "fixture".into(),
                storage_name: "fixture-key".into(),
                description: "Fixture".into(),
                origin: SkillOrigin::Unknown,
                update_available: false,
                group_id: None,
                last_operated_at: 0,
            },
        }
    }

    #[test]
    fn agent_filter_precedes_alias_grouping_and_reuses_completed_rows() {
        let estimates = vec![
            estimate(
                AgentId::from("codex"),
                vec![
                    skill("alpha", "file-a".into()),
                    skill("beta", "file-b".into()),
                ],
            ),
            estimate(
                AgentId::from("claude-code"),
                vec![skill("alpha", "file-b".into())],
            ),
        ];
        let snapshot = ProjectRenderSnapshot::new(estimates, &ProjectSnapshot::default());
        assert_eq!(snapshot.view(None).rows.len(), 2);
        assert_eq!(snapshot.view(Some(AgentId::from("codex"))).rows.len(), 2);
        assert_eq!(
            snapshot.view(Some(AgentId::from("claude-code"))).rows.len(),
            1
        );
        assert!(snapshot.view(Some(AgentId::from("cursor"))).rows.is_empty());
        assert!(Arc::ptr_eq(
            &snapshot.view(None).rows,
            &snapshot.view(None).rows
        ));
    }

    #[test]
    fn same_file_has_one_row_and_is_manual_only_when_all_readers_are_manual() {
        let file = PathBuf::from("shared/SKILL.md");
        let mut manual = skill("one-name", file.clone());
        manual.visibility = SkillVisibility::ManualOnly;
        let automatic = skill("another-name", file);
        let snapshot = ProjectRenderSnapshot::new(
            vec![
                estimate(AgentId::from("codex"), vec![manual]),
                estimate(AgentId::from("droid"), vec![automatic]),
            ],
            &ProjectSnapshot::default(),
        );
        let all = snapshot.view(None);
        assert_eq!(all.rows.len(), 1);
        assert_eq!(all.rows[0].agents.len(), 2);
        assert!(!all.rows[0].manual_only);
        assert!(snapshot.view(Some(AgentId::from("codex"))).rows[0].manual_only);
    }

    #[test]
    fn cached_installations_keep_source_and_installation_identities_distinct() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("project");
        let source = temp.path().join("source");
        let other = temp.path().join("other");
        for path in [&root, &source, &other] {
            std::fs::create_dir_all(path).unwrap();
        }
        for path in [&source, &other] {
            std::fs::write(
                path.join("SKILL.md"),
                "---\nname: fixture\ndescription: Fixture\n---\n",
            )
            .unwrap();
        }
        crate::project::install_from_path(
            &root,
            &source,
            "fixture",
            &[
                InstallTarget::universal(),
                InstallTarget::from("claude-code"),
            ],
        )
        .unwrap();
        crate::project::install_from_path(
            &root,
            &other,
            "fixture",
            &[InstallTarget::from("codex")],
        )
        .unwrap();
        let library = vec![summary(source.clone())];
        let project =
            ProjectSnapshot::new(crate::project::list(&root, temp.path()).unwrap(), &library);
        assert_eq!(project.installations_for(&library[0]).len(), 2);
        let snapshot = ProjectRenderSnapshot::new(
            vec![
                estimate(
                    AgentId::from("codex"),
                    vec![skill("agent-alias", source.join("SKILL.md"))],
                ),
                estimate(
                    AgentId::from("droid"),
                    vec![skill("fixture", other.join("SKILL.md"))],
                ),
            ],
            &project,
        );
        let rows = snapshot.view(None).rows;
        assert_eq!(rows.len(), 2);
        let managed = rows
            .iter()
            .find(|row| !row.library_keys.is_empty())
            .unwrap();
        assert_eq!(managed.direct_installations.len(), 2);
        assert_eq!(managed.library_keys, ["fixture-key"]);
        assert_eq!(
            snapshot.agents_for_skill(&library[0]),
            [AgentId::from("codex")]
        );
        assert_ne!(rows[0].key, rows[1].key);
        // Rendering remains a pure cache read even when a source changes.
        // A new scan, rather than a hover, publishes the changed file state.
        std::fs::remove_file(source.join("SKILL.md")).unwrap();
        assert_eq!(project.installations_for(&library[0]).len(), 2);
        assert!(Arc::ptr_eq(&rows, &snapshot.view(None).rows));
    }
}
