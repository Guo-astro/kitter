use super::*;

fn plugin(id: &str, name: &str) -> String {
    format!("export default {{apiVersion:1,id:{id:?},name:{name:?}}}")
}

struct Fixture {
    _temp: tempfile::TempDir,
    dir: PathBuf,
    settings: PathBuf,
    source: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("plugins");
        let settings = temp.path().join("agents.json");
        let source = temp.path().join("source");
        fs::create_dir_all(&source).unwrap();
        Self {
            _temp: temp,
            dir,
            settings,
            source,
        }
    }
    fn fetch(&self, builtins: &[(&str, &str)]) -> FetchedSource {
        fetch_at(
            SourceInput::Local(self.source.clone()),
            &self.dir,
            builtins,
            None,
        )
        .unwrap()
    }
    fn registry(&self, builtins: &[(&str, &str)]) -> AgentRegistry {
        AgentRegistry::load_from_paths(builtins, &self.dir, &self.settings)
    }
}

#[test]
fn parses_github_urls_and_local_paths() {
    for input in [
        "https://github.com/a/b",
        " http://github.com/a/b.git/ ",
        "github.com/A-._1/B-._2/",
    ] {
        assert!(matches!(
            parse_source(input).unwrap(),
            SourceInput::Github(_)
        ));
    }
    assert_eq!(
        parse_source("github.com/a/b.git/").unwrap(),
        SourceInput::Github("a/b".into())
    );
    for input in [
        "github.com/a",
        "github.com/a/b/tree/main",
        "github.com/a/b?x=1",
        "github.com/a//b",
        "github.com/a/b//",
        "github.com/a/b#x",
        "github.com/../b",
        "github.com/a/",
        "",
        "https://other.example/a/b",
        "owner/repo",
    ] {
        assert!(parse_source(input).is_err(), "{input}");
    }
    let fixture = Fixture::new();
    let file = fixture.source.join("test.js");
    fs::write(&file, plugin("test", "Test")).unwrap();
    assert_eq!(
        parse_source(&file.to_string_lossy()).unwrap(),
        SourceInput::Local(file.canonicalize().unwrap())
    );
    assert!(matches!(
        parse_source(&fixture.source.to_string_lossy()).unwrap(),
        SourceInput::Local(_)
    ));
    let other = fixture.source.join("test.txt");
    fs::write(&other, "").unwrap();
    assert!(parse_source(&other.to_string_lossy()).is_err());
}

#[test]
fn scans_root_or_agents_nonrecursively_and_single_files() {
    let fixture = Fixture::new();
    assert!(
        fetch_at(
            SourceInput::Local(fixture.source.clone()),
            &fixture.dir,
            &[],
            None
        )
        .is_err()
    );
    fs::write(fixture.source.join("z.js"), plugin("last", "Last")).unwrap();
    fs::write(fixture.source.join("a.js"), plugin("first", "First")).unwrap();
    fs::write(fixture.source.join("ignored.txt"), "").unwrap();
    let fetched = fixture.fetch(&[]);
    assert_eq!(
        fetched
            .candidates
            .iter()
            .map(|c| c.id.as_str())
            .collect::<Vec<_>>(),
        ["first", "last"]
    );
    let single = fetch_at(
        SourceInput::Local(fixture.source.join("z.js")),
        &fixture.dir,
        &[],
        None,
    )
    .unwrap();
    assert_eq!(single.candidates.len(), 1);
    fs::create_dir_all(fixture.source.join("agents/nested")).unwrap();
    fs::write(
        fixture.source.join("agents/chosen.js"),
        plugin("chosen", "Chosen"),
    )
    .unwrap();
    fs::write(
        fixture.source.join("agents/nested/no.js"),
        plugin("no", "No"),
    )
    .unwrap();
    let fetched = fixture.fetch(&[]);
    assert_eq!(fetched.candidates.len(), 1);
    assert_eq!(fetched.candidates[0].path, "agents/chosen.js");
}

