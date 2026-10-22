use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use crate::{
    InstallTarget, ProjectSkill, ProjectSkillInstallation, agent_plugins::AgentRegistry,
    agents::installation_root, directory_link,
};

mod install;
pub use install::{InstallConflict, InstallPlan};

#[derive(Clone, Debug)]
pub enum RemovalKind {
    ManagedInstallation,
    ExternalLink { source: PathBuf },
    SourceFiles,
}

#[derive(Default)]
pub struct RemovalReport {
    pub removed: usize,
    pub failures: Vec<String>,
}

pub fn install(
    project: &Path,
    library_dir: &Path,
    name: &str,
    targets: &[InstallTarget],
) -> Result<()> {
    crate::library::validate_name(name)?;
    if !project.is_dir() {
        bail!(t!("errors.project_missing", path = project.display()).to_string());
    }
    let source = library_dir
        .join(name)
        .canonicalize()
        .context(crate::user_error!("errors.skill_missing"))?;
    install_from_path(project, &source, name, targets)
}

pub fn install_from_path(
    project: &Path,
    source: &Path,
    name: &str,
    targets: &[InstallTarget],
) -> Result<()> {
    crate::library::validate_name(name)?;
    if !project.is_dir() {
        bail!(t!("errors.project_missing", path = project.display()).to_string());
    }
    install_to_roots(source, name, installation_roots(project, targets)?)
}

fn installation_roots(project: &Path, targets: &[InstallTarget]) -> Result<Vec<PathBuf>> {
    if targets.is_empty() {
        crate::user_bail!("errors.install_location_required");
    }
    targets
        .iter()
        .map(|target| {
            installation_root(project, target).ok_or_else(|| {
                let agent = AgentRegistry::global()
                    .agent(target.as_str())
                    .map(|agent| agent.name.as_str())
                    .unwrap_or(target.as_str());
                crate::user_error!("errors.install_target_unavailable", agent = agent).into()
            })
        })
        .collect()
}

fn install_to_roots(
    source: &Path,
    name: &str,
    roots: impl IntoIterator<Item = PathBuf>,
) -> Result<()> {
    let mut plan = InstallPlan::default();
    plan.add_to_roots(source, name, roots)?;
    if let Some(conflict) = plan.conflicts().next() {
        bail!(
            t!(
                "errors.location_occupied",
                path = conflict.destination.display()
            )
            .to_string()
        );
    }
    plan.execute()
}

pub fn uninstall(
    project: &Path,
    library_dir: &Path,
    name: &str,
    targets: &[InstallTarget],
) -> Result<()> {
    crate::library::validate_name(name)?;
    let source = library_dir.join(name).canonicalize().ok();
    uninstall_from_source(project, source.as_deref(), name, targets)
}

pub fn uninstall_from_path(
    project: &Path,
    source: &Path,
    name: &str,
    targets: &[InstallTarget],
) -> Result<()> {
    crate::library::validate_name(name)?;
    let source = source.canonicalize().ok();
    uninstall_from_source(project, source.as_deref(), name, targets)
}

fn uninstall_from_source(
    project: &Path,
    source: Option<&Path>,
    name: &str,
    targets: &[InstallTarget],
) -> Result<()> {
    let mut removable = Vec::new();
    let mut removable_keys = HashSet::new();
    for target in targets {
        let Some(root) = installation_root(project, target) else {
            continue;
        };
        let link = root.join(name);
        match link.symlink_metadata() {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error).context(
                    t!("errors.location_check_failed", path = link.display()).to_string(),
                );
            }
        }
        let Some(directory_link) = directory_link::inspect(&link)? else {
            bail!(t!("errors.unmanaged_delete_protected", path = link.display()).to_string());
        };
        let Some(source) = source else {
            bail!(t!("errors.link_skill_missing", path = link.display()).to_string());
        };
        if link.canonicalize().ok().as_deref() != Some(source) {
            bail!(t!("errors.link_target_mismatch", path = link.display()).to_string());
        }
        if removable_keys.insert(installation_key(&link)) {
            removable.push((link, directory_link));
        }
    }
    for (link, directory_link) in removable {
        directory_link::remove(&link, directory_link)?;
    }
    Ok(())
}

