//! Managing user Agent plugins: adding them from GitHub or local files,
//! removing, refreshing, enabling/disabling and ordering.
//!
//! Persistent state lives in `<data>/agents.json`; plugin files themselves are
//! always copied into `plugin_dir()` as `<id>.js`, so loading stays unchanged.

use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

use anyhow::{Context, Result};
use rust_i18n::_tr as t;
use serde::{Deserialize, Serialize};

use super::{
    AgentDef, AgentIcon, AgentRegistry, BUILTIN_PLUGINS, PluginOrigin, PluginSource,
    host::SpecLoader, plugin_dir,
};
use crate::source::{run, run_output, tool_command};

/// Contents of `<data>/agents.json`. Missing file = default.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentSettings {
    /// Agent ids in display order. Unknown ids are ignored; agents not listed
    /// keep their default order after the listed ones.
    #[serde(default)]
    pub order: Vec<String>,
    #[serde(default)]
    pub disabled: BTreeSet<String>,
    /// Where each managed plugin file came from, keyed by Agent id. Plugins
    /// without an entry were placed in the plugin directory by hand.
    #[serde(default)]
    pub sources: BTreeMap<String, SourceRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SourceRecord {
    #[serde(rename_all = "camelCase")]
    Github {
        /// `owner/repo`.
        repo: String,
        /// File path inside the repository, `/`-separated.
        path: String,
        branch: String,
        commit: String,
    },
    #[serde(rename_all = "camelCase")]
    Local { path: PathBuf },
}

/// Parsed user input for "add Agent".
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceInput {
    /// `owner/repo`.
    Github(String),
    /// A `.js` file or a directory.
    Local(PathBuf),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CandidateStatus {
    Ready,
    /// Same id as a built-in Agent; adding it overrides the built-in.
    Override,
    /// A user plugin with this id already exists.
    AlreadyAdded,
    /// Failed to load; the message is the plugin error.
    Invalid(String),
}

pub struct Candidate {
    /// Agent id; for invalid files without a readable id, the file stem.
    pub id: String,
    pub name: String,
    pub icon: AgentIcon,
    /// Path relative to the source root, `/`-separated.
    pub path: String,
    pub status: CandidateStatus,
}

/// A fetched source, ready to pick candidates from. Holds the temporary
/// checkout (if any) until dropped.
pub struct FetchedSource {
    pub input: SourceInput,
    /// Branch and commit for GitHub sources.
    pub revision: Option<(String, String)>,
    pub candidates: Vec<Candidate>,
    root: PathBuf,
    _checkout: Option<tempfile::TempDir>,
}

/// Accepts `https://github.com/owner/repo` (optionally without scheme, with a
/// trailing `/` or `.git`), or a path to an existing `.js` file or directory.
pub fn parse_source(input: &str) -> Result<SourceInput> {
    let input = input.trim();
    if let Some(repo) = ["https://github.com/", "http://github.com/", "github.com/"]
        .iter()
        .find_map(|prefix| input.strip_prefix(prefix))
    {
        let repo = repo.strip_suffix('/').unwrap_or(repo);
        let repo = repo.strip_suffix(".git").unwrap_or(repo);
        let parts: Vec<_> = repo.split('/').collect();
        if parts.len() != 2
            || parts.iter().any(|part| {
                part.is_empty()
                    || *part == "."
                    || *part == ".."
                    || !part
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
            })
        {
            crate::user_bail!("agent_manage.invalid_source");
        }
        return Ok(SourceInput::Github(repo.into()));
    }
    let path = PathBuf::from(input);
    if !(path.is_dir() || path.is_file() && path.extension().is_some_and(|ext| ext == "js")) {
        crate::user_bail!("agent_manage.invalid_source");
    }
    Ok(SourceInput::Local(path.canonicalize()?))
}

pub(super) fn settings_path() -> PathBuf {
    crate::config::app_data_dir().join("agents.json")
}

pub(super) fn read_settings(path: &Path) -> Result<AgentSettings> {
    match fs::read(path) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(AgentSettings::default()),
        Err(error) => Err(error.into()),
    }
}

