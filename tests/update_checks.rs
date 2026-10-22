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
