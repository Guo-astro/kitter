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
