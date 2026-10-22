//! Check process boundaries as well as the comparison result: one refs query
//! per source, no npx invocation or clone, and no tree transfer on a warm check.
#![cfg(unix)]
use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command};

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn single_update_readds_only_the_selected_skill_and_preserves_it_on_failure() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let data = temp.path().join("data");
    let bin = temp.path().join("bin");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir(&bin).unwrap();
    fs::write(source.join("SKILL.md"), "---\nname: alpha\n---\nold").unwrap();
    let mut library = kitter::SkillLibrary::open_in(&data).unwrap();
    for name in ["alpha", "beta"] {
        library
            .import(
                &source,
                kitter::SkillRecord {
                    name: name.into(),
                    storage_name: name.into(),
                    description: String::new(),
                    origin: kitter::SkillOrigin::Npx {
                        repository: "owner/repo".into(),
                        skill: name.into(),
                        source_hash: None,
                    },
                    update_available: true,
                    group_id: None,
                    last_operated_at: 0,
                },
            )
            .unwrap();
    }
    let stub = bin.join("npx");
    fs::write(
        &stub,
        r#"#!/bin/sh
printf '%s\n' "$*" >> "$KITTER_TEST_TRACE"
[ "$*" = '-y skills add owner/repo --skill alpha --agent universal --yes --copy' ] || exit 90
[ -z "$KITTER_TEST_FAIL" ] || exit 91
mkdir -p .agents/skills/alpha
printf '%s\n' '---' 'name: alpha' '---' 'new' > .agents/skills/alpha/SKILL.md
printf '%s' '{"skills":{"alpha":{"computedHash":"new-hash"}}}' > skills-lock.json
"#,
    )
    .unwrap();
    fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(bin).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let trace = temp.path().join("trace");
    let run = |fail: bool| {
        Command::new(env!("CARGO_BIN_EXE_kitter"))
            .env("KITTER_HOME", &data)
            .env("PATH", &path)
            .env("KITTER_TEST_TRACE", &trace)
            .env("KITTER_TEST_FAIL", if fail { "1" } else { "" })
            .args(["update", "alpha"])
            .output()
            .unwrap()
    };
    let failed = run(true);
    assert!(!failed.status.success());
    assert!(
        fs::read_to_string(library.skill_path("alpha").unwrap().join("SKILL.md"))
            .unwrap()
            .ends_with("old")
    );
    let updated = run(false);
    assert!(
        updated.status.success(),
        "{}",
        String::from_utf8_lossy(&updated.stderr)
    );
    library = kitter::SkillLibrary::open_in(&data).unwrap();
    assert!(
        fs::read_to_string(library.skill_path("alpha").unwrap().join("SKILL.md"))
            .unwrap()
            .contains("new")
    );
    assert!(!library.record("alpha").unwrap().update_available);
    assert!(library.record("beta").unwrap().update_available);
    assert!(
        fs::read_to_string(library.skill_path("beta").unwrap().join("SKILL.md"))
            .unwrap()
            .ends_with("old")
    );
    assert_eq!(fs::read_to_string(trace).unwrap().lines().count(), 2);
}

#[test]
fn cli_groups_git_and_npx_skills_and_reuses_remote_metadata() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repository");
    let bin = temp.path().join("bin");
    let data = temp.path().join("data");
    let log = temp.path().join("git.log");
    fs::create_dir_all(&repo).unwrap();
    fs::create_dir(&bin).unwrap();
    git(&repo, &["init", "-q", "-b", "main", "--object-format=sha1"]);
    for name in ["alpha", "beta"] {
        fs::create_dir_all(repo.join("skills").join(name)).unwrap();
        fs::write(
            repo.join("skills").join(name).join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Fixture\n---\n"),
        )
        .unwrap();
    }
    git(&repo, &["add", "."]);
    git(
        &repo,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-qm",
            "initial",
        ],
    );
    let mut library = kitter::SkillLibrary::open_in(data.clone()).unwrap();
    for name in ["alpha", "beta"] {
        let origin = if name == "alpha" {
            kitter::SkillOrigin::Git {
                repository: repo.to_str().unwrap().into(),
                subdir: Some(format!("skills/{name}")),
            }
        } else {
            kitter::SkillOrigin::Npx {
                repository: repo.to_str().unwrap().into(),
                skill: name.into(),
                source_hash: None,
            }
        };
        library
            .import(
                &repo.join("skills").join(name),
                kitter::SkillRecord {
                    name: name.into(),
                    storage_name: String::new(),
                    description: String::new(),
                    origin,
                    update_available: false,
                    group_id: None,
                    last_operated_at: 0,
                },
            )
            .unwrap();
    }
    let real_git =
        String::from_utf8(Command::new("which").arg("git").output().unwrap().stdout).unwrap();
    fs::write(bin.join("git"), "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$KITTER_TEST_TRACE\"\nexec \"$KITTER_TEST_GIT\" \"$@\"\n").unwrap();
    fs::write(
        bin.join("npx"),
        "#!/bin/sh\nprintf 'npx invoked\\n' >> \"$KITTER_TEST_TRACE\"\nexit 99\n",
    )
    .unwrap();
    for name in ["git", "npx"] {
        fs::set_permissions(bin.join(name), fs::Permissions::from_mode(0o755)).unwrap();
    }
    let run = || {
        let output = Command::new(env!("CARGO_BIN_EXE_kitter"))
            .env("KITTER_HOME", &data)
            .env("KITTER_TEST_TRACE", &log)
            .env("KITTER_TEST_GIT", real_git.trim())
            .env(
                "PATH",
                std::env::join_paths(
                    std::iter::once(bin.clone())
                        .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
                )
                .unwrap(),
            )
            .arg("check")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let trace = fs::read_to_string(&log).unwrap();
        fs::write(&log, "").unwrap();
        trace
    };
    let cold = run();
    assert_eq!(
        cold.lines()
            .filter(|line| line.starts_with("ls-remote "))
            .count(),
        1,
        "{cold}"
    );
    assert_eq!(
        cold.lines().filter(|line| line.contains(" fetch ")).count(),
        1,
        "{cold}"
    );
    assert!(!cold.contains("clone"));
    assert!(!cold.contains("npx invoked"));
    let warm = run();
    assert_eq!(
        warm.lines()
            .filter(|line| line.starts_with("ls-remote "))
            .count(),
        1,
        "{warm}"
    );
    assert!(!warm.contains(" fetch "));
    assert!(!warm.contains("ls-tree"));
    assert!(!data.join("npx-sources").exists());
    assert_eq!(
        fs::read(library.skill_path("alpha").unwrap().join("SKILL.md")).unwrap(),
        fs::read(repo.join("skills/alpha/SKILL.md")).unwrap()
    );
}

