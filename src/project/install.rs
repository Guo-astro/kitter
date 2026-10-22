use crate::{
    InstallTarget,
    directory_link::{self, DirectoryLink},
};
use anyhow::{Context, Result, bail};
use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct InstallConflict {
    pub name: String,
    pub destination: PathBuf,
    pub source: PathBuf,
    pub current_source: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum EntryState {
    Missing,
    Link(DirectoryLink),
    Files(u64),
}

fn state(path: &Path) -> Result<EntryState> {
    match path.symlink_metadata() {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(EntryState::Missing),
        Err(e) => return Err(e.into()),
        Ok(_) => {}
    }
    if let Some(link) = directory_link::inspect(path)? {
        return Ok(EntryState::Link(link));
    }
    let mut hash = DefaultHasher::new();
    // Do not follow links, including links inside a directly installed folder.
    for entry in walkdir::WalkDir::new(path).sort_by_file_name() {
        let entry = entry?;
        entry.path().strip_prefix(path)?.hash(&mut hash);
        let metadata = entry.path().symlink_metadata()?;
        metadata.len().hash(&mut hash);
        metadata.modified()?.hash(&mut hash);
        if metadata.file_type().is_symlink() {
            fs::read_link(entry.path())?.hash(&mut hash);
        }
    }
    Ok(EntryState::Files(hash.finish()))
}

#[derive(Default, Debug)]
pub struct InstallPlan {
    entries: Vec<(InstallConflict, EntryState)>,
}

impl InstallPlan {
    pub fn add(
        &mut self,
        project: &Path,
        source: &Path,
        name: &str,
        targets: &[InstallTarget],
    ) -> Result<()> {
        if !project.is_dir() {
            bail!(t!("errors.project_missing", path = project.display()).to_string());
        }
        self.add_to_roots(source, name, super::installation_roots(project, targets)?)
    }

    pub(super) fn add_to_roots(
        &mut self,
        source: &Path,
        name: &str,
        roots: impl IntoIterator<Item = PathBuf>,
    ) -> Result<()> {
        crate::library::validate_name(name)?;
        let source = source
            .canonicalize()
            .context(crate::user_error!("errors.skill_missing"))?;
        for parent in roots {
            let destination = parent.join(name);
            if let Some((previous, _)) = self.entries.iter().find(|(entry, _)| {
                super::installation_key(&entry.destination) == super::installation_key(&destination)
            }) {
                if previous.source != source {
                    bail!(t!("errors.install_name_conflict", name = name).to_string());
                }
                continue;
            }
            let existing = state(&destination)?;
            let current_source = match &existing {
                EntryState::Link(link) => Some(link.target.clone()),
                _ => None,
            };
            // Keep no-op entries in the plan so a second selection with the same
            // destination cannot silently replace the first one.
            self.entries.push((
                InstallConflict {
                    name: name.to_string(),
                    destination,
                    source: source.clone(),
                    current_source,
                },
                existing,
            ));
        }
        Ok(())
    }

    fn unchanged_source(entry: &InstallConflict, existing: &EntryState) -> bool {
        matches!(existing, EntryState::Link(_))
            && entry.destination.canonicalize().ok().as_ref() == Some(&entry.source)
    }

    pub fn conflicts(&self) -> impl Iterator<Item = &InstallConflict> {
        self.entries
            .iter()
            .filter(|(entry, existing)| {
                *existing != EntryState::Missing && !Self::unchanged_source(entry, existing)
            })
            .map(|(entry, _)| entry)
    }

    /// Execute only a plan the caller has reviewed. All destinations are checked
    /// again before the first change. Backups stay on the same filesystem.
    pub fn execute(self) -> Result<()> {
        self.execute_with(directory_link::create)
    }

    fn execute_with(
        self,
        mut create: impl FnMut(&Path, &Path) -> std::io::Result<()>,
    ) -> Result<()> {
        for (entry, expected) in &self.entries {
            if state(&entry.destination)? != *expected {
                bail!(
                    t!(
                        "errors.install_target_changed",
                        path = entry.destination.display()
                    )
                    .to_string()
                );
            }
            if !entry.source.join("SKILL.md").is_file() {
                return Err(crate::user_error!("errors.skill_missing").into());
            }
            // Never move an ancestor of the source into a temporary backup.
            if entry
                .source
                .starts_with(super::installation_key(&entry.destination))
            {
                bail!(
                    t!(
                        "errors.location_occupied",
                        path = entry.destination.display()
                    )
                    .to_string()
                );
            }
        }
        let mut changed: Vec<(PathBuf, Option<tempfile::TempDir>, bool)> = Vec::new();
        let result = (|| -> Result<()> {
            for (entry, expected) in &self.entries {
                if Self::unchanged_source(entry, expected) {
                    continue;
                }
                let parent = entry.destination.parent().unwrap();
                fs::create_dir_all(parent)?;
                let backup = if *expected != EntryState::Missing {
                    let backup = tempfile::Builder::new()
                        .prefix(".kitter-replace-")
                        .tempdir_in(parent)?;
                    fs::rename(&entry.destination, backup.path().join("original"))?;
                    Some(backup)
                } else {
                    None
                };
                changed.push((entry.destination.clone(), backup, false));
                create(&entry.source, &entry.destination)?;
                changed.last_mut().unwrap().2 = true;
            }
            Ok(())
        })();
        if let Err(error) = result {
            let mut failures = Vec::new();
            for (destination, backup, created) in changed.into_iter().rev() {
                let restored = (|| -> Result<()> {
                    if created {
                        remove_entry(&destination)?;
                    }
                    if let Some(backup) = &backup {
                        fs::rename(backup.path().join("original"), &destination)?;
                    }
                    Ok(())
                })();
                if let Err(failure) = restored {
                    let recovery = backup.map(|backup| backup.keep()).unwrap_or(destination);
                    failures.push(format!("{}: {failure}", recovery.display()));
                }
            }
            if !failures.is_empty() {
                bail!(
                    "{}\n{}",
                    error,
                    t!("errors.rollback_failed", failures = failures.join("\n"))
                );
            }
            return Err(error);
        }
        for (_, backup, _) in changed {
            if let Some(backup) = backup {
                if let Err(error) = remove_entry(&backup.path().join("original")) {
                    let recovery = backup.keep();
                    bail!(
                        t!(
                            "errors.install_backup_kept",
                            path = recovery.display(),
                            error = error
                        )
                        .to_string()
                    );
                }
            }
        }
        Ok(())
    }
}

fn remove_entry(path: &Path) -> Result<()> {
    if let Some(link) = directory_link::inspect(path)? {
        directory_link::remove(path, link)?;
    } else {
        match path.symlink_metadata() {
            Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path)?,
            Ok(_) => fs::remove_file(path)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source(root: &Path, name: &str) -> PathBuf {
        let path = root.join(name);
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("SKILL.md"), "test").unwrap();
        path.canonicalize().unwrap()
    }

    #[test]
    fn replaces_only_the_link_and_keeps_both_sources() {
        let temp = tempfile::tempdir().unwrap();
        let old = source(temp.path(), "old");
        let new = source(temp.path(), "new");
        let root = temp.path().join("target");
        fs::create_dir(&root).unwrap();
        directory_link::create(&old, &root.join("demo")).unwrap();
        let mut plan = InstallPlan::default();
        plan.add_to_roots(&new, "demo", [root.clone()]).unwrap();
        assert_eq!(plan.conflicts().count(), 1);
        assert_eq!(root.join("demo").canonicalize().unwrap(), old);
        plan.execute().unwrap();
        assert_eq!(root.join("demo").canonicalize().unwrap(), new);
        assert!(old.join("SKILL.md").is_file());
        assert!(new.join("SKILL.md").is_file());
        let mut plan = InstallPlan::default();
        plan.add_to_roots(&new, "demo", [root.clone()]).unwrap();
        assert_eq!(plan.conflicts().count(), 0);
        plan.execute().unwrap();
        assert_eq!(fs::read_dir(root).unwrap().count(), 1);
    }

    #[test]
    fn failure_restores_replaced_folders_and_removes_new_links() {
        let temp = tempfile::tempdir().unwrap();
        let new = source(temp.path(), "new");
        let root = temp.path().join("target");
        fs::create_dir_all(root.join("one")).unwrap();
        fs::write(root.join("one/keep"), "user content").unwrap();
        let mut plan = InstallPlan::default();
        for name in ["fresh", "one", "two"] {
            plan.add_to_roots(&new, name, [root.clone()]).unwrap();
        }
        let mut calls = 0;
        let error = plan.execute_with(|source, link| {
            calls += 1;
            if calls == 3 {
                return Err(std::io::Error::other("injected failure"));
            }
            directory_link::create(source, link)
        });
        assert!(error.is_err());
        assert_eq!(
            fs::read_to_string(root.join("one/keep")).unwrap(),
            "user content"
        );
        assert!(root.join("fresh").symlink_metadata().is_err());
        assert!(root.join("two").symlink_metadata().is_err());
        assert_eq!(fs::read_dir(root).unwrap().count(), 1);
    }

    #[test]
    fn changed_destination_requires_another_review_before_any_writes() {
        let temp = tempfile::tempdir().unwrap();
        let new = source(temp.path(), "new");
        let root = temp.path().join("target");
        let mut plan = InstallPlan::default();
        plan.add_to_roots(&new, "fresh", [root.clone()]).unwrap();
        plan.add_to_roots(&new, "occupied", [root.clone()]).unwrap();
        fs::create_dir_all(root.join("occupied")).unwrap();
        fs::write(root.join("occupied/keep"), "new content").unwrap();
        assert!(plan.execute().is_err());
        assert!(root.join("fresh").symlink_metadata().is_err());
        assert!(root.join("occupied/keep").is_file());
    }

    #[test]
    fn conflicting_selections_are_rejected_before_installation() {
        let temp = tempfile::tempdir().unwrap();
        let old = source(temp.path(), "old");
        let new = source(temp.path(), "new");
        let root = temp.path().join("target");
        let mut plan = InstallPlan::default();
        plan.add_to_roots(&old, "demo", [root.clone()]).unwrap();
        assert!(plan.add_to_roots(&new, "demo", [root.clone()]).is_err());
        assert!(!root.exists());
    }

    #[test]
    fn broken_links_require_confirmation_and_can_be_replaced() {
        let temp = tempfile::tempdir().unwrap();
        let old = source(temp.path(), "old");
        let new = source(temp.path(), "new");
        let root = temp.path().join("target");
        fs::create_dir(&root).unwrap();
        directory_link::create(&old, &root.join("demo")).unwrap();
        fs::remove_dir_all(&old).unwrap();
        let mut plan = InstallPlan::default();
        plan.add_to_roots(&new, "demo", [root.clone()]).unwrap();
        assert_eq!(plan.conflicts().count(), 1);
        plan.execute().unwrap();
        assert_eq!(root.join("demo").canonicalize().unwrap(), new);
    }
}
