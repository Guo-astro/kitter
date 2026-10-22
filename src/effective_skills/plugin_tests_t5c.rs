use super::*;
use tempfile::tempdir;

fn write(path: &Path, text: impl AsRef<str>) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text.as_ref()).unwrap();
}

fn skill(root: &Path, name: &str, extra: &str) {
    write(
        &root.join(name).join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Use {name}\n{extra}---\nBody.\n"),
    );
}

#[test]
fn pi_jsonc_custom_directories_scoped_package_and_conditional_visibility() {
    let temp = tempdir().unwrap();
    let home = temp.path().join("home");
    let cwd = temp.path().join("repo");
    skill(&cwd.join("extra"), "configured", "paths: [src/**]\n");
    skill(
        &cwd.join(".pi/npm/node_modules/@team/tools/skills"),
        "packaged",
        "",
    );
    write(
        &cwd.join(".pi/settings.json"),
        "// Config\n{\"skills\":{\"customDirectories\":[\"../extra\",\"!ignored\"]},\"packages\":[{\"source\":\"npm:@team/tools@1\"}]}\n",
    );
    let context = DiscoveryContext {
        home,
        cwd: cwd.clone(),
        repository_root: Some(cwd),
    };
    let result = adapters::inspect_agent(&AgentId::from("pi"), &context).unwrap();
    assert_eq!(
        result
            .skills
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        ["configured", "packaged"]
    );
    assert!(
        result
            .skills
            .iter()
            .all(|s| s.visibility == SkillVisibility::Automatic)
    );
}

#[test]
fn omp_merges_yaml_and_json_and_applies_include_after_namespacing() {
    let temp = tempdir().unwrap();
    let home = temp.path().join("home");
    let cwd = temp.path().join("repo");
    skill(&cwd.join(".omp/skills"), "same", "");
    skill(&cwd.join(".agents/skills"), "same", "hide: true\n");
    write(
        &cwd.join(".omp/config.yaml"),
        "skills:\n  includeSkills: ['agents/*']\n  enableAgentsUser: false\n",
    );
    write(
        &cwd.join(".omp/settings.json"),
        r#"{"skills":{"enableClaudeProject":false}}"#,
    );
    let context = DiscoveryContext {
        home,
        cwd: cwd.clone(),
        repository_root: Some(cwd),
    };
    let result = adapters::inspect_agent(&AgentId::from("omp"), &context).unwrap();
    assert_eq!(result.skills.len(), 1);
    assert_eq!(result.skills[0].name, "agents/same");
    assert_eq!(result.skills[0].visibility, SkillVisibility::ManualOnly);
    assert_eq!(
        result.skills[0].prompt_path.as_deref(),
        Some("skill://agents/same")
    );
}

#[test]
fn grok_configured_exact_markdown_normalizes_and_disables_names() {
    let temp = tempdir().unwrap();
    let home = temp.path().join("home");
    let cwd = temp.path().join("repo");
    fs::create_dir_all(&cwd).unwrap();
    write(
        &home.join(".grok/config.toml"),
        "[skills]\npaths = [\"extra.md\"]\ndisabled = [\"Hello World\"]\n",
    );
    write(
        &home.join(".grok/extra.md"),
        "---\nname: Hello World\ndescription: 中文 🦀\n---\nBody\n",
    );
    let context = DiscoveryContext {
        home,
        cwd: cwd.clone(),
        repository_root: Some(cwd),
    };
    let result = adapters::inspect_agent(&AgentId::from("grok"), &context).unwrap();
    assert_eq!(result.skills.len(), 1);
    assert_eq!(result.skills[0].id, "hello-world");
    assert_eq!(result.skills[0].visibility, SkillVisibility::ManualOnly);
}

