// Converted from reference/upstream/tests/test_live_safety.py.
mod support;

use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};
#[cfg(unix)]
use std::{
    io::Write,
    os::unix::{fs::PermissionsExt, process::CommandExt},
    process::{Command, Stdio},
};
use support::TempDir;
use tokeneconomizer::{
    plugin::{
        denylist::{self, NativeRuntime},
        paths,
    },
    store::Store,
};

fn repo(root: &Path) -> PathBuf {
    let repo = root.join("repo");
    fs::create_dir_all(repo.join(".git/info")).unwrap();
    fs::write(repo.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
    fs::write(
        repo.join(".git/config"),
        "[remote \"origin\"]\n\turl = https://github.com/acme/widget.git\n",
    )
    .unwrap();
    fs::write(repo.join("pyproject.toml"), "[project]\nname='w'\n").unwrap();
    repo
}
fn link(target: &Path, path: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, path).unwrap();
    #[cfg(windows)]
    if target.is_dir() {
        std::os::windows::fs::symlink_dir(target, path).unwrap();
    } else {
        std::os::windows::fs::symlink_file(target, path).unwrap();
    }
}

#[test]
fn a_symlinked_settings_directory_is_refused() {
    for (host, relative, item) in [
        ("claude", ".claude", "Workflow"),
        ("copilot", ".github", "github-mcp-server"),
        ("codex", ".codex", "goals"),
    ] {
        let root = TempDir::new("safety-settings-dir");
        let repo = repo(root.path());
        let outside = root.path().join("outside");
        fs::create_dir(&outside).unwrap();
        link(&outside, &repo.join(relative));
        let error = denylist::apply(
            &repo,
            &[item.into()],
            &Store::new(&root.path().join("st")).unwrap(),
            host,
            &mut NativeRuntime,
        )
        .unwrap_err();
        assert!(
            matches!(error, denylist::Error::Refused(_)) && error.to_string().contains("symlink"),
            "{error}"
        );
        assert_eq!(fs::read_dir(outside).unwrap().count(), 0);
    }
}

#[test]
fn a_symlinked_settings_file_is_refused_and_its_target_untouched() {
    let root = TempDir::new("safety-settings-file");
    let repo = repo(root.path());
    let target = root.path().join("elsewhere.json");
    fs::write(&target, "{\"keep\": true}\n").unwrap();
    fs::create_dir(repo.join(".claude")).unwrap();
    link(&target, &repo.join(".claude/settings.local.json"));
    let store = Store::new(&root.path().join("st")).unwrap();
    let error = denylist::apply(
        &repo,
        &["Workflow".into()],
        &store,
        "claude",
        &mut NativeRuntime,
    )
    .unwrap_err();
    assert!(matches!(error, denylist::Error::Refused(_)) && error.to_string().contains("symlink"));
    assert!(
        denylist::restore(
            &repo,
            &store,
            &["Workflow".into()],
            "claude",
            &mut NativeRuntime
        )
        .unwrap()
        .is_empty()
    );
    assert_eq!(fs::read_to_string(target).unwrap(), "{\"keep\": true}\n");
}

#[test]
fn a_settings_file_that_does_not_parse_is_refused_not_overwritten() {
    let root = TempDir::new("safety-settings-json");
    let repo = repo(root.path());
    let path = repo.join(".claude/settings.local.json");
    fs::create_dir(path.parent().unwrap()).unwrap();
    let broken = "{\"permissions\": {\"allow\": [\"Bash(ls)\"],}}\n";
    fs::write(&path, broken).unwrap();
    let error = denylist::apply(
        &repo,
        &["Workflow".into()],
        &Store::new(&root.path().join("st")).unwrap(),
        "claude",
        &mut NativeRuntime,
    )
    .unwrap_err();
    assert!(
        matches!(error, denylist::Error::Refused(_))
            && error.to_string().contains("not valid JSON")
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), broken);
    fs::write(&path, "").unwrap();
    assert_eq!(
        denylist::apply(
            &repo,
            &["Workflow".into()],
            &Store::new(&root.path().join("st2")).unwrap(),
            "claude",
            &mut NativeRuntime
        )
        .unwrap(),
        ["Workflow"]
    );
    let saved: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(saved, json!({"permissions":{"deny":["Workflow"]}}));
}

