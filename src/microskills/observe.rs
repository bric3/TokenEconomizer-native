use std::{collections::BTreeSet, path::Path, sync::LazyLock};

use fancy_regex::Regex as FancyRegex;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::{
    lifecycle,
    trace::calls::{call_cwds, load_trajectory, strip_rtk_prefix},
    value::truthy,
};

fn words(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|s| (*s).into()).collect()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Vocabulary {
    pub name: String,
    pub interpreters: BTreeSet<String>,
    pub canonical_interpreter: String,
    pub runner_heads: BTreeSet<String>,
    pub runner_scripts: Vec<String>,
    pub runner_modules: BTreeSet<String>,
    pub package_tools: BTreeSet<String>,
    pub fetch_heads: BTreeSet<String>,
    pub env_probe_heads: BTreeSet<String>,
    pub task_heads: BTreeSet<String>,
    pub build_heads: BTreeSet<String>,
    pub generic: bool,
}

impl Default for Vocabulary {
    fn default() -> Self {
        Self {
            name: "python-git".into(),
            interpreters: words(&["python", "python2", "python3", "pypy", "pypy3"]),
            canonical_interpreter: "python".into(),
            runner_heads: words(&["pytest", "py.test", "tox", "nox", "nosetests", "trial"]),
            runner_scripts: ["runtests.py", "bin/test", "bin/doctest", "manage.py"]
                .map(String::from)
                .into(),
            runner_modules: words(&["pytest", "unittest", "nose", "django", "tox", "nox"]),
            package_tools: words(&[
                "pip", "pip3", "conda", "mamba", "poetry", "pipenv", "uv", "apt", "apt-get",
            ]),
            fetch_heads: words(&["curl", "wget"]),
            env_probe_heads: words(&["which", "command", "type", "whereis"]),
            task_heads: words(&["make", "npm", "yarn", "pnpm", "cargo", "go"]),
            build_heads: BTreeSet::new(),
            generic: false,
        }
    }
}

impl Vocabulary {
    pub fn node_web() -> Self {
        Self {
            name: "node-web".into(),
            interpreters: words(&["node", "nodejs", "deno", "bun", "tsx", "ts-node"]),
            canonical_interpreter: "node".into(),
            runner_heads: words(&[
                "jest",
                "vitest",
                "mocha",
                "playwright",
                "cypress",
                "ava",
                "karma",
            ]),
            runner_scripts: Vec::new(),
            runner_modules: BTreeSet::new(),
            package_tools: words(&["npm", "yarn", "pnpm", "corepack", "apt", "apt-get"]),
            ..Self::default()
        }
    }

    pub fn generic() -> Self {
        let mut vocabulary = Self::default();
        let node = Self::node_web();
        vocabulary.name = "generic".into();
        vocabulary.interpreters.extend(node.interpreters);
        vocabulary.interpreters.extend(words(&[
            "ruby", "perl", "php", "Rscript", "julia", "lua", "elixir",
        ]));
        vocabulary.runner_heads.extend(node.runner_heads);
        vocabulary
            .runner_heads
            .extend(words(&["rspec", "phpunit", "ctest", "bats"]));
        vocabulary.package_tools.extend(node.package_tools);
        vocabulary.package_tools.extend(words(&[
            "gem", "bundle", "composer", "cargo", "go", "mvn", "gradle", "dotnet", "brew",
            "rustup", "pyenv", "nvm", "asdf",
        ]));
        vocabulary.task_heads.extend(words(&[
            "mvn", "gradle", "gradlew", "dotnet", "mix", "swift", "rake",
        ]));
        vocabulary.build_heads = words(&[
            "make",
            "cmake",
            "ninja",
            "bazel",
            "tsc",
            "eslint",
            "prettier",
            "ruff",
            "flake8",
            "pylint",
            "mypy",
            "pyright",
            "black",
            "isort",
            "golangci-lint",
            "gcc",
            "g++",
            "clang",
            "javac",
            "webpack",
            "vite",
            "pre-commit",
            "shellcheck",
        ]);
        vocabulary.generic = true;
        vocabulary
    }
}

const BUILD_SUBS: &[&str] = &[
    "build",
    "check",
    "compile",
    "lint",
    "vet",
    "typecheck",
    "type-check",
    "clippy",
    "package",
    "verify",
    "fmt",
    "format",
];
pub const NETWORK_ERRORS: &[&str] = &["connection_refused", "name_resolution", "unreachable"];
pub const FORM_ERRORS: &[&str] = &[
    "module_missing",
    "command_missing",
    "script_missing",
    "usage_error",
    "permission_denied",
];
const WORK_INTENTS: &[&str] = &[
    "run_interpreter",
    "run_tests",
    "run_build",
    "package_tool",
    "fetch_remote",
];

static ACTIVATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:source|\.)\s+\S*activate\s+([\w.-]+)\s*$").unwrap());
static CD: LazyLock<FancyRegex> =
    LazyLock::new(|| FancyRegex::new(r#"^cd\s+(['"]?)([^\s;&|'"]+)\1\s*$"#).unwrap());
static CD_FAILED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\bcd: [^\n]*: (?:No such file or directory|Not a directory)").unwrap()
});
static CONDA_ACTIVATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:conda|mamba)\s+activate\s+([\w.-]+)\s*$").unwrap());
static CONDA_RUN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:conda|mamba)\s+run\s+(?:--no-capture-output\s+|--live-stream\s+)*(?:-n|--name)\s+([\w.-]+)\s+").unwrap()
});
static TOOL_RUN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(poetry|pipenv|uv|npx|bundle|yarn|pnpm)\s+(?:run|exec)\s+").unwrap()
});
static NPX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^npx\s+(?:(?:--yes|-y|--no-install|--no)\s+)*").unwrap());
static TOOL_LAUNCH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(uvx|uv\s+tool\s+run|pipx\s+run)\s+").unwrap());
static INTERP_VERSIONED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:python|node)\d(?:\.\d+)?$").unwrap());
static ENV_ASSIGN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^(?:[A-Za-z_][A-Za-z0-9_]*=(?:"[^"]*"|'[^']*'|\S+)\s+)+"#).unwrap()
});
static ASSIGN_TOKEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z_][A-Za-z0-9_]*=").unwrap());
static WRAP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)^/bin/(?:ba)?sh\s+-l?c\s+(.*)$").unwrap());
static HEREDOC: LazyLock<FancyRegex> = LazyLock::new(|| {
    FancyRegex::new(r#"(?s)(?<!<)<<(?!<)-?\s*['"]?(\w+)['"]?[^\n]*\n.*?\n\s*\1\b"#).unwrap()
});
static URL_HOST: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"https?://([^/\s'"`]+)"#).unwrap());

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Observation {
    pub call: usize,
    pub intent: String,
    pub form: String,
    pub head: String,
    pub prefix: String,
    pub hosts: Vec<String>,
    pub probe: Option<String>,
    pub outcome: Option<String>,
    pub segment: String,
    pub labels: Vec<String>,
    pub label_err: bool,
    pub loaded: bool,
    pub tests_ran: bool,
    pub env: bool,
    pub not_found: bool,
}

pub fn unwrap(command: &str) -> String {
    let Some(m) = WRAP.captures(command.trim()) else {
        return command.into();
    };
    let inner = m[1].trim();
    if inner.len() >= 2
        && (inner.starts_with('"') && inner.ends_with('"')
            || inner.starts_with('\'') && inner.ends_with('\''))
    {
        let stripped = &inner[1..inner.len() - 1];
        if inner.starts_with('"') {
            stripped
                .replace("\\\"", "\"")
                .replace("\\$", "$")
                .replace("\\\\", "\\")
        } else {
            stripped.into()
        }
    } else {
        inner.into()
    }
}

pub fn segments(command: &str) -> Result<Vec<String>, String> {
    let mut text = String::new();
    let mut start = 0;
    for m in HEREDOC.find_iter(command) {
        let m = m.map_err(|e| e.to_string())?;
        text.push_str(&command[start..m.start()]);
        text.push_str(m.as_str().split('\n').next().unwrap_or(""));
        text.push_str(" <heredoc>");
        start = m.end();
    }
    text.push_str(&command[start..]);
    let chars: Vec<_> = text.chars().collect();
    let mut out = Vec::new();
    let mut buffer = String::new();
    let mut quote = None;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if let Some(q) = quote {
            buffer.push(c);
            if c == q {
                quote = None;
            } else if c == '\\' && i + 1 < chars.len() {
                i += 1;
                buffer.push(chars[i]);
            }
        } else if ['\'', '"'].contains(&c) {
            quote = Some(c);
            buffer.push(c);
        } else if ['\n', ';', '|'].contains(&c) || c == '&' && chars.get(i + 1) == Some(&'&') {
            out.push(std::mem::take(&mut buffer));
            if chars.get(i + 1) == Some(&c) && ['&', '|'].contains(&c) {
                i += 1;
            }
        } else {
            buffer.push(c);
        }
        i += 1;
    }
    out.push(buffer);
    Ok(out
        .into_iter()
        .map(|s| s.trim().trim_matches(['(', ')']).trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect())
}

pub fn posix_normpath(path: &str) -> String {
    let root = if path.starts_with("//") && !path.starts_with("///") {
        "//"
    } else if path.starts_with('/') {
        "/"
    } else {
        ""
    };
    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." if parts.last().is_some_and(|p| *p != "..") => {
                parts.pop();
            }
            ".." if !root.is_empty() => {}
            _ => parts.push(part),
        }
    }
    let path = root.to_owned() + &parts.join("/");
    if path.is_empty() { ".".into() } else { path }
}

fn posix_join(left: &str, right: &str) -> String {
    if right.starts_with('/') || left.is_empty() {
        right.into()
    } else {
        format!("{left}/{right}")
    }
}

pub fn shell_path(path: &str) -> String {
    static DRIVE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^/([A-Za-z])(?:/|$)|^([A-Za-z]):(?:/|$)").unwrap());
    if !cfg!(windows) {
        return path.into();
    }
    let path = path.replace('\\', "/");
    if let Some(m) = DRIVE.captures(&path) {
        let letter = m.get(1).or_else(|| m.get(2)).unwrap();
        format!("{}:{}", letter.as_str().to_lowercase(), &path[2..])
    } else {
        path
    }
}

pub fn shell_isabs(path: &str) -> bool {
    path.starts_with('/')
        || cfg!(windows)
            && path.as_bytes().get(1) == Some(&b':')
            && path.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
            && (path.len() == 2 || path.as_bytes().get(2) == Some(&b'/'))
}

pub fn repository_top(start: &str) -> Option<String> {
    let path = Path::new(start);
    if start.is_empty() || !path.is_absolute() || !path.is_dir() {
        return None;
    }
    let mut directory = std::path::PathBuf::new();
    for component in path.components() {
        if component == std::path::Component::ParentDir {
            directory.pop();
        } else {
            directory.push(component);
        }
    }
    loop {
        if directory.join(".git").exists() {
            return Some(directory.to_string_lossy().into());
        }
        if !directory.pop() {
            return None;
        }
    }
}

pub fn cd_target(
    current: &str,
    target: &str,
    root: Option<&str>,
    top: Option<&str>,
) -> Option<String> {
    if target.is_empty() || target.starts_with(['~', '$', '-']) {
        return None;
    }
    let target = shell_path(target);
    let current = shell_path(if current.is_empty() { "." } else { current });
    let relative = if shell_isabs(&target) {
        let root = root.filter(|s| !s.is_empty())?;
        let target = posix_normpath(&target);
        let root = posix_normpath(&shell_path(root));
        let target: Vec<_> = target
            .split('/')
            .filter(|s| !s.is_empty() && *s != ".")
            .collect();
        let root: Vec<_> = root
            .split('/')
            .filter(|s| !s.is_empty() && *s != ".")
            .collect();
        let common = target.iter().zip(&root).take_while(|(a, b)| a == b).count();
        let mut parts = vec![".."; root.len() - common];
        parts.extend_from_slice(&target[common..]);
        if parts.is_empty() {
            ".".into()
        } else {
            parts.join("/")
        }
    } else {
        posix_normpath(&posix_join(&current, &target))
    };
    if relative == "." {
        return Some(String::new());
    }
    if relative == ".." || relative.starts_with("../") {
        let (root, top) = (
            root.filter(|s| !s.is_empty())?,
            top.filter(|s| !s.is_empty())?,
        );
        let at = posix_normpath(&posix_join(&shell_path(root), &relative));
        return (at == posix_normpath(&shell_path(top))).then(String::new);
    }
    Some(relative)
}

pub fn classify_command(
    command: &str,
    call: usize,
    vocab: &Vocabulary,
    cwd: &str,
    root: Option<&str>,
    top: Option<&str>,
) -> Result<Vec<Observation>, String> {
    let mut observations = Vec::new();
    let mut carried = String::new();
    let mut here = Some(cwd.to_owned());
    let mut cd = if cwd.is_empty() {
        String::new()
    } else {
        format!("cd {cwd} &&")
    };
    for segment in segments(&unwrap(command))? {
        let typed = segment.trim();
        let stripped = ENV_ASSIGN.replace(typed, "");
        let stripped = stripped.trim();
        let mut assigned = stripped != typed;
        let unwrapped = strip_rtk_prefix(stripped);
        let unwrapped = unwrapped.trim();
        let stripped = ENV_ASSIGN.replace(unwrapped, "");
        let segment = stripped.trim();
        assigned |= segment != unwrapped;
        if segment.is_empty() {
            continue;
        }
        if let Some(m) = CD.captures(segment).map_err(|e| e.to_string())? {
            let there = here
                .as_deref()
                .and_then(|here| cd_target(here, &m[2], root, top));
            cd = match &there {
                None => format!("cd {} &&", &m[2]),
                Some(path) if !path.is_empty() => format!("cd {path} &&"),
                _ => String::new(),
            };
            here = there;
            continue;
        }
        if let Some(m) = ACTIVATE
            .captures(segment)
            .or_else(|| CONDA_ACTIVATE.captures(segment))
        {
            carried = format!(
                "{} activate {} &&",
                if segment.starts_with("conda") || segment.starts_with("mamba") {
                    "conda"
                } else {
                    "source"
                },
                &m[1]
            );
            continue;
        }
        let mut prefix = [&cd, &carried]
            .into_iter()
            .filter(|s| !s.is_empty())
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" ");
        let mut body = segment;
        if let Some(m) = CONDA_RUN.captures(body) {
            prefix = format!("{prefix} conda run -n {}", &m[1])
                .trim_start()
                .into();
            body = &body[m.get(0).unwrap().end()..];
        }
        if let Some(m) = TOOL_RUN.captures(body) {
            prefix = format!("{prefix} {} run", &m[1]).trim_start().into();
            body = &body[m.get(0).unwrap().end()..];
        } else if let Some(m) = NPX.find(body) {
            prefix = format!("{prefix} npx").trim_start().into();
            body = &body[m.end()..];
        } else if vocab.generic
            && let Some(m) = TOOL_LAUNCH.captures(body)
        {
            prefix = format!(
                "{prefix} {}",
                if m[1].starts_with("pipx") {
                    "pipx run"
                } else {
                    "uvx"
                }
            )
            .trim_start()
            .into();
            body = &body[m.get(0).unwrap().end()..];
        }
        let mut tokens: Vec<_> = body.split_whitespace().collect();
        while let Some(head) = tokens.first() {
            let n = if ASSIGN_TOKEN.is_match(head) {
                assigned = true;
                1
            } else if ["sudo", "env", "time", "exec", "nice", "nohup", "timeout"].contains(head) {
                if *head == "timeout" { 2 } else { 1 }
            } else {
                break;
            };
            tokens.drain(..n.min(tokens.len()));
        }
        if let Some((head, rest)) = tokens.split_first()
            && let Some(mut observation) = classify(head, rest, &prefix, segment, call, vocab)
        {
            observation.env = assigned;
            observations.push(observation);
        }
    }
    Ok(observations)
}

pub fn runner_script(rest: &[&str], vocab: &Vocabulary) -> Option<String> {
    for token in rest {
        if token.starts_with('-') {
            continue;
        }
        for script in &vocab.runner_scripts {
            if *token == script || token.ends_with(&format!("/{script}")) || token.ends_with(script)
            {
                return Some(
                    if token.starts_with("./") {
                        token.trim_start_matches(['.', '/'])
                    } else {
                        token
                    }
                    .to_string(),
                );
            }
        }
        return None;
    }
    None
}

fn classify(
    head: &str,
    rest: &[&str],
    prefix: &str,
    segment: &str,
    call: usize,
    vocab: &Vocabulary,
) -> Option<Observation> {
    let base = head.rsplit('/').next().unwrap_or("");
    let observation = |intent: &str, form: String| Observation {
        call,
        intent: intent.into(),
        form: if prefix.is_empty() {
            form
        } else {
            format!("{prefix} {form}").trim().into()
        },
        head: head.into(),
        prefix: prefix.into(),
        segment: segment.into(),
        ..Observation::default()
    };
    if vocab.interpreters.contains(base) || INTERP_VERSIONED.is_match(base) {
        if !rest.is_empty()
            && rest
                .iter()
                .all(|s| ["--version", "-V", "-VV", "-v"].contains(s))
        {
            return Some(observation("probe_env", head.into()));
        }
        if let Some(i) = rest.iter().position(|s| *s == "-m") {
            let module = rest.get(i + 1).copied().unwrap_or("");
            if vocab.runner_modules.contains(module) {
                return Some(observation("run_tests", format!("{head} -m {module}")));
            }
            if vocab.generic && vocab.build_heads.contains(module) {
                return Some(observation("run_build", format!("{head} -m {module}")));
            }
        }
        if let Some(script) = runner_script(rest, vocab)
            && (script != "manage.py" || rest.contains(&"test"))
        {
            return Some(observation("run_tests", format!("{head} {script}")));
        }
        return Some(observation("run_interpreter", head.into()));
    }
    if vocab.runner_heads.contains(base) {
        return Some(observation("run_tests", head.into()));
    }
    for script in &vocab.runner_scripts {
        if head == script || head.ends_with(&format!("/{script}")) || head == format!("./{script}")
        {
            if script == "manage.py" && !rest.contains(&"test") {
                break;
            }
            return Some(observation(
                "run_tests",
                format!(
                    "{} {}",
                    vocab.canonical_interpreter,
                    head.trim_start_matches(['.', '/'])
                ),
            ));
        }
    }
    if vocab.task_heads.contains(base) && rest.first() == Some(&"test") {
        return Some(observation("run_tests", format!("{base} test")));
    }
    if vocab.generic {
        if ["npm", "yarn", "pnpm", "bun"].contains(&base) && rest.len() > 1 && rest[0] == "run" {
            let script = rest[1];
            let kind = script.split(':').next().unwrap_or("");
            if kind == "test" {
                return Some(observation("run_tests", format!("{base} run {script}")));
            }
            if BUILD_SUBS.contains(&kind) {
                return Some(observation("run_build", format!("{base} run {script}")));
            }
        }
        if vocab.task_heads.contains(base) && rest.first().is_some_and(|s| BUILD_SUBS.contains(s)) {
            return Some(observation("run_build", format!("{base} {}", rest[0])));
        }
        if vocab.build_heads.contains(base) {
            return Some(observation("run_build", base.into()));
        }
    }
    if vocab.env_probe_heads.contains(base) {
        return Some(observation("probe_env", base.into()));
    }
    let hosts = || {
        URL_HOST
            .captures_iter(segment)
            .map(|m| m[1].into())
            .collect()
    };
    if vocab.fetch_heads.contains(base) {
        return Some(Observation {
            hosts: hosts(),
            ..observation("fetch_remote", base.into())
        });
    }
    if base == "git"
        && let Some(sub) = rest.first()
    {
        if ["fetch", "clone", "pull", "ls-remote"].contains(sub) {
            return Some(Observation {
                hosts: hosts(),
                ..observation("fetch_remote", format!("git {sub}"))
            });
        }
        return probe_kind(sub, &rest[1..]).map(|kind| Observation {
            probe: Some(kind.into()),
            ..observation("probe_vcs_refs", format!("git {sub}"))
        });
    }
    if vocab.package_tools.contains(base) {
        if ["pip", "npm", "yarn", "pnpm"]
            .iter()
            .any(|s| base.starts_with(s))
            && rest
                .first()
                .is_some_and(|s| ["install", "download", "add", "ci"].contains(s))
        {
            return Some(Observation {
                hosts: hosts(),
                ..observation("fetch_remote", format!("{base} {}", rest[0]))
            });
        }
        return Some(observation("package_tool", base.into()));
    }
    None
}

pub fn probe_kind(sub: &str, args: &[&str]) -> Option<&'static str> {
    match sub {
        "remote" => Some("remote"),
        "branch"
            if args
                .iter()
                .any(|s| ["-a", "-r", "--all", "--remotes", "-ar", "-ra"].contains(s)) =>
        {
            Some("branch")
        }
        "tag" if args.contains(&"--contains") => Some("tag"),
        "fsck" => Some("fsck"),
        "stash" if args.first() == Some(&"list") => Some("stash"),
        "show-ref" | "for-each-ref" => Some("refs"),
        _ => None,
    }
}

static ERRORS: LazyLock<Vec<(&str, Regex)>> = LazyLock::new(|| {
    [
    ("module_missing", r"ModuleNotFoundError|No module named|Cannot find module|ERR_MODULE_NOT_FOUND"),
    ("command_missing", r"command not found|: not found\b|is not recognized"),
    ("script_missing", r"can't open file .*No such file or directory"),
    ("permission_denied", r"Permission denied|EACCES"),
    ("usage_error", r"(?m)^usage:|unrecognized arguments|no such option|error: argument|unknown option"),
    ("connection_refused", r"Connection refused|Failed to connect to|couldn't connect to|Could not connect|ECONNREFUSED"),
    ("name_resolution", r"Could not resolve host|Temporary failure in name resolution|Name or service not known|getaddrinfo|nodename nor servname|ENOTFOUND"),
    ("unreachable", r"Network is unreachable|No route to host|Connection timed out|Operation timed out|timed out after|ETIMEDOUT"),
].into_iter().map(|(name, pattern)| (name, Regex::new(&format!("(?i){pattern}")).unwrap())).collect()
});

pub fn error_class(output: &str) -> Option<&'static str> {
    ERRORS
        .iter()
        .find(|(_, r)| r.is_match(output))
        .map(|(name, _)| *name)
}

pub fn error_line<'a>(output: &'a str, error: &str) -> &'a str {
    let Some(m) = ERRORS
        .iter()
        .find(|(name, _)| *name == error)
        .and_then(|(_, r)| r.find(output))
    else {
        return "";
    };
    let start = output[..m.start()].rfind('\n').map_or(0, |i| i + 1);
    let end = output[m.end()..]
        .find('\n')
        .map_or(output.len(), |i| m.end() + i);
    &output[start..end]
}

pub fn exit_code_hint(output: &str) -> Option<u64> {
    static HINT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)\bexit code[:=\s]+(\d+)").unwrap());
    HINT.captures(output).and_then(|m| m[1].parse().ok())
}

pub fn label_failed(labels: &[String], output: &str) -> bool {
    static ERROR: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)Failed to import test module|unittest\.loader\._FailedTest|ERROR: (?:file or directory )?not found|no tests ran|module '[\w.]+' has no attribute").unwrap()
    });
    static MODULE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"No module named ['"]?([\w.]+)"#).unwrap());
    if ERROR.is_match(output) {
        return true;
    }
    let heads: BTreeSet<_> = labels
        .iter()
        .map(|lab| lab.split(['.', '/', ':']).next().unwrap_or(""))
        .collect();
    MODULE
        .captures_iter(output)
        .any(|m| heads.contains(m[1].split('.').next().unwrap_or("")))
}

pub fn test_labels(observation: &Observation) -> Vec<String> {
    static TOKEN: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^[A-Za-z_][\w.\-/]*(?:::[\w.\-\[\]]+)*$").unwrap());
    let tokens: Vec<_> = observation.segment.split_whitespace().collect();
    let last = observation
        .form
        .split_whitespace()
        .next_back()
        .unwrap_or("");
    let start = tokens
        .iter()
        .position(|t| {
            *t == last
                || t.trim_start_matches(['.', '/']) == last
                || t.ends_with(&format!("/{last}"))
        })
        .map_or(tokens.len(), |i| i + 1);
    let mut out = Vec::new();
    let mut skip = false;
    let mut previous: &str = "";
    for token in &tokens[start..] {
        if skip {
            skip = false;
            previous = token;
            continue;
        }
        if [">", ">>", "2>", "<", "|"].contains(token)
            || ["2>", ">", "<", "|"].iter().any(|s| token.starts_with(s))
        {
            skip = [">", ">>", "2>", "<"].contains(token);
            previous = token;
            continue;
        }
        let structured = token.contains('.') || token.contains("::") || token.contains('/');
        let after_option = previous.starts_with('-') && !previous.contains('=');
        if !token.starts_with('-') && TOKEN.is_match(token) && (structured || !after_option) {
            out.push((*token).into());
        }
        previous = token;
    }
    out
}

pub fn label_rule(a: &str, b: &str) -> Option<(&'static str, String)> {
    if a.contains("::") && !b.contains("::") {
        let dotted = a.replace("::", ".");
        if b == dotted
            || dotted.starts_with(&format!("{b}."))
            || b.starts_with(&format!("{dotted}."))
        {
            return Some(("sep", "::".into()));
        }
    }
    for separator in ['.', '/'] {
        let Some((head, rest)) = a.split_once(separator) else {
            continue;
        };
        let prefix = format!("{head}{separator}");
        if !rest.is_empty()
            && !b.starts_with(&prefix)
            && (b == rest
                || rest.starts_with(&format!("{b}{separator}"))
                || b.starts_with(&format!("{rest}{separator}")))
        {
            return Some(("strip", prefix));
        }
    }
    None
}

pub fn env_calls(command: &str, output: &str, success: bool) -> Result<Value, String> {
    static NEED: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"environment variable\s+['"`]?([A-Z][A-Z0-9_]{2,})"#).unwrap()
    });
    static PREFIX: LazyLock<FancyRegex> = LazyLock::new(|| {
        FancyRegex::new(r#"(?:^|[\s;&(])(?:export\s+)?([A-Z][A-Z0-9_]{2,})=(['"]?)([\w.\-/:]+)\2"#)
            .unwrap()
    });
    static INCODE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"os\.environ(?:\.setdefault\(\s*|\[\s*)['"]([A-Z][A-Z0-9_]{2,})['"]\s*(?:,|\]\s*=)\s*['"]([\w.\-/:]+)['"]"#).unwrap()
    });
    static FAILED: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?m)Traceback \(most recent call last\)|^\w*Error\b|\b[Ee]xit [Cc]ode[:=\s]+[1-9]",
        )
        .unwrap()
    });
    let mut sets = Map::new();
    for m in PREFIX.captures_iter(command) {
        let m = m.map_err(|e| e.to_string())?;
        sets.insert(m[1].into(), json!(&m[3]));
    }
    for m in INCODE.captures_iter(command) {
        sets.insert(m[1].into(), json!(&m[2]));
    }
    let need = NEED
        .captures_iter(output)
        .map(|m| m[1].to_owned())
        .collect::<BTreeSet<_>>();
    Ok(json!({"need": need, "sets": sets, "ok": success && !FAILED.is_match(output)}))
}