#[test]
fn dimagent_requires_body_and_keeps_unicode_descriptions_within_character_budget() {
    let temp = tempdir().unwrap();
    let home = temp.path().join("home");
    let cwd = temp.path().join("repo");
    skill(&cwd.join(".agents/skills"), "valid", "");
    write(
        &cwd.join(".agents/skills/empty/SKILL.md"),
        "---\nname: empty\ndescription: Empty\n---\n\n",
    );
    let context = DiscoveryContext {
        home,
        cwd: cwd.clone(),
        repository_root: Some(cwd),
    };
    let result = adapters::inspect_dimagent(&context, context.home.join(".dimcode/v2"));
    assert_eq!(result.skills.len(), 1);
    let mut entries = vec![result.skills[0].clone(); 2];
    entries[0].name = "a".into();
    entries[0].description = "🦀".repeat(3000);
    entries[1].name = "b".into();
    entries[1].description = "中".repeat(2000);
    let text = adapters::render_catalog(&AgentId::from("dimagent"), &entries);
    assert_eq!(text, format!("- a\n- b: {}", "中".repeat(2000)));
}

fn context(home: &Path, repo: &Path, cwd: &Path) -> DiscoveryContext {
    fs::create_dir_all(repo.join(".git")).unwrap();
    fs::create_dir_all(cwd).unwrap();
    DiscoveryContext {
        home: home.into(),
        cwd: cwd.into(),
        repository_root: Some(repo.into()),
    }
}

fn inspect(agent: &str, context: &DiscoveryContext) -> AgentContextEstimate {
    adapters::inspect_agent(&AgentId::from(agent), context).unwrap()
}

#[test]
fn agy_aliases_plugins_globals_and_ancestor_boundary_share_one_priority_order() {
    let temp = tempdir().unwrap();
    let home = temp.path().join("home");
    let repo = temp.path().join("repo");
    let cwd = repo.join("nested");
    let ctx = context(&home, &repo, &cwd);
    for (suffix, name) in [
        (".agents/skills", "plural"),
        (".agent/skills", "singular"),
        ("_agents/skills", "underscore-plural"),
        ("_agent/skills", "underscore-singular"),
    ] {
        skill(&repo.join(suffix), name, "");
    }
    skill(&cwd.join(".agents/skills"), "same", "");
    skill(&home.join(".gemini/config/skills"), "same", "");
    skill(&home.join(".gemini/antigravity-cli/skills"), "cli", "");
    skill(&home.join(".gemini/skills"), "shared", "");
    skill(&home.join(".gemini/antigravity/skills"), "ide", "");
    skill(
        &home.join(".gemini/antigravity-cli/builtin/skills"),
        "bundled",
        "",
    );
    skill(
        &repo.join(".agents/plugins/tools/skills"),
        "project-plugin",
        "",
    );
    skill(
        &home.join(".gemini/config/plugins/tools/skills"),
        "global-plugin",
        "",
    );
    for root in [
        home.join(".agents/skills"),
        home.join(".agent/skills"),
        temp.path().join(".agents/skills"),
        cwd.join("descendant/.agents/skills"),
    ] {
        skill(&root, "excluded", "");
    }
    let result = inspect("antigravity", &ctx);
    assert_eq!(result.skills.len(), 11);
    assert!(!result.skills.iter().any(|s| s.name == "excluded"));
    assert_eq!(
        result
            .skills
            .iter()
            .find(|s| s.name == "same")
            .unwrap()
            .path,
        cwd.join(".agents/skills/same/SKILL.md")
    );
    assert_eq!(result.plugin_skills().count(), 2);
    let global = inspect(
        "antigravity",
        &DiscoveryContext {
            cwd: home.clone(),
            home,
            repository_root: None,
        },
    );
    assert_eq!(global.skills.len(), 6);
}

#[test]
fn selected_openclaw_workspace_and_trusted_hermes_repository_are_project_sources() {
    let temp = tempdir().unwrap();
    let home = temp.path().join("home");
    let repo = temp.path().join("custom-workspace");
    let ctx = context(&home, &repo, &repo);
    skill(&repo.join("skills"), "workspace", "");
    skill(&repo.join(".hermes/skills"), "trusted", "");
    skill(&repo.join(".agents/skills"), "shared", "");
    let hermes = AgentRegistry::global().agent("hermes").unwrap();
    assert_eq!(inspect("openclaw", &ctx).skills.len(), 2);
    assert!(inspect("hermes", &ctx).skills.is_empty());
    assert_eq!(hermes.project_root(&repo, &home), None);
    assert_eq!(hermes.global_dir(&home), Some(home.join(".hermes/skills")));
    write(
        &home.join(".hermes/config.yaml"),
        format!(
            "skills:\n  trusted_project_dirs: [{}]\n",
            serde_json::to_string(&repo).unwrap()
        ),
    );
    assert_eq!(inspect("hermes", &ctx).skills.len(), 2);
    assert_eq!(
        hermes.project_root(&repo, &home),
        Some(repo.join(".hermes/skills"))
    );
    write(
        &home.join(".hermes/config.yaml"),
        format!(
            "skills:\n  project_discovery: false\n  trusted_project_dirs: [{}]\n",
            serde_json::to_string(&repo).unwrap()
        ),
    );
    assert!(inspect("hermes", &ctx).skills.is_empty());
    assert_eq!(hermes.project_root(&repo, &home), None);
}

