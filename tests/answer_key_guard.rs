// Converted from reference/upstream/experiments/tests/test_answer_key_guard.py.
pub mod fixtures;
mod support;

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};
use support::TempDir;
use tokeneconomizer::experiments::{
    net_guard,
    swebench::{
        self, Failure, codex_config, codex_trace, containers, guard_install, invocation, providers,
    },
};

const ABUSIVE: &[&str] = &[
    "timeout 60 curl -sSL https://github.com/django/django/commit/2cbd3967.patch -o /tmp/fix.patch",
    "cd /tmp && curl -s -m 30 'https://patch-diff.githubusercontent.com/raw/scikit-learn/scikit-learn/pull/25102.diff'",
    "timeout 25 curl -s https://raw.githubusercontent.com/django/django/stable/3.0.x/tests/mail/tests.py",
    "cd /tmp && curl -s -m 30 'https://api.github.com/search/issues?q=repo:scikit-learn/scikit-learn+set_output'",
    "cd /tmp && timeout 120 git clone --filter=blob:none --no-checkout https://github.com/sympy/sympy.git sympy-git",
    "python -c \"import urllib.request; u='https://raw.githubusercontent.com/sympy/sympy/master/x.py'\"",
    "timeout 90 curl -s 'https://api.github.com/repos/sympy/sympy/pulls/17340/files?per_page=100'",
    "curl -sL 'https://github.com/sympy/sympy/commit/9f4fb03c.diff' -o pr12419.diff",
];
const LEGITIMATE: &[&str] = &[
    "pip install -e .",
    "source activate testbed && pip install pytest",
    "pip download --no-deps -d /tmp/dl six",
    "npm install",
    "npx playwright test",
    "yarn --frozen-lockfile",
    "curl -s http://localhost:3000/",
    "curl -sS https://pypi.org/simple/",
    "curl -sS https://files.pythonhosted.org/packages/x.whl",
    "python -m pytest -q",
];

fn hook(payload: &str, allow: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_te-net-guard"))
        .env_remove(net_guard::LOG_ENV)
        .env("TE_ALLOW_NETWORK_TOOLS", allow)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn payload(command: &str) -> String {
    json!({"tool_name":"Bash","tool_input":{"command":command}}).to_string()
}

#[test]
fn every_answer_key_fetch_is_blocked() {
    for command in ABUSIVE {
        assert!(net_guard::decide(&json!(command)).unwrap().0, "{command}");
    }
}

#[test]
fn no_legitimate_call_is_blocked() {
    for command in LEGITIMATE {
        assert!(!net_guard::decide(&json!(command)).unwrap().0, "{command}");
    }
}

#[test]
fn a_block_carries_a_reason_the_model_can_act_on() {
    let (blocked, reason) = net_guard::decide(&json!(ABUSIVE[0])).unwrap();
    assert!(blocked && !reason.trim().is_empty());
    assert!(reason.contains("answer key") || reason.contains("grading"));
}

#[test]
fn every_blocked_host_is_also_caught_by_the_shell_rule() {
    for host in net_guard::ANSWER_KEY_HOSTS {
        assert!(
            net_guard::decide(&json!(format!("curl https://{host}/x")))
                .unwrap()
                .0
        );
    }
}

#[test]
fn hosts_block_is_wellformed_and_covers_the_answer_key_hosts() {
    let text = net_guard::hosts_block();
    assert!(text.starts_with(net_guard::HOSTS_MARKER));
    assert!(text.ends_with('\n'));
    let mut entries = BTreeMap::new();
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let parts: Vec<_> = line.split_whitespace().collect();
        assert_eq!(parts.len(), 2, "{line}");
        entries.insert(parts[1], parts[0]);
    }
    assert_eq!(
        entries.values().copied().collect::<BTreeSet<_>>(),
        BTreeSet::from(["127.0.0.1"])
    );
    for host in [
        "github.com",
        "raw.githubusercontent.com",
        "api.github.com",
        "patch-diff.githubusercontent.com",
        "codeload.github.com",
    ] {
        assert!(entries.contains_key(host), "{host}");
    }
}