#[test]
fn candidates_cover_all_statuses_duplicate_ids_and_icon_validation() {
    let fixture = Fixture::new();
    let builtin = plugin("builtin", "Builtin");
    fs::create_dir_all(&fixture.dir).unwrap();
    fs::write(
        fixture.dir.join("existing.js"),
        plugin("existing", "Existing"),
    )
    .unwrap();
    for (file, code) in [
        ("a.js", plugin("ready", "Ready")),
        ("b.js", plugin("builtin", "Override")),
        ("c.js", plugin("existing", "Existing")),
        ("d.js", "bad javascript".into()),
        ("e.js", plugin("ready", "Duplicate")),
        (
            "f.js",
            "export default {apiVersion:1,id:'icon',name:'Icon',icon:'builtin:missing'}".into(),
        ),
    ] {
        fs::write(fixture.source.join(file), code).unwrap();
    }
    let fetched = fixture.fetch(&[("builtin", &builtin)]);
    let statuses: Vec<_> = fetched.candidates.iter().map(|c| &c.status).collect();
    assert_eq!(statuses[0], &CandidateStatus::Ready);
    assert_eq!(statuses[1], &CandidateStatus::Override);
    assert_eq!(statuses[2], &CandidateStatus::AlreadyAdded);
    assert!(matches!(statuses[3], CandidateStatus::Invalid(_)));
    assert_eq!(fetched.candidates[3].id, "d");
    assert!(matches!(statuses[4], CandidateStatus::Invalid(_)));
    assert!(matches!(statuses[5], CandidateStatus::Invalid(_)));
}

#[test]
fn add_remove_and_override_restoration() {
    let fixture = Fixture::new();
    let builtin = plugin("builtin", "Builtin");
    let builtins = [("builtin", builtin.as_str())];
    fs::write(
        fixture.source.join("different-filename.js"),
        plugin("builtin", "Override"),
    )
    .unwrap();
    fs::write(fixture.source.join("new.js"), plugin("new", "New")).unwrap();
    let fetched = fixture.fetch(&builtins);
    assert!(
        add_at(
            &fetched,
            &["missing".into()],
            &fixture.dir,
            &fixture.settings
        )
        .is_err()
    );
    assert!(!fixture.dir.exists());
    add_at(
        &fetched,
        &["builtin".into(), "new".into()],
        &fixture.dir,
        &fixture.settings,
    )
    .unwrap();
    assert_eq!(
        fixture.registry(&builtins).agent("builtin").unwrap().name,
        "Override"
    );
    assert!(fixture.registry(&builtins).agent("new").is_some());
    let settings = read_settings(&fixture.settings).unwrap();
    assert!(
        matches!(&settings.sources["builtin"], SourceRecord::Local { path } if path.is_absolute() && path.ends_with("different-filename.js"))
    );
    set_enabled_at("builtin", false, &fixture.dir, &fixture.settings, &builtins).unwrap();
    move_agent_at(
        "builtin",
        "new",
        true,
        &fixture.dir,
        &fixture.settings,
        &builtins,
    )
    .unwrap();
    remove_at("builtin", &fixture.dir, &fixture.settings).unwrap();
    assert_eq!(
        fixture.registry(&builtins).agent("builtin").unwrap().name,
        "Builtin"
    );
    let settings = read_settings(&fixture.settings).unwrap();
    assert!(!settings.sources.contains_key("builtin"));
    assert!(!settings.disabled.contains("builtin"));
    assert!(!settings.order.contains(&"builtin".into()));
    assert!(remove_at("builtin", &fixture.dir, &fixture.settings).is_err());
    fs::write(fixture.dir.join("manual.js"), plugin("manual", "Manual")).unwrap();
    assert!(remove_at("manual", &fixture.dir, &fixture.settings).is_err());
    remove_at("new", &fixture.dir, &fixture.settings).unwrap();
    assert!(fixture.registry(&builtins).agent("new").is_none());
}

#[test]
fn refresh_preserves_old_file_when_new_file_is_invalid_or_missing() {
    let fixture = Fixture::new();
    let file = fixture.source.join("test.js");
    let old = plugin("test", "Old");
    fs::write(&file, &old).unwrap();
    add_at(
        &fixture.fetch(&[]),
        &["test".into()],
        &fixture.dir,
        &fixture.settings,
    )
    .unwrap();
    assert!(!refresh_at("test", &fixture.dir, &fixture.settings, None).unwrap());
    for invalid in [
        "bad javascript".into(),
        plugin("changed", "Changed"),
        "export default {apiVersion:1,id:'test',name:'Test',icon:'builtin:missing'}".into(),
    ] {
        fs::write(&file, invalid).unwrap();
        assert!(refresh_at("test", &fixture.dir, &fixture.settings, None).is_err());
        assert_eq!(
            fs::read_to_string(fixture.dir.join("test.js")).unwrap(),
            old
        );
    }
    fs::remove_file(&file).unwrap();
    assert!(refresh_at("test", &fixture.dir, &fixture.settings, None).is_err());
    fs::write(&file, plugin("test", "New")).unwrap();
    assert!(refresh_at("test", &fixture.dir, &fixture.settings, None).unwrap());
    assert_eq!(fixture.registry(&[]).agent("test").unwrap().name, "New");
}