#[test]
fn droid_disabled_settings_and_frontmatter_remove_skills_from_counts() {
    let temp = tempdir().unwrap();
    let home = temp.path().join("home");
    let repo = temp.path().join("repo");
    let ctx = context(&home, &repo, &repo.join("nested"));
    for name in ["user-off", "project-off", "kept"] {
        skill(&repo.join(".factory/skills"), name, "");
    }
    skill(
        &repo.join(".factory/skills"),
        "metadata-off",
        "enabled: false\n",
    );
    write(
        &home.join(".factory/settings.json"),
        r#"{"disabledSkills":["user-off"]}"#,
    );
    write(
        &repo.join(".factory/settings.json"),
        r#"{"disabledSkills":["project-off"]}"#,
    );
    let result = inspect("droid", &ctx);
    assert_eq!(result.discovered_count, 1);
    assert_eq!(result.skills[0].name, "kept");
}

#[test]
fn amp_disabled_compatibility_roots_and_depth_limit_apply_to_discovery() {
    let temp = tempdir().unwrap();
    let home = temp.path().join("home");
    let repo = temp.path().join("repo");
    let ctx = context(&home, &repo, &repo);
    skill(&home.join(".agents/skills"), "global", "");
    skill(&repo.join(".agents/skills"), "local", "");
    skill(&repo.join(".claude/skills"), "claude", "");
    skill(
        &home.join(".claude/plugins/cache/market/plugin/1/skills"),
        "cached",
        "",
    );
    skill(&repo.join(".agents/skills/a/b/c/d"), "at-limit", "");
    skill(&repo.join(".agents/skills/a/b/c/d/e"), "too-deep", "");
    assert_eq!(inspect("amp", &ctx).skills.len(), 5);
    write(
        &home.join(".config/amp/settings.json"),
        r#"{"amp.skills.disableClaudeCodeSkills":true,"amp.skills.disableGlobalAgentsSkills":true}"#,
    );
    let result = inspect("amp", &ctx);
    assert_eq!(result.skills.len(), 2);
    assert!(
        result
            .skills
            .iter()
            .all(|s| matches!(s.name.as_str(), "local" | "at-limit"))
    );
}

