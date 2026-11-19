use super::Context;
use chrono::Local;
use regex::Regex;
use std::{fs, path::Path, sync::LazyLock};

static DOT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[0-9]\.[0-9]").unwrap());
static DASH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[0-9]-[0-9]").unwrap());

pub fn tree_name(agent: &str, domain: &str, tag: &str) -> String {
    format!(
        "{agent}_{domain}{}",
        if tag.is_empty() {
            String::new()
        } else {
            format!("_{tag}")
        }
    )
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::{
        thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn record_detached_child() {
        if std::env::var("TE_LAUNCH_DETACH_PROBE").as_deref() != Ok("1") {
            return;
        }
        let record = std::env::var("TE_LAUNCH_DETACH_RECORD").unwrap();
        let pidfile = std::env::var("TE_LAUNCH_DETACH_PIDFILE").unwrap();
        let pid = unsafe { libc::getpid() };
        let group = unsafe { libc::getpgrp() };
        let session = unsafe { libc::getsid(0) };
        let mut disposition = std::mem::MaybeUninit::<libc::sigaction>::uninit();
        assert_eq!(
            unsafe { libc::sigaction(libc::SIGHUP, std::ptr::null(), disposition.as_mut_ptr()) },
            0
        );
        let ignored = unsafe { disposition.assume_init() }.sa_sigaction == libc::SIG_IGN;
        let data = json!([
            pid,
            group,
            session,
            fs::read_to_string(pidfile)
                .unwrap()
                .trim()
                .parse::<i32>()
                .unwrap(),
            ignored,
            std::env::var("TE_DETACHED").unwrap(),
            std::env::var("PYTHONUNBUFFERED").unwrap()
        ]);
        fs::write(&record, data.to_string()).unwrap();
        fs::write(format!("{record}.done"), "").unwrap();
    }

    #[test]
    fn detached_pid_identifies_the_session_before_the_child_runs() {
        let root = std::env::temp_dir().join(format!(
            "te-launch-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let record = root.join("record.json");
        let pidfile = root.join("task.pid");
        let mut env: serde_json::Map<String, Value> =
            std::env::vars().map(|(k, v)| (k, json!(v))).collect();
        env.insert("TE_LOG".into(), json!(root.join("task.log")));
        env.insert("TE_LAUNCH_DETACH_PROBE".into(), json!("1"));
        env.insert("TE_LAUNCH_DETACH_RECORD".into(), json!(record));
        env.insert("TE_LAUNCH_DETACH_PIDFILE".into(), json!(pidfile));
        env.remove("DETACH");
        env.remove("TE_DETACHED");
        let exported = env.keys().cloned().collect();
        let mut context = Context {
            env,
            exported,
            cwd: root.clone(),
            stdout: String::new(),
            stderr: String::new(),
            code: 0,
            stopped: false,
        };
        let test = format!(
            "{}::record_detached_child",
            module_path!().split_once("::").unwrap().1
        );
        context.detach(
            "kernel-probe",
            &[
                std::env::current_exe()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
                "--exact".into(),
                test,
                "--nocapture".into(),
            ],
        );
        assert!(context.stopped && context.code == 0, "{}", context.stderr);
        let done = root.join("record.json.done");
        let deadline = Instant::now() + Duration::from_secs(10);
        while !done.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            done.exists(),
            "detached child did not finish: {}",
            fs::read_to_string(root.join("task.log")).unwrap_or_default()
        );
        let data: Value = serde_json::from_slice(&fs::read(&record).unwrap()).unwrap();
        assert_eq!(data[0], data[1]);
        assert_eq!(data[0], data[2]);
        assert_eq!(data[0], data[3]);
        assert_eq!(data[4], json!(true));
        assert_eq!(data[5], json!("1"));
        assert_eq!(data[6], json!("1"));
        fs::remove_dir_all(root).unwrap();
    }
}
pub fn model_tag(model: &str) -> String {
    let lower = model.to_lowercase();
    if matches!(lower.as_str(), "" | "agent" | "auto" | "copilot") {
        return String::new();
    }
    lower
        .strip_prefix("claude-")
        .unwrap_or(&lower)
        .replace(['.', '/', '-'], "")
}
pub fn model_alt(model: &str) -> String {
    let (regex, separator) = if DOT.is_match(model) {
        (&*DOT, '-')
    } else {
        (&*DASH, '.')
    };
    let Some(found) = regex.find(model) else {
        return String::new();
    };
    let mut out = model.to_owned();
    out.replace_range(found.start() + 1..found.start() + 2, &separator.to_string());
    out
}
pub fn copilot_refused(text: &str) -> bool {
    static QUOTA: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?i)quota_exceeded|exceeded your monthly quota|"errorType"\s*:\s*"quota"|"statusCode"\s*:\s*402"#).unwrap()
    });
    QUOTA.is_match(text)
}
pub fn model_unserved(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    ["is not available", "not found on provider", "(http 404)"]
        .iter()
        .any(|v| lower.contains(v))
}