#[test]
fn settings_apply_order_and_disable() {
    let a = "export default {apiVersion:1,id:'first',name:'First'}";
    let b = "export default {apiVersion:1,id:'second',name:'Second',install:{project:'.b',global:'~/.b'}}";
    let c = plugin("third", "Third");
    let builtins = [("first", a), ("second", b), ("third", c.as_str())];
    let mut registry = AgentRegistry::load(&builtins, &[]);
    let settings = AgentSettings {
        order: vec!["missing".into(), "third".into(), "third".into()],
        disabled: BTreeSet::from(["second".into()]),
        ..Default::default()
    };
    registry.apply_settings(&settings);
    assert!(registry.diagnostics().is_empty());
    assert_eq!(
        registry
            .all_agents()
            .iter()
            .map(|a| a.id.as_str())
            .collect::<Vec<_>>(),
        ["third", "first", "second"]
    );
    assert_eq!(
        registry
            .agents()
            .iter()
            .map(|a| a.id.as_str())
            .collect::<Vec<_>>(),
        ["third", "first"]
    );
    assert!(registry.agent("second").is_none());
    assert!(registry.resolve("second").is_none());
    assert!(!registry.is_enabled("second"));
    assert!(!registry.is_enabled("missing"));
    assert_eq!(registry.install_targets().len(), 1);
}

#[test]
fn invalid_settings_emit_diagnostic_without_overwriting_file() {
    let fixture = Fixture::new();
    let builtin = plugin("test", "Test");
    let builtins = [("test", builtin.as_str())];
    assert!(fixture.registry(&builtins).diagnostics().is_empty());
    fs::write(&fixture.settings, "{broken").unwrap();
    let registry = fixture.registry(&builtins);
    assert_eq!(registry.agents().len(), 1);
    assert_eq!(registry.diagnostics().len(), 1);
    let diagnostic = &registry.diagnostics()[0];
    assert_eq!(
        diagnostic.origin,
        PluginOrigin::User(fixture.settings.clone())
    );
    assert_eq!(diagnostic.id, None);
    assert!(!diagnostic.message.is_empty());
    assert!(set_enabled_at("test", false, &fixture.dir, &fixture.settings, &builtins).is_err());
    assert_eq!(fs::read_to_string(&fixture.settings).unwrap(), "{broken");
}

#[test]
fn enable_and_move_validate_ids_and_write_complete_order() {
    let fixture = Fixture::new();
    let a = plugin("first", "First");
    let b = plugin("second", "Second");
    let c = plugin("third", "Third");
    let builtins = [
        ("first", a.as_str()),
        ("second", b.as_str()),
        ("third", c.as_str()),
    ];
    assert!(set_enabled_at("missing", false, &fixture.dir, &fixture.settings, &builtins).is_err());
    assert!(
        move_agent_at(
            "first",
            "missing",
            false,
            &fixture.dir,
            &fixture.settings,
            &builtins
        )
        .is_err()
    );
    move_agent_at(
        "first",
        "first",
        false,
        &fixture.dir,
        &fixture.settings,
        &builtins,
    )
    .unwrap();
    assert!(!fixture.settings.exists());
    set_enabled_at("first", false, &fixture.dir, &fixture.settings, &builtins).unwrap();
    move_agent_at(
        "first",
        "third",
        true,
        &fixture.dir,
        &fixture.settings,
        &builtins,
    )
    .unwrap();
    assert_eq!(
        read_settings(&fixture.settings).unwrap().order,
        ["second", "third", "first"]
    );
    move_agent_at(
        "first",
        "second",
        false,
        &fixture.dir,
        &fixture.settings,
        &builtins,
    )
    .unwrap();
    assert_eq!(
        read_settings(&fixture.settings).unwrap().order,
        ["first", "second", "third"]
    );
    set_enabled_at("first", true, &fixture.dir, &fixture.settings, &builtins).unwrap();
    assert!(fixture.registry(&builtins).is_enabled("first"));
}

fn git(root: &Path, args: &[&str]) -> String {
    git_text(Some(root), args).unwrap()
}
fn commit(root: &Path) {
    git(root, &["add", "."]);
    git(
        root,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-m",
            "test",
        ],
    );
}

