//! I/O shim for the original SDK console assertions; processing stays in the native executable.
use serde_json::json;
use std::{
    env,
    fs::OpenOptions,
    io::{self, Write},
    process::Command,
};
#[cfg(windows)]
use tokeneconomizer::plugin::winspawn::NO_WINDOW;

#[cfg(windows)]
fn console() -> (bool, bool) {
    use std::ffi::c_void;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetConsoleWindow() -> *mut c_void;
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn IsWindowVisible(window: *mut c_void) -> i32;
    }
    // These queries inspect the calling process's console without changing it.
    let window = unsafe { GetConsoleWindow() };
    (
        !window.is_null(),
        !window.is_null() && unsafe { IsWindowVisible(window) } != 0,
    )
}
fn run() -> io::Result<()> {
    let executable = env::current_exe()?;
    let hook = executable
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .starts_with("te-hook");
    let record = json!({"pid":std::process::id(),"hook":hook});
    #[cfg(windows)]
    let record = {
        let (window, visible) = console();
        let mut record = record;
        record["console_window"] = json!(window);
        record["visible"] = json!(visible);
        record
    };
    if let Some(log) = env::var_os("TE_CONSOLE_PROBE").filter(|s| !s.is_empty()) {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)?
            .write_all(format!("{record}\n").as_bytes())?;
    }
    let mut command = Command::new(executable.with_file_name(format!(
        "{}-native{}",
        if hook { "te-hook" } else { "te" },
        env::consts::EXE_SUFFIX
    )));
    command.args(env::args_os().skip(1));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        Err(command.exec())
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        if !console().0 {
            command.creation_flags(NO_WINDOW);
        }
        std::process::exit(command.status()?.code().unwrap_or(1));
    }
    #[cfg(not(any(unix, windows)))]
    {
        std::process::exit(command.status()?.code().unwrap_or(1));
    }
}
fn main() {
    if let Err(error) = run() {
        eprintln!("console probe: {error}");
        std::process::exit(1);
    }
}
