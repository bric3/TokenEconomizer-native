mod policy;
mod state;
pub use policy::*;
pub use state::State;

// shortcut: integer counters and policy fields use i64, complete numeric parity before release.

use crate::{
    loopguard::shell_edits,
    trace::codex,
    value::{decimal_integer, int_value, number, truthy},
};
use fancy_regex::Regex as FancyRegex;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeSet, VecDeque},
    sync::LazyLock,
};

static TEST: LazyLock<Regex> = LazyLock::new(|| {
    insensitive(concat!(
        r#"(?:^|[;&|][\s\x1c-\x1f]*|\n[\s\x1c-\x1f]*|["'])"#,
        r"(?:[A-Za-z_İı][A-Za-z0-9_İı]*=[^\s\x1c-\x1f]+[\s\x1c-\x1f]+|rtk[\s\x1c-\x1f]+(?:proxy[\s\x1c-\x1f]+)?)*",
        r"(?:python(?:\d+(?:\.\d+)?)?[\s\x1c-\x1f]+-m[\s\x1c-\x1f]+(?:pytest|unittest|django[\s\x1c-\x1f]+test)|",
        r"pytest|py\.test|(?:python(?:\d+(?:\.\d+)?)?[\s\x1c-\x1f]+)?(?:\./)?bin/test|",
        r"(?:python(?:\d+(?:\.\d+)?)?[\s\x1c-\x1f]+)?(?:\./)?(?:tests/)?runtests\.py|tox(?:[\s\x1c-\x1f]|$)|nox(?:[\s\x1c-\x1f]|$))"
    ))
});
static ASSERT: LazyLock<Regex> = LazyLock::new(|| {
    insensitive(
        r"(?:(?:^|[^\p{L}\p{N}_])assert(?:$|[^\p{L}\p{N}_])|assert_|np\.testing|testing\.assert|raise_if_dask_computes)",
    )
});
static BLOCKED: LazyLock<Regex> = LazyLock::new(|| {
    insensitive(
        r"(?:gold-isolation|network (?:is )?unreachable|temporary failure in name resolution|could not resolve host|connection refused|request to .* blocked)",
    )
});
static FAILURE_ID: LazyLock<Regex> = LazyLock::new(|| {
    insensitive(
        r"(?m)^(?:FAILED|ERROR)[\s\x1c-\x1f]+([^\s\x1c-\x1f]+)|^_{3,}[\s\x1c-\x1f]*(.*?)[\s\x1c-\x1f]*_{3,}$",
    )
});
static FAILURE_LINE: LazyLock<Regex> = LazyLock::new(|| {
    insensitive(
        r"(?:assertionerror|traceback|(?:^|[^\p{L}\p{N}_])failed(?:$|[^\p{L}\p{N}_])|(?:^|[^\p{L}\p{N}_])error:|exception)",
    )
});
static RESEARCH: LazyLock<Regex> = LazyLock::new(|| {
    insensitive(concat!(
        r"(?:(?:^|[^\p{L}\p{N}_])(?:rg|grep|find|curl|wget)(?:$|[^\p{L}\p{N}_])|",
        r"(?:^|[^\p{L}\p{N}_])sed[\s\x1c-\x1f]+-n(?:$|[^\p{L}\p{N}_])|",
        r"(?:^|[^\p{L}\p{N}_])git[\s\x1c-\x1f]+(?:log|show|blame|branch|tag|fsck)(?:$|[^\p{L}\p{N}_])|",
        r"(?:^|[^\p{L}\p{N}_])nl[\s\x1c-\x1f]+-ba(?:$|[^\p{L}\p{N}_])|",
        r"(?:^|[^\p{L}\p{N}_])pip[\s\x1c-\x1f]+cache(?:$|[^\p{L}\p{N}_])|python(?:\d+(?:\.\d+)?)?[\s\x1c-\x1f]+-[\s\x1c-\x1f]*<<)"
    ))
});
static FINALIZATION: LazyLock<Regex> = LazyLock::new(|| {
    insensitive(
        r"(?:git[\s\x1c-\x1f]+(?:diff|status)(?:[\s\x1c-\x1f]|$)|git[\s\x1c-\x1f]+diff[\s\x1c-\x1f]+--check|compileall|py_compile|black[\s\x1c-\x1f]+--check|ruff[\s\x1c-\x1f]+check|pre-commit[\s\x1c-\x1f]+run)",
    )
});
static FILTERED: LazyLock<Regex> = LazyLock::new(|| {
    insensitive(
        r#"(?:[\s\x1c-\x1f]-k(?:$|[^\p{L}\p{N}_])|--deselect(?:$|[^\p{L}\p{N}_])|(?:^|[^\p{L}\p{N}_])deselect(?:$|[^\p{L}\p{N}_])|[\s\x1c-\x1f]-m[\s\x1c-\x1f]+['"]|--lf(?:$|[^\p{L}\p{N}_])|--last-failed(?:$|[^\p{L}\p{N}_])|--ff(?:$|[^\p{L}\p{N}_])|--failed-first(?:$|[^\p{L}\p{N}_]))"#,
    )
});