#[test]
fn a_git_file_pointing_out_of_the_repository_gets_no_exclude_line() {
    let root = TempDir::new("safety-git-file");
    let repo = repo(root.path());
    fs::remove_dir_all(repo.join(".git")).unwrap();
    let elsewhere = root.path().join("not-a-git-dir");
    fs::create_dir(&elsewhere).unwrap();
    fs::write(
        repo.join(".git"),
        format!("gitdir: {}\n", elsewhere.display()),
    )
    .unwrap();
    denylist::apply(
        &repo,
        &["Workflow".into()],
        &Store::new(&root.path().join("st")).unwrap(),
        "claude",
        &mut NativeRuntime,
    )
    .unwrap();
    assert_eq!(fs::read_dir(elsewhere).unwrap().count(), 0);
}

#[test]
fn a_symlinked_exclude_file_is_not_written_through() {
    let root = TempDir::new("safety-git-exclude");
    let repo = repo(root.path());
    let victim = root.path().join("victim");
    fs::write(&victim, "keep\n").unwrap();
    link(&victim, &repo.join(".git/info/exclude"));
    denylist::apply(
        &repo,
        &["Workflow".into()],
        &Store::new(&root.path().join("st")).unwrap(),
        "claude",
        &mut NativeRuntime,
    )
    .unwrap();
    assert_eq!(fs::read_to_string(victim).unwrap(), "keep\n");
}

#[cfg(unix)]
#[test]
fn a_loose_state_root_is_made_private_and_the_launcher_is_owner_only() {
    let root = TempDir::new("safety-root-mode");
    let home = root.path().join("home");
    fs::create_dir(&home).unwrap();
    fs::set_permissions(&home, fs::Permissions::from_mode(0o775)).unwrap();
    let env = json!({"TOKENECONOMIZER_HOME":home})
        .as_object()
        .unwrap()
        .clone();
    paths::secure_root(&env, root.path()).unwrap();
    assert_eq!(
        fs::metadata(home).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let launcher = paths::ensure_native_launcher(
        &env,
        root.path(),
        Path::new(env!("CARGO_BIN_EXE_te")),
        false,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        fs::metadata(launcher).unwrap().permissions().mode() & 0o777,
        0o700
    );
}

#[cfg(unix)]
#[test]
fn the_command_line_creates_its_state_private() {
    for hook in [false, true] {
        let root = TempDir::new("safety-entry-mode");
        let repo = repo(root.path());
        let home = root.path().join("home");
        let mut command = Command::new(if hook {
            env!("CARGO_BIN_EXE_te-hook")
        } else {
            env!("CARGO_BIN_EXE_te")
        });
        command
            .env_clear()
            .env("HOME", root.path())
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("TOKENECONOMIZER_HOME", &home)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Give only the child a permissive mask, so each entry point must tighten it.
        unsafe {
            command.pre_exec(|| {
                libc::umask(0o022);
                Ok(())
            });
        }
        if hook {
            command
                .args(["--host", "claude"])
                .stdin(Stdio::piped())
                .env("TE_MICROSKILLS", "off")
                .env("TE_TOOLPRUNE", "off")
                .env("TE_LOOPGUARD", "off")
                .env("TE_REPOMAP", "off");
        } else {
            command.args([
                "--cwd",
                repo.to_str().unwrap(),
                "config",
                "set",
                "spiral=off",
            ]);
        }
        let mut child = command.spawn().unwrap();
        if hook {
            writeln!(
                child.stdin.take().unwrap(),
                "{}",
                json!({"hook_event_name":"UserPromptSubmit","session_id":"s1","cwd":repo})
            )
            .unwrap();
        }
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        if hook {
            let response: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert!(response.is_object());
            assert!(
                !home.join("errors.log").exists(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let mut made = vec![home];
        let mut i = 0;
        while i < made.len() {
            let path = &made[i];
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o077,
                0,
                "{}",
                path.display()
            );
            if path.is_dir() {
                made.extend(fs::read_dir(path).unwrap().map(|e| e.unwrap().path()));
            }
            i += 1;
        }
        assert!(made.len() > 1, "The entry point must create state");
    }
}
