// Converted from reference/upstream/experiments/tests/test_fleet_budget.py.
#![cfg(unix)]

mod support;

use serde_json::json;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
};
use support::TempDir;
use tokeneconomizer::experiments::{CommandOutput, launch::fleet};

fn tree(root: &Path, scope: bool, writable: bool) -> PathBuf {
    let uid = unsafe { libc::getuid() };
    let base = root.join(format!(
        "user.slice/user-{uid}.slice/user@{uid}.service/user.slice"
    ));
    fs::create_dir_all(&base).unwrap();
    fs::write(base.join("cpu.max"), "max 100000\n").unwrap();
    fs::write(base.join("cpu.stat"), "nr_throttled 0\nthrottled_usec 0\n").unwrap();
    if scope {
        fs::create_dir(base.join("docker-abc123.scope")).unwrap();
    }
    if !writable {
        fs::set_permissions(base.join("cpu.max"), fs::Permissions::from_mode(0o444)).unwrap();
    }
    base
}
fn run(root: &Path, args: &[&str]) -> CommandOutput {
    fleet::command(
        &args.iter().map(|s| (*s).into()).collect::<Vec<_>>(),
        json!({"TE_CGROUP_ROOT":root}).as_object().unwrap().clone(),
        root.into(),
    )
}

#[test]
fn the_cap_lands_on_the_cgroup_the_container_scopes_share() {
    let root = TempDir::new("fleet-parent");
    let base = tree(root.path(), true, true);
    let result = run(root.path(), &["16"]);
    assert_eq!(result.returncode, Some(0), "{}", result.stderr);
    assert_eq!(
        fs::read_to_string(base.join("cpu.max")).unwrap().trim(),
        "1600000 100000"
    );
    assert!(result.stdout.contains("capped at 16 cores"));
    assert!(!base.join("docker-abc123.scope/cpu.max").exists());
}

#[test]
fn the_canonical_path_is_the_fallback_when_no_container_is_running() {
    let root = TempDir::new("fleet-fallback");
    let base = tree(root.path(), false, true);
    let result = run(root.path(), &["8"]);
    assert_eq!(result.returncode, Some(0), "{}", result.stderr);
    assert_eq!(
        fs::read_to_string(base.join("cpu.max")).unwrap().trim(),
        "800000 100000"
    );
}

#[test]
fn a_report_reads_the_budget_back() {
    let root = TempDir::new("fleet-report");
    tree(root.path(), true, true);
    assert!(run(root.path(), &[]).stdout.contains("UNCAPPED"));
    run(root.path(), &["16"]);
    assert!(
        run(root.path(), &[])
            .stdout
            .contains("CPU budget: 16 cores")
    );
}

#[test]
fn zero_leaves_the_cgroup_alone() {
    let root = TempDir::new("fleet-zero");
    let base = tree(root.path(), true, true);
    let result = run(root.path(), &["0"]);
    assert_eq!(result.returncode, Some(0));
    assert!(result.stdout.contains("not enforced"));
    assert_eq!(
        fs::read_to_string(base.join("cpu.max")).unwrap().trim(),
        "max 100000"
    );
}

#[test]
fn an_unwritable_cgroup_warns_and_never_fails_the_launch() {
    let root = TempDir::new("fleet-permissions");
    tree(root.path(), true, false);
    let result = run(root.path(), &["16"]);
    assert_eq!(result.returncode, Some(0));
    assert!(result.stderr.contains("NOT in force"));
}

#[test]
fn a_missing_cgroup_tree_warns_and_never_fails_the_launch() {
    let root = TempDir::new("fleet-missing");
    let result = run(&root.path().join("nothing-here"), &["16"]);
    assert_eq!(result.returncode, Some(0));
    assert!(result.stderr.contains("NOT in force"));
}

#[test]
fn the_codex_launcher_asserts_a_budget_and_a_per_container_cap() {
    let root = TempDir::new("fleet-launcher");
    let base = tree(root.path(), true, true);
    let env_file = root.path().join("empty.env");
    fs::write(&env_file, "").unwrap();
    let tasks = root.path().join("tasks.json");
    fs::write(&tasks, "[]").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_te-bench"))
        .arg("codex-ab")
        .current_dir(root.path())
        .env_clear()
        .env("TOKENECONOMIZER_ENV_FILE", env_file)
        .env("TE_CGROUP_ROOT", root.path())
        .env("TOKENECONOMIZER_RESULTS", root.path().join("results"))
        .env("TASKS_FILE", tasks)
        .env("WORKLOAD", "none")
        .env("REPOS", "r")
        .env("SEEDS", " ")
        .env("DETACH", "0")
        .env("RUNNER", "stub")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(base.join("cpu.max")).unwrap().trim(),
        "1600000 100000"
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("cpus: per-container 4, fleet 16"));
}
