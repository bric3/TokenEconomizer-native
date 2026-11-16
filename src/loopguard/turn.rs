use std::sync::LazyLock;

use fancy_regex::Regex as FancyRegex;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{
    shell_edits::{
        SED_INPLACE, fold_shell_edits, shell_edit_targets, shell_revert_targets, split_heredocs,
        tokens,
    },
    state::{ERR_RE, SpiralState, TEST_RUN_RE, canon_tool},
};
use crate::{
    rules,
    value::{number, truthy},
};

const PYTHON: &str = concat!(
    r"python\d?(?:\.\d+)?",
    r"(?-i:(?:\s+(?:-[bBdEiIOPqsSuvx]+|-[WX]\s*[^\s\-]\S*))*)"
);
pub static VERIFY_RE: LazyLock<FancyRegex> = LazyLock::new(|| {
    FancyRegex::new(&format!(concat!(
        r"(?i)(?:^|[\s;&|(])(?:(?:~?/)?(?:[\w.\-]+/)*{}\s+)?",
        r"(?:~?/)?(?:[\w.\-]+/)*(?:pytest|py\.test|tox|nox|{}\s+-m\s+(?:pytest|unittest)|",
        r"manage\.py\s+test|runtests(?:\.py)?|jest|vitest|mocha|ava|playwright\s+test|",
        r"(?:npm|pnpm|yarn|bun)\s+(?:run\s+)?test|go\s+test|cargo\s+(?:test|nextest)|",
        r"(?:mvn|mvnw|\./mvnw)\s+(?:\S+\s+)*test|(?:gradle|gradlew|\./gradlew)\s+(?:\S+\s+)*test|",
        r"make\s+(?:test|check)|ctest|rspec|phpunit|dotnet\s+test|swift\s+test|mix\s+test|",
        r"rake(?:\s+\w+=\S*)*\s+(?:test|spec)(?::[\w:]+)?|rails\s+test(?::[\w:]+)?|",
        r"ruby(?-i:\s+(?:-[Ir](?:\S+|\s+[^\s\-]\S*)|-C\s*\S+|-W[0-2]?(?::\S+)?|",
        r"--[\w\-]+(?:=\S+)?|-[abdf-zABDF-Z]+))*\s+",
        r"(?:[\w.\-~]*/)*(?:test_[\w.\-]*|[\w.\-]*_(?:test|spec))\.rb|",
        r"deno\s+test|node\s+--test|bazel\s+test|(?:\./)?bin/test)(?![\w.\-/])"
    ), PYTHON, PYTHON)).unwrap()
});
static QUOTED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"'[^']*'|"(?:\\.|[^"\\])*""#).unwrap());
static SEPARATOR: LazyLock<FancyRegex> =
    LazyLock::new(|| FancyRegex::new(r"&&|\|\||\|&|(?<!>)\||;|\n").unwrap());
static NOT_A_RUN: LazyLock<FancyRegex> = LazyLock::new(|| {
    FancyRegex::new(r"\s(?:-h|--help|--version|--collect-only|--co|--list\S*)(?!\S)").unwrap()
});
static PREFIX: LazyLock<FancyRegex> = LazyLock::new(|| {
    FancyRegex::new(concat!(
        r"^\s*(?:(?:\w+=\S*|time|nohup|command|",
        r"nice(?:\s+(?:-n\s*|--adjustment=)?[-+]?\d+)?|",
        r"env(?:\s+(?:(?:-[uC]|--unset|--chdir)(?:=|\s*)[^\s-]\S*|-[-\w]*))*|",
        r"timeout(?:\s+(?:-\S+|[A-Z]+|\d\S*))+|",
        r"(?:(?:uv|poetry|pipenv|pdm|hatch|rye)\s+run|npx|bunx|",
        r"(?:npm|pnpm|yarn)\s+(?:exec|dlx)|bundle\s+exec|xvfb-run)",
        r"(?:\s+-\S+)*|",
        r"dotenv(?:\s+(?!--(?:\s|$))\S+)*\s+--)\s+)*"
    ))
    .unwrap()
});
static REPORT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(?:echo|printf|true|:|exit)(?:\s|$)").unwrap());

pub fn pieces(command: &str) -> Result<(Vec<String>, bool), String> {
    let parts = tokens(command, true).unwrap_or_default();
    let command = if parts.len() >= 3
        && ["sh", "bash", "zsh", "dash", "ksh"].contains(&parts[0].rsplit('/').next().unwrap_or(""))
        && parts[1].starts_with('-')
        && parts[1].contains('c')
    {
        &parts[2]
    } else {
        command
    };
    let command = split_heredocs(command)?.0;
    let masked = QUOTED
        .replace_all(&command, "Q")
        .replace(['(', ')', '{', '}'], " ");
    let mut pieces: Vec<String> = Vec::new();
    let mut start = 0;
    for m in SEPARATOR.find_iter(&masked) {
        let m = m.map_err(|e| e.to_string())?;
        pieces.push(masked[start..m.start()].into());
        pieces.push(m.as_str().into());
        start = m.end();
    }
    pieces.push(masked[start..].into());
    while pieces.len() > 1 && pieces.last().is_some_and(|p| p.trim().is_empty()) {
        pieces.truncate(pieces.len() - 2);
    }
    Ok((pieces, command.contains("pipefail")))
}

fn strip_prefix(stage: &str) -> Result<&str, String> {
    let end = PREFIX
        .find(stage)
        .map_err(|e| e.to_string())?
        .map_or(0, |m| m.end());
    Ok(&stage[end..])
}

fn quiet(stage: &str) -> Result<bool, String> {
    let stage = format!("{} ", stage.trim());
    let stripped = strip_prefix(&stage)?;
    let words: Vec<_> = stripped.split_whitespace().collect();
    if words.is_empty() {
        return Ok(true);
    }
    let head = words[0].rsplit('/').next().unwrap_or("");
    if ![
        "cat", "head", "tail", "less", "more", "wc", "sort", "uniq", "cut", "tr", "nl", "column",
        "ls", "tree", "pwd", "stat", "file", "du", "which", "type", "whereis", "grep", "egrep",
        "fgrep", "rg", "ag", "jq", "find", "echo", "printf", "true", ":", "cd", "pushd", "popd",
        "export", "source", ".", "set", "sed", "git",
    ]
    .contains(&head)
    {
        return Ok(false);
    }
    Ok(match head {
        "git" => words.get(1).is_some_and(|s| {
            [
                "show",
                "diff",
                "log",
                "status",
                "grep",
                "ls-files",
                "blame",
                "rev-parse",
                "branch",
            ]
            .contains(s)
        }),
        "sed" => !words[1..].iter().any(|s| SED_INPLACE.is_match(s)),
        "find" => !words[1..]
            .iter()
            .any(|s| ["-delete", "-exec", "-execdir", "-ok"].contains(s)),
        _ => true,
    })
}

pub fn exit_status_covers(command: &str) -> Result<bool, String> {
    if command.trim().is_empty() {
        return Ok(false);
    }
    let (pieces, pipefail) = pieces(command)?;
    for pair in pieces[..pieces.len() - 1].as_chunks::<2>().0 {
        if pair[1] == "&&" || pipefail && ["|", "|&"].contains(&pair[1].as_str()) {
            continue;
        }
        if !quiet(&pair[0])? {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn status_of_tests(command: &str) -> Result<Option<&'static str>, String> {
    let (pieces, pipefail) = pieces(command)?;
    let mut found = None;
    for i in (0..pieces.len()).step_by(2) {
        let stage = &pieces[i];
        let stripped = strip_prefix(stage)?;
        if !VERIFY_RE
            .find(&stripped)
            .map_err(|e| e.to_string())?
            .is_some_and(|m| m.start() == 0)
            || NOT_A_RUN.is_match(stage).map_err(|e| e.to_string())?
        {
            continue;
        }
        let mut rank = 0;
        for pair in pieces[i + 1..].as_chunks::<2>().0 {
            let sep = pair[0].as_str();
            if sep == "&&" || pipefail && ["|", "|&"].contains(&sep) {
                continue;
            }
            if ["|", "|&", "||"].contains(&sep) {
                rank = 1;
                break;
            }
            if !REPORT.is_match(&pair[1]) {
                rank = 2;
                break;
            }
            rank = 1;
        }
        found = Some(found.map_or(rank, |f: usize| f.max(rank)));
    }
    Ok(found.map(|rank| ["own", "hidden", "mixed"][rank]))
}

static RUN_FAILED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
    r"(?m)\b[1-9]\d* (?:failed|errors?)\b.*\bin \d+(?:\.\d+)?s\b|",
    r"^FAILED \((?:failures|errors)=|^tests finished:.*\b[1-9]\d* (?:failed|exceptions?)\b|",
    r"^# fail [1-9]|^Tests:.*\b[1-9]\d* failed\b|^\s*Tests\s+[1-9]\d* failed\b|",
    r"^\s*[1-9]\d* failing\b|^\s*[1-9]\d* failed\s*$|^(?:--- FAIL: |FAIL\t|FAIL$)|",
    r"^test result: FAILED|\b\d+ examples?, [1-9]\d* failures?\b|",
    r"^\d+ (?:runs|tests), \d+ assertions, (?:[1-9]\d* failures, \d+ errors|\d+ failures, [1-9]\d* errors)\b|",
    r"Tests run: \d+, Failures: (?:[1-9]|0, Errors: [1-9])|BUILD FAIL(?:URE|ED)")).unwrap()
});
static RUN_PASSED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
    r"(?m)\b\d+ passed\b.*\bin \d+(?:\.\d+)?s\b|^OK(?: \(.*\))?\s*$|",
    r"^tests finished: \d+ passed|^# fail 0\b|^Tests:\s+\d+ passed|^\s*Tests\s+\d+ passed|",
    r"^\s*\d+ passing\b|^\s*\d+ passed \(\d+(?:\.\d+)?(?:ms|s|m|h)\)|",
    r"^ok\s+\S+\s+(?:\d+(?:\.\d+)?s|\(cached\))|^test result: ok\.|\b\d+ examples?, 0 failures\b|",
    r"^\d+ (?:runs|tests), \d+ assertions, 0 failures, 0 errors\b|BUILD SUCCESS")).unwrap()
});

