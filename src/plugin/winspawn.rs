use super::process::{self, Launch};
use crate::value::json_text;
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

pub const FLAGS: u32 = 0x00000200 | 0x08000000;
pub const BREAKAWAY: u32 = 0x01000000;
pub const NO_WINDOW: u32 = 0x08000000;
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub trait Runtime {
    fn launch(&mut self, spec: &Launch, flags: u32) -> io::Result<()> {
        process::spawn_logged(&spec.cmd, &spec.cwd, &spec.env, &spec.log, flags, false)
    }
    fn powershell(&mut self, script: &str) -> io::Result<i32> {
        Ok(process::run_configured(
            &[
                "powershell".into(),
                "-NoProfile".into(),
                "-NonInteractive".into(),
                "-Command".into(),
                script.into(),
            ],
            &std::env::current_dir()?,
            15.0,
            NO_WINDOW,
        )?
        .code)
    }
    fn wmi(&mut self, spec: &Launch, executable: &Path, runner: &Path) -> io::Result<bool>;
    fn call(&mut self, spec: &Launch, flags: u32) -> io::Result<i32> {
        process::call_logged(&spec.cmd, &spec.cwd, &spec.env, &spec.log, flags)
    }
}
pub struct NativeRuntime;
impl Runtime for NativeRuntime {
    fn wmi(&mut self, spec: &Launch, executable: &Path, runner: &Path) -> io::Result<bool> {
        spawn_wmi(spec, executable, runner, self)
    }
}
pub fn windowless_python(executable: &Path) -> PathBuf {
    let folder = executable.parent().unwrap_or(Path::new(""));
    let name = executable.file_name().unwrap_or_default().to_string_lossy();
    let mut names = Vec::new();
    if name.to_lowercase().starts_with("python") {
        names.push(format!("pythonw{}", &name[6..]));
    }
    names.push("pythonw.exe".into());
    names
        .into_iter()
        .map(|name| folder.join(name))
        .find(|path| path.is_file())
        .unwrap_or_else(|| executable.into())
}
pub fn spawn(
    spec: &Launch,
    executable: &Path,
    runner: &Path,
    runtime: &mut dyn Runtime,
) -> io::Result<bool> {
    if runtime.launch(spec, FLAGS | BREAKAWAY).is_ok() {
        return Ok(true);
    }
    runtime.wmi(spec, executable, runner)
}
pub fn command_line(argv: &[String]) -> String {
    let mut out = String::new();
    for (index, arg) in argv.iter().enumerate() {
        if index > 0 {
            out.push(' ');
        }
        let quote = arg.is_empty() || arg.contains([' ', '\t']);
        if quote {
            out.push('"');
        }
        let mut slashes = 0;
        for c in arg.chars() {
            if c == '\\' {
                slashes += 1;
                continue;
            }
            out.extend(std::iter::repeat_n(
                '\\',
                if c == '"' { slashes * 2 + 1 } else { slashes },
            ));
            out.push(c);
            slashes = 0;
        }
        out.extend(std::iter::repeat_n(
            '\\',
            if quote { slashes * 2 } else { slashes },
        ));
        if quote {
            out.push('"');
        }
    }
    out
}
fn write_spec(spec: &Launch) -> io::Result<PathBuf> {
    let parent = spec
        .log
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = std::path::absolute(parent)?;
    loop {
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = parent.join(format!(
            "te-spawn-{}-{time}-{}.json",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(mut file) => {
                file.write_all(json_text(&serde_json::to_value(spec)?).as_bytes())?;
                return Ok(path);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
}
pub fn spawn_wmi(
    spec: &Launch,
    executable: &Path,
    runner: &Path,
    runtime: &mut dyn Runtime,
) -> io::Result<bool> {
    let path = write_spec(spec)?;
    // shortcut: use the migration's runner script, switch to the native CLI worker during packaging.
    let line = command_line(&[
        windowless_python(executable).to_string_lossy().into_owned(),
        std::path::absolute(runner)?.to_string_lossy().into_owned(),
        path.to_string_lossy().into_owned(),
    ]);
    let ps = format!(
        "$r = Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{{ CommandLine = '{}'; CurrentDirectory = '{}' }}; exit $r.ReturnValue",
        line.replace('\'', "''"),
        spec.cwd.to_string_lossy().replace('\'', "''")
    );
    if !runtime.powershell(&ps).is_ok_and(|code| code == 0) {
        let _ = fs::remove_file(path);
        return Ok(false);
    }
    Ok(true)
}
pub fn run(path: &Path, runtime: &mut dyn Runtime) -> io::Result<i32> {
    let spec: Launch = serde_json::from_slice(&fs::read(path)?)?;
    let _ = fs::remove_file(path);
    runtime.call(&spec, NO_WINDOW)
}