pub fn uninstall_all(project: &Path, name: &str, library_dir: &Path) -> Result<()> {
    crate::library::validate_name(name)?;
    let source = library_dir.join(name).canonicalize().ok();
    uninstall_all_from_source(project, name, source.as_deref())
}

pub fn uninstall_all_from_path(project: &Path, name: &str, source: &Path) -> Result<()> {
    crate::library::validate_name(name)?;
    let source = source.canonicalize().ok();
    uninstall_all_from_source(project, name, source.as_deref())
}

fn uninstall_all_from_source(project: &Path, name: &str, source: Option<&Path>) -> Result<()> {
    for target in AgentRegistry::global().install_targets() {
        let Some(root) = installation_root(project, &target) else {
            continue;
        };
        let link = root.join(name);
        let Some(directory_link) = directory_link::inspect(&link)? else {
            continue;
        };
        if let Some(source) = source
            && link
                .canonicalize()
                .ok()
                .is_some_and(|resolved| resolved == source)
        {
            directory_link::remove(&link, directory_link)?;
        }
    }
    Ok(())
}

pub fn removal_kind(installation: &ProjectSkillInstallation) -> RemovalKind {
    if installation.managed {
        return RemovalKind::ManagedInstallation;
    }
    let Ok(Some(directory_link)) = directory_link::inspect(&installation.path) else {
        return RemovalKind::SourceFiles;
    };
    let source = directory_link
        .target
        .canonicalize()
        .unwrap_or(directory_link.target);
    RemovalKind::ExternalLink { source }
}

fn is_supported_installation_path(installation: &ProjectSkillInstallation) -> bool {
    installation.path.parent().is_some_and(|parent| {
        installation
            .target
            .project_dir()
            .is_some_and(|dir| parent.ends_with(dir))
            || dirs::home_dir().is_some_and(|home| {
                if parent.ancestors().any(|project| {
                    installation.target.project_root(project, &home).as_deref() == Some(parent)
                }) {
                    return true;
                }
                crate::agents::global_target_root(&home, &installation.target)
                    .is_some_and(|root| parent == root)
            })
    })
}

pub fn remove_project_skill(installation: &ProjectSkillInstallation) -> Result<()> {
    if !is_supported_installation_path(installation) {
        bail!(
            t!(
                "errors.skill_path_outside_target",
                path = installation.path.display()
            )
            .to_string()
        );
    }
    let metadata = installation
        .path
        .symlink_metadata()
        .with_context(|| t!("errors.not_found", path = installation.path.display()).to_string())?;
    if let Some(directory_link) = directory_link::inspect(&installation.path)? {
        directory_link::remove(&installation.path, directory_link)?;
    } else if metadata.is_file() {
        fs::remove_file(&installation.path)?;
    } else if metadata.is_dir() {
        fs::remove_dir_all(&installation.path)?;
    } else {
        bail!(t!("errors.skill_delete_failed").to_string());
    }
    Ok(())
}

/// Return the physical installation entry, resolving aliases in the parent
/// directory without resolving the Skill link itself. This keeps two paths
/// such as `.agents/skills/foo` and `.claude/skills/foo` from being removed
/// twice when one parent directory is a symlink to the other.
pub fn installation_key(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let parent = parent
        .canonicalize()
        .unwrap_or_else(|_| parent.to_path_buf());
    path.file_name()
        .map(|name| parent.join(name))
        .unwrap_or(parent)
}

