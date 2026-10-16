use super::*;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_skill(root: &Path, name: &str, extra: &str) {
        fs::create_dir_all(root.join(name)).unwrap();
        fs::write(
            root.join(name).join("SKILL.md"),
            format!(
                "---\nname: {name}\ndescription: Use {name}\n{extra}---\nSkill instructions.\n"
            ),
        )
        .unwrap();
    }
    fn context(home: PathBuf, cwd: PathBuf, repo: PathBuf) -> DiscoveryContext {
        DiscoveryContext {
            home,
            cwd,
            repository_root: Some(repo),
        }
    }
    fn estimate(context: &DiscoveryContext, agent: AgentId) -> AgentContextEstimate {
        if agent.as_str() == "dimagent" {
            return adapters::inspect_dimagent(context, context.home.join(".dimcode/v2"));
        }
        adapters::inspect_agent(&agent, context).unwrap()
    }

    #[test]
    fn omp_project_install_round_trip_and_direct_discovery() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("home");
        let repo = temp.path().join("repo");
        let cwd = repo.join("child");
        fs::create_dir_all(&cwd).unwrap();
        let library = temp.path().join("library");
        write_skill(&library, "installed", "");
        crate::project::install(
            &repo,
            &library,
            "installed",
            &[crate::InstallTarget::from("omp")],
        )
        .unwrap();
        write_skill(&repo.join(".omp/skills/group"), "nested", "");
        write_skill(&repo.join(".omp/skills"), ".hidden", "");
        let context = context(home, cwd, repo.clone());
        let result = estimate(&context, AgentId::from("omp"));
        assert_eq!(
            result
                .skills
                .iter()
                .filter(|s| s.name == "installed")
                .count(),
            1
        );
        assert!(
            !result
                .skills
                .iter()
                .any(|s| s.name == "nested" || s.name == ".hidden")
        );
        assert!(
            crate::project::list(&repo, &library)
                .unwrap()
                .iter()
                .find(|s| s.name == "installed")
                .unwrap()
                .installations[0]
                .managed
        );
        crate::project::uninstall(
            &repo,
            &library,
            "installed",
            &[crate::InstallTarget::from("omp")],
        )
        .unwrap();
        assert!(library.join("installed/SKILL.md").is_file());
        assert!(!repo.join(".omp/skills/installed").exists());
    }

    #[test]
    fn omp_settings_filter_hidden_disabled_and_custom_skills() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("home");
        let repo = temp.path().join("repo");
        write_skill(&repo.join(".omp/skills"), "hidden", "hide: true\n");
        write_skill(&repo.join(".omp/skills"), "disabled", "enabled: false\n");
        write_skill(&repo.join(".agents/skills"), "shared", "");
        write_skill(&repo.join("custom"), "custom", "");
        fs::write(
            repo.join(".omp/config.yml"),
            "skills:\n  customDirectories: [custom]\n  ignoredSkills: [shared]\n",
        )
        .unwrap();
        let result = estimate(&context(home, repo.clone(), repo), AgentId::from("omp"));
        assert!(result.skills.iter().any(|s| s.name == "custom"));
        assert!(
            result
                .skills
                .iter()
                .any(|s| s.name == "hidden" && s.visibility == SkillVisibility::ManualOnly)
        );
        assert!(
            !result
                .skills
                .iter()
                .any(|s| s.name == "disabled" || s.name == "shared")
        );
    }

    #[test]
    fn omp_keeps_distinct_collisions_and_deduplicates_linked_sources() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("home");
        let repo = temp.path().join("repo");
        write_skill(&repo.join(".omp/skills"), "same", "");
        write_skill(&repo.join(".agents/skills"), "same", "hide: true\n");
        write_skill(&repo.join(".omp/skills"), "linked", "");
        crate::directory_link::create(
            &repo.join(".omp/skills/linked"),
            &repo.join(".agents/skills/linked"),
        )
        .unwrap();
        let result = estimate(&context(home, repo.clone(), repo), AgentId::from("omp"));
        assert!(result.skills.iter().any(|s| s.name == "same"));
        assert!(result.skills.iter().any(|s| s.name == "agents/same"));
        assert_eq!(
            result.skills.iter().filter(|s| s.name == "linked").count(),
            1
        );
    }

    #[test]
    fn dimagent_uses_shared_skills_user_first_and_only_current_workspace() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("home");
        let repo = temp.path().join("repo");
        let cwd = repo.join("child");
        write_skill(&home.join(".agents/skills"), "same", "");
        write_skill(&cwd.join(".agents/skills"), "same", "");
        write_skill(
            &cwd.join(".agents/skills/group"),
            "nested",
            "disable-model-invocation: true\n",
        );
        write_skill(&repo.join(".agents/skills"), "parent", "");
        let result = estimate(
            &context(home.clone(), cwd.clone(), repo),
            AgentId::from("dimagent"),
        );
        assert_eq!(result.skills.iter().filter(|s| s.name == "same").count(), 1);
        assert_eq!(
            result
                .skills
                .iter()
                .find(|s| s.name == "same")
                .unwrap()
                .scope,
            SkillScope::User
        );
        assert!(
            result
                .skills
                .iter()
                .any(|s| s.name == "nested" && s.visibility == SkillVisibility::Automatic)
        );
        assert!(!result.skills.iter().any(|s| s.name == "parent"));
    }
    #[test]
    fn dimagent_disabled_user_skill_still_shadows_the_workspace() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("home");
        let repo = temp.path().join("repo");
        write_skill(&home.join(".agents/skills"), "same", "");
        write_skill(&repo.join(".agents/skills"), "same", "");
        let context = context(home, repo.clone(), repo);
        let state = context.home.join(".dimcode/v2");
        fs::create_dir_all(&state).unwrap();
        fs::write(
            state.join("skills.json"),
            r#"{"disabledSkillIds":["user:same"]}"#,
        )
        .unwrap();
        assert!(
            !estimate(&context, AgentId::from("dimagent"))
                .skills
                .iter()
                .any(|s| s.name == "same")
        );
    }

    #[test]
    fn missing_descriptions_follow_each_agents_validation_rules() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("home");
        let repo = temp.path().join("repo");
        for root in [".omp/skills", ".agents/skills"] {
            let dir = repo.join(root).join("no-description");
            fs::create_dir_all(&dir).unwrap();
            fs::write(
                dir.join("SKILL.md"),
                "---\nname: no-description\n---\nBody.\n",
            )
            .unwrap();
        }
        let context = context(home, repo.clone(), repo);
        let omp = estimate(&context, AgentId::from("omp"));
        let skill = omp
            .skills
            .iter()
            .find(|s| s.name == "no-description")
            .unwrap();
        assert!(skill.path.to_string_lossy().contains(".agents/skills"));
        assert!(skill.description.is_empty());
        assert!(
            estimate(&context, AgentId::from("dimagent"))
                .skills
                .is_empty()
        );
    }
}
