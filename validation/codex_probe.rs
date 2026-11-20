//! A Codex stand-in with a descendant for the native probe-cleanup tests.
use std::{env, fs, io, process::Command, thread, time::Duration};

fn main() -> io::Result<()> {
    if env::args().nth(1).as_deref() == Some("--child") {
        thread::sleep(Duration::from_secs(600));
        return Ok(());
    }
    assert_eq!(env::args().nth(1).as_deref(), Some("exec"));
    let pidfile = env::var_os("TE_CODEX_PROBE_PIDFILE").expect("missing fixture PID file");
    let mut child = Command::new(env::current_exe()?).arg("--child").spawn()?;
    fs::write(pidfile, child.id().to_string())?;
    if env::var("TE_CODEX_PROBE_MODE").as_deref() == Ok("skills") {
        let root = std::path::PathBuf::from(env::var_os("CODEX_HOME").expect("missing CODEX_HOME"))
            .join("skills/.system");
        fs::create_dir_all(root.join("alpha"))?;
        fs::write(root.join("alpha/SKILL.md"), "")?;
        fs::write(root.join(".codex-system-skills.marker"), "")?;
    }
    child.wait()?;
    Ok(())
}