pub fn remove_project_skills<'a>(
    installations: impl IntoIterator<Item = &'a ProjectSkillInstallation>,
) -> RemovalReport {
    let mut report = RemovalReport::default();
    let mut removed_keys = HashSet::new();
    for installation in installations {
        if !removed_keys.insert(installation_key(&installation.path)) {
            continue;
        }
        match installation.path.symlink_metadata() {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                report
                    .failures
                    .push(t!("errors.not_found", path = installation.path.display()).to_string());
            }
            Err(error) => report.failures.push(
                t!(
                    "errors.item_detail",
                    item = installation.path.display(),
                    error = error
                )
                .to_string(),
            ),
            Ok(_) => match remove_project_skill(installation) {
                Ok(()) => report.removed += 1,
                Err(error) => report.failures.push(
                    t!(
                        "errors.item_detail",
                        item = installation.path.display(),
                        error = error
                    )
                    .to_string(),
                ),
            },
        }
    }
    report
}

pub fn list(project: &Path, library_dir: &Path) -> Result<Vec<ProjectSkill>> {
    let mut grouped = BTreeMap::<String, Vec<ProjectSkillInstallation>>::new();
    let canonical_library = library_dir
        .canonicalize()
        .unwrap_or_else(|_| library_dir.to_path_buf());
    let linked_sources = fs::read_dir(library_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            directory_link::inspect(&path).ok().flatten()?;
            path.join("SKILL.md")
                .is_file()
                .then(|| path.canonicalize().ok())
                .flatten()
        })
        .collect::<HashSet<_>>();
    let mut listed_roots = HashSet::new();
    for target in AgentRegistry::global().install_targets() {
        let Some(parent) = installation_root(project, &target) else {
            continue;
        };
        // Several targets can install to the same path. Preserve distinct
        // alias paths for removal, but never list the identical path twice.
        if !listed_roots.insert(parent.clone()) {
            continue;
        }
        let Ok(entries) = fs::read_dir(&parent) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.join("SKILL.md").is_file() {
                continue;
            }
            let managed = path.canonicalize().ok().is_some_and(|resolved| {
                resolved.starts_with(&canonical_library)
                    || (linked_sources.contains(&resolved)
                        && directory_link::inspect(&path).ok().flatten().is_some())
            });
            grouped
                .entry(
                    crate::library::read_frontmatter(&path.join("SKILL.md"))
                        .ok()
                        .and_then(|(name, _)| name)
                        .unwrap_or_else(|| entry.file_name().to_string_lossy().into_owned()),
                )
                .or_default()
                .push(ProjectSkillInstallation {
                    target: target.clone(),
                    path,
                    managed,
                });
        }
    }
    Ok(grouped
        .into_iter()
        .map(|(name, installations)| ProjectSkill {
            name,
            installations,
        })
        .collect())
}

pub fn is_installed_any(project: &Path, name: &str, library_dir: &Path) -> bool {
    is_installed_any_from_path(project, name, &library_dir.join(name))
}

