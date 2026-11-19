//! Launch configuration and preflights shared by the sweep entry points.
mod environment;
pub mod fleet;

use crate::plugin::{paths, process};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

pub const FUNCTIONS: &[&str] = &[
    "te_env_file",
    "te_load_env",
    "te_dryrun_skips_billed",
    "te_require_key",
    "te_detach",
    "te_tree_name",
    "te_model_tag",
    "te_model_alt_spelling",
    "te_model_for_agent",
    "te_drop_anthropic_endpoint",
    "te_check_openai_base_url",
    "te_check_routing",
    "te_copilot_refused",
    "te_copilot_refused_die",
    "te_check_copilot_auth",
    "te_copilot_model_unserved",
    "te_check_copilot_model",
    "te_fleet_cgroup",
    "te_fleet_cpu_cap",
    "te_fleet_cpu_report",
];

#[derive(Serialize)]
pub struct ShellOutput {
    pub stdout: String,
    pub stderr: String,
    pub code: i32,
    pub stop: bool,
    pub changed: Map<String, Value>,
}

#[derive(Deserialize)]
pub struct Context {
    pub env: Map<String, Value>,
    pub exported: BTreeSet<String>,
    pub cwd: PathBuf,
    #[serde(skip)]
    pub stdout: String,
    #[serde(skip)]
    pub stderr: String,
    #[serde(skip)]
    pub code: i32,
    #[serde(skip)]
    pub stopped: bool,
}
impl Context {
    pub fn get(&self, key: &str) -> &str {
        self.env.get(key).and_then(Value::as_str).unwrap_or("")
    }
    pub fn setting<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        let value = self.get(key);
        if value.is_empty() { default } else { value }
    }
    fn export(&mut self, key: &str, value: impl Into<String>) {
        self.env.insert(key.into(), json!(value.into()));
        self.exported.insert(key.into());
    }
    fn unset(&mut self, key: &str) {
        self.env.remove(key);
        self.exported.remove(key);
    }
    fn out(&mut self, text: impl AsRef<str>) {
        self.stdout.push_str(text.as_ref());
        self.stdout.push('\n');
    }
    fn err(&mut self, text: impl AsRef<str>) {
        self.stderr.push_str(text.as_ref());
        self.stderr.push('\n');
    }
    fn stop(&mut self, code: i32) {
        self.code = code;
        self.stopped = true;
    }
    fn process_env(&self) -> Map<String, Value> {
        self.env
            .iter()
            .filter(|(key, _)| self.exported.contains(*key))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
    fn executable(&self, path: &Path) -> bool {
        self.cwd.join(path).is_file() && paths::executable(&self.cwd.join(path)).unwrap_or(false)
    }
    fn which(&self, name: &str) -> Option<String> {
        let path = Path::new(name);
        if path.is_absolute() || name.contains(std::path::MAIN_SEPARATOR) {
            return self.executable(path).then(|| name.into());
        }
        std::env::split_paths(self.get("PATH"))
            .map(|dir| dir.join(name))
            .find(|path| self.executable(path))
            .map(|path| path.to_string_lossy().into_owned())
    }
    fn probe(&self, argv: &[String], seconds: f64) -> String {
        match process::run_group(argv, &self.cwd, &self.process_env(), seconds) {
            Ok(output) => (output.stdout + &output.stderr)
                .trim_end_matches('\n')
                .to_owned(),
            Err(error) if error.kind() == std::io::ErrorKind::TimedOut => String::new(),
            Err(error) => error.to_string(),
        }
    }
    pub fn function(mut self, name: &str, args: &[String]) -> Result<ShellOutput, String> {
        if !self.env.values().all(Value::is_string) {
            return Err("launch variables must be strings".into());
        }
        let before = self.env.clone();
        let exported_before = self.exported.clone();
        let a = |index: usize| args.get(index).map(String::as_str).unwrap_or("");
        match name {
            "te_env_file" => self.out(self.env_file()),
            "te_load_env" => self.load_env(),
            "te_dryrun_skips_billed" => self.code = if self.dryrun(a(0)) { 0 } else { 1 },
            "te_require_key" => self.require_keys(args),
            "te_detach" => {
                let mut command = vec![self.which("bash").unwrap_or("bash".into())];
                command.extend_from_slice(args.get(1..).unwrap_or(&[]));
                self.detach(a(0), &command);
            }
            "te_tree_name" => self.out(environment::tree_name(a(0), a(1), a(2))),
            "te_model_tag" => self.out(environment::model_tag(a(0))),
            "te_model_alt_spelling" => {
                let alt = environment::model_alt(a(0));
                if !alt.is_empty() {
                    self.out(alt);
                }
            }
            "te_model_for_agent" => {
                let model = self.model_for_agent(a(0), a(1));
                self.out(model);
            }
            "te_drop_anthropic_endpoint" => self.drop_anthropic_endpoint(),
            "te_check_openai_base_url" => self.check_openai_base_url(),
            "te_check_routing" => self.check_routing(a(0)),
            "te_copilot_refused" => self.code = i32::from(!environment::copilot_refused(a(0))),
            "te_copilot_refused_die" => {
                self.refused_die(a(0), a(1));
            }
            "te_check_copilot_auth" => self.check_copilot_auth(),
            "te_copilot_model_unserved" => {
                self.code = i32::from(!environment::model_unserved(a(0)))
            }
            "te_check_copilot_model" => self.check_copilot_model(a(0), a(1)),
            "te_fleet_cgroup" => match self.fleet_cgroup() {
                Some((path, from_scope)) => {
                    self.stdout.push_str(&path.to_string_lossy());
                    if from_scope {
                        self.stdout.push('\n');
                    }
                }
                None => self.code = 1,
            },
            "te_fleet_cpu_cap" => self.fleet_cap(args.first().map(String::as_str)),
            "te_fleet_cpu_report" => self.fleet_report(),
            _ => return Err(format!("unknown launch function: {name}")),
        }
        let mut changed = Map::new();
        for (key, value) in &self.env {
            if before.get(key) != Some(value)
                || (!exported_before.contains(key) && self.exported.contains(key))
            {
                changed.insert(key.clone(), value.clone());
            }
        }
        for key in before.keys() {
            if !self.env.contains_key(key) {
                changed.insert(key.clone(), Value::Null);
            }
        }
        Ok(ShellOutput {
            stdout: self.stdout,
            stderr: self.stderr,
            code: self.code,
            stop: self.stopped,
            changed,
        })
    }
}
