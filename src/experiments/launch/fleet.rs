//! One CPU ceiling on the parent cgroup shared by rootless Docker scopes.
use super::Context;
use crate::experiments::CommandOutput;
use crate::plugin::process;
use std::{
    fs,
    path::{Path, PathBuf},
};

fn scope(root: &Path, actual: &Path, depth: usize) -> Option<PathBuf> {
    for entry in fs::read_dir(actual).ok()?.flatten() {
        let path = root.join(entry.file_name());
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("docker-") && name.ends_with(".scope") {
            return path.parent().map(Path::to_path_buf);
        }
        if depth < 3
            && entry.file_type().is_ok_and(|kind| kind.is_dir())
            && let Some(found) = scope(&path, &entry.path(), depth + 1)
        {
            return Some(found);
        }
    }
    None
}
fn shell_integer(value: &str) -> Option<i64> {
    let (negative, value) = if let Some(value) = value.strip_prefix('-') {
        (true, value)
    } else {
        (false, value.strip_prefix('+').unwrap_or(value))
    };
    let (base, digits) = if let Some(value) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        (16, value)
    } else if value.len() > 1 && value.starts_with('0') {
        (8, value)
    } else {
        (10, value)
    };
    let integer = u64::from_str_radix(digits, base).ok()? as i64;
    Some(if negative {
        integer.wrapping_neg()
    } else {
        integer
    })
}
impl Context {
    /// The flag distinguishes a discovered scope's parent from the canonical fallback.
    pub fn fleet_cgroup(&self) -> Option<(PathBuf, bool)> {
        #[cfg(unix)]
        let uid = unsafe { libc::getuid() };
        #[cfg(not(unix))]
        let uid = 0;
        let base = Path::new(self.setting("TE_CGROUP_ROOT", "/sys/fs/cgroup"))
            .join(format!("user.slice/user-{uid}.slice/user@{uid}.service"));
        if let Some(parent) = scope(&base, &self.cwd.join(&base), 1) {
            return Some((parent, true));
        }
        let canonical = base.join("user.slice");
        self.cwd
            .join(&canonical)
            .is_dir()
            .then_some((canonical, false))
    }
    pub fn fleet_cap(&mut self, cores: Option<&str>) {
        let cores = cores
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| self.setting("TE_FLEET_CPUS", "16"))
            .to_owned();
        if matches!(cores.as_str(), "0" | "none" | "") {
            self.out(format!(
                "[fleet] CPU budget: not enforced (TE_FLEET_CPUS={cores})"
            ));
            return;
        }
        let Some((group, _)) = self.fleet_cgroup() else {
            self.err(format!("[fleet] !! no rootless-docker cgroup found -- the {cores}-core fleet cap is NOT in force"));
            return;
        };
        let file = group.join("cpu.max");
        let actual = self.cwd.join(&file);
        #[cfg(unix)]
        let writable = std::ffi::CString::new(actual.as_os_str().as_encoded_bytes())
            .is_ok_and(|path| unsafe { libc::access(path.as_ptr(), libc::W_OK) == 0 });
        #[cfg(not(unix))]
        let writable = fs::metadata(&actual).is_ok_and(|m| !m.permissions().readonly());
        if !writable {
            self.err(format!(
                "[fleet] !! {} is not writable -- the {cores}-core fleet cap is NOT in force",
                file.display()
            ));
            self.err("[fleet] !! (cgroup v2 must delegate the cpu controller to the user manager)");
            return;
        }
        let Some(cores_integer) = shell_integer(&cores) else {
            self.err(format!("[fleet] !! invalid CPU budget: {cores}"));
            self.stop(1);
            return;
        };
        let quota = cores_integer.wrapping_mul(100000);
        if fs::write(actual, format!("{quota} 100000\n")).is_ok() {
            self.out(format!(
                "[fleet] CPU budget: every TE container, collectively, capped at {cores} cores"
            ));
            self.out(format!("[fleet]   cgroup: {}", group.display()));
        } else {
            self.err(format!(
                "[fleet] !! could not write {} -- the {cores}-core fleet cap is NOT in force",
                file.display()
            ));
        }
    }
    pub fn fleet_report(&mut self) {
        let Some((group, _)) = self.fleet_cgroup() else {
            self.out("[fleet] no rootless-docker cgroup on this host");
            return;
        };
        let cur = fs::read_to_string(self.cwd.join(&group).join("cpu.max"))
            .unwrap_or("?".into())
            .trim_end_matches('\n')
            .to_owned();
        if cur.starts_with("max ") {
            self.out(format!(
                "[fleet] CPU budget: UNCAPPED ({}/cpu.max = {cur})",
                group.display()
            ));
        } else {
            let quota = shell_integer(cur.split(' ').next().unwrap_or("")).unwrap_or(0);
            self.out(format!(
                "[fleet] CPU budget: {} cores ({}/cpu.max = {cur})",
                quota / 100000,
                group.display()
            ));
        }
        if let Some(docker) = self.which("docker") {
            let ids = process::run_group(
                &[docker.clone(), "ps".into(), "-q".into()],
                &self.cwd,
                &self.process_env(),
                30.0,
            )
            .map(|o| o.stdout)
            .unwrap_or_default();
            let n = ids.bytes().filter(|b| *b == b'\n').count();
            #[cfg(target_os = "macos")]
            let count = format!("{n:8}");
            #[cfg(not(target_os = "macos"))]
            let count = n.to_string();
            self.out(format!("[fleet] live task containers: {count}"));
            if n > 0 {
                let stats = process::run_group(
                    &[
                        docker,
                        "stats".into(),
                        "--no-stream".into(),
                        "--format".into(),
                        "{{.CPUPerc}}".into(),
                    ],
                    &self.cwd,
                    &self.process_env(),
                    30.0,
                )
                .map(|o| o.stdout)
                .unwrap_or_default();
                let sum: f64 = stats
                    .lines()
                    .filter_map(|line| line.replace('%', "").trim().parse::<f64>().ok())
                    .sum();
                self.out(format!("[fleet] drawing now: {:.1} cores", sum / 100.0));
            }
        }
        let stats = fs::read_to_string(self.cwd.join(&group).join("cpu.stat")).unwrap_or_default();
        let mut throttling = String::new();
        for line in stats
            .lines()
            .filter(|line| line.contains("nr_throttled") || line.contains("throttled_usec"))
        {
            let mut fields = line.split_whitespace();
            throttling.push_str(&format!(
                "{}={} ",
                fields.next().unwrap_or(""),
                fields.next().unwrap_or("")
            ));
        }
        self.out(format!("[fleet] throttling since boot: {throttling}"));
    }
}

pub fn command(
    args: &[String],
    env: serde_json::Map<String, serde_json::Value>,
    cwd: PathBuf,
) -> CommandOutput {
    let exported = env.keys().cloned().collect();
    let mut ctx = Context {
        env,
        exported,
        cwd,
        stdout: String::new(),
        stderr: String::new(),
        code: 0,
        stopped: false,
    };
    if let Some(cores) = args.first() {
        ctx.fleet_cap(Some(cores));
    }
    if !ctx.stopped {
        ctx.fleet_report();
    }
    CommandOutput {
        returncode: Some(ctx.code),
        stdout: ctx.stdout,
        stderr: ctx.stderr,
        exit: None,
    }
}