fn insensitive(pattern: &str) -> Regex {
    // Python also folds dotted/dotless I; consuming word boundaries are used only for boolean searches.
    let pattern = pattern.replace(['i', 'I'], "[iİı]");
    let pattern = pattern.replace(r"[^\p{L}\p{N}_]", r"(?-i:[^\p{L}\p{N}_])");
    Regex::new(&format!("(?i){pattern}")).unwrap()
}

fn space(c: char) -> bool {
    c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}')
}
pub fn normalize(text: &str, limit: i64) -> String {
    static HEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"0x[0-9a-f]+").unwrap());
    static TMP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"/tmp/[^\s\x1c-\x1f]+").unwrap());
    static DIGITS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\p{Nd}+").unwrap());
    static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\s\x1c-\x1f]+").unwrap());
    let text = text.to_lowercase();
    let text = HEX.replace_all(&text, "<hex>");
    let text = TMP.replace_all(&text, "/tmp/<tmp>");
    let text = DIGITS.replace_all(&text, "<n>");
    let text = SPACES.replace_all(&text, " ");
    let text = text.trim_matches(space);
    let length = text.chars().count() as i64;
    text.chars()
        .take(if limit < 0 {
            (length + limit).max(0)
        } else {
            limit
        } as usize)
        .collect()
}
pub fn command_fingerprint(command: &str) -> String {
    let parts = shell_edits::tokens(command, true).unwrap_or_default();
    let command = if parts.len() >= 3
        && ["bash", "sh"].contains(&parts[0].rsplit('/').next().unwrap_or(""))
        && ["-c", "-lc"].contains(&parts[1].as_str())
    {
        &parts[2]
    } else {
        command
    };
    normalize(command, 320)
}
pub fn is_test_command(command: &str) -> bool {
    TEST.is_match(command)
}
pub fn is_verification_command(command: &str) -> bool {
    is_test_command(command)
        || (command.contains("python - <<") || command.contains("python3 - <<"))
            && ASSERT.is_match(command)
}
pub fn is_research_command(command: &str) -> bool {
    !is_verification_command(command)
        && !FINALIZATION.is_match(command)
        && RESEARCH.is_match(command)
}
pub fn green_is_trustworthy(command: &str) -> bool {
    !FILTERED.is_match(command)
}
pub fn verification_scopes(command: &str) -> Option<BTreeSet<String>> {
    static PATHS: LazyLock<FancyRegex> = LazyLock::new(|| {
        FancyRegex::new(r"(?<![\p{L}\p{N}_/.-])((?:[\p{L}\p{N}_./-]*/)?[\p{L}\p{N}_.-]*test[\p{L}\p{N}_.-]*\.py)").unwrap()
    });
    if !is_test_command(command) {
        return None;
    }
    let mut paths: BTreeSet<_> = PATHS
        .captures_iter(command)
        .flatten()
        .filter_map(|c| {
            c.get(1).map(|m| {
                m.as_str()
                    .split("::")
                    .next()
                    .unwrap()
                    .rsplit('/')
                    .next()
                    .unwrap()
                    .to_owned()
            })
        })
        .filter(|s| !s.is_empty())
        .collect();
    if paths.is_empty() {
        paths.insert("__ALL__".into());
    }
    Some(paths)
}
pub fn failure_signatures(output: &str) -> BTreeSet<String> {
    let mut found: Vec<String> = FAILURE_ID
        .captures_iter(output)
        .filter_map(|c| {
            c.get(1)
                .or_else(|| c.get(2))
                .filter(|m| !m.as_str().is_empty())
                .map(|m| normalize(m.as_str(), 180))
        })
        .collect();
    if found.is_empty() {
        found.extend(
            output
                .split([
                    '\n', '\r', '\u{b}', '\u{c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}',
                    '\u{2028}', '\u{2029}',
                ])
                .filter(|line| FAILURE_LINE.is_match(line))
                .take(2)
                .map(|line| normalize(line.trim_matches(space), 180)),
        );
    }
    found.into_iter().filter(|s| !s.is_empty()).collect()
}