#[test]
fn omp_installed_plugins_respect_lockfile_and_project_overrides() {
    let temp = tempdir().unwrap();
    let home = temp.path().join("home");
    let repo = temp.path().join("repo");
    let ctx = context(&home, &repo, &repo);
    let base = home.join(".omp/plugins");
    write(
        &base.join("package.json"),
        r#"{"dependencies":{"good":"1","off":"1","overridden":"1"}}"#,
    );
    write(
        &base.join("omp-plugins.lock.json"),
        r#"{"plugins":{"good":{"enabled":true},"off":{"enabled":false},"stale":{"enabled":true},"overridden":{"enabled":true}}}"#,
    );
    for name in ["good", "off", "stale", "overridden"] {
        let dir = base.join("node_modules").join(name);
        write(&dir.join("package.json"), r#"{"omp":{},"version":"1.0.0"}"#);
        skill(&dir.join("skills"), name, "");
    }
    write(
        &repo.join(".omp/plugin-overrides.json"),
        r#"{"disabled":["overridden"]}"#,
    );
    let result = inspect("omp", &ctx);
    assert_eq!(result.skills.len(), 1);
    assert_eq!(result.skills[0].name, "good");
    assert!(result.skills[0].is_plugin());
}

#[test]
fn grok_bundled_and_enabled_plugins_keep_qualified_collisions_and_manifest_paths() {
    let temp = tempdir().unwrap();
    let home = temp.path().join("home");
    let repo = temp.path().join("repo");
    let ctx = context(&home, &repo, &repo);
    skill(&home.join(".grok/bundled/skills"), "bundled", "");
    skill(&repo.join(".grok/skills"), "same", "");
    for name in ["on", "off", "default-off"] {
        let dir = home.join(".grok/plugins").join(name);
        write(
            &dir.join("plugin.json"),
            format!(r#"{{"name":"{name}","skills":["custom"],"commands":"commands"}}"#),
        );
        skill(&dir.join("custom"), "same", "");
    }
    write(
        &home.join(".grok/config.toml"),
        "[plugins]\nenabled = [\"on\", \"off\"]\ndisabled = [\"off\"]\n",
    );
    let result = inspect("grok", &ctx);
    assert_eq!(result.skills.len(), 3);
    assert!(
        result
            .skills
            .iter()
            .any(|s| s.name == "on:same" && s.is_plugin())
    );
    assert!(
        result
            .skills
            .iter()
            .any(|s| s.name == "same" && !s.is_plugin())
    );
}

#[test]
fn dimagent_manifest_budget_is_shared_across_user_roots_and_separate_from_workspace() {
    let temp = tempdir().unwrap();
    let home = temp.path().join("home");
    let repo = temp.path().join("repo");
    let ctx = context(&home, &repo, &repo);
    let dim = home.join(".dimcode/v2");
    for index in 0..1000 {
        write(
            &dim.join(format!("plugins/test/skills/{index:04}/SKILL.md")),
            "invalid",
        );
    }
    skill(&dim.join("skills"), "over-budget", "");
    skill(&repo.join(".agents/skills"), "workspace", "");
    let result = adapters::inspect_dimagent(&ctx, dim);
    assert_eq!(result.skills.len(), 1);
    assert_eq!(result.skills[0].name, "workspace");
}

#[test]
fn trae_cn_reads_current_project_agents_skills_but_not_global_agents_skills() {
    let temp = tempdir().unwrap();
    let home = temp.path().join("home");
    let repo = temp.path().join("repo");
    let cwd = repo.join("nested");
    let ctx = context(&home, &repo, &cwd);
    skill(&cwd.join(".trae/skills"), "local", "");
    skill(&cwd.join(".agents/skills"), "shared", "");
    skill(&home.join(".trae-cn/skills"), "global", "");
    for root in [
        repo.join(".trae/skills"),
        repo.join(".agents/skills"),
        home.join(".agents/skills"),
        home.join(".trae/skills"),
    ] {
        skill(&root, "excluded", "");
    }
    let result = inspect("trae", &ctx);
    assert_eq!(result.discovered_count, 3);
    assert_eq!(result.model_visible_count, 3);
    assert!(result.skills.iter().any(|skill| skill.name == "shared"));
    assert!(!result.skills.iter().any(|skill| skill.name == "excluded"));
    let readers = agents_reading_root_with_home(&cwd, &home, &cwd.join(".agents/skills"));
    assert!(readers.iter().any(|agent| agent.as_str() == "trae"));
    let global = inspect(
        "trae",
        &DiscoveryContext {
            home: home.clone(),
            cwd: home,
            repository_root: None,
        },
    );
    assert_eq!(global.skills.len(), 1);
    assert_eq!(global.skills[0].name, "global");
}

#[cfg(unix)]
#[test]
fn home_alias_is_global_for_discovery_and_destination_readers() {
    let temp = tempdir().unwrap();
    let home = temp.path().join("home");
    let alias = temp.path().join("home-alias");
    fs::create_dir_all(&home).unwrap();
    std::os::unix::fs::symlink(&home, &alias).unwrap();
    skill(&home.join(".agents/skills"), "excluded", "");
    skill(&home.join(".gemini/config/skills"), "included", "");
    let result = estimate_project_with_home(&alias, &home)
        .into_iter()
        .find(|estimate| estimate.agent.as_str() == "antigravity")
        .unwrap();
    assert_eq!(result.skills.len(), 1);
    assert_eq!(result.skills[0].name, "included");
    let readers = agents_reading_root_with_home(&alias, &home, &home.join(".agents/skills"));
    assert!(!readers.iter().any(|agent| agent.as_str() == "antigravity"));
}
