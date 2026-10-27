use std::{collections::HashSet, fs, path::Path, process::Command};

use kitter::{LocalInstallMode, SkillLibrary, SkillOrigin, TriggerMode, source};

fn write_skill(path: &Path) {
    fs::create_dir_all(path.join(".hidden/empty")).unwrap();
    fs::create_dir_all(path.join(".git")).unwrap();
    fs::write(
        path.join("SKILL.md"),
        "---\nname: demo\ndescription: Example\n---\nHello",
    )
    .unwrap();
    fs::write(path.join(".hidden/data.bin"), [0, 255, 1]).unwrap();
    fs::write(path.join(".git/config"), "local metadata").unwrap();
}

fn import(
    library: &mut SkillLibrary,
    source_path: &Path,
    mode: LocalInstallMode,
) -> source::ImportSummary {
    source::scan_local(source_path)
        .unwrap()
        .import_selected_with_mode(library, &HashSet::from(["demo".to_string()]), None, mode)
        .unwrap()
}

fn saved_hash(library: &SkillLibrary) -> String {
    let record = library.record("demo").unwrap();
    let SkillOrigin::Local {
        install_mode: LocalInstallMode::Copy,
        source_hash: Some(ref hash),
        ..
    } = record.origin
    else {
        panic!("expected copied local origin");
    };
    assert!(source::can_update(library, &record));
    hash.clone()
}

#[test]
fn local_copy_checks_source_snapshot_and_updates_without_changing_mode() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("source");
    write_skill(&root);
    let data = temp.path().join("data");
    let mut library = SkillLibrary::open_in(&data).unwrap();
    assert_eq!(import(&mut library, &root, LocalInstallMode::Copy).added, 1);
    let record = library.record("demo").unwrap();
    let storage = record.storage_name.clone();
    let installed = library.skill_path("demo").unwrap();
    let project = temp.path().join("project");
    fs::create_dir(&project).unwrap();
    kitter::project::install_from_path(
        &project,
        &installed,
        "demo",
        &[kitter::InstallTarget::universal()],
    )
    .unwrap();
    let project_skill = project.join(".agents/skills/demo");
    assert!(!library.is_linked_source(&storage));
    assert_ne!(
        installed.canonicalize().unwrap(),
        root.canonicalize().unwrap()
    );
    assert!(installed.join(".hidden/empty").is_dir());
    assert_eq!(
        fs::read(installed.join(".hidden/data.bin")).unwrap(),
        [0, 255, 1]
    );
    assert!(installed.join(".git/config").is_file());
    assert!(
        matches!(record.origin, SkillOrigin::Local { path, source_root: Some(source_root), .. }
        if path == root.canonicalize().unwrap() && source_root == path)
    );
    let original_hash = saved_hash(&library);
    assert_eq!(original_hash.len(), 64);
    assert_eq!(source::check_updates(&mut library).unwrap(), 0);

    // Display/trigger edits in the library must not be mistaken for source updates.
    library
        .set_trigger_mode_by_storage(&storage, TriggerMode::Manual)
        .unwrap();
    assert_eq!(source::check_updates(&mut library).unwrap(), 0);
    assert_eq!(
        import(&mut library, &root, LocalInstallMode::Link).skipped,
        1
    );
    assert!(!library.is_linked_source(&storage));
    fs::write(root.join(".hidden/data.bin"), [0, 255, 2]).unwrap();
    fs::remove_dir(root.join(".hidden/empty")).unwrap();
    fs::write(root.join("new.txt"), "new").unwrap();
    assert_eq!(source::check_updates(&mut library).unwrap(), 1);
    assert!(library.record("demo").unwrap().update_available);
    assert_eq!(
        fs::read(installed.join(".hidden/data.bin")).unwrap(),
        [0, 255, 1]
    );

    // Exercise the same batch API used by the desktop.
    let report = source::update_skills_with_progress(&data, &[storage.clone()], |_| {}).unwrap();
    assert_eq!(report.updated, 1);
    assert!(report.failures.is_empty());
    library = SkillLibrary::open_in(&data).unwrap();
    assert_ne!(saved_hash(&library), original_hash);
    assert!(!library.is_linked_source(&storage));
    assert!(!library.record("demo").unwrap().update_available);
    assert_eq!(
        library.trigger_mode_by_storage(&storage),
        TriggerMode::Manual
    );
    assert_eq!(
        fs::read(installed.join(".hidden/data.bin")).unwrap(),
        [0, 255, 2]
    );
    assert_eq!(
        fs::read(project_skill.join(".hidden/data.bin")).unwrap(),
        [0, 255, 2]
    );
    assert!(!installed.join(".hidden/empty").exists());
    assert_eq!(source::check_updates(&mut library).unwrap(), 0);

    // A missing source clears a stale badge, and direct update keeps the copy usable.
    fs::write(root.join("new.txt"), "changed").unwrap();
    assert_eq!(source::check_updates(&mut library).unwrap(), 1);
    fs::remove_dir_all(&root).unwrap();
    assert_eq!(source::check_updates(&mut library).unwrap(), 0);
    assert!(!library.record("demo").unwrap().update_available);
    source::update_by_storage(&mut library, &storage).unwrap();
    assert!(installed.join("SKILL.md").is_file());
    assert_eq!(
        fs::read_to_string(installed.join("new.txt")).unwrap(),
        "new"
    );
    assert!(project_skill.join("SKILL.md").is_file());
}