fn write_settings(path: &Path, settings: &AgentSettings) -> Result<()> {
    let parent = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(&serde_json::to_vec_pretty(settings)?)?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}

/// Serializes read-modify-write of `agents.json`; the desktop app may run
/// operations for different Agents concurrently.
fn write_lock() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|error| error.into_inner())
}

fn github_url(repo: &str) -> String {
    format!("https://github.com/{repo}.git")
}

fn git_text(root: Option<&Path>, args: &[&str]) -> Result<String> {
    let mut command = tool_command("git");
    if let Some(root) = root {
        command.current_dir(root);
    }
    let output = run_output(command.args(args))?;
    Ok(String::from_utf8(output.stdout)?.trim().into())
}

fn checkout(url: &str, branch: Option<&str>) -> Result<tempfile::TempDir> {
    let temp = tempfile::tempdir()?;
    let mut command = tool_command("git");
    command.args(["clone", "--depth", "1"]);
    if let Some(branch) = branch {
        command.args(["--branch", branch]);
    }
    run(command.arg("--").arg(url).arg(temp.path()))?;
    Ok(temp)
}

fn load_candidate(loader: &SpecLoader, path: &Path) -> Result<AgentDef> {
    let source = PluginSource {
        name: path.to_string_lossy().into_owned(),
        code: Cow::Owned(fs::read_to_string(path)?),
    };
    let spec = loader.load(&source)?;
    AgentDef::from_spec(spec, PluginOrigin::User(path.into()), source).map_err(anyhow::Error::msg)
}

/// Clones GitHub sources (shallow) and scans their plugin files.
pub fn fetch(input: SourceInput) -> Result<FetchedSource> {
    fetch_at(input, &plugin_dir(), BUILTIN_PLUGINS, None)
}

fn fetch_at(
    input: SourceInput,
    dir: &Path,
    builtins: &[(&str, &str)],
    url: Option<&str>,
) -> Result<FetchedSource> {
    let (root, revision, temporary) = match &input {
        SourceInput::Github(repo) => {
            let temp = checkout(url.unwrap_or(&github_url(repo)), None)?;
            let branch = git_text(Some(temp.path()), &["rev-parse", "--abbrev-ref", "HEAD"])?;
            let commit = git_text(Some(temp.path()), &["rev-parse", "HEAD"])?;
            (temp.path().to_owned(), Some((branch, commit)), Some(temp))
        }
        SourceInput::Local(path) => (path.canonicalize()?, None, None),
    };
    let mut paths = if root.is_file() {
        vec![root.clone()]
    } else {
        let scan = if root.join("agents").is_dir() {
            root.join("agents")
        } else {
            root.clone()
        };
        fs::read_dir(scan)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()?
            .into_iter()
            .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "js"))
            .collect()
    };
    paths.sort();
    if paths.is_empty() {
        crate::user_bail!("agent_manage.no_candidates");
    }
    let loader = SpecLoader::new();
    let mut seen = BTreeSet::new();
    let candidates = paths
        .into_iter()
        .map(|path| {
            let relative = if root.is_file() {
                path.file_name().unwrap().into()
            } else {
                path.strip_prefix(&root).unwrap().to_path_buf()
            };
            let relative = relative.to_string_lossy().replace('\\', "/");
            match load_candidate(&loader, &path) {
                Ok(agent) => {
                    let id = agent.id.to_string();
                    let status = if !seen.insert(id.clone()) {
                        CandidateStatus::Invalid(t!("agent_manage.duplicate", id = &id).to_string())
                    } else if dir.join(format!("{id}.js")).exists() {
                        CandidateStatus::AlreadyAdded
                    } else if builtins.iter().any(|(key, _)| *key == id) {
                        CandidateStatus::Override
                    } else {
                        CandidateStatus::Ready
                    };
                    Candidate {
                        id,
                        name: agent.name,
                        icon: agent.icon,
                        path: relative,
                        status,
                    }
                }
                Err(error) => {
                    let id = path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned();
                    Candidate {
                        name: id.clone(),
                        id,
                        icon: AgentIcon::Monogram,
                        path: relative,
                        status: CandidateStatus::Invalid(error.to_string()),
                    }
                }
            }
        })
        .collect();
    Ok(FetchedSource {
        input,
        revision,
        candidates,
        root,
        _checkout: temporary,
    })
}