impl Context {
    pub fn env_file(&self) -> String {
        if !self.get("TOKENECONOMIZER_ENV_FILE").is_empty() {
            return self.get("TOKENECONOMIZER_ENV_FILE").into();
        }
        let root = self.setting("REPO_ROOT", ".");
        let local = format!("{root}/.env");
        let parent = format!("{root}/../.env");
        if self.cwd.join(&local).is_file() || !self.cwd.join(&parent).is_file() {
            local
        } else {
            parent
        }
    }
    pub fn load_env(&mut self) {
        let file = self.env_file();
        if !self.cwd.join(&file).is_file() {
            return;
        }
        let text = match fs::read_to_string(self.cwd.join(&file)) {
            Ok(text) => text,
            Err(error) => {
                self.err(format!("{file}: {error}"));
                self.code = 1;
                return;
            }
        };
        for raw in text.split('\n') {
            let mut line = raw.trim_start();
            if line.is_empty() || line.starts_with('#') || !line.contains('=') {
                continue;
            }
            if let Some(rest) = line.strip_prefix("export ") {
                line = rest;
            }
            let (key, value) = line.split_once('=').unwrap();
            let key = key.trim_end();
            let mut value = value.trim();
            if value.len() >= 2
                && ((value.starts_with('"') && value.ends_with('"'))
                    || (value.starts_with('\'') && value.ends_with('\'')))
            {
                value = &value[1..value.len() - 1];
            }
            if key.is_empty() || value.is_empty() || !self.get(key).is_empty() {
                continue;
            }
            if !key
                .chars()
                .enumerate()
                .all(|(i, c)| c == '_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit()))
            {
                self.err(format!("export: `{key}={value}': not a valid identifier"));
                self.code = 1;
                continue;
            }
            self.export(key, value);
        }
    }
    pub fn dryrun(&mut self, what: &str) -> bool {
        if self.get("DRYRUN") != "1" {
            return false;
        }
        self.err(format!("[env] DRYRUN: skipping the billed {what} probe"));
        true
    }
    pub fn require_keys(&mut self, names: &[String]) {
        let missing: Vec<_> = names
            .iter()
            .filter(|name| self.get(name).is_empty())
            .collect();
        if missing.is_empty() {
            return;
        }
        self.err(format!(
            "!! missing credential(s): {}",
            missing
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        ));
        self.err(format!(
            "   set them in {} (copy .env.example)",
            self.env_file()
        ));
        self.stop(2);
    }
    pub fn model_for_agent(&mut self, agent: &str, model: &str) -> String {
        if !self.get("TE_STRICT_MODEL_ID").is_empty()
            || !model.starts_with("claude-")
            || (!DOT.is_match(model) && !DASH.is_match(model))
        {
            return model.into();
        }
        let dotted = agent == "copilot" && self.get("COPILOT_PROVIDER_BASE_URL").is_empty();
        let out = if (dotted && DOT.is_match(model)) || (!dotted && DASH.is_match(model)) {
            model.into()
        } else {
            model_alt(model)
        };
        if out != model {
            self.err(format!("[env] model id: '{model}' -> '{out}' (the spelling this agent's backend serves; the"));
            self.err(format!(
                "[env]           tree tag is {} either way)",
                model_tag(model)
            ));
        }
        out
    }
    pub fn drop_anthropic_endpoint(&mut self) {
        let keys: Vec<_> = ["ANTHROPIC_BASE_URL", "ANTHROPIC_AUTH_TOKEN"]
            .into_iter()
            .filter(|key| !self.get(key).is_empty())
            .collect();
        if keys.is_empty() {
            return;
        }
        self.err(format!(
            "[env] {} set but IGNORED: Claude Code bills the Anthropic API on",
            keys.join(" ")
        ));
        self.err(format!(
            "[env]   ANTHROPIC_API_KEY. Unset for this run; remove from {}.",
            self.env_file()
        ));
        for key in keys {
            self.unset(key);
        }
    }
    pub fn check_openai_base_url(&mut self) {
        let url = self.get("OPENAI_BASE_URL").to_owned();
        if url.is_empty() {
            return;
        }
        let host = url
            .split_once("://")
            .map_or(url.as_str(), |(_, s)| s)
            .split('/')
            .next()
            .unwrap()
            .split(':')
            .next()
            .unwrap()
            .to_ascii_lowercase();
        if url.starts_with("https://")
            && (host == "api.openai.com" || host.ends_with(".api.openai.com"))
        {
            return;
        }
        self.err(format!(
            "!! OPENAI_BASE_URL={url} is not an OpenAI API endpoint."
        ));
        self.err("   It is only for a regional OpenAI key (https://<region>.api.openai.com/v1);");
        self.err("   unset it for a global key.");
        self.stop(2);
    }
    pub fn check_routing(&mut self, agent: &str) {
        match agent {
            "claude" => self.drop_anthropic_endpoint(),
            "codex" => self.check_openai_base_url(),
            "copilot" => (),
            _ => {
                self.err(format!(
                    "!! unknown agent '{agent}' (claude | copilot | codex)"
                ));
                self.stop(2);
                return;
            }
        }
        if self.stopped {
            return;
        }
        if self.setting("RUNNER", "stub") == "stub" {
            self.err("[env] RUNNER=stub -- SYNTHETIC run: no agent, no grading, numbers are NOT a benchmark");
            self.err("[env]   set RUNNER=<module>:<factory> for a real run");
            return;
        }
        if agent == "copilot" {
            self.check_copilot_auth();
            return;
        }
        let key = if agent == "claude" {
            "ANTHROPIC_API_KEY"
        } else {
            "OPENAI_API_KEY"
        };
        self.require_keys(&[key.into()]);
        if self.stopped {
            return;
        }
        let suffix = self
            .get(key)
            .chars()
            .rev()
            .take(4)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<String>();
        self.out(if agent == "claude" {
            format!("[env] routing: Anthropic API (key ...{suffix})")
        } else {
            format!(
                "[env] routing: OpenAI API at {} (key ...{suffix})",
                self.setting("OPENAI_BASE_URL", "https://api.openai.com/v1")
            )
        });
    }
    pub fn refused_die(&mut self, out: &str, where_: &str) -> bool {
        if !copilot_refused(out) {
            return false;
        }
        self.err(format!("!! the Copilot subscription REFUSED the {where_} probe: monthly quota exceeded (HTTP 402)."));
        self.err("   Nothing aborts a lane on this, so launching now would fill cells with");
        self.err("   turns=1 / eff_tok=0 rows.");
        self.err("   Wait for the quota to reset, or run the Copilot CLI on your own provider:");
        self.err("     COPILOT_PROVIDER_TYPE=anthropic COPILOT_PROVIDER_BASE_URL=https://api.anthropic.com \\");
        self.err("     COPILOT_PROVIDER_API_KEY=...   (see README.md)");
        self.stop(2);
        true
    }
    fn copilot_bin(&self, auth: bool) -> Option<String> {
        let default = format!("{}/.local/bin/copilot", self.get("HOME"));
        let bin = self.setting("COPILOT_HOST_BIN", &default);
        if (auth && self.which(bin).is_some()) || self.executable(Path::new(bin)) {
            Some(bin.into())
        } else {
            self.which("copilot")
        }
    }
    pub fn check_copilot_auth(&mut self) {
        let Some(bin) = self.copilot_bin(true) else {
            self.err("!! copilot CLI not found.");
            self.err("   install: curl -fsSL https://gh.io/copilot-install | bash");
            self.err("   or set COPILOT_HOST_BIN to its path");
            self.stop(2);
            return;
        };
        if !self.get("COPILOT_PROVIDER_BASE_URL").is_empty() {
            self.out(format!(
                "[env] routing: Copilot CLI BYOK -> {} at {} (GitHub auth not used)",
                self.setting("COPILOT_PROVIDER_TYPE", "openai"),
                self.get("COPILOT_PROVIDER_BASE_URL")
            ));
            if self.get("COPILOT_PROVIDER_API_KEY").is_empty()
                && self.get("COPILOT_PROVIDER_BEARER_TOKEN").is_empty()
            {
                self.err("!! BYOK is active but neither COPILOT_PROVIDER_API_KEY nor COPILOT_PROVIDER_BEARER_TOKEN is set.");
                self.err("   a remote provider will refuse every task; unset COPILOT_PROVIDER_BASE_URL to go back to the subscription.");
            }
            return;
        }
        if ["COPILOT_GITHUB_TOKEN", "GH_TOKEN", "GITHUB_TOKEN"]
            .iter()
            .any(|key| !self.get(key).is_empty())
        {
            self.out("[env] routing: native Copilot (token from the environment)");
            return;
        }
        if self.dryrun("copilot auth") {
            self.out(
                "[env] routing: native Copilot (on-disk GitHub auth, not probed on a dry run)",
            );
            return;
        }
        let mut out = String::new();
        let mut ok = false;
        for attempt in 0..2 {
            out = self.probe(
                &[
                    bin.clone(),
                    "-p".into(),
                    "ok".into(),
                    "--no-color".into(),
                    "--output-format".into(),
                    "json".into(),
                    "--log-level".into(),
                    "none".into(),
                ],
                120.0,
            );
            if !out
                .to_ascii_lowercase()
                .contains("no authentication information found")
            {
                ok = true;
                break;
            }
            if attempt == 0 {
                self.err("[env] cold copilot token; retrying once to let it refresh...");
            }
        }
        if !ok {
            self.err("!! the copilot CLI is not signed in on this host.");
            self.err("   run 'copilot' then '/login' (OAuth device flow), or");
            self.err("   export COPILOT_GITHUB_TOKEN (or GH_TOKEN / GITHUB_TOKEN).");
            self.stop(2);
            return;
        }
        if self.refused_die(&out, "auth") {
            return;
        }
        self.out("[env] routing: native Copilot (on-disk GitHub auth, token warm)");
    }
    fn model_probe(&self, bin: &str, model: &str, effort: &str) -> String {
        let mut argv = vec![
            bin.into(),
            "-p".into(),
            "ok".into(),
            "--no-color".into(),
            "--log-level".into(),
            "none".into(),
            "--model".into(),
            model.into(),
        ];
        if !effort.is_empty() {
            argv.extend(["--effort".into(), effort.into()]);
        }
        self.probe(&argv, 120.0)
    }
    pub fn check_copilot_model(&mut self, model: &str, effort: &str) {
        if model.is_empty() || model == "auto" || self.dryrun("copilot model/effort") {
            return;
        }
        if self.get("TE_EFFORT_APPLIES_FOR") == model && !self.get("TE_EFFORT_APPLIES").is_empty() {
            return;
        }
        let Some(bin) = self.copilot_bin(false) else {
            return;
        };
        let mut model = model.to_owned();
        let mut out = self.model_probe(&bin, &model, effort);
        if self.refused_die(&out, "model") {
            return;
        }
        if model_unserved(&out)
            && model.starts_with("claude-")
            && self.get("TE_STRICT_MODEL_ID").is_empty()
        {
            let alt = model_alt(&model);
            if !alt.is_empty() {
                let out2 = self.model_probe(&bin, &alt, effort);
                if self.refused_die(&out2, "model") {
                    return;
                }
                if !model_unserved(&out2) {
                    self.err(format!("[env] !! copilot does not serve '{model}' but DOES serve '{alt}' -- the same model,"));
                    self.err(format!(
                        "[env] !! other separator. Using '{alt}' for this lane (tree tag {}",
                        model_tag(&model)
                    ));
                    self.err("[env] !! either way; TE_STRICT_MODEL_ID=1 to refuse instead).");
                    self.export("MODEL", alt.clone());
                    model = alt;
                    out = out2;
                }
            }
        }
        if model_unserved(&out) {
            if !self.get("COPILOT_PROVIDER_BASE_URL").is_empty() {
                self.err(format!(
                    "!! copilot model '{model}' is not served by the BYOK provider at {}.",
                    self.get("COPILOT_PROVIDER_BASE_URL")
                ));
                self.err(format!(
                    "   {}",
                    out.lines()
                        .find(|line| line.to_ascii_lowercase().contains("not found on provider"))
                        .unwrap_or("")
                ));
                self.err("   Under COPILOT_PROVIDER_TYPE=anthropic the provider takes Anthropic's own ids");
                self.err("   (claude-sonnet-5, claude-opus-4-8); pin one it serves.");
            } else {
                self.err(format!(
                    "!! copilot model '{model}' is not available on this subscription."
                ));
                self.err("   pick one it serves, or set COPILOT_MODEL=auto (which cannot apply an");
                self.err("   effort level).");
            }
            self.stop(2);
            return;
        }
        if out
            .to_ascii_lowercase()
            .contains("does not support reasoning effort")
        {
            self.err(format!(
                "[env] WARNING: model '{model}' does not accept --effort {effort}."
            ));
            self.err("[env]   running WITHOUT an effort level; rows record effort: null, so these");
            self.err("[env]   numbers are NOT comparable to effort-pinned runs.");
            self.export("TE_EFFORT_APPLIES", "0");
            self.export("TE_EFFORT_APPLIES_FOR", model);
            return;
        }
        self.export("TE_EFFORT_APPLIES", "1");
        self.export("TE_EFFORT_APPLIES_FOR", model.clone());
        self.out(format!(
            "[env] copilot model '{model}' serves; effort={} accepted",
            if effort.is_empty() { "<unset>" } else { effort }
        ));
    }
    pub fn detach(&mut self, tag: &str, argv: &[String]) {
        if self.setting("DETACH", "1") == "0" || !self.get("TE_DETACHED").is_empty() {
            return;
        }
        let stamp = Local::now().format("%Y%m%d-%H%M%S");
        let default = format!(
            "{}/logs/{tag}__{stamp}.log",
            self.setting("RESULTS_ROOT", self.cwd.to_str().unwrap_or("."))
        );
        let log = self.setting("TE_LOG", &default).to_owned();
        let path = self.cwd.join(&log);
        let parent = path.parent().unwrap_or(Path::new("."));
        if fs::create_dir_all(parent).is_err() {
            self.err(format!("!! cannot create log dir for {log}"));
            self.stop(2);
            return;
        }
        let pidfile = format!("{}.pid", log.strip_suffix(".log").unwrap_or(&log));
        self.export("TE_DETACHED", "1");
        self.export("PYTHONUNBUFFERED", "1");
        let Some(program) = argv.first() else {
            self.err("!! cannot detach an empty command");
            self.stop(2);
            return;
        };
        let mut command = std::process::Command::new(program);
        command
            .args(&argv[1..])
            .current_dir(&self.cwd)
            .env_clear()
            .envs(
                self.process_env()
                    .iter()
                    .map(|(k, v)| (k, v.as_str().unwrap())),
            )
            .stdin(std::process::Stdio::null());
        let spawn = (|| -> std::io::Result<()> {
            let file = fs::File::create(&path)?;
            command.stdout(file.try_clone()?).stderr(file);
            #[cfg(unix)]
            {
                use std::ffi::CString;
                use std::os::unix::process::CommandExt;
                let pidpath = CString::new(self.cwd.join(&pidfile).as_os_str().as_encoded_bytes())?;
                let mut hup: libc::sigaction = unsafe { std::mem::zeroed() };
                hup.sa_sigaction = libc::SIG_IGN;
                unsafe {
                    libc::sigemptyset(&mut hup.sa_mask);
                }
                // Write the session leader's PID before exec, using only async-signal-safe calls.
                unsafe {
                    command.pre_exec(move || {
                        if libc::setsid() < 0 {
                            return Err(std::io::Error::last_os_error());
                        }
                        if libc::sigaction(libc::SIGHUP, &hup, std::ptr::null_mut()) < 0 {
                            return Err(std::io::Error::last_os_error());
                        }
                        let fd = libc::open(
                            pidpath.as_ptr(),
                            libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC | libc::O_CLOEXEC,
                            0o666,
                        );
                        if fd < 0 {
                            return Err(std::io::Error::last_os_error());
                        }
                        let mut digits = [0_u8; 32];
                        let mut pos = 31;
                        digits[pos] = b'\n';
                        let mut pid = libc::getpid() as u32;
                        while pid > 0 {
                            pos -= 1;
                            digits[pos] = b'0' + (pid % 10) as u8;
                            pid /= 10;
                        }
                        let mut offset = pos;
                        while offset < digits.len() {
                            let n = libc::write(
                                fd,
                                digits[offset..].as_ptr().cast(),
                                digits.len() - offset,
                            );
                            if n <= 0 {
                                let error = if n == 0 {
                                    std::io::Error::from_raw_os_error(libc::EIO)
                                } else {
                                    std::io::Error::last_os_error()
                                };
                                if n < 0 && error.kind() == std::io::ErrorKind::Interrupted {
                                    continue;
                                }
                                libc::close(fd);
                                return Err(error);
                            }
                            offset += n as usize;
                        }
                        libc::close(fd);
                        Ok(())
                    });
                }
            }
            #[cfg(not(unix))]
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "detached sweep sessions require Unix",
            ));
            #[cfg(unix)]
            {
                command.spawn()?;
                Ok(())
            }
        })();
        if let Err(error) = spawn {
            self.err(format!("!! cannot detach {tag}: {error}"));
            self.stop(2);
            return;
        }
        self.out(format!(
            "[detach] {tag} started in the background (DETACH=0 to run in the foreground)"
        ));
        self.out(format!("[detach] log:    {log}"));
        self.out(format!("[detach] follow: tail -f {log}"));
        self.out(format!(
            "[detach] stop:   kill -TERM -$(cat {pidfile})     # leading '-' = whole group"
        ));
        self.stop(0);
    }
}