#[test]
fn hosts_block_leaves_the_package_registries_alone() {
    let text = net_guard::hosts_block();
    let blocked: BTreeSet<_> = text.split_whitespace().collect();
    for host in [
        "pypi.org",
        "files.pythonhosted.org",
        "registry.npmjs.org",
        "localhost",
        "127.0.0.1:3000",
    ] {
        assert!(!blocked.contains(host), "{host}");
    }
}

#[test]
fn hook_exits_2_with_a_reason_on_stderr() {
    let output = hook(&payload(ABUSIVE[0]), "");
    assert_eq!(output.status.code(), Some(2));
    assert!(!String::from_utf8(output.stderr).unwrap().trim().is_empty());
}

#[test]
fn hook_allows_a_legitimate_command() {
    assert_eq!(
        hook(&payload("pip install -e ."), "").status.code(),
        Some(0)
    );
}

#[test]
fn hook_fails_open_on_unreadable_input() {
    assert_eq!(hook("not json", "").status.code(), Some(0));
}

#[test]
fn the_lift_switch_lifts_the_hook() {
    assert_eq!(hook(&payload(ABUSIVE[0]), "1").status.code(), Some(0));
}

#[test]
fn answer_key_attempts_counts_from_the_trace() {
    let root = TempDir::new("guard-attempts");
    let calls: Vec<_> = [
        "python -m pytest -q",
        ABUSIVE[0],
        "pip install six",
        ABUSIVE[2],
    ]
    .iter()
    .map(|command| json!(["bash", {"command":command}, "ok"]))
    .collect();
    fixtures::write_claude_transcript(&root.path().join("transcript.jsonl"), &calls);
    assert_eq!(
        codex_trace::answer_key_attempts(root.path(), root.path()),
        2
    );
}

#[test]
fn answer_key_attempts_is_none_when_there_is_no_transcript() {
    let root = TempDir::new("guard-no-transcript");
    assert!(codex_trace::answer_key_attempts(root.path(), root.path()).is_null());
}