fn candidate_file(source: &FetchedSource, candidate: &Candidate) -> PathBuf {
    if source.root.is_file() {
        source.root.clone()
    } else {
        source.root.join(&candidate.path)
    }
}

/// Copies selected candidates and records their sources.
pub fn add(source: &FetchedSource, ids: &[String]) -> Result<()> {
    let _lock = write_lock();
    add_at(source, ids, &plugin_dir(), &settings_path())?;
    AgentRegistry::reload();
    Ok(())
}

fn add_at(source: &FetchedSource, ids: &[String], dir: &Path, settings_path: &Path) -> Result<()> {
    let mut settings = read_settings(settings_path)?;
    let candidates = ids
        .iter()
        .map(|id| {
            source
                .candidates
                .iter()
                .find(|c| {
                    &c.id == id
                        && matches!(c.status, CandidateStatus::Ready | CandidateStatus::Override)
                })
                .with_context(|| crate::user_error!("agent_manage.not_addable", id = id))
        })
        .collect::<Result<Vec<_>>>()?;
    fs::create_dir_all(dir)?;
    for candidate in candidates {
        let file = candidate_file(source, candidate);
        fs::copy(&file, dir.join(format!("{}.js", candidate.id)))?;
        let record = match &source.input {
            SourceInput::Github(repo) => {
                let (branch, commit) = source.revision.as_ref().unwrap();
                SourceRecord::Github {
                    repo: repo.clone(),
                    path: candidate.path.clone(),
                    branch: branch.clone(),
                    commit: commit.clone(),
                }
            }
            SourceInput::Local(_) => SourceRecord::Local {
                path: file.canonicalize()?,
            },
        };
        settings.sources.insert(candidate.id.clone(), record);
    }
    write_settings(settings_path, &settings)
}

pub fn remove(id: &str) -> Result<()> {
    let _lock = write_lock();
    remove_at(id, &plugin_dir(), &settings_path())?;
    AgentRegistry::reload();
    Ok(())
}

fn remove_at(id: &str, dir: &Path, path: &Path) -> Result<()> {
    let mut settings = read_settings(path)?;
    if settings.sources.remove(id).is_none() {
        crate::user_bail!("agent_manage.not_managed", id = id);
    }
    match fs::remove_file(dir.join(format!("{id}.js"))) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    settings.disabled.remove(id);
    settings.order.retain(|other| other != id);
    write_settings(path, &settings)
}

pub fn refresh(id: &str) -> Result<bool> {
    let _lock = write_lock();
    let changed = refresh_at(id, &plugin_dir(), &settings_path(), None)?;
    AgentRegistry::reload();
    Ok(changed)
}

fn refresh_at(id: &str, dir: &Path, path: &Path, url: Option<&str>) -> Result<bool> {
    let mut settings = read_settings(path)?;
    let record = settings
        .sources
        .get_mut(id)
        .with_context(|| crate::user_error!("agent_manage.not_managed", id = id))?;
    let temporary;
    let mut new_commit = None;
    let file = match record {
        SourceRecord::Local { path } => path.clone(),
        SourceRecord::Github {
            repo, path, branch, ..
        } => {
            temporary = checkout(url.unwrap_or(&github_url(repo)), Some(branch))?;
            new_commit = Some(git_text(Some(temporary.path()), &["rev-parse", "HEAD"])?);
            temporary.path().join(path)
        }
    };
    let agent = load_candidate(&SpecLoader::new(), &file)?;
    if agent.id.as_str() != id {
        crate::user_bail!("agent_manage.id_changed", id = id);
    }
    let bytes = fs::read(&file)?;
    let destination = dir.join(format!("{id}.js"));
    let mut changed = fs::read(&destination).ok().as_ref() != Some(&bytes);
    if let SourceRecord::Github { commit, .. } = record {
        let latest = new_commit.unwrap();
        changed |= *commit != latest;
        *commit = latest;
    }
    fs::create_dir_all(dir)?;
    fs::copy(&file, &destination)?;
    write_settings(path, &settings)?;
    Ok(changed)
}