pub fn is_installed_any_from_path(project: &Path, name: &str, source: &Path) -> bool {
    let source = source.canonicalize().ok();
    let Some(source) = source else {
        return false;
    };
    AgentRegistry::global()
        .install_targets()
        .iter()
        .any(|target| {
            let Some(parent) = installation_root(project, target) else {
                return false;
            };
            let path = parent.join(name);
            path.canonicalize()
                .ok()
                .is_some_and(|resolved| resolved == source)
                || fs::read_dir(parent)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .any(|entry| entry.path().canonicalize().ok().as_ref() == Some(&source))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    struct Fixture {
        _temp: tempfile::TempDir,
        project: PathBuf,
        library: PathBuf,
        source: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();
            let project = temp.path().join("project");
            let library = temp.path().join("library");
            let source = library.join("example");
            fs::create_dir_all(project.join(".git")).unwrap();
            fs::create_dir_all(&source).unwrap();
            fs::write(source.join("SKILL.md"), "# Example").unwrap();
            Self {
                _temp: temp,
                project,
                library,
                source,
            }
        }

        fn installation(&self, target: &InstallTarget) -> PathBuf {
            crate::agents::installation_root(&self.project, target)
                .unwrap()
                .join("example")
        }
    }

    #[test]
    fn global_installation_uses_user_roots_and_keeps_source_intact() {
        let fixture = Fixture::new();
        let home = fixture._temp.path().join("home");
        // These targets have no environment overrides, so this test cannot write to real user roots.
        let targets = [
            InstallTarget::universal(),
            InstallTarget::from("antigravity"),
            InstallTarget::from("copilot"),
        ];
        let roots =
            targets.map(|target| crate::agents::global_target_root(&home, &target).unwrap());
        for _ in 0..2 {
            install_to_roots(&fixture.source, "example", roots.clone()).unwrap();
        }
        for root in roots {
            assert_eq!(
                root.join("example").canonicalize().unwrap(),
                fixture.source.canonicalize().unwrap()
            );
        }
        assert!(!home.join(".agent/skills").exists());
        assert!(!home.join(".github/skills").exists());
        assert!(fixture.source.join("SKILL.md").is_file());
    }

    #[test]
    fn installs_idempotently_and_uninstalls_without_touching_the_source() {
        let fixture = Fixture::new();
        let targets = [
            InstallTarget::universal(),
            InstallTarget::from("claude-code"),
        ];

        install(&fixture.project, &fixture.library, "example", &targets).unwrap();
        install(&fixture.project, &fixture.library, "example", &targets).unwrap();

        for target in &targets {
            let path = fixture.installation(target);
            assert_eq!(
                path.canonicalize().unwrap(),
                fixture.source.canonicalize().unwrap()
            );
            assert!(directory_link::inspect(&path).unwrap().is_some());
        }

        uninstall(&fixture.project, &fixture.library, "example", &targets).unwrap();
        for target in &targets {
            assert!(fixture.installation(target).symlink_metadata().is_err());
        }
        assert_eq!(
            fs::read_to_string(fixture.source.join("SKILL.md")).unwrap(),
            "# Example"
        );
    }

    #[test]
    fn lists_and_removes_every_supported_project_install_target() {
        let fixture = Fixture::new();
        let targets = AgentRegistry::global()
            .install_targets()
            .into_iter()
            .filter(|target| installation_root(&fixture.project, target).is_some())
            .collect::<Vec<_>>();

        install(&fixture.project, &fixture.library, "example", &targets).unwrap();

        let listed = list(&fixture.project, &fixture.library).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(
            listed[0].installations.len(),
            targets
                .iter()
                .map(|target| fixture.installation(target))
                .collect::<HashSet<_>>()
                .len()
        );
        for target in &targets {
            assert!(fixture.installation(target).symlink_metadata().is_ok());
            assert!(listed[0].installations.iter().any(|installation| {
                installation.path == fixture.installation(target) && installation.managed
            }));
        }

        uninstall(&fixture.project, &fixture.library, "example", &targets).unwrap();
        for target in &targets {
            assert!(fixture.installation(target).symlink_metadata().is_err());
        }
    }

    #[test]
    fn unavailable_targets_fail_before_any_installation() {
        let fixture = Fixture::new();
        let targets = [InstallTarget::universal(), InstallTarget::from("hermes")];
        assert!(install(&fixture.project, &fixture.library, "example", &targets).is_err());
        let mut plan = InstallPlan::default();
        assert!(
            plan.add(&fixture.project, &fixture.source, "example", &targets)
                .is_err()
        );
        assert!(!fixture.project.join(".agents/skills/example").exists());
        assert!(!fixture.project.join(".hermes/skills/example").exists());
    }

    #[test]
    fn refuses_an_occupied_directory_before_creating_any_links() {
        let fixture = Fixture::new();
        let occupied = fixture.installation(&InstallTarget::from("claude-code"));
        fs::create_dir_all(&occupied).unwrap();
        fs::write(occupied.join("keep"), "user data").unwrap();

        let error = install(
            &fixture.project,
            &fixture.library,
            "example",
            &[
                InstallTarget::universal(),
                InstallTarget::from("claude-code"),
            ],
        )
        .unwrap_err();

        assert!(["en", "zh-CN"].iter().any(|locale| {
            error
                .to_string()
                .contains(t!("errors.location_occupied", locale = locale, path = "").trim())
        }));
        assert!(
            fixture
                .installation(&InstallTarget::universal())
                .symlink_metadata()
                .is_err()
        );
        assert_eq!(
            fs::read_to_string(occupied.join("keep")).unwrap(),
            "user data"
        );
    }

    #[test]
    fn refuses_to_uninstall_a_link_to_another_source() {
        let fixture = Fixture::new();
        let other = fixture.library.join("other");
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join("SKILL.md"), "# Other").unwrap();
        let link = fixture.installation(&InstallTarget::universal());
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        directory_link::create(&other, &link).unwrap();

        let error = uninstall(
            &fixture.project,
            &fixture.library,
            "example",
            &[InstallTarget::universal()],
        )
        .unwrap_err();

        assert!(["en", "zh-CN"].iter().any(|locale| {
            error
                .to_string()
                .contains(t!("errors.link_target_mismatch", locale = locale, path = "").trim())
        }));
        assert_eq!(link.canonicalize().unwrap(), other.canonicalize().unwrap());
    }

    #[test]
    fn refuses_to_uninstall_when_the_managed_source_is_missing() {
        let fixture = Fixture::new();
        let link = fixture.installation(&InstallTarget::universal());
        install(
            &fixture.project,
            &fixture.library,
            "example",
            &[InstallTarget::universal()],
        )
        .unwrap();
        fs::remove_dir_all(&fixture.source).unwrap();

        let error = uninstall(
            &fixture.project,
            &fixture.library,
            "example",
            &[InstallTarget::universal()],
        )
        .unwrap_err();

        assert!(["en", "zh-CN"].iter().any(|locale| {
            error
                .to_string()
                .contains(t!("errors.link_skill_missing", locale = locale, path = "").trim())
        }));
        assert!(link.symlink_metadata().is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn removes_an_aliased_installation_only_once() {
        let fixture = Fixture::new();
        install(
            &fixture.project,
            &fixture.library,
            "example",
            &[InstallTarget::universal()],
        )
        .unwrap();
        let claude_root = fixture.project.join(".claude");
        fs::create_dir_all(&claude_root).unwrap();
        symlink(
            fixture.project.join(".agents/skills"),
            claude_root.join("skills"),
        )
        .unwrap();

        let listed = list(&fixture.project, &fixture.library).unwrap();
        assert_eq!(listed[0].installations.len(), 2);
        assert_eq!(
            installation_key(&listed[0].installations[0].path),
            installation_key(&listed[0].installations[1].path)
        );

        let report = remove_project_skills(listed[0].installations.iter());
        assert_eq!(report.removed, 1);
        assert!(report.failures.is_empty());
        assert!(
            fixture
                .installation(&InstallTarget::universal())
                .symlink_metadata()
                .is_err()
        );
    }

    #[test]
    fn removes_a_direct_unmanaged_installation() {
        let fixture = Fixture::new();
        let path = fixture.installation(&InstallTarget::universal());
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("SKILL.md"), "# External").unwrap();
        let installation = ProjectSkillInstallation {
            target: InstallTarget::universal(),
            path: path.clone(),
            managed: false,
        };

        let report = remove_project_skills(std::iter::once(&installation));

        assert_eq!(report.removed, 1);
        assert!(report.failures.is_empty());
        assert!(path.symlink_metadata().is_err());
    }

    #[test]
    fn refuses_to_remove_an_installation_outside_a_supported_skill_root() {
        let fixture = Fixture::new();
        let path = fixture.project.join("external").join("example");
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("SKILL.md"), "# External").unwrap();
        let installation = ProjectSkillInstallation {
            target: InstallTarget::universal(),
            path: path.clone(),
            managed: false,
        };

        let report = remove_project_skills(std::iter::once(&installation));

        assert_eq!(report.removed, 0);
        assert_eq!(report.failures.len(), 1);
        assert!(path.join("SKILL.md").is_file());
    }

    #[test]
    fn rejects_skill_names_that_can_escape_the_target_directory() {
        let fixture = Fixture::new();
        let error = install(
            &fixture.project,
            &fixture.library,
            "../example",
            &[InstallTarget::universal()],
        )
        .unwrap_err();

        assert!(error.downcast_ref::<crate::i18n::UserError>().is_some());
    }
}