#[test]
fn github_fetch_update_and_refresh_use_local_git_without_network() {
    let fixture = Fixture::new();
    git(&fixture.source, &["init", "-b", "main"]);
    fs::create_dir(fixture.source.join("agents")).unwrap();
    fs::write(
        fixture.source.join("agents/other-name.js"),
        plugin("test", "First"),
    )
    .unwrap();
    commit(&fixture.source);
    let url = format!("file://{}", fixture.source.display());
    let fetched = fetch_at(
        SourceInput::Github("owner/repo".into()),
        &fixture.dir,
        &[],
        Some(&url),
    )
    .unwrap();
    assert_eq!(fetched.revision.as_ref().unwrap().0, "main");
    add_at(&fetched, &["test".into()], &fixture.dir, &fixture.settings).unwrap();
    let checkout_path = fetched.root.clone();
    assert!(checkout_path.is_dir());
    drop(fetched);
    assert!(!checkout_path.exists());
    let settings = read_settings(&fixture.settings).unwrap();
    assert!(
        matches!(&settings.sources["test"], SourceRecord::Github { repo, path, branch, commit: revision }
        if repo == "owner/repo" && path == "agents/other-name.js" && branch == "main" && revision == &git(&fixture.source, &["rev-parse", "HEAD"]))
    );
    assert!(check_updates_at(&settings, |_| url.clone()).0.is_empty());
    fs::write(fixture.source.join("unrelated.txt"), "changed").unwrap();
    commit(&fixture.source);
    let mut multiple = settings.clone();
    multiple
        .sources
        .insert("second".into(), settings.sources["test"].clone());
    multiple.sources.insert(
        "failed".into(),
        SourceRecord::Github {
            repo: "missing/repo".into(),
            path: "test.js".into(),
            branch: "main".into(),
            commit: "old".into(),
        },
    );
    let mut queries = Vec::new();
    // Count calls without global environment changes.
    let count = std::cell::Cell::new(0);
    let (updates, failures) = check_updates_at(&multiple, |repo| {
        count.set(count.get() + 1);
        if repo == "owner/repo" {
            url.clone()
        } else {
            format!("file://{}/missing", fixture.source.display())
        }
    });
    queries.extend(updates);
    assert_eq!(queries, ["second", "test"]);
    assert_eq!(count.get(), 2);
    assert_eq!(failures.len(), 1);
    assert!(refresh_at("test", &fixture.dir, &fixture.settings, Some(&url)).unwrap());
    assert!(!refresh_at("test", &fixture.dir, &fixture.settings, Some(&url)).unwrap());
    fs::write(
        fixture.source.join("agents/other-name.js"),
        plugin("test", "Latest"),
    )
    .unwrap();
    commit(&fixture.source);
    assert!(refresh_at("test", &fixture.dir, &fixture.settings, Some(&url)).unwrap());
    assert_eq!(fixture.registry(&[]).agent("test").unwrap().name, "Latest");
    let saved = fs::read(&fixture.settings).unwrap();
    fs::write(fixture.source.join("agents/other-name.js"), "invalid").unwrap();
    commit(&fixture.source);
    assert!(refresh_at("test", &fixture.dir, &fixture.settings, Some(&url)).is_err());
    assert_eq!(fixture.registry(&[]).agent("test").unwrap().name, "Latest");
    assert_eq!(fs::read(&fixture.settings).unwrap(), saved);
    fs::remove_file(fixture.source.join("agents/other-name.js")).unwrap();
    commit(&fixture.source);
    assert!(refresh_at("test", &fixture.dir, &fixture.settings, Some(&url)).is_err());
    assert_eq!(fs::read(&fixture.settings).unwrap(), saved);
}

#[test]
fn reload_increments_generation_and_keeps_old_snapshots_valid() {
    let fixture = Fixture::new();
    let a = plugin("test", "Original");
    let builtins = [("test", a.as_str())];
    let original: &'static AgentRegistry = Box::leak(Box::new(fixture.registry(&builtins)));
    let current = std::sync::RwLock::new(original);
    assert_eq!(original.generation(), 0);
    set_enabled_at("test", false, &fixture.dir, &fixture.settings, &builtins).unwrap();
    let updated =
        AgentRegistry::reload_from_paths(&current, &builtins, &fixture.dir, &fixture.settings);
    assert_eq!(updated.generation(), 1);
    assert!(!updated.is_enabled("test"));
    assert!(original.is_enabled("test"));
    assert_eq!(original.agent("test").unwrap().name, "Original");
    assert!(std::ptr::eq(*current.read().unwrap(), updated));
    let next =
        AgentRegistry::reload_from_paths(&current, &builtins, &fixture.dir, &fixture.settings);
    assert_eq!(next.generation(), 2);
    assert_eq!(updated.generation(), 1);
}