#[test]
fn batch_update_scope_continues_after_failure_and_does_not_touch_other_skills() {
    let temp = tempfile::tempdir().unwrap();
    let bin = temp.path().join("bin");
    fs::create_dir(&bin).unwrap();
    fs::write(
        bin.join("npx"),
        r#"#!/bin/sh
name="$6"
printf '%s\n' "$name" >> "$KITTER_BATCH_FIXTURE/calls"
[ "$name" != broken ] || exit 91
mkdir -p ".agents/skills/$name"
printf '%s\n' '---' "name: $name" '---' 'updated' > ".agents/skills/$name/SKILL.md"
printf '%s' '{"skills":{}}' > skills-lock.json
"#,
    )
    .unwrap();
    fs::set_permissions(bin.join("npx"), fs::Permissions::from_mode(0o755)).unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "batch_update_fixture_child", "--nocapture"])
        .env("KITTER_BATCH_FIXTURE", temp.path())
        .env("KITTER_HOME", temp.path().join("wrong-home"))
        .env(
            "PATH",
            std::env::join_paths(
                std::iter::once(bin)
                    .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
            )
            .unwrap(),
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("calls")).unwrap(),
        "alpha\nbroken\nbeta\n"
    );
    assert!(!temp.path().join("wrong-home/npx-sources").exists());
}

#[test]
fn batch_update_fixture_child() {
    let Some(root) = std::env::var_os("KITTER_BATCH_FIXTURE") else {
        return;
    };
    let root = std::path::PathBuf::from(root);
    let data = root.join("data");
    let mut library = kitter::SkillLibrary::open_in(&data).unwrap();
    for name in ["alpha", "broken", "beta", "untouched", "linked"] {
        let source = root.join("sources").join(name);
        fs::create_dir_all(&source).unwrap();
        fs::write(
            source.join("SKILL.md"),
            format!("---\nname: {name}\n---\noriginal"),
        )
        .unwrap();
        let origin = if name == "linked" {
            kitter::SkillOrigin::Local {
                path: source.clone(),
                source_root: None,
            }
        } else {
            kitter::SkillOrigin::Npx {
                repository: "owner/repo".into(),
                skill: name.into(),
                source_hash: None,
            }
        };
        library
            .import(
                &source,
                kitter::SkillRecord {
                    name: name.into(),
                    storage_name: name.into(),
                    description: String::new(),
                    origin,
                    update_available: true,
                    group_id: None,
                    last_operated_at: 0,
                },
            )
            .unwrap();
    }
    let mut progress = Vec::new();
    let report = kitter::source::update_skills_with_progress(
        &data,
        &[
            "alpha",
            "broken",
            "alpha",
            "beta",
            "linked",
            "_kitter-builtin",
        ]
        .map(String::from),
        |snapshot| progress.push(snapshot),
    )
    .unwrap();
    assert_eq!(report.updated, 2);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].0, "broken");
    assert_eq!(progress.last().unwrap().completed, 3);
    assert_eq!(progress.last().unwrap().total, 3);
    assert!(
        progress
            .windows(2)
            .all(|p| p[0].completed <= p[1].completed)
    );
    let library = kitter::SkillLibrary::open_in(&data).unwrap();
    for name in ["alpha", "beta"] {
        assert!(!library.record(name).unwrap().update_available);
        assert!(
            fs::read_to_string(library.skill_path(name).unwrap().join("SKILL.md"))
                .unwrap()
                .contains("updated")
        );
    }
    for name in ["broken", "untouched", "linked"] {
        assert!(
            fs::read_to_string(library.skill_path(name).unwrap().join("SKILL.md"))
                .unwrap()
                .contains("original")
        );
    }
    assert!(library.record("broken").unwrap().update_available);
}