pub fn integer(value: &Value) -> Option<Value> {
    if let Some(n) = value.as_i64() {
        return Some(json!(n));
    }
    if let Some(n) = value.as_u64() {
        return Some(json!(n));
    }
    let normalized = value.as_str().and_then(decimal_integer);
    if let Some(text) = &normalized {
        if let Ok(n) = text.parse::<i64>() {
            return Some(json!(n));
        }
        if let Ok(n) = text.parse::<u64>() {
            return Some(json!(n));
        }
    }
    let n = int_value(value)?;
    Some(if n >= i64::MIN as f64 && n < i64::MAX as f64 {
        json!(n as i64)
    } else {
        json!(n)
    })
}
pub fn patch_paths(command: &str) -> Vec<String> {
    codex::shell_patch_paths(command)
}
pub fn shell_patch_applied(command: &str, output: &str, exit_code: &Value) -> bool {
    static SUCCESS: LazyLock<Regex> = LazyLock::new(|| {
        insensitive(r"(?:^|[^\p{L}\p{N}_])Success\.[\s\x1c-\x1f]*(?:Updated|Applied)")
    });
    !patch_paths(command).is_empty()
        && (SUCCESS.is_match(output) || integer(exit_code).as_ref().and_then(number) == Some(0.0))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Event {
    pub kind: String,
    pub command: String,
    pub output: String,
    pub exit_code: Value,
    pub paths: Vec<String>,
    pub thread_id: String,
}
fn string(value: &Value) -> Result<String, String> {
    if !truthy(value) {
        Ok(String::new())
    } else {
        value
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| "TypeError: expected text".into())
    }
}
fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                "int"
            } else {
                "float"
            }
        }
    }
}
fn missing_get(value: &Value) -> String {
    format!(
        "AttributeError: '{}' object has no attribute 'get'",
        type_name(value)
    )
}
pub fn parse_events(line: &str) -> Result<Vec<Event>, String> {
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return Ok(Vec::new());
    };
    let event = value.as_object().ok_or_else(|| missing_get(&value))?;
    if event.get("type").and_then(Value::as_str) == Some("thread.started") {
        return Ok(vec![Event {
            kind: "thread".into(),
            thread_id: string(&value["thread_id"])?,
            ..Event::default()
        }]);
    }
    if event.get("type").and_then(Value::as_str) != Some("item.completed") {
        return Ok(Vec::new());
    }
    let item = &value["item"];
    if truthy(item) && !item.is_object() {
        return Err(missing_get(item));
    }
    match item["type"].as_str() {
        Some("command_execution") => {
            let command = string(&item["command"])?;
            let output = string(&item["aggregated_output"])?;
            let code = &item["exit_code"];
            let exit_code = integer(code).unwrap_or_else(|| code.clone());
            let mut events = Vec::new();
            if shell_patch_applied(&command, &output, &exit_code) {
                events.push(Event {
                    kind: "edit".into(),
                    paths: patch_paths(&command),
                    ..Event::default()
                });
            }
            events.push(Event {
                kind: "command".into(),
                command,
                output,
                exit_code,
                ..Event::default()
            });
            Ok(events)
        }
        Some("file_change") => {
            let changes = &item["changes"];
            if truthy(changes)
                && !changes.is_array()
                && !changes.is_object()
                && !changes.is_string()
            {
                return Err(format!(
                    "TypeError: '{}' object is not iterable",
                    type_name(changes)
                ));
            }
            let paths = item["changes"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|c| c.is_object() && truthy(&c["path"]))
                .map(|c| string(&c["path"]))
                .collect::<Result<_, _>>()?;
            Ok(vec![Event {
                kind: "edit".into(),
                paths,
                ..Event::default()
            }])
        }
        _ => Ok(Vec::new()),
    }
}
pub fn iter_events<I>(
    lines: I,
    first_thread_only: bool,
) -> impl Iterator<Item = Result<Event, String>>
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    let mut lines = lines.into_iter();
    let mut pending = VecDeque::new();
    let mut cursor = EventCursor::new(first_thread_only);
    std::iter::from_fn(move || {
        if cursor.finished {
            return None;
        }
        loop {
            if let Some(event) = pending.pop_front() {
                let event: Event = event;
                return Some(Ok(event));
            }
            if cursor.finished {
                return None;
            }
            let Some(line) = lines.next() else {
                cursor.finished = true;
                return None;
            };
            match cursor.feed(line.as_ref()) {
                Ok(events) => pending.extend(events),
                Err(error) => {
                    cursor.finished = true;
                    return Some(Err(error));
                }
            }
        }
    })
}

