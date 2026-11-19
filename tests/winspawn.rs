// Converted from reference/upstream/tests/test_winspawn.py.
mod support;

use serde_json::Map;
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use support::TempDir;
use tokeneconomizer::plugin::{process::Launch, winspawn};

#[derive(Default)]
struct Runtime {
    flags: Vec<u32>,
    denied: bool,
    wmi_calls: usize,
    scripts: Vec<String>,
}
impl winspawn::Runtime for Runtime {
    fn launch(&mut self, _spec: &Launch, flags: u32) -> io::Result<()> {
        self.flags.push(flags);
        if self.denied {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "access denied",
            ))
        } else {
            Ok(())
        }
    }
    fn wmi(&mut self, _spec: &Launch, _executable: &Path, _runner: &Path) -> io::Result<bool> {
        self.wmi_calls += 1;
        Ok(true)
    }
    fn powershell(&mut self, script: &str) -> io::Result<i32> {
        self.scripts.push(script.into());
        Ok(0)
    }
}
fn spec(root: &Path) -> Launch {
    Launch {
        cmd: vec!["te".into()],
        cwd: root.into(),
        env: Map::new(),
        log: root.join("te.log"),
    }
}
fn interpreter(root: &Path, name: &str, beside: &[&str]) -> PathBuf {
    for file in beside {
        fs::write(root.join(file), "").unwrap();
    }
    root.join(name)
}

#[test]
fn te_gets_a_hidden_console_and_leaves_the_job() {
    let root = TempDir::new("winspawn-flags");
    let mut runtime = Runtime::default();
    assert!(
        winspawn::spawn(
            &spec(root.path()),
            &root.path().join("python.exe"),
            &root.path().join("runner.py"),
            &mut runtime
        )
        .unwrap()
    );
    let flags = runtime.flags[0];
    assert_eq!(flags & 0x00000008, 0);
    assert_ne!(flags & 0x08000000, 0);
    assert_ne!(flags & 0x01000000, 0);
}

#[test]
fn a_job_without_breakaway_goes_through_wmi() {
    let root = TempDir::new("winspawn-denied");
    let mut runtime = Runtime {
        denied: true,
        ..Default::default()
    };
    assert!(
        winspawn::spawn(
            &spec(root.path()),
            &root.path().join("python.exe"),
            &root.path().join("runner.py"),
            &mut runtime
        )
        .unwrap()
    );
    assert_eq!(runtime.wmi_calls, 1);
}

#[test]
fn the_runner_is_pythonw() {
    let root = TempDir::new("winspawn-pythonw");
    let exe = interpreter(root.path(), "python.exe", &["python.exe", "pythonw.exe"]);
    assert_eq!(
        winspawn::windowless_python(&exe),
        root.path().join("pythonw.exe")
    );
}

#[test]
fn the_runner_matches_a_suffixed_interpreter() {
    let root = TempDir::new("winspawn-suffix");
    let exe = interpreter(
        root.path(),
        "python3.13t.exe",
        &["python3.13t.exe", "pythonw3.13t.exe", "pythonw.exe"],
    );
    assert_eq!(
        winspawn::windowless_python(&exe),
        root.path().join("pythonw3.13t.exe")
    );
}

#[test]
fn the_runner_falls_back_to_this_interpreter() {
    let root = TempDir::new("winspawn-fallback");
    let exe = interpreter(root.path(), "python.exe", &["python.exe"]);
    assert_eq!(winspawn::windowless_python(&exe), exe);
}

#[test]
fn wmi_starts_the_windowless_runner() {
    let root = TempDir::new("winspawn-wmi");
    let exe = interpreter(root.path(), "python.exe", &["python.exe", "pythonw.exe"]);
    let mut runtime = Runtime::default();
    assert!(
        winspawn::spawn_wmi(
            &spec(root.path()),
            &exe,
            &root.path().join("runner.py"),
            &mut runtime
        )
        .unwrap()
    );
    let pythonw = winspawn::command_line(&[root
        .path()
        .join("pythonw.exe")
        .to_string_lossy()
        .into_owned()]);
    assert!(
        runtime.scripts[0].contains(&format!("CommandLine = '{} ", pythonw.replace('\'', "''")))
    );
    for entry in fs::read_dir(root.path()).unwrap() {
        let path = entry.unwrap().path();
        if path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("te-spawn-")
        {
            fs::remove_file(path).unwrap();
        }
    }
}