pub fn form_failed(observation: &Observation) -> bool {
    observation
        .outcome
        .as_deref()
        .is_some_and(|s| FORM_ERRORS.contains(&s))
        && !observation.label_err
        && !(observation.intent == "run_tests"
            && observation.outcome.as_deref() == Some("usage_error"))
}

pub fn form_worked(observation: &Observation) -> bool {
    observation.outcome.as_deref() == Some("ran")
        && (observation.intent != "run_tests" || observation.tests_ran)
}

pub fn base(path: &str) -> String {
    path.replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or("")
        .into()
}

pub fn not_found_name(line: &str) -> Result<String, String> {
    static MISSING: LazyLock<FancyRegex> = LazyLock::new(|| {
        FancyRegex::new(r#"(?i)([^\s:'"]+):\s+(?:command\s+)?not found\b(?!:)|command not found:\s+(\S+)|'([^']+)' is not recognized"#).unwrap()
    });
    Ok(MISSING
        .captures(line)
        .map_err(|e| e.to_string())?
        .map(|m| {
            base(
                (1..m.len())
                    .find_map(|i| m.get(i))
                    .map_or("", |m| m.as_str()),
            )
        })
        .unwrap_or_default())
}

pub fn is_bare(form: &str) -> bool {
    let head = form.split_whitespace().next().unwrap_or("");
    !head.contains('/')
        && !form.contains(" activate ")
        && !form.contains(" run ")
        && ![
            "conda run",
            "poetry run",
            "pipenv run",
            "uv run",
            "npx ",
            "uvx ",
        ]
        .iter()
        .any(|s| form.starts_with(s))
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Digest {
    pub obs: Vec<Observation>,
    pub edited: Vec<String>,
    pub n_calls: usize,
    pub env: Vec<Value>,
}

pub fn observe_task(path: &Path, vocab: &Vocabulary) -> Result<Digest, String> {
    static LOADED: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\bRan [1-9]\d* tests?\b|\b[1-9]\d* (?:passed|failed|passing|failing)\b|\bcollected [1-9]\d* items?\b").unwrap()
    });
    static GENERIC_LOADED: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^ok\s+\S+\s+[\d.]+s\b|^--- (?:PASS|FAIL)\b|\btest result: (?:ok|FAILED)\b|\b[1-9]\d* examples?, \d+ failures?\b|\b(?:Passed|Failed)!\s+-|\bTests run: [1-9]\d*").unwrap()
    });
    let calls = load_trajectory(path);
    let cwds = call_cwds(path);
    let root = cwds
        .iter()
        .flatten()
        .find(|s| !s.is_empty())
        .map(String::as_str);
    let top = root.and_then(repository_top);
    let mut all = Vec::new();
    let mut env = Vec::new();
    let mut triples = Vec::new();
    for (index, (name, args, output, ok)) in calls.iter().enumerate() {
        let tool = name.as_str().unwrap_or("").to_lowercase();
        triples.push((tool.clone(), args));
        if !["shell", "bash", "stop_bash"].contains(&tool.as_str()) {
            continue;
        }
        let command = args["command"]
            .as_str()
            .filter(|s| !s.is_empty())
            .or_else(|| args["cmd"].as_str())
            .unwrap_or("");
        if CD_FAILED.is_match(output) {
            continue;
        }
        let here = if root.is_some()
            && let Some(cwd) = cwds
                .get(index)
                .and_then(Option::as_deref)
                .filter(|s| !s.is_empty())
        {
            cd_target("", cwd, root, top.as_deref())
        } else {
            Some(String::new())
        };
        let mut observations = if let Some(here) = here {
            classify_command(command, index, vocab, &here, root, top.as_deref())?
        } else {
            classify_command(command, index, vocab, "", None, None)?
        };
        let hint = exit_code_hint(output);
        let success = hint.map_or_else(|| truthy(ok), |code| code == 0);
        if vocab.generic || command.contains("python") || command.contains('=') {
            let mut call = env_calls(command, output, success)?;
            if truthy(&call["need"]) || truthy(&call["sets"]) {
                call["call"] = json!(index);
                env.push(call);
            }
        }
        if observations.is_empty() {
            continue;
        }
        for observation in observations.iter_mut().filter(|o| o.intent == "run_tests") {
            observation.labels = test_labels(observation);
            observation.label_err =
                !observation.labels.is_empty() && label_failed(&observation.labels, output);
            observation.tests_ran =
                LOADED.is_match(output) || vocab.generic && GENERIC_LOADED.is_match(output);
            observation.loaded =
                !observation.labels.is_empty() && !observation.label_err && observation.tests_ran;
        }
        let n_segments = segments(&unwrap(command))?
            .iter()
            .filter(|s| {
                let head = s
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .rsplit('/')
                    .next()
                    .unwrap_or("");
                ![
                    "cd", "echo", "head", "tail", "cat", "sort", "uniq", "tee", "true", "printf",
                    "less", "more",
                ]
                .contains(&head)
            })
            .count();
        let homogeneous = n_segments == observations.len()
            && observations
                .iter()
                .map(|o| &o.intent)
                .collect::<BTreeSet<_>>()
                .len()
                == 1;
        let error = error_class(output);
        if let Some(error) = error.filter(|e| FORM_ERRORS.contains(e) || NETWORK_ERRORS.contains(e))
        {
            let targets: Vec<_> = observations
                .iter()
                .enumerate()
                .filter(|(_, o)| WORK_INTENTS.contains(&o.intent.as_str()))
                .map(|(i, _)| i)
                .collect();
            let mut named: Vec<_> = targets
                .iter()
                .copied()
                .filter(|i| {
                    let o = &observations[*i];
                    !o.head.is_empty()
                        && (output.contains(&o.head)
                            || error_line(output, error)
                                .contains(o.head.rsplit('/').next().unwrap_or("")))
                })
                .collect();
            if NETWORK_ERRORS.contains(&error) {
                named = targets
                    .iter()
                    .copied()
                    .filter(|i| {
                        let o = &observations[*i];
                        o.intent == "fetch_remote"
                            && (o.hosts.is_empty() || o.hosts.iter().any(|h| output.contains(h)))
                    })
                    .collect();
                if named.is_empty() {
                    named = targets
                        .iter()
                        .copied()
                        .filter(|i| observations[*i].intent == "fetch_remote")
                        .collect();
                }
            }
            if named.is_empty() && !success {
                named.extend(targets.last().copied());
            }
            let missing = if error == "command_missing" {
                not_found_name(error_line(output, error))?
            } else {
                String::new()
            };
            for i in named {
                observations[i].outcome = Some(error.into());
                observations[i].not_found =
                    !missing.is_empty() && base(&observations[i].head) == missing;
            }
            if success {
                for o in &mut observations {
                    if o.outcome.is_none() {
                        o.outcome = Some("ran".into());
                    }
                }
            }
        } else if success {
            for o in &mut observations {
                o.outcome = Some("ran".into());
            }
        } else {
            let mut first = true;
            for o in &mut observations {
                if WORK_INTENTS.contains(&o.intent.as_str()) {
                    o.outcome = first.then(|| "ran".into());
                    first = false;
                } else {
                    o.outcome = (o.intent != "probe_vcs_refs").then(|| "ran".into());
                }
            }
        }
        for o in &mut observations {
            if o.intent == "probe_vcs_refs" {
                o.outcome = if homogeneous {
                    if probe_hit(o.probe.as_deref().unwrap_or(""), output) {
                        Some("hit".into())
                    } else {
                        success.then(|| "empty".into())
                    }
                } else {
                    None
                };
            }
            if o.intent == "fetch_remote"
                && o.outcome.as_deref() == Some("ran")
                && (!homogeneous
                    || !success
                    || output.trim().is_empty()
                    || error.is_some_and(|e| NETWORK_ERRORS.contains(&e)))
            {
                o.outcome = None;
            }
        }
        all.extend(observations);
    }
    let edited = lifecycle::edited_files(
        triples.iter().map(|(tool, args)| (tool.as_str(), *args)),
        !vocab.generic,
    )
    .into_iter()
    .collect();
    Ok(Digest {
        obs: all,
        edited,
        n_calls: calls.len(),
        env,
    })
}

fn probe_hit(kind: &str, output: &str) -> bool {
    static PROBES: LazyLock<Vec<(&str, Regex)>> = LazyLock::new(|| {
        [
            ("remote", r"(?m)^\S+\s+\S+\s+\((?:fetch|push)\)\s*$"),
            ("branch", r"(?m)^\s*remotes/\S+"),
            (
                "tag",
                r"(?m)^v?\d+(?:\.\d+)+(?:[a-z]+\d*|\.post\d+|-\S+)?\s*$",
            ),
            (
                "fsck",
                r"(?m)^(?:unreachable|dangling)\s+(?:commit|tree|blob|tag)\s+[0-9a-f]{7,}",
            ),
            ("stash", r"(?m)^stash@\{\d+\}"),
            ("refs", r"(?m)^[0-9a-f]{7,}\s+refs/"),
        ]
        .into_iter()
        .map(|(name, p)| (name, Regex::new(p).unwrap()))
        .collect()
    });
    PROBES
        .iter()
        .find(|(name, _)| *name == kind)
        .is_some_and(|(_, r)| r.is_match(output))
}