#[derive(Serialize, Deserialize)]
pub struct EventCursor {
    pub first_thread_only: bool,
    pub threads: usize,
    pub finished: bool,
}
impl EventCursor {
    pub fn new(first_thread_only: bool) -> Self {
        Self {
            first_thread_only,
            threads: 0,
            finished: false,
        }
    }
    pub fn feed(&mut self, line: &str) -> Result<Vec<Event>, String> {
        if self.finished {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for event in parse_events(line)? {
            if event.kind == "thread" {
                self.threads += 1;
                if self.first_thread_only && self.threads > 1 {
                    self.finished = true;
                    break;
                }
            }
            out.push(event);
        }
        Ok(out)
    }
}
pub fn replay_events(
    events: impl IntoIterator<Item = Result<Event, String>>,
    policy: &Policy,
) -> Result<(Option<Value>, State), String> {
    let mut state = State::new(policy.recent_window)?;
    let mut decision = None;
    for event in events {
        state.update(&event?);
        if decision.is_none()
            && let Some(reason) = state.decision(policy)
        {
            decision =
                Some(json!({"turn":state.turns,"reason":reason,"snapshot":state.snapshot()}));
        }
    }
    Ok((decision, state))
}

pub fn percentile(values: &[f64], pct: f64) -> Result<Option<f64>, String> {
    if values.is_empty() {
        return Ok(None);
    }
    let mut ordered = values.to_vec();
    ordered.sort_by(f64::total_cmp);
    if ordered.len() == 1 {
        return Ok(Some(ordered[0]));
    }
    let position = (pct / 100.0) * (ordered.len() - 1) as f64;
    let lower = position.trunc() as i64;
    let fraction = position - lower as f64;
    if lower + 1 == ordered.len() as i64 {
        return Ok(ordered.last().copied());
    }
    let at = |i: i64| {
        let i = if i < 0 { ordered.len() as i64 + i } else { i };
        ordered
            .get(i as usize)
            .copied()
            .ok_or_else(|| String::from("IndexError: list index out of range"))
    };
    Ok(Some(at(lower)? + fraction * (at(lower + 1)? - at(lower)?)))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnvelopeTracker {
    pub cold_start: i64,
    pub floor: i64,
    pub min_samples: i64,
    pub percentile: f64,
    pub margin: f64,
    pub resolved_tool_turns: Vec<i64>,
}
impl Default for EnvelopeTracker {
    fn default() -> Self {
        Self {
            cold_start: 24,
            floor: 20,
            min_samples: 5,
            percentile: 90.0,
            margin: 1.0,
            resolved_tool_turns: Vec::new(),
        }
    }
}
impl EnvelopeTracker {
    pub fn threshold(&self) -> Result<f64, String> {
        if (self.resolved_tool_turns.len() as i64) < self.min_samples {
            return Ok(self.cold_start as f64);
        }
        let learned = percentile(
            &self
                .resolved_tool_turns
                .iter()
                .map(|n| *n as f64)
                .collect::<Vec<_>>(),
            self.percentile,
        )?
        .unwrap_or(0.0);
        Ok((self.floor as f64).max(learned * self.margin))
    }
    pub fn update(&mut self, turns: &Value, resolved: bool) -> Result<(), String> {
        if resolved && truthy(turns) {
            let turns = integer(turns)
                .and_then(|v| v.as_i64())
                .ok_or("ValueError: invalid Codex tool-turn count")?;
            self.resolved_tool_turns.push(turns);
        }
        Ok(())
    }
    pub fn snapshot(&self) -> Result<Value, String> {
        Ok(
            json!({"threshold":self.threshold()?,"resolved_samples":self.resolved_tool_turns.len(),"resolved_tool_turns":self.resolved_tool_turns}),
        )
    }
}

#[derive(Serialize, Deserialize)]
pub struct ResumeGuard {
    pub policy: Policy,
    pub verbose: bool,
    pub state: State,
    pub nudges_done: i64,
    pub pending_reason: Option<String>,
    pub fired_reasons: BTreeSet<String>,
    pub events: Vec<Value>,
}
impl ResumeGuard {
    pub fn new(policy: Policy, verbose: bool) -> Result<Self, String> {
        Ok(Self {
            state: State::new(policy.recent_window)?,
            policy,
            verbose,
            nudges_done: 0,
            pending_reason: None,
            fired_reasons: BTreeSet::new(),
            events: Vec::new(),
        })
    }
    pub fn monitor(&mut self, line: &str, messages: &mut Vec<String>) -> Result<bool, String> {
        let events = parse_events(line)?;
        if events.is_empty() {
            return Ok(false);
        }
        for event in &events {
            self.state.update(event);
        }
        if events.last().unwrap().kind == "thread" || self.nudges_done >= self.policy.nudge_budget {
            return Ok(false);
        }
        let Some(reason) = self.state.decision(&self.policy) else {
            return Ok(false);
        };
        if self.state.thread_id.is_empty() || self.fired_reasons.contains(reason) {
            return Ok(false);
        }
        self.fired_reasons.insert(reason.into());
        self.pending_reason = Some(reason.into());
        let snapshot = self.state.snapshot();
        if self.verbose {
            messages.push(format!(
                "  [codex-spiral] RESUME-NUDGE at tool turn {} ({reason}) {}",
                self.state.turns,
                crate::value::python_repr(&snapshot)
            ));
        }
        self.events.push(json!({"action":"resume-nudge","reason":reason,"thread_id":self.state.thread_id,"snapshot":snapshot}));
        Ok(true)
    }
    pub fn continuation(&mut self) -> Option<Value> {
        if self.state.thread_id.is_empty() {
            return None;
        }
        let reason = self.pending_reason.take().filter(|r| !r.is_empty())?;
        self.nudges_done += 1;
        let mut request =
            json!({"mode":"resume","session_id":self.state.thread_id,"prompt":nudge_text(&reason)});
        if !self.policy.resume_effort.is_empty() {
            request["effort"] = json!(self.policy.resume_effort);
        }
        Some(request)
    }
    pub fn telemetry(&self) -> Value {
        json!({"policy":self.policy,"nudges":self.nudges_done,"events":self.events,"snapshot":self.state.snapshot(),"thread_id_captured":!self.state.thread_id.is_empty(),"paid_judge_effective_tokens":0,"resume_effort":if self.policy.resume_effort.is_empty(){None}else{Some(&self.policy.resume_effort)}})
    }
}

pub(super) fn bounded_push<T>(queue: &mut VecDeque<T>, value: T, max: usize) {
    if max == 0 {
        return;
    }
    if queue.len() == max {
        queue.pop_front();
    }
    queue.push_back(value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn event_iteration_stops_reading_at_the_second_thread() {
        let reads = Cell::new(0);
        let lines = [
            r#"{"type":"thread.started","thread_id":"first"}"#,
            r#"{"type":"thread.started","thread_id":"second"}"#,
            "must not read",
        ];
        let input = lines.into_iter().inspect(|_| reads.set(reads.get() + 1));
        let mut events = iter_events(input, true);
        assert_eq!(reads.get(), 0);
        assert_eq!(events.next().unwrap().unwrap().thread_id, "first");
        assert_eq!(reads.get(), 1);
        assert!(events.next().is_none());
        assert!(events.next().is_none());
        assert_eq!(reads.get(), 2);
    }

    #[test]
    fn a_narrow_green_keeps_an_outstanding_whole_suite_failure() {
        let mut state = State::new(12).unwrap();
        state.update(&Event {
            kind: "edit".into(),
            paths: vec!["file.py".into()],
            ..Event::default()
        });
        for (command, exit_code) in [("pytest", 1), ("pytest tests/test_a.py", 0)] {
            state.update(&Event {
                kind: "command".into(),
                command: command.into(),
                exit_code: json!(exit_code),
                ..Event::default()
            });
        }
        let policy = Policy {
            min_tool_turns: 1,
            post_green_finalize_grace_turns: 0,
            ..Policy::shipped()
        };
        assert!(state.currently_green());
        assert!(state.decision(&policy).is_none());
        state.update(&Event {
            kind: "command".into(),
            command: "pytest".into(),
            exit_code: json!(0),
            ..Event::default()
        });
        assert_eq!(
            state.decision(&policy),
            Some("validated-patch-should-finalize")
        );
    }

    #[test]
    fn filtered_green_uses_python_word_boundaries_and_case_folding() {
        assert!(!green_is_trustworthy("pytest -k\u{301}name"));
        assert!(!green_is_trustworthy("pytest -k\u{345}name"));
        assert!(!green_is_trustworthy("pytest --last-faİled"));
        assert!(!green_is_trustworthy("pytest\u{1c}-k name"));
    }
}