#[test]
fn old_local_registry_defaults_to_link_mode() {
    for json in [
        r#"{"type":"local","path":"/old/skill"}"#,
        r#"{"type":"local","path":"/old/skill","source_root":"/old"}"#,
    ] {
        let origin: SkillOrigin = serde_json::from_str(json).unwrap();
        assert!(matches!(
            origin,
            SkillOrigin::Local {
                install_mode: LocalInstallMode::Link,
                source_hash: None,
                ..
            }
        ));
        assert_eq!(
            serde_json::from_value::<SkillOrigin>(serde_json::to_value(&origin).unwrap()).unwrap(),
            origin
        );
    }
}

#[test]
fn cli_add_copy_and_existing_update_commands_work_together() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("source");
    write_skill(&root);
    let data = temp.path().join("data");
    let run = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_kitter"))
            .env("KITTER_HOME", &data)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        output
    };
    run(&[
        "add",
        "local",
        root.to_str().unwrap(),
        "--copy",
        "--skill",
        "demo",
        "--group",
        "Local",
    ]);
    let library = SkillLibrary::open_in(&data).unwrap();
    let installed = library.skill_path("demo").unwrap();
    let before = saved_hash(&library);
    fs::write(root.join("new.txt"), "updated via CLI").unwrap();
    run(&["check", "--json"]);
    assert!(
        SkillLibrary::open_in(&data)
            .unwrap()
            .record("demo")
            .unwrap()
            .update_available
    );
    run(&["update", "demo"]);
    let library = SkillLibrary::open_in(&data).unwrap();
    assert_ne!(saved_hash(&library), before);
    assert_eq!(
        fs::read_to_string(installed.join("new.txt")).unwrap(),
        "updated via CLI"
    );
    let output = run(&["show", "demo", "--json"]);
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["origin"]["install_mode"], "copy");
}

#[cfg(unix)]
#[test]
fn copy_materializes_file_and_directory_links() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("source");
    write_skill(&root);
    let external = temp.path().join("external");
    fs::create_dir(&external).unwrap();
    fs::write(external.join("data"), "external content").unwrap();
    symlink(external.join("data"), root.join("file-link")).unwrap();
    symlink(&external, root.join("directory-link")).unwrap();
    let mut library = SkillLibrary::open_in(temp.path().join("data")).unwrap();
    import(&mut library, &root, LocalInstallMode::Copy);
    let installed = library.skill_path("demo").unwrap();
    assert!(
        !installed
            .join("file-link")
            .symlink_metadata()
            .unwrap()
            .is_symlink()
    );
    assert!(
        !installed
            .join("directory-link")
            .symlink_metadata()
            .unwrap()
            .is_symlink()
    );
    assert_eq!(source::check_updates(&mut library).unwrap(), 0);
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(external).unwrap();
    assert_eq!(
        fs::read_to_string(installed.join("directory-link/data")).unwrap(),
        "external content"
    );
}

#[cfg(unix)]
#[test]
fn failed_local_copy_update_preserves_the_installed_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("source");
    write_skill(&root);
    let mut library = SkillLibrary::open_in(temp.path().join("data")).unwrap();
    import(&mut library, &root, LocalInstallMode::Copy);
    let before = saved_hash(&library);
    let installed = library.skill_path("demo").unwrap();
    std::os::unix::fs::symlink(&root, root.join("cycle")).unwrap();
    assert!(source::update(&mut library, "demo").is_err());
    assert_eq!(saved_hash(&library), before);
    assert!(installed.join("SKILL.md").is_file());
    assert!(!installed.join("cycle").exists());
}