pub fn check_updates() -> Result<Vec<String>> {
    Ok(check_updates_with_failures()?.0)
}

/// Per-repository errors are exposed for CLI reporting; other callers can skip them.
pub fn check_updates_with_failures() -> Result<UpdateCheck> {
    Ok(check_updates_at(
        &read_settings(&settings_path())?,
        github_url,
    ))
}

type UpdateCheck = (Vec<String>, Vec<(String, String)>);

fn check_updates_at(settings: &AgentSettings, url: impl Fn(&str) -> String) -> UpdateCheck {
    let mut heads = BTreeMap::new();
    let mut failures = Vec::new();
    let mut updates = Vec::new();
    for (id, record) in &settings.sources {
        if let SourceRecord::Github {
            repo,
            branch,
            commit,
            ..
        } = record
        {
            let head =
                heads
                    .entry((repo.clone(), branch.clone()))
                    .or_insert_with(|| {
                        match git_text(
                            None,
                            &["ls-remote", &url(repo), &format!("refs/heads/{branch}")],
                        ) {
                            Ok(output) => output.split_whitespace().next().map(str::to_owned),
                            Err(error) => {
                                failures
                                    .push((repo.clone(), crate::i18n::error_diagnostic(&error)));
                                None
                            }
                        }
                    });
            if head.as_ref().is_some_and(|head| head != commit) {
                updates.push(id.clone());
            }
        }
    }
    (updates, failures)
}

pub fn set_enabled(id: &str, enabled: bool) -> Result<()> {
    let _lock = write_lock();
    set_enabled_at(
        id,
        enabled,
        &plugin_dir(),
        &settings_path(),
        BUILTIN_PLUGINS,
    )?;
    AgentRegistry::reload();
    Ok(())
}

fn set_enabled_at(
    id: &str,
    enabled: bool,
    dir: &Path,
    path: &Path,
    builtins: &[(&str, &str)],
) -> Result<()> {
    let registry = AgentRegistry::load_from_paths(builtins, dir, path);
    require_agent(&registry, id)?;
    let mut settings = read_settings(path)?;
    if enabled {
        settings.disabled.remove(id);
    } else {
        settings.disabled.insert(id.into());
    }
    write_settings(path, &settings)
}

fn require_agent(registry: &AgentRegistry, id: &str) -> Result<()> {
    if !registry
        .all_agents()
        .iter()
        .any(|agent| agent.id.as_str() == id)
    {
        crate::user_bail!("agent_manage.unknown", id = id);
    }
    Ok(())
}

pub fn move_agent(id: &str, target: &str, after: bool) -> Result<()> {
    let _lock = write_lock();
    move_agent_at(
        id,
        target,
        after,
        &plugin_dir(),
        &settings_path(),
        BUILTIN_PLUGINS,
    )?;
    AgentRegistry::reload();
    Ok(())
}

fn move_agent_at(
    id: &str,
    target: &str,
    after: bool,
    dir: &Path,
    path: &Path,
    builtins: &[(&str, &str)],
) -> Result<()> {
    let registry = AgentRegistry::load_from_paths(builtins, dir, path);
    require_agent(&registry, id)?;
    require_agent(&registry, target)?;
    if id == target {
        return Ok(());
    }
    let mut settings = read_settings(path)?;
    settings.order = registry
        .all_agents()
        .iter()
        .map(|agent| agent.id.to_string())
        .filter(|other| other != id)
        .collect();
    let position = settings
        .order
        .iter()
        .position(|other| other == target)
        .unwrap();
    settings
        .order
        .insert(position + usize::from(after), id.into());
    write_settings(path, &settings)
}

pub fn settings() -> AgentSettings {
    read_settings(&settings_path()).unwrap_or_default()
}

#[cfg(test)]
#[path = "manage_tests.rs"]
mod tests;
