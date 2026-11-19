pub mod generator;

use super::{context::LiveContext, hosts::session::Environment, process, sidecall};
use crate::value::{json_pretty, python_space, round_decimal, scalar_text, truthy};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::BufRead,
    path::{Path, PathBuf},
    sync::LazyLock,
};

pub const HEADER: &str = "# Repository map\nRanked key files and symbols (signatures only) of this repository. Consult it to find where things are before searching.\n\n";
pub const PROBE: &str = "import aider.repomap";
pub const REFRESH_TIMEOUT_S: f64 = 900.;

pub trait Runtime: sidecall::Runtime {
    fn spawn(&mut self, args: &[&str], log: &str) -> Result<Value, String>;
    fn run_generator(
        &mut self,
        argv: &[String],
        cwd: &Path,
        env: &Map<String, Value>,
        timeout: f64,
    ) -> std::io::Result<process::Output> {
        process::run_group(argv, cwd, env, timeout)
    }
}
pub struct RepoMap<'a> {
    pub ctx: &'a LiveContext,
    pub environment: &'a Environment,
    pub generator: &'a Path,
    pub interpreter: Option<&'a str>,
}
fn map_text(value: &Value) -> Result<&str, String> {
    if !truthy(value) {
        Ok("")
    } else {
        value
            .as_str()
            .ok_or_else(|| "repository map must be text".into())
    }
}
fn saved_python(state: &Value) -> Result<Option<&str>, String> {
    let value = &state["python"];
    if !truthy(value) {
        return Ok(None);
    }
    value
        .as_str()
        .map(Some)
        .ok_or_else(|| "repository-map interpreter must be text".into())
}
pub fn prompt_idents(text: &str) -> Vec<String> {
    static PATTERNS: LazyLock<[Regex; 3]> = LazyLock::new(|| {
        [
            Regex::new(r"`([A-Za-z_][A-Za-z0-9_.]*)`").unwrap(),
            Regex::new(r"([a-z_][a-z0-9_]*_[a-z0-9_]+)").unwrap(),
            Regex::new(r"([A-Z][a-zA-Z0-9]+[A-Z][a-zA-Z0-9]*)").unwrap(),
        ]
    });
    // Python's word boundaries include letters, numbers and underscore, excluding marks.
    static WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[\p{L}\p{N}_]$").unwrap());
    let word = |c: char| WORD.is_match(c.encode_utf8(&mut [0; 4]));
    let mut ids = BTreeSet::new();
    for (index, pattern) in PATTERNS.iter().enumerate() {
        for found in pattern.captures_iter(text) {
            let name = found.get(1).unwrap();
            if index != 0
                && (text[..name.start()].chars().next_back().is_some_and(word)
                    || text[name.end()..].chars().next().is_some_and(word))
            {
                continue;
            }
            if name.as_str().len() > 2 {
                ids.insert(name.as_str().rsplit('.').next().unwrap().to_string());
            }
        }
    }
    ids.into_iter().take(60).collect()
}
impl RepoMap<'_> {
    pub fn repo_dir(&self) -> &Path {
        self.ctx.store.base.parent().unwrap_or(Path::new(""))
    }
    pub fn state_path(&self) -> PathBuf {
        self.repo_dir().join("repomap.json")
    }
    pub fn cache_dir(&self) -> PathBuf {
        self.repo_dir().join("repomap-tags")
    }
    pub fn read_state(&self) -> Value {
        fs::read(self.state_path())
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .filter(Value::is_object)
            .unwrap_or(json!({}))
    }
    pub fn write_state(&self, doc: &Value) -> Result<(), String> {
        let path = self.state_path();
        fs::create_dir_all(self.repo_dir()).map_err(|e| e.to_string())?;
        let tmp = PathBuf::from(format!("{}.{}.tmp", path.display(), std::process::id()));
        fs::write(&tmp, json_pretty(doc, false)).map_err(|e| e.to_string())?;
        fs::rename(tmp, path).map_err(|e| e.to_string())
    }
    pub fn head_commit<R: Runtime>(&self, root: &Path, runtime: &mut R) -> String {
        runtime
            .run(
                &["git".into(), "rev-parse".into(), "HEAD".into()],
                &self.environment.path(root),
                &self.environment.env,
                None,
                10.,
            )
            .ok()
            .filter(|out| out.code == 0)
            .map(|out| out.stdout.trim_matches(python_space).to_string())
            .unwrap_or_default()
    }
    pub fn gen_env(&self) -> Map<String, Value> {
        let mut env = self.ctx.env.clone();
        env.remove("PYTHONPATH");
        env.insert("TE_SIDECALL".into(), json!("1"));
        env
    }
    pub fn has_aider<R: Runtime>(
        &self,
        python: &str,
        env: &Map<String, Value>,
        runtime: &mut R,
    ) -> bool {
        runtime
            .run(
                &[python.into(), "-c".into(), PROBE.into()],
                &self.environment.cwd,
                env,
                None,
                60.,
            )
            .is_ok_and(|out| out.code == 0)
    }
    pub fn aider_command_python<R: Runtime>(
        &self,
        runtime: &mut R,
    ) -> Result<Option<String>, String> {
        let Some(exe) = runtime.which("aider")? else {
            return Ok(None);
        };
        let Ok(file) = File::open(self.environment.path(Path::new(&exe))) else {
            return Ok(None);
        };
        let mut first = Vec::new();
        if std::io::BufReader::new(file)
            .read_until(b'\n', &mut first)
            .is_err()
        {
            return Ok(None);
        }
        let first = String::from_utf8_lossy(&first);
        let first = first.trim_matches(python_space);
        let candidate = first
            .strip_prefix("#!")
            .and_then(|s| s.split(python_space).find(|s| !s.is_empty()));
        Ok(candidate
            .filter(|p| {
                Path::new(p).file_name() != Some(std::ffi::OsStr::new("env"))
                    && self.environment.path(Path::new(p)).exists()
            })
            .map(String::from))
    }
    pub fn find_python<R: Runtime>(&self, runtime: &mut R) -> Result<Option<String>, String> {
        let env = self.gen_env();
        let named = self.ctx.config["repomap_python"].as_str().unwrap_or("");
        if !named.is_empty() {
            return Ok(self
                .has_aider(named, &env, runtime)
                .then(|| named.to_string()));
        }
        let state = self.read_state();
        let candidates = [
            saved_python(&state)?.map(String::from),
            self.aider_command_python(runtime)?,
            self.interpreter.map(String::from),
            runtime.which("python3")?,
        ];
        let mut seen = BTreeSet::new();
        for candidate in candidates.into_iter().flatten() {
            if !candidate.is_empty()
                && seen.insert(candidate.clone())
                && self.has_aider(&candidate, &env, runtime)
            {
                return Ok(Some(candidate));
            }
        }
        Ok(None)
    }
    pub fn generate<R: Runtime>(
        &self,
        python: &str,
        idents: &[String],
        timeout: f64,
        runtime: &mut R,
    ) -> Result<(String, String), String> {
        let workspace = sidecall::WorkDir::new("te-repomap-").map_err(|e| e.to_string())?;
        let cmd = [
            self.generator.to_string_lossy().into_owned(),
            "--python".into(),
            python.into(),
            self.ctx.repo.root.to_string_lossy().into_owned(),
            scalar_text(&self.ctx.config["repomap_tokens"]),
            idents.join(","),
            "--git".into(),
            "--cache".into(),
            self.cache_dir().to_string_lossy().into_owned(),
            "--workspace".into(),
            workspace.path().to_string_lossy().into_owned(),
        ];
        let out = match runtime.run_generator(&cmd, &self.ctx.repo.root, &self.gen_env(), timeout) {
            Ok(out) => out,
            Err(error) if error.kind() == std::io::ErrorKind::TimedOut => {
                return Ok(("".into(), format!("timed out after {timeout}s")));
            }
            Err(error) => return Ok(("".into(), error.to_string())),
        };
        if out.code != 0 {
            let text = out.stderr.trim_matches(python_space);
            let error = text
                .chars()
                .skip(text.chars().count().saturating_sub(300))
                .collect::<String>();
            return Ok((
                "".into(),
                if error.is_empty() {
                    format!("exit {}", out.code)
                } else {
                    error
                },
            ));
        }
        Ok((out.stdout, String::new()))
    }
    pub fn refresh<R: Runtime>(&self, runtime: &mut R) -> Result<Value, String> {
        let start = runtime.now()?;
        let commit = self.head_commit(&self.ctx.repo.root, runtime);
        let Some(python) = self.find_python(runtime)? else {
            let named = self.ctx.config["repomap_python"].as_str().unwrap_or("");
            let mut state = self.read_state();
            state["commit"] = json!(commit);
            state["updated"] = json!(runtime.now()?);
            state["error"] = json!(if named.is_empty() {
                "no Python with aider found (install aider-chat, or set repomap_python)".to_string()
            } else {
                format!("repomap_python {named} cannot import aider")
            });
            self.write_state(&state)?;
            return Ok(state);
        };
        let (text, error) = self.generate(&python, &[], REFRESH_TIMEOUT_S, runtime)?;
        let elapsed = round_decimal(runtime.now()? - start, 1);
        let mut state = json!({"commit":commit,"python":python,"map":text,"chars":text.chars().count(),"tokens_budget":self.ctx.config["repomap_tokens"],"secs":elapsed,"error":if error.is_empty() {Value::Null} else {json!(error)},"updated":runtime.now()?});
        if text.is_empty() && truthy(&self.read_state()["map"]) {
            let prior = self.read_state();
            state["map"] = prior["map"].clone();
            state["chars"] = json!(map_text(&prior["map"])?.chars().count());
            state["commit"] = prior["commit"].clone();
        }
        self.write_state(&state)?;
        Ok(state)
    }
    pub fn stale<R: Runtime>(&self, runtime: &mut R) -> bool {
        let state = self.read_state();
        !truthy(&state["map"])
            || state["commit"] != self.head_commit(&self.ctx.repo.root, runtime)
            || state["tokens_budget"] != self.ctx.config["repomap_tokens"]
    }
    pub fn spawn_refresh<R: Runtime>(&self, runtime: &mut R) -> Result<Value, String> {
        if self.ctx.env.get("TE_FOLD_SYNC").and_then(Value::as_str) == Some("1") {
            self.refresh(runtime)
        } else {
            runtime.spawn(&["repomap", "refresh"], "repomap.log")
        }
    }
    pub fn cached_block(&self) -> Result<String, String> {
        let state = self.read_state();
        let text = map_text(&state["map"])?;
        Ok(if text.trim_matches(python_space).is_empty() {
            String::new()
        } else {
            format!("{HEADER}{text}")
        })
    }
    pub fn for_prompt<R: Runtime>(
        &self,
        prompt: &str,
        runtime: &mut R,
    ) -> Result<(String, &'static str), String> {
        let idents = prompt_idents(prompt);
        let state = self.read_state();
        if !idents.is_empty()
            && let Some(python) = saved_python(&state)?
        {
            let (text, _) = self.generate(
                python,
                &idents,
                self.ctx.config["repomap_timeout"].as_f64().unwrap_or(0.),
                runtime,
            )?;
            if !text.trim_matches(python_space).is_empty() {
                return Ok((format!("{HEADER}{text}"), "prompt"));
            }
        }
        let text = self.cached_block()?;
        let how = if text.is_empty() { "none" } else { "cached" };
        Ok((text, how))
    }
}
