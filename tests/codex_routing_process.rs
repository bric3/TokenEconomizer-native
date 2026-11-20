// Converted from reference/upstream/experiments/tests/test_codex_routing.py.
#![cfg(unix)]
#[path = "support/codex_install_fixture.rs"]
pub mod install_fixture;
mod support;

use install_fixture::Install;
use serde_json::json;
use std::{
    fs,
    path::Path,
    thread,
    time::{Duration, Instant},
};
use support::TempDir;

fn assert_reaped(path: &Path) {
    let pid = fs::read_to_string(path).unwrap().parse::<i32>().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        #[cfg(target_os = "linux")]
        let alive = fs::read_to_string(format!("/proc/{pid}/stat"))
            .is_ok_and(|stat| stat.split_whitespace().nth(2) != Some("Z"));
        #[cfg(not(target_os = "linux"))]
        // Signal 0 checks this fixture descendant without delivering a signal.
        let alive = unsafe { libc::kill(pid, 0) } == 0;
        if !alive {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "the CLI's child {pid} outlived the probe"
        );
        thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn the_skills_probe_kills_the_cli_it_started_not_just_the_shim() {
    let root = TempDir::new("codex-skills-kill");
    let pidfile = root.path().join("child.pid");
    let mut runtime = Install::new(root.path());
    runtime.extra_env = json!({"TE_CODEX_PROBE_MODE":"skills","TE_CODEX_PROBE_PIDFILE":pidfile})
        .as_object()
        .unwrap()
        .clone();
    assert_eq!(runtime.run("bundled_system_skills",json!({"binary":env!("CARGO_BIN_EXE_te-codex-probe"),"home":root.path(),"timeout":20})).unwrap(),json!(["alpha"]));
    // Check the descendant before the runtime's fallback cleanup runs in Drop.
    assert_reaped(&pidfile);
}

#[test]
fn the_config_validator_kills_a_hung_cli_and_its_children() {
    let root = TempDir::new("codex-config-kill");
    let pidfile = root.path().join("child.pid");
    let mut runtime = Install::new(root.path());
    runtime.extra_env = json!({"TE_CODEX_PROBE_MODE":"hang","TE_CODEX_PROBE_PIDFILE":pidfile})
        .as_object()
        .unwrap()
        .clone();
    let error = runtime.run("validate_config",json!({"text":"model = \"gpt-5.6-sol\"\n","binary":env!("CARGO_BIN_EXE_te-codex-probe"),"home":root.path(),"timeout":2})).unwrap();
    assert!(
        error
            .as_str()
            .unwrap()
            .starts_with("could not run the config validator")
    );
    assert_reaped(&pidfile);
}

#[test]
fn bundled_system_skills_are_read_from_the_installed_cli() {
    let Some(binary) = Install::installed() else {
        return;
    };
    let root = TempDir::new("codex-installed-skills");
    let found = Install::new(root.path())
        .run(
            "bundled_system_skills",
            json!({"binary":binary,"home":root.path(),"timeout":20}),
        )
        .unwrap();
    let found = found.as_array().unwrap();
    assert!(!found.is_empty());
    assert!(
        found
            .iter()
            .all(|name| !name.as_str().unwrap().contains('/'))
    );
    assert!(found.contains(&json!("skill-creator")) && found.contains(&json!("skill-installer")));
}
