// Converted from reference/upstream/tests/test_live_core.py (the executable probe repository).
#![cfg(unix)]

mod support;

use serde_json::{Value, json};
use tokeneconomizer::plugin::{cli, probe, process};

struct Git;
impl probe::Runtime for Git {
    fn say(&mut self, _text: &str) -> cli::Result<()> {
        panic!("Unexpected probe output")
    }
    fn call(&mut self, method: &str, args: Value) -> cli::Result<Value> {
        assert_eq!(method, "git");
        let args = cli::words(&args)?;
        let output = std::process::Command::new(&args[0])
            .args(&args[1..])
            .output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(json!(String::from_utf8_lossy(&output.stdout)))
    }
}

#[test]
fn the_probe_repository_carries_its_trap() {
    let root = support::TempDir::new("core-probe-trap");
    let python = process::which("python3").expect("The probe fixture requires Python 3");
    let (repo, trap) = probe::make_repo(root.path(), python.to_str().unwrap(), &mut Git).unwrap();
    let bare = std::process::Command::new(trap.join("pytest"))
        .arg("-q")
        .current_dir(&repo)
        .output()
        .unwrap();
    assert_eq!(bare.status.code(), Some(127));
    assert!(String::from_utf8_lossy(&bare.stderr).contains("command not found"));
    for name in ["python", "python3"] {
        let output = std::process::Command::new(trap.join(name))
            .args(["-m", "pytest", "-q"])
            .current_dir(&repo)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains("No module named 'pytest'"));
    }
    let output = std::process::Command::new(trap.join("python"))
        .args(["-m", "unittest", "-q"])
        .current_dir(&repo)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("Ran 1 test"));
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["status", "--porcelain"])
        .output()
        .unwrap();
    assert!(status.status.success());
    assert!(status.stdout.is_empty());
}
