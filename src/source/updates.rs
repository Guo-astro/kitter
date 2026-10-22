use std::{
    fs::{self, File},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;

use crate::{SkillLibrary, SkillOrigin, SkillRecord};

const CHECK_INTERVAL: u64 = 6 * 60 * 60;

struct CheckGuard(File);

impl Drop for CheckGuard {
    fn drop(&mut self) {
        // Release explicitly: a concurrently spawned child can briefly inherit
        // the file descriptor before exec closes it.
        let _ = self.0.unlock();
    }
}

// Keep the timestamp separate from the locked file's atomic replacement.
fn claim_check(data_dir: &Path, now: u64) -> Result<Option<CheckGuard>> {
    fs::create_dir_all(data_dir)?;
    let lock = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(data_dir.join("update-check.lock"))?;
    match lock.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => return Ok(None),
        Err(std::fs::TryLockError::Error(error)) => return Err(error.into()),
    }
    let guard = CheckGuard(lock);
    let timestamp = data_dir.join("update-check.json");
    if let Ok(bytes) = fs::read(&timestamp)
        && let Ok(last) = serde_json::from_slice::<u64>(&bytes)
        && now.saturating_sub(last) < CHECK_INTERVAL
    {
        return Ok(None);
    }
    // Count attempts, including failed checks, to avoid repeated startup traffic.
    crate::config::save_json(&timestamp, &now)?;
    Ok(Some(guard))
}

pub fn check_updates_if_due(library: &mut SkillLibrary) -> Result<bool> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let Some(_lock) = claim_check(library.data_dir(), now)? else {
        return Ok(false);
    };
    super::check_updates(library)?;
    Ok(true)
}

pub fn can_update(library: &SkillLibrary, record: &SkillRecord) -> bool {
    !matches!(record.origin, SkillOrigin::Builtin | SkillOrigin::Unknown)
        && !library.is_linked_source(&record.storage_name)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UpdateProgress {
    pub completed: usize,
    pub total: usize,
    pub current: Option<String>,
}

#[derive(Debug, Default)]
pub struct BatchUpdateReport {
    pub updated: usize,
    pub failures: Vec<(String, String)>,
}

pub fn update_skills_with_progress(
    data_dir: &Path,
    storage_names: &[String],
    mut on_progress: impl FnMut(UpdateProgress),
) -> Result<BatchUpdateReport> {
    let library = SkillLibrary::open_in(data_dir)?;
    let mut seen = std::collections::HashSet::new();
    let records = storage_names
        .iter()
        .filter(|name| seen.insert((*name).clone()))
        .filter_map(|name| library.record_by_storage(name).ok())
        .filter(|record| can_update(&library, record))
        .collect::<Vec<_>>();
    let mut progress = UpdateProgress {
        total: records.len(),
        ..Default::default()
    };
    let mut report = BatchUpdateReport::default();
    on_progress(progress.clone());
    for record in records {
        progress.current = Some(record.storage_name.clone());
        on_progress(progress.clone());
        let result = (|| {
            let mut library = SkillLibrary::open_in(data_dir)?;
            super::update_by_storage(&mut library, &record.storage_name)
        })();
        match result {
            Ok(()) => report.updated += 1,
            Err(error) => report
                .failures
                .push((record.name, crate::i18n::error_message(&error))),
        }
        progress.completed += 1;
        progress.current = None;
        on_progress(progress.clone());
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_checks_are_limited_across_restarts_and_overlapping_processes() {
        let temp = tempfile::tempdir().unwrap();
        let first = claim_check(temp.path(), 100_000).unwrap().unwrap();
        assert!(
            claim_check(temp.path(), 100_000 + CHECK_INTERVAL)
                .unwrap()
                .is_none()
        );
        drop(first);
        assert!(claim_check(temp.path(), 100_001).unwrap().is_none());
        assert!(claim_check(temp.path(), 99_000).unwrap().is_none());
        assert!(
            claim_check(temp.path(), 100_000 + CHECK_INTERVAL - 1)
                .unwrap()
                .is_none()
        );
        assert!(
            claim_check(temp.path(), 100_000 + CHECK_INTERVAL)
                .unwrap()
                .is_some()
        );
    }
}