pub fn runner_verdict(text: &str) -> Option<bool> {
    if RUN_FAILED.is_match(text) {
        Some(true)
    } else if RUN_PASSED.is_match(text) {
        Some(false)
    } else {
        None
    }
}

pub trait BackgroundJudge {
    fn poll(&mut self, guard: &mut TurnGuard) -> Result<(bool, Value), String>;
    fn start(&mut self, guard: &mut TurnGuard, digest: &str) -> Result<Value, String>;
}

pub type TextCallback<'a> = &'a mut dyn FnMut(&str) -> Result<Value, String>;

pub enum Judge<'a> {
    Sampled(TextCallback<'a>),
    Background(&'a mut dyn BackgroundJudge),
}

pub struct FoldOptions<'a> {
    pub envelope: Option<f64>,
    pub thresholds: Option<&'a Value>,
    pub judge: Option<Judge<'a>>,
    pub nudge_budget: u64,
}

impl Default for FoldOptions<'_> {
    fn default() -> Self {
        Self {
            envelope: None,
            thresholds: None,
            judge: None,
            nudge_budget: 1,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct TurnGuard {
    pub turn_key: String,
    pub st: SpiralState,
    pub whole: SpiralState,
    pub cum_turns: u64,
    pub segment: u64,
    pub nudges_done: u64,
    pub aborted: bool,
    pub abort_delivered: bool,
    pub events: Vec<Value>,
    pub judge_last: i64,
    pub judge_gap: i64,
    pub judge_calls: u64,
    pub judge_tokens: f64,
    pub writer_tokens: f64,
    pub last_edit: u64,
    pub last_verify: u64,
    pub last_verify_failed: bool,
    pub edits: u64,
    pub verifications: u64,
    pub judge_pending: Value,
    pub nudge_ready: Option<String>,
    pub held: Value,
}

impl Default for TurnGuard {
    fn default() -> Self {
        Self {
            turn_key: String::new(),
            st: SpiralState::default(),
            whole: SpiralState::default(),
            cum_turns: 0,
            segment: 0,
            nudges_done: 0,
            aborted: false,
            abort_delivered: false,
            events: Vec::new(),
            judge_last: -999,
            judge_gap: 5,
            judge_calls: 0,
            judge_tokens: 0.0,
            writer_tokens: 0.0,
            last_edit: 0,
            last_verify: 0,
            last_verify_failed: false,
            edits: 0,
            verifications: 0,
            judge_pending: Value::Null,
            nudge_ready: None,
            held: Value::Null,
        }
    }
}

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

impl TurnGuard {
    pub const JUDGE_BASE_GAP: i64 = 5;
    pub const JUDGE_MAX_GAP: i64 = 20;
    pub const JUDGE_STALE_S: f64 = 300.0;

    pub fn new(turn_key: &str, data: &Value) -> Result<Self, String> {
        let mut data = data.as_object().cloned().unwrap_or_default();
        data.entry("turn_key").or_insert_with(|| json!(turn_key));
        for key in ["st", "whole"] {
            if !data.get(key).is_some_and(truthy) {
                data.insert(key.into(), json!({}));
            }
        }
        for key in [
            "cum_turns",
            "segment",
            "nudges_done",
            "judge_last",
            "judge_gap",
            "judge_calls",
            "last_edit",
            "last_verify",
            "edits",
            "verifications",
        ] {
            if let Some(value) = data.get_mut(key) {
                let integer = match value {
                    Value::String(s) => s.trim().parse::<i64>().map_err(|e| e.to_string())?,
                    _ => number(value).ok_or_else(|| format!("{key} must be an integer"))? as i64,
                };
                *value = json!(integer);
            }
        }
        for key in ["aborted", "abort_delivered", "last_verify_failed"] {
            if let Some(value) = data.get_mut(key) {
                *value = json!(truthy(value));
            }
        }
        for key in ["judge_tokens", "writer_tokens"] {
            if let Some(value) = data.get_mut(key) {
                let n = if !truthy(value) {
                    0.0
                } else if let Value::String(s) = value {
                    s.trim().parse::<f64>().map_err(|e| e.to_string())?
                } else {
                    number(value).ok_or_else(|| format!("{key} must be numeric"))?
                };
                *value = json!(n);
            }
        }
        for key in ["judge_pending", "nudge_ready", "held"] {
            if data.get(key).is_some_and(|v| !truthy(v)) {
                data.insert(key.into(), Value::Null);
            }
        }
        serde_json::from_value(Value::Object(data)).map_err(|e| e.to_string())
    }

    pub fn to_dict(&self) -> Value {
        let mut data = serde_json::to_value(self).unwrap();
        data["events"] = json!(self.events[self.events.len().saturating_sub(20)..]);
        data
    }

    fn track(
        &mut self,
        name: &str,
        args: &Value,
        result: &str,
        exit_code: Option<i64>,
        shell_edits: &[String],
    ) -> Result<(), String> {
        let canon = canon_tool(name);
        if ["edit", "create"].contains(&canon.as_str()) {
            self.last_edit = self.cum_turns;
            self.edits += 1;
            return Ok(());
        }
        let command = if canon == "bash" {
            args["command"].as_str().unwrap_or("")
        } else {
            ""
        };
        let verifies = status_of_tests(command)?.is_some();
        if !shell_edits.is_empty() {
            self.last_edit = self.cum_turns.saturating_sub(u64::from(verifies));
            self.edits += 1;
        }
        if verifies {
            self.last_verify = self.cum_turns;
            self.verifications += 1;
            self.last_verify_failed =
                exit_code.map_or_else(|| ERR_RE.is_match(result), |code| code != 0);
        }
        Ok(())
    }

    pub fn resolved_proxy(&self, stopped_normally: bool) -> bool {
        stopped_normally
            && !self.aborted
            && self.cum_turns > 0
            && !(self.last_verify_failed && self.last_verify > self.last_edit)
    }

    pub fn verification_state(&self) -> String {
        if self.verifications == 0 {
            return "No test or verification run has been seen in this turn.".into();
        }
        let state = if self.last_verify_failed {
            "failed"
        } else {
            "passed"
        };
        let since = if self.last_edit > self.last_verify {
            " There have been edits since."
        } else {
            ""
        };
        format!("The last verification run {state}.{since}")
    }

    pub fn live_digest(&self, digest: &str) -> String {
        digest.replace("Tests are still NOT green.", &self.verification_state())
    }

    fn backoff(&mut self, verdict: &Value) {
        self.judge_gap = if truthy(verdict) && verdict["stuck"] == false {
            Self::JUDGE_MAX_GAP.min(self.judge_gap + Self::JUDGE_BASE_GAP)
        } else {
            Self::JUDGE_BASE_GAP
        };
    }

    fn judge(&mut self, digest: &str, judge: &mut Judge<'_>) -> Result<Value, String> {
        match judge {
            Judge::Sampled(judge) => {
                if self.st.turns as i64 - self.judge_last < self.judge_gap {
                    return Ok(Value::Null);
                }
                self.judge_last = self.st.turns as i64;
                self.judge_calls += 1;
                let verdict = judge(&self.live_digest(digest)).unwrap_or(Value::Null);
                if truthy(&verdict) {
                    self.judge_tokens += number(&verdict["_tokens"]).unwrap_or(0.0);
                }
                self.backoff(&verdict);
                Ok(verdict)
            }
            Judge::Background(judge) => {
                let (found, verdict) = judge.poll(self)?;
                if found {
                    self.judge_pending = Value::Null;
                    if truthy(&verdict) {
                        self.nudge_ready = verdict["_nudge"]
                            .as_str()
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(str::to_owned);
                    }
                    self.backoff(&verdict);
                    return Ok(if truthy(&verdict) {
                        verdict
                    } else {
                        Value::Null
                    });
                }
                if truthy(&self.judge_pending) {
                    let since = match &self.judge_pending["since"] {
                        Value::String(s) => s.parse::<f64>().map_err(|e| e.to_string())?,
                        v => number(v).unwrap_or(0.0),
                    };
                    if now() - since < Self::JUDGE_STALE_S {
                        return Ok(Value::Null);
                    }
                    self.judge_pending = Value::Null;
                }
                if self.st.turns as i64 - self.judge_last < self.judge_gap {
                    return Ok(Value::Null);
                }
                self.judge_last = self.st.turns as i64;
                self.judge_calls += 1;
                let job = judge.start(self, &self.live_digest(digest))?;
                if truthy(&job) {
                    self.judge_pending =
                        json!({"id": job, "since": now(), "segment": self.segment});
                }
                Ok(Value::Null)
            }
        }
    }

    pub fn fold(
        &mut self,
        name: &str,
        args: &Value,
        result: &str,
        mut exit_code: Option<i64>,
        mut options: FoldOptions<'_>,
    ) -> Result<Option<(String, String)>, String> {
        let defaults = rules::scaled_thresholds("moderate");
        let thr = options
            .thresholds
            .filter(|v| truthy(v))
            .unwrap_or(&defaults);
        let command = if canon_tool(name) == "bash" {
            args["command"].as_str().unwrap_or("")
        } else {
            ""
        };
        let mut edits = shell_edit_targets(command)?;
        if !edits.is_empty() {
            let reverted = shell_revert_targets(command)?;
            edits.retain(|name| {
                !reverted.contains(name) || self.whole.edit_counts.get(name).is_some_and(truthy)
            });
        }
        let status = status_of_tests(command)?;
        if matches!(status, Some("hidden" | "mixed")) {
            if let Some(verdict) = runner_verdict(result) {
                exit_code = Some(i64::from(verdict));
            } else if status == Some("hidden") {
                exit_code = None;
            }
        }
        let failed = exit_code.map(|n| n != 0);
        self.st.update(name, args, result, failed);
        self.whole.update(name, args, result, failed);
        if status.is_some() && !TEST_RUN_RE.is_match(command) {
            self.st.test_runs += 1;
            self.whole.test_runs += 1;
        }
        fold_shell_edits(&mut self.st, &edits);
        fold_shell_edits(&mut self.whole, &edits);
        self.cum_turns += 1;
        self.track(name, args, result, exit_code, &edits)?;
        if self.aborted || truthy(&self.held) {
            return Ok(None);
        }
        let backstop = number(&thr["backstop_turns"]).or_else(|| {
            options
                .envelope
                .map(|n| n * rules::threshold(thr, "backstop_mult"))
        });
        if let Some(backstop) = backstop
            && self.cum_turns as f64 >= backstop
            && rules::is_stuck(&self.st.signals(), thr)
        {
            let reason = format!("cum-backstop:{}>={backstop:.0}t", self.cum_turns);
            let action = if self.nudges_done < options.nudge_budget {
                "nudge"
            } else {
                "abort"
            };
            return Ok(Some(self.stop(action, &reason, options.envelope)));
        }
        let signals = self.st.signals();
        let digest = self.st.recent_digest(24, options.envelope);
        let (abort, reason) = if let Some(judge) = &mut options.judge {
            rules::assess_abort(
                &signals,
                options.envelope,
                Some(&mut || self.judge(&digest, judge)),
                thr,
            )?
        } else {
            rules::assess_abort(&signals, options.envelope, None, thr)?
        };
        if !abort {
            if truthy(&thr["nudge_on_ambiguous"])
                && reason == "judge-ambiguous-spared"
                && self.nudges_done < options.nudge_budget
            {
                return Ok(Some(self.stop(
                    "nudge",
                    "ambiguous-past-envelope",
                    options.envelope,
                )));
            }
            return Ok(None);
        }
        let action = if self.nudges_done < options.nudge_budget {
            "nudge"
        } else {
            "abort"
        };
        Ok(Some(self.stop(action, &reason, options.envelope)))
    }

    fn stop(&mut self, action: &str, reason: &str, envelope: Option<f64>) -> (String, String) {
        self.events.push(json!({"segment": self.segment, "action": action, "turn_in_segment": self.st.turns,
            "cum_turns": self.cum_turns, "envelope": envelope, "reason": reason, "snapshot": self.st.snapshot()}));
        if action == "abort" {
            self.aborted = true;
        }
        (action.into(), reason.into())
    }

    pub fn nudge_body(
        &mut self,
        envelope: Option<f64>,
        writer: Option<TextCallback<'_>>,
    ) -> String {
        if let Some(text) = self.nudge_ready.take().filter(|s| !s.is_empty()) {
            return text;
        }
        if let Some(writer) = writer {
            let out = writer(&self.live_digest(&self.st.recent_digest(24, envelope)))
                .unwrap_or(Value::Null);
            if truthy(&out) {
                self.writer_tokens += number(&out["_tokens"]).unwrap_or(0.0);
                let text = out["text"].as_str().unwrap_or("").trim();
                if !text.is_empty() {
                    return text.into();
                }
            }
        }
        rules::nudge_text(&self.st.signals())
    }

    pub fn after_nudge(&mut self) {
        self.nudges_done += 1;
        self.segment += 1;
        self.st = SpiralState::default();
        self.judge_last = -999;
        self.judge_gap = Self::JUDGE_BASE_GAP;
        self.judge_pending = Value::Null;
        self.nudge_ready = None;
    }

    pub fn summary(&self, stopped_normally: bool) -> Value {
        json!({"turn_key": self.turn_key, "tool_calls": self.cum_turns, "nudges": self.nudges_done,
            "aborted": self.aborted, "resolved_proxy": self.resolved_proxy(stopped_normally), "edits": self.edits,
            "verifications": self.verifications, "judge_tokens": self.judge_tokens, "writer_tokens": self.writer_tokens})
    }
}
