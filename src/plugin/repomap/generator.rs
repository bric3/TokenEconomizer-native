use super::super::{paths, process};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::{
    collections::BTreeSet,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::LazyLock,
};

pub const SKIP: &[&str] = &[
    "/.git",
    "/__pycache__",
    "/.tox",
    "/build",
    "/dist",
    "/.eggs",
    ".egg-info",
    "/doc/",
    "/docs/",
    "/examples/",
    "/benchmarks/",
    "/tutorial/",
    "/asv_benchmarks/",
];
pub const VENDORED: &[&str] = &[
    "/vendor/",
    "/vendors/",
    "/third_party/",
    "/third-party/",
    "/node_modules/",
    "/bower_components/",
    "/static/",
    "/assets/",
];
pub const MINIFIED: &[&str] = &[".min.js", ".min.css", ".bundle.js", "-min.js"];
pub const MAX_BYTES: u64 = 256 * 1024;
const SDK: &str = include_str!("aider_sdk.py");

pub struct Options {
    pub python: String,
    pub repo: PathBuf,
    pub budget: String,
    pub idents: BTreeSet<String>,
    pub git: bool,
    pub cache: Option<PathBuf>,
    pub workspace: Option<PathBuf>,
}
fn option(args: &mut Vec<String>, name: &str) -> Result<Option<String>, String> {
    let Some(index) = args.iter().position(|a| a == name) else {
        return Ok(None);
    };
    if index + 1 >= args.len() {
        return Err(format!("{name} requires a value"));
    }
    let value = args.remove(index + 1);
    args.remove(index);
    Ok(Some(value))
}
fn integer(text: &str) -> Result<String, String> {
    static INTEGER: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^[+-]?\p{Nd}+(?:_\p{Nd}+)*$").unwrap());
    static DIGIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\p{Nd}$").unwrap());
    let text = text.trim();
    if !INTEGER.is_match(text) {
        return Err(format!("invalid map token budget: {text}"));
    }
    let mut normalized = String::new();
    for c in text.chars().filter(|c| *c != '_') {
        if c.is_ascii() {
            normalized.push(c);
            continue;
        }
        let code = c as u32;
        let mut start = code;
        // Decimal digit blocks contain consecutive groups of ten values.
        while char::from_u32(start - 1).is_some_and(|c| DIGIT.is_match(c.encode_utf8(&mut [0; 4])))
        {
            start -= 1;
        }
        normalized.push(char::from(b'0' + ((code - start) % 10) as u8));
    }
    Ok(normalized)
}
impl Options {
    pub fn parse(args: &[String], cwd: &Path) -> Result<Self, String> {
        let mut args = args.to_vec();
        let python = option(&mut args, "--python")?.unwrap_or_else(|| "python3".into());
        let workspace = option(&mut args, "--workspace")?.map(|p| paths::normalize(&cwd.join(p)));
        let git = args.iter().any(|a| a == "--git");
        let cache = args
            .iter()
            .position(|a| a == "--cache")
            .map(|index| args.get(index + 1).ok_or("--cache requires a directory"))
            .transpose()?
            .filter(|s| !s.is_empty())
            .map(|p| paths::normalize(&cwd.join(p)));
        let pos: Vec<_> = args
            .iter()
            .enumerate()
            .filter(|(i, a)| !a.starts_with("--") && (*i == 0 || args[*i - 1] != "--cache"))
            .map(|(_, a)| a)
            .collect();
        let repo = paths::normalize(&cwd.join(pos.first().ok_or("expected repository directory")?));
        let budget = integer(pos.get(1).map(|s| s.as_str()).unwrap_or("2048"))?;
        let idents = pos
            .get(2)
            .filter(|s| !s.is_empty())
            .map(|s| s.split(',').map(String::from).collect())
            .unwrap_or_default();
        Ok(Self {
            python,
            repo,
            budget,
            idents,
            git,
            cache,
            workspace,
        })
    }
    pub fn files(&self, env: &Map<String, Value>) -> io::Result<Vec<PathBuf>> {
        if self.git {
            git_files(&self.repo, env)
        } else {
            let mut files = Vec::new();
            walk(&self.repo, &mut files);
            Ok(files)
        }
    }
    pub fn run(&self, env: &Map<String, Value>, cwd: &Path) -> io::Result<process::Output> {
        let files = self.files(env)?;
        let request = json!({"repo":self.repo,"budget":self.budget,"idents":self.idents,"cache":self.cache,"files":files}).to_string();
        let owned = if self.workspace.is_none() {
            Some(super::super::sidecall::WorkDir::new("te-repomap-")?)
        } else {
            None
        };
        let workspace = self
            .workspace
            .as_deref()
            .unwrap_or_else(|| owned.as_ref().unwrap().path());
        let script = workspace.join("aider_sdk.py");
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&script)?
            .write_all(SDK.as_bytes())?;
        let mut cmd = vec![
            self.python.clone(),
            script.to_string_lossy().into_owned(),
            self.repo.to_string_lossy().into_owned(),
            self.budget.clone(),
            self.idents.iter().cloned().collect::<Vec<_>>().join(","),
        ];
        if self.git {
            cmd.push("--git".into());
        }
        if let Some(cache) = &self.cache {
            cmd.extend(["--cache".into(), cache.to_string_lossy().into_owned()]);
        }
        process::run_unbounded(&cmd, cwd, env, Some(&request))
    }
}
pub fn git_files(repo: &Path, env: &Map<String, Value>) -> io::Result<Vec<PathBuf>> {
    let out = process::run_unbounded(
        &["git".into(), "ls-files".into(), "-z".into()],
        repo,
        env,
        None,
    )?;
    let mut files = Vec::new();
    for name in out.stdout.split('\0') {
        let path = format!("/{name}");
        if name.is_empty()
            || SKIP.iter().chain(VENDORED).any(|s| path.contains(s))
            || MINIFIED.iter().any(|s| name.ends_with(s))
        {
            continue;
        }
        let full = repo.join(name);
        if fs::metadata(&full).is_ok_and(|m| m.len() <= MAX_BYTES) {
            files.push(full);
        }
    }
    Ok(files)
}
fn walk(dir: &Path, files: &mut Vec<PathBuf>) {
    if SKIP.iter().any(|s| dir.to_string_lossy().contains(s)) {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut dirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let kind = entry.file_type().ok();
        let symlink = kind.as_ref().is_some_and(|k| k.is_symlink());
        let is_dir = kind.as_ref().is_some_and(|k| k.is_dir()) || symlink && path.is_dir();
        if is_dir {
            if !symlink {
                dirs.push(path);
            }
        } else if entry.file_name().to_string_lossy().ends_with(".py") {
            files.push(path);
        }
    }
    for dir in dirs {
        walk(&dir, files);
    }
}