#[test]
fn the_merge_program_appends_once_and_keeps_what_was_there() {
    let root = TempDir::new("guard-merge");
    let path = root.path().join("hooks.json");
    fs::write(&path, json!({"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"rtk-rewrite"}]}]},"other":1}).to_string()).unwrap();
    let entry = json!({"matcher":"Bash","hooks":[{"type":"command","command":"/usr/bin/python3 /x/net_guard.py"}]}).to_string();
    for _ in 0..2 {
        let mut child = Command::new(env!("CARGO_BIN_EXE_te-net-guard"))
            .args(["--install-entry", "PreToolUse"])
            .arg(&path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(entry.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "armed");
    }
    let doc: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(doc["other"], 1);
    let commands: Vec<_> = doc["hooks"]["PreToolUse"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["hooks"][0]["command"].clone())
        .collect();
    assert_eq!(
        commands,
        [
            json!("rtk-rewrite"),
            json!("/usr/bin/python3 /x/net_guard.py")
        ]
    );
}

#[derive(Default)]
struct Codex {
    slots: Map<String, Value>,
    scripts: Vec<String>,
    copies: Vec<Value>,
    guard: String,
}

impl Codex {
    fn argv(&mut self, operation: &str, hook: bool) -> Value {
        self.slots = json!({"hook":hook,"session_id":"th","model":null,"effort":null})
            .as_object()
            .unwrap()
            .clone();
        let result = invocation::dispatch(operation, "codex", self).unwrap();
        self.slots[result["provider_ref"].as_str().unwrap()].clone()
    }
}

impl swebench::Runtime for Codex {
    fn call(&mut self, method: &str, args: Value) -> swebench::Result<Value> {
        let slot = |index: usize| args[index].as_str().unwrap();
        match method {
            "config_function" => {
                assert_eq!(args, json!(["network_tool_block", []]));
                return Ok(json!(["web_fetch"]));
            }
            "config_attr" => {
                return Ok(match (slot(0), slot(1)) {
                    ("scaffold_profile", "kind") => json!("codex"),
                    ("scaffold_profile", "home") => json!("/home/agent"),
                    ("runner", "NET_GUARD_PROBE") => json!("curl https://github.com/x"),
                    _ => panic!("unexpected setting: {args}"),
                });
            }
            "codex_constant" => {
                assert_eq!(args, json!(["C_HOME"]));
                return Ok(json!(codex_config::HOME));
            }
            "container_run" => {
                self.copies.push(args);
                return Ok(json!({"returncode":0,"stdout":"","stderr":""}));
            }
            "_exec" => {
                assert_eq!(args[0], "ctr");
                let script = slot(1).to_owned();
                let stdout = if script == "uname -m" {
                    "x86_64\n"
                } else if script.starts_with("mkdir -p ") {
                    "ready\n"
                } else if script.starts_with("chmod 755 ") {
                    "armed\n"
                } else if script.starts_with("printf '%s' ") {
                    "rc=2\n"
                } else {
                    panic!("unexpected script: {script}")
                };
                self.scripts.push(script);
                return Ok(json!({"returncode":0,"stdout":stdout,"stderr":""}));
            }
            "config_set" => {
                assert_eq!(args[0], "_guard_python");
                self.guard = slot(1).into();
            }
            "runner_function" => {
                self.slots.insert(
                    "function".into(),
                    providers::optimizer_constants()["codex"][slot(0)].clone(),
                );
            }
            "provider_copy" => {
                self.slots
                    .insert(slot(0).into(), self.slots[slot(1)].clone());
            }
            "provider_literal" => {
                self.slots.insert(slot(0).into(), args[1].clone());
            }
            "provider_bool" => {
                let value = &self.slots[slot(0)];
                assert!(value.is_null() || value.is_boolean());
                return Ok(json!(value.as_bool().unwrap_or(false)));
            }
            "eval_list" => {
                let values: Vec<_> = args[1]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|name| self.slots[name.as_str().unwrap()].clone())
                    .collect();
                self.slots.insert(slot(0).into(), json!(values));
            }
            "eval_append" => {
                let value = self.slots[slot(1)].clone();
                self.slots[slot(0)].as_array_mut().unwrap().push(value);
            }
            "web_binary" => {
                assert!(matches!(slot(0), "add" | "iadd"));
                let mut values = self.slots[slot(1)].as_array().unwrap().clone();
                values.extend(self.slots[slot(2)].as_array().unwrap().iter().cloned());
                self.slots.insert(slot(3).into(), json!(values));
            }
            "eval_builtin" => {
                assert_eq!(args, json!(["str"]));
                self.slots.insert("function".into(), json!("str"));
            }
            "runner_invoke" => {
                assert_eq!(self.slots["function"], "str");
                let value = self.slots[args[0][0].as_str().unwrap()]
                    .as_str()
                    .unwrap()
                    .to_owned();
                self.slots.insert("called".into(), json!(value));
            }
            _ => panic!("unexpected callback: {method} {args}"),
        }
        Ok(Value::Null)
    }
}

fn helper(root: &Path) {
    let directory = root.join("amd64");
    fs::create_dir(&directory).unwrap();
    // The process boundary is faked; this ELF fixture only exercises artifact validation.
    let mut bytes = vec![0; 120];
    bytes[..6].copy_from_slice(b"\x7fELF\x02\x01");
    bytes[18..20].copy_from_slice(&62u16.to_le_bytes());
    bytes[32..40].copy_from_slice(&64u64.to_le_bytes());
    bytes[54..56].copy_from_slice(&56u16.to_le_bytes());
    bytes[56..58].copy_from_slice(&1u16.to_le_bytes());
    bytes[64..68].copy_from_slice(&1u32.to_le_bytes());
    fs::write(directory.join("te-net-guard"), &bytes).unwrap();
    fs::write(directory.join("manifest.json"), json!({"platform":"linux/amd64","binary":"te-net-guard","sha256":format!("{:x}", Sha256::digest(&bytes))}).to_string()).unwrap();
}

#[test]
fn codex_arms_its_hook_in_codex_home_and_with_it_the_trust_bypass() {
    let root = TempDir::new("guard-codex");
    helper(root.path());
    let mut runtime = Codex::default();
    assert!(runtime.guard.is_empty());
    assert_eq!(
        guard_install::dispatch(
            "install",
            &json!({"name":"ctr","helper_root":root.path()}),
            &mut runtime
        )
        .unwrap(),
        true
    );
    let script = runtime
        .scripts
        .iter()
        .find(|script| script.starts_with("chmod 755 "))
        .unwrap();
    assert!(script.contains("--install-hook codex /opt/codex-home/hooks.json"));
    assert_eq!(runtime.guard, "/home/agent/te-net-guard");
    assert_eq!(runtime.copies.len(), 1);
    assert_eq!(runtime.copies[0][0][0], "docker");
    assert_eq!(runtime.copies[0][0][1], "cp");
    assert_eq!(runtime.copies[0][0][3], "ctr:/home/agent/te-net-guard");
    let hooks = root.path().join("hooks.json");
    net_guard::install_hook("codex", &hooks, &runtime.guard, 4300).unwrap();
    let doc: Value = serde_json::from_slice(&fs::read(hooks).unwrap()).unwrap();
    assert_eq!(doc["hooks"]["PreToolUse"][0]["matcher"], "Bash");
    for operation in ["build", "resume"] {
        let argv = runtime.argv(operation, !runtime.guard.is_empty());
        assert!(
            argv.as_array()
                .unwrap()
                .contains(&json!("--dangerously-bypass-hook-trust"))
        );
    }
    assert!(
        !runtime
            .argv("build", false)
            .as_array()
            .unwrap()
            .contains(&json!("--dangerously-bypass-hook-trust"))
    );
}

#[test]
fn the_codex_config_declares_no_hooks_table() {
    let text = codex_config::config_toml(&json!({"routing":{"base_url":"https://api.openai.com/v1"},"model":"gpt-5.6-sol","effort":"high"}), &Map::new()).unwrap();
    assert!(!text.contains("[hooks]"));
    assert!(!text.contains("pre_tool_use"));
}

#[test]
fn stop_never_raises_when_docker_hangs() {
    struct Docker {
        live: BTreeSet<String>,
        removals: usize,
    }
    impl swebench::Runtime for Docker {
        fn call(&mut self, method: &str, args: Value) -> swebench::Result<Value> {
            Ok(match method {
                "config_attr" => {
                    assert_eq!(args, json!(["runner", "keep", "bool"]));
                    json!(false)
                }
                "container_env" => args[1].clone(),
                "container_run" => {
                    assert_eq!(args[0], json!(["docker", "rm", "-f", "te-fake-container"]));
                    self.removals += 1;
                    return Err(Failure::new("TimeoutExpired", "docker removal timed out"));
                }
                "log" => Value::Null,
                "live_discard" => {
                    self.live.remove(args[0].as_str().unwrap());
                    Value::Null
                }
                _ => panic!("unexpected callback: {method} {args}"),
            })
        }
    }
    let mut runtime = Docker {
        live: BTreeSet::from(["te-fake-container".into()]),
        removals: 0,
    };
    containers::stop(&json!("te-fake-container"), &mut runtime).unwrap();
    assert_eq!(runtime.removals, 1);
    assert!(!runtime.live.contains("te-fake-container"));
}
