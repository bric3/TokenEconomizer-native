use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use indexmap::IndexMap;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use super::observe::{
    self, Digest, FORM_ERRORS, NETWORK_ERRORS, Observation, Vocabulary, form_failed, form_worked,
    is_bare, label_rule,
};
use crate::{
    lifecycle::{self, FactRecord, Lifecycle},
    names::repo_filename,
    store::write_json_atomic,
    value::{number, round_decimal, truthy},
};

pub const STATE_SUBDIR: &str = "envfacts_state";
pub const MAX_FACTS: i64 = 4;
pub const MAX_TOKENS: i64 = 150;
pub const HEADER: &str = "# environment facts learned from earlier tasks in this checkout";
pub const GENERIC_HEADER: &str =
    "# environment facts learned from earlier sessions in this repository";
pub const CONTRADICT_STREAK: u64 = 2;

fn error_phrase(error: &str) -> &str {
    match error {
        "module_missing" => "ModuleNotFoundError",
        "command_missing" => "command not found",
        "script_missing" => "a file-not-found error",
        "usage_error" => "a usage error",
        "permission_denied" => "permission denied",
        _ => error,
    }
}

pub fn moves_shell(form: &str) -> bool {
    form.starts_with("cd ") && form.contains(" && ")
}
pub fn usage(form: &str, args: &str) -> String {
    if moves_shell(form) {
        format!("`({form}{args})`")
    } else {
        format!("`{form}{args}`")
    }
}
pub fn root(form: &str) -> String {
    if let Some((root, _)) = form.split_once(" && ") {
        return root.into();
    }
    if ["conda run", "poetry run", "pipenv run", "uv run", "npx"]
        .iter()
        .any(|p| form.starts_with(p))
    {
        return form
            .split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join(" ");
    }
    let head = form.split_whitespace().next().unwrap_or("");
    head.rsplit_once('/').map_or(head, |(root, _)| root).into()
}
pub fn join(items: &[String]) -> String {
    match items {
        [] => "the hosts it tried".into(),
        [only] => only.clone(),
        _ => format!(
            "{} and {}",
            items[..items.len() - 1].join(", "),
            items.last().unwrap()
        ),
    }
}
pub fn tool_of(form: &str) -> &str {
    form.split_whitespace()
        .next_back()
        .unwrap_or("")
        .rsplit('/')
        .next()
        .unwrap_or("")
}
pub fn repairs(form: &str, a_form: &str, intent: &str) -> bool {
    if form == a_form {
        return false;
    }
    if intent == "run_build" {
        const RUN_WORDS: &[&str] = &[
            "run", "npx", "uv", "uvx", "pipx", "poetry", "pipenv", "conda", "-m", "python",
            "python3",
        ];
        let failing: BTreeSet<_> = a_form.split_whitespace().collect();
        return form
            .split_whitespace()
            .any(|word| failing.contains(word) && !RUN_WORDS.contains(&word));
    }
    intent == "run_tests" || !is_bare(form)
}
pub fn typed_ok(observation: &Observation) -> bool {
    form_worked(observation) && !observation.env
}
pub fn lint_ok(text: &str) -> bool {
    static LINT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)\b(skip|no need|don'?t|do not|stop|done|completed?|finish(?:ed)?|verify|verification|instead of (?:running|testing))\b").unwrap()
    });
    !LINT.is_match(text)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnvFact {
    pub id: String,
    pub family: String,
    pub params: Map<String, Value>,
    pub life: Lifecycle,
}

impl EnvFact {
    pub fn new(id: &str, family: &str, high: bool, params: Map<String, Value>) -> Self {
        Self {
            id: id.into(),
            family: family.into(),
            params,
            // Environment facts hold across edited files, so each session can confirm them.
            life: Lifecycle::new(id.into(), high, false),
        }
    }
    fn param(&self, key: &str) -> &Value {
        self.params.get(key).unwrap_or(&Value::Null)
    }
    fn text(&self, key: &str) -> &str {
        self.param(key).as_str().unwrap_or("")
    }
    fn count(&self, field: &str, key: &str) -> f64 {
        number(&self.param(field)[key]).unwrap_or(0.0)
    }
    fn counts(&self, key: &str) -> impl Iterator<Item = (&String, &Value)> {
        self.param(key).as_object().into_iter().flatten()
    }

    pub fn best(&self, key: &str) -> Option<String> {
        let mut best: Option<(&str, f64)> = None;
        for (key, value) in self.counts(key) {
            let count = number(value).unwrap_or(0.0);
            if best.is_none_or(|(_, prior)| count > prior) {
                best = Some((key, count));
            }
        }
        best.map(|(key, _)| key.into())
    }

    pub fn working_form(&self) -> Option<String> {
        let net: Vec<_> = self
            .counts("b_counts")
            .filter_map(|(form, value)| {
                let count = number(value).unwrap_or(0.0);
                let failed = self.count("b_fails", form);
                (count > failed).then_some((form, count - failed, count))
            })
            .collect();
        // Keeping the shell directory avoids breaking later relative commands.
        let stationary = net.iter().any(|(form, _, _)| !moves_shell(form));
        let mut best: Option<(&String, f64, f64)> = None;
        for &(form, net, count) in &net {
            if stationary && moves_shell(form) {
                continue;
            }
            if best.is_none_or(|(_, n, c)| net > n || net == n && count > c) {
                best = Some((form, net, count));
            }
        }
        best.map(|(form, _, _)| form.clone())
    }

    pub fn claim(&self) -> String {
        match self.family.as_str() {
            "form_repair" => {
                let Some(working) = self.working_form() else { return String::new(); };
                let error = self.best("error_counts").unwrap_or_else(|| "module_missing".into());
                format!("{} fails here with {}; {} works.", usage(self.text("a"), " …"), error_phrase(&error), usage(&working, " …"))
            }
            "boundary" => {
                let mut hosts: Vec<_> = self.counts("hosts").collect();
                hosts.sort_by(|(_,a),(_,b)| number(b).unwrap_or(0.0).total_cmp(&number(a).unwrap_or(0.0)));
                let hosts = hosts.iter().take(3).map(|(host,_)| (*host).clone()).collect::<Vec<_>>();
                format!("Outbound connections to {} are refused here; nothing can be fetched from them.", join(&hosts))
            }
            "arg_repair" => {
                if self.text("kind") == "sep" { return format!("{} takes dotted test labels (`module.Class.test`, not `module::Class::test`).", usage(self.text("form"), "")); }
                let mut examples: Vec<_> = self.counts("examples").filter_map(|(key,_)| key.split_once(' ')).collect();
                if examples.is_empty() { examples.push(("x","x")); }
                examples.sort_by_key(|(a,_)| a.chars().count());
                let (a,b) = examples[0];
                format!("{} takes test labels without the `{}` prefix (`{b}`, not `{a}`).", usage(self.text("form"), ""), self.text("param"))
            }
            "env_value" => {
                let good = self.best("good").unwrap_or_default();
                let mut bad: Vec<_> = self.counts("bad").map(|(key,_)| key.as_str()).filter(|v| *v != good).collect();
                bad.sort();
                let suffix = bad.first().map_or(".".into(), |bad| format!(", `{bad}` does not."));
                let var = self.text("var");
                format!("Code that imports the project needs `{var}` set here: `{var}={good}` works{suffix}")
            }
            "absent_tool" => format!("`{} …` fails here with command not found, and no other way to run it has worked.", self.text("tool")),
            "empty_probe" => "This checkout has no git remote and no branches, tags or unreachable objects beyond HEAD; its history ends at the checked-out commit.".into(),
            _ => String::new(),
        }
    }
}

impl FactRecord for EnvFact {
    fn claim(&self) -> String {
        self.claim()
    }
    fn from_record(record: &Value) -> Result<Self, String> {
        let mut life = Lifecycle::from_record(&record["life"])?;
        life.drift = false;
        Ok(Self {
            id: record["id"].as_str().ok_or("expected fact id")?.into(),
            family: record["family"]
                .as_str()
                .ok_or("expected fact family")?
                .into(),
            params: record["params"].as_object().cloned().unwrap_or_default(),
            life,
        })
    }
}

pub fn fold(
    fact: &mut EnvFact,
    now: i64,
    verdict: &str,
    fired: bool,
    exercised: bool,
    files: Vec<String>,
) {
    let mut verdict = verdict;
    // Repeated successful sessions establish that the environment was repaired.
    if fact.family == "form_repair" && exercised {
        let streak = if verdict == "contradict" {
            fact.param("contra_streak").as_u64().unwrap_or(0) + 1
        } else {
            0
        };
        fact.params.insert("contra_streak".into(), json!(streak));
        if verdict == "contradict" && streak < CONTRADICT_STREAK {
            verdict = "";
        }
    }
    fact.life.update(now, verdict, fired, exercised, files);
    if verdict == "contradict" && fact.life.high && fact.life.state == "CANDIDATE" {
        fact.life.state = "RETIRED".into();
        fact.life.retired_at = Some(now);
    }
}

fn increment(counts: &mut Value, key: &str, amount: u64) -> Result<(), String> {
    let counts = counts.as_object_mut().ok_or("expected fact counter map")?;
    let count = counts.get(key).and_then(Value::as_u64).unwrap_or(0);
    counts.insert(key.into(), json!(count.saturating_add(amount)));
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct EnvFactsDistiller {
    pub max_facts: i64,
    pub max_tokens: i64,
    pub repo: String,
    pub vocab: Vocabulary,
    pub now: i64,
    pub facts: IndexMap<String, EnvFact>,
    pub forgotten: Map<String, Value>,
    pub opt_tokens: f64,
    pub last_injected: BTreeSet<String>,
    pub last_digest: Option<Value>,
}

impl Default for EnvFactsDistiller {
    fn default() -> Self {
        Self::new("", Vocabulary::default(), None, None)
    }
}

impl EnvFactsDistiller {
    pub fn new(
        repo: &str,
        vocab: Vocabulary,
        max_facts: Option<i64>,
        max_tokens: Option<i64>,
    ) -> Self {
        Self {
            max_facts: max_facts.filter(|n| *n != 0).unwrap_or(MAX_FACTS),
            max_tokens: max_tokens.filter(|n| *n != 0).unwrap_or(MAX_TOKENS),
            repo: repo.into(),
            vocab,
            now: 0,
            facts: IndexMap::new(),
            forgotten: Map::new(),
            opt_tokens: 0.0,
            last_injected: BTreeSet::new(),
            last_digest: None,
        }
    }
    pub fn header(&self) -> &str {
        if self.vocab.generic {
            GENERIC_HEADER
        } else {
            HEADER
        }
    }
    pub fn select(&self) -> Vec<&EnvFact> {
        let mut candidates: Vec<_> = self
            .facts
            .values()
            .filter(|f| f.life.injectable() && !f.claim().is_empty() && lint_ok(&f.claim()))
            .collect();
        let priority = |f: &EnvFact| {
            let family = match f.family.as_str() {
                "form_repair" => 0,
                "arg_repair" => 1,
                "env_value" => 2,
                "absent_tool" => 3,
                "empty_probe" => 4,
                "boundary" => 5,
                _ => 9,
            };
            (family, i32::from(f.text("intent") != "run_interpreter"))
        };
        candidates.sort_by(|a, b| {
            priority(a)
                .cmp(&priority(b))
                .then_with(|| b.life.firing_rate().total_cmp(&a.life.firing_rate()))
        });
        let mut chosen = Vec::new();
        for fact in candidates {
            let mut trial = chosen.clone();
            trial.push(fact);
            if trial.len() as i64 > self.max_facts {
                break;
            }
            let text = format!(
                "{}\n{}",
                self.header(),
                self.merged_lines(&trial)
                    .iter()
                    .map(|line| format!("- {line}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            if text.chars().count() as i128 > self.max_tokens as i128 * 4 {
                continue;
            }
            chosen = trial;
        }
        chosen
    }
    pub fn render(&mut self) -> String {
        let chosen = self.select();
        let ids = chosen.iter().map(|f| f.id.clone()).collect();
        let text = if chosen.is_empty() {
            String::new()
        } else {
            format!(
                "{}\n{}\n",
                self.header(),
                self.merged_lines(&chosen)
                    .iter()
                    .map(|line| format!("- {line}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        };
        self.last_injected = ids;
        text
    }
    pub fn seed_injected(&mut self, ids: &[String]) {
        self.last_injected = ids
            .iter()
            .filter(|id| self.facts.contains_key(id.as_str()))
            .cloned()
            .collect();
    }
    pub fn injected_ids(&self) -> Vec<String> {
        self.last_injected.iter().cloned().collect()
    }
    pub fn injectable_claims(&self) -> Vec<String> {
        self.select().iter().map(|f| f.claim()).collect()
    }
    pub fn merged_lines(&self, facts: &[&EnvFact]) -> Vec<String> {
        let mut lines = Vec::new();
        let mut groups: IndexMap<String, Vec<&EnvFact>> = IndexMap::new();
        for &fact in facts {
            if fact.family == "form_repair" {
                groups
                    .entry(root(&fact.working_form().unwrap_or_default()))
                    .or_default()
                    .push(fact);
            } else {
                lines.push(fact.claim());
            }
        }
        for facts in groups.values() {
            if facts.len() == 1 {
                lines.insert(0, facts[0].claim());
                continue;
            }
            let forms = facts
                .iter()
                .map(|f| usage(f.text("a"), " …"))
                .collect::<Vec<_>>();
            let errors = facts
                .iter()
                .filter_map(|f| f.best("error_counts"))
                .filter(|e| FORM_ERRORS.contains(&e.as_str()))
                .map(|e| error_phrase(&e).to_owned())
                .collect::<BTreeSet<_>>();
            let working = facts
                .iter()
                .map(|f| usage(&f.working_form().unwrap_or_default(), " …"))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let errors = if errors.is_empty() {
                "environment error".into()
            } else {
                errors.into_iter().collect::<Vec<_>>().join(", ")
            };
            lines.insert(
                0,
                format!(
                    "{} fail here ({errors}); {} {}.",
                    join(&forms),
                    join(&working),
                    if working.len() == 1 { "works" } else { "work" }
                ),
            );
        }
        lines
    }
    pub fn update(
        &mut self,
        path: &Path,
        iid: &str,
        resolved: Value,
        edited: Option<BTreeSet<String>>,
    ) -> Result<(), String> {
        match observe::observe_task(path, &self.vocab) {
            Ok(digest) => self.update_digest(&digest, iid, resolved, edited),
            Err(_) => {
                self.now += 1;
                self.last_injected.clear();
                Ok(())
            }
        }
    }
    fn apply(
        fact: &mut EnvFact,
        now: i64,
        verdict: &str,
        fired: bool,
        exercised: bool,
        files: &BTreeSet<String>,
        verdicts: &mut Map<String, Value>,
    ) {
        fold(
            fact,
            now,
            verdict,
            fired,
            exercised,
            files.iter().cloned().collect(),
        );
        verdicts.insert(
            fact.id.clone(),
            json!([
                if verdict.is_empty() {
                    Value::Null
                } else {
                    json!(verdict)
                },
                fired,
                exercised
            ]),
        );
    }
    pub fn update_digest(
        &mut self,
        digest: &Digest,
        iid: &str,
        resolved: Value,
        edited: Option<BTreeSet<String>>,
    ) -> Result<(), String> {
        let files = edited.unwrap_or_else(|| digest.edited.iter().cloned().collect());
        let injected = self.last_injected.clone();
        let mut verdicts = Map::new();
        let obs = &digest.obs;
        for intent in ["run_interpreter", "run_tests", "run_build", "package_tool"] {
            let seq: Vec<_> = obs.iter().filter(|o| o.intent == intent).collect();
            let fails: Vec<_> = seq
                .iter()
                .copied()
                .filter(|o| {
                    form_failed(o)
                        && (["run_tests", "run_build"].contains(&intent) || is_bare(&o.form))
                })
                .collect();
            for a in fails {
                if !seq
                    .iter()
                    .any(|o| typed_ok(o) && o.call > a.call && repairs(&o.form, &a.form, intent))
                {
                    continue;
                }
                let id = format!("form_repair:{intent}:{}", a.form);
                let fact = self.facts.entry(id.clone()).or_insert_with(|| {
                    EnvFact::new(
                        &id,
                        "form_repair",
                        false,
                        json!({"intent":intent,"a":a.form,"b_counts":{},"error_counts":{}})
                            .as_object()
                            .unwrap()
                            .clone(),
                    )
                });
                increment(
                    fact.params.entry("error_counts").or_insert(json!({})),
                    a.outcome.as_deref().unwrap_or(""),
                    1,
                )?;
            }
            // Count each form once per session so retries do not outweigh earlier sessions.
            let ok: BTreeSet<_> = seq
                .iter()
                .filter(|o| typed_ok(o))
                .map(|o| o.form.as_str())
                .collect();
            let failed: BTreeSet<_> = seq
                .iter()
                .filter(|o| form_failed(o) && !o.env && !ok.contains(o.form.as_str()))
                .map(|o| o.form.as_str())
                .collect();
            for fact in self
                .facts
                .values_mut()
                .filter(|f| f.family == "form_repair" && f.text("intent") == intent)
            {
                let failing = fact.text("a").to_owned();
                for form in &ok {
                    if repairs(form, &failing, intent) {
                        increment(fact.params.entry("b_counts").or_insert(json!({})), form, 1)?;
                    }
                }
                let known: BTreeSet<_> = fact
                    .counts("b_counts")
                    .map(|(form, _)| form.clone())
                    .collect();
                let counts = fact.params.entry("b_fails").or_insert(json!({}));
                for form in &failed {
                    if known.contains(*form) {
                        increment(counts, form, 1)?;
                    }
                }
            }
        }
        for fact in self
            .facts
            .values_mut()
            .filter(|f| f.family == "form_repair")
        {
            let intent = fact.text("intent");
            let failing = fact.text("a");
            let seq: Vec<_> = obs.iter().filter(|o| o.intent == intent).collect();
            let a_failed = seq.iter().any(|o| o.form == failing && form_failed(o));
            let b_ran = seq
                .iter()
                .any(|o| fact.param("b_counts").get(&o.form).is_some() && form_worked(o))
                || seq.iter().any(|o| {
                    form_worked(o) && !is_bare(&o.form) && repairs(&o.form, failing, intent)
                });
            // A successful test or build run in the original form contradicts the repair.
            let a_runner_ok = obs
                .iter()
                .filter(|o| {
                    o.intent
                        == if intent == "run_build" {
                            "run_build"
                        } else {
                            "run_tests"
                        }
                })
                .any(|o| o.form == failing && form_worked(o));
            let fired = a_failed || injected.contains(&fact.id) && b_ran;
            let verdict = if a_failed || b_ran && !a_runner_ok {
                "confirm"
            } else if a_runner_ok {
                "contradict"
            } else {
                ""
            };
            Self::apply(
                fact,
                self.now,
                verdict,
                fired,
                !seq.is_empty(),
                &files,
                &mut verdicts,
            );
        }
        if self.vocab.generic {
            let repaired: BTreeSet<_> = self
                .facts
                .values()
                .filter(|f| f.family == "form_repair" && f.working_form().is_some())
                .map(|f| f.text("a").to_owned())
                .collect();
            let mut missing = IndexMap::new();
            for o in obs {
                if ["run_build", "run_tests", "package_tool"].contains(&o.intent.as_str())
                    && o.outcome.as_deref() == Some("command_missing")
                    && o.not_found
                    && is_bare(&o.form)
                    && !o.form.contains(' ')
                    && !repaired.contains(&o.form)
                {
                    missing.entry(o.form.clone()).or_insert(o.intent.clone());
                }
            }
            let worked: BTreeSet<_> = obs
                .iter()
                .filter(|o| form_worked(o))
                .map(|o| tool_of(&o.form))
                .collect();
            for (tool, intent) in missing {
                if worked.contains(tool.as_str()) {
                    continue;
                }
                let id = format!("absent_tool:{tool}");
                self.facts.entry(id.clone()).or_insert_with(|| {
                    EnvFact::new(
                        &id,
                        "absent_tool",
                        true,
                        json!({"tool":tool,"intent":intent})
                            .as_object()
                            .unwrap()
                            .clone(),
                    )
                });
            }
            for fact in self
                .facts
                .values_mut()
                .filter(|f| f.family == "absent_tool")
            {
                let tool = fact.text("tool");
                let seen: Vec<_> = obs.iter().filter(|o| tool_of(&o.form) == tool).collect();
                let contradicted = repaired.contains(tool) || seen.iter().any(|o| form_worked(o));
                let missing = seen.iter().any(|o| {
                    o.outcome.as_deref() == Some("command_missing")
                        && o.not_found
                        && o.form == tool
                        && is_bare(&o.form)
                });
                let prevented = injected.contains(&fact.id)
                    && !contradicted
                    && obs.iter().any(|o| o.intent == fact.text("intent"));
                let fired = missing || prevented;
                Self::apply(
                    fact,
                    self.now,
                    if contradicted {
                        "contradict"
                    } else if fired {
                        "confirm"
                    } else {
                        ""
                    },
                    fired,
                    fired || contradicted,
                    &files,
                    &mut verdicts,
                );
            }
        }
        let runs: Vec<_> = obs
            .iter()
            .filter(|o| o.intent == "run_tests" && !o.labels.is_empty())
            .collect();
        for (i, a) in runs.iter().enumerate() {
            if !a.label_err {
                continue;
            }
            for label in &a.labels {
                for b in &runs[i + 1..] {
                    if b.form != a.form || !b.loaded {
                        continue;
                    }
                    let Some((working, rule)) = b
                        .labels
                        .iter()
                        .find_map(|working| label_rule(label, working).map(|rule| (working, rule)))
                    else {
                        continue;
                    };
                    let id = format!("arg_repair:{}:{}:{}", a.form, rule.0, rule.1);
                    let fact = self.facts.entry(id.clone()).or_insert_with(|| {
                        EnvFact::new(
                            &id,
                            "arg_repair",
                            false,
                            json!({"form":a.form,"kind":rule.0,"param":rule.1,"examples":{}})
                                .as_object()
                                .unwrap()
                                .clone(),
                        )
                    });
                    increment(
                        fact.params.entry("examples").or_insert(json!({})),
                        &format!("{label} {working}"),
                        1,
                    )?;
                    break;
                }
            }
        }
        for fact in self.facts.values_mut().filter(|f| f.family == "arg_repair") {
            let mine: Vec<_> = runs
                .iter()
                .copied()
                .filter(|o| o.form == fact.text("form"))
                .collect();
            let bad = |label: &String| {
                if fact.text("kind") == "sep" {
                    label.contains("::")
                } else {
                    label.starts_with(fact.text("param"))
                }
            };
            let failed = mine.iter().any(|o| o.label_err && o.labels.iter().any(bad));
            let bad_loaded = mine.iter().any(|o| o.loaded && o.labels.iter().any(bad));
            let good_loaded = mine
                .iter()
                .any(|o| o.loaded && o.labels.iter().any(|l| !bad(l)));
            let fired = failed || injected.contains(&fact.id) && good_loaded;
            Self::apply(
                fact,
                self.now,
                if bad_loaded {
                    "contradict"
                } else if failed || good_loaded {
                    "confirm"
                } else {
                    ""
                },
                fired,
                !mine.is_empty(),
                &files,
                &mut verdicts,
            );
        }
        for (i, call) in digest.env.iter().enumerate() {
            for var in call["need"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                let later: Vec<_> = digest.env[i + 1..]
                    .iter()
                    .filter(|c| c["sets"].get(var).is_some())
                    .collect();
                let good: Vec<_> = later
                    .iter()
                    .filter(|c| {
                        truthy(&c["ok"])
                            && !c["need"]
                                .as_array()
                                .is_some_and(|a| a.contains(&json!(var)))
                    })
                    .filter_map(|c| c["sets"][var].as_str())
                    .collect();
                let Some(good) = good.first() else {
                    continue;
                };
                let id = format!("env_value:{var}");
                let fact = self.facts.entry(id.clone()).or_insert_with(|| {
                    EnvFact::new(
                        &id,
                        "env_value",
                        false,
                        json!({"var":var,"good":{},"bad":{}})
                            .as_object()
                            .unwrap()
                            .clone(),
                    )
                });
                increment(fact.params.entry("good").or_insert(json!({})), good, 1)?;
                for call in later {
                    if !truthy(&call["ok"]) && call["sets"][var].as_str() != Some(good) {
                        increment(
                            fact.params.entry("bad").or_insert(json!({})),
                            call["sets"][var].as_str().unwrap_or(""),
                            1,
                        )?;
                    }
                }
            }
        }
        for fact in self.facts.values_mut().filter(|f| f.family == "env_value") {
            let var = fact.text("var");
            let good = fact.best("good");
            let need = |c: &&Value| {
                c["need"]
                    .as_array()
                    .is_some_and(|a| a.contains(&json!(var)))
            };
            let asked = digest.env.iter().any(|c| need(&c));
            let good_ok = digest.env.iter().any(|c| {
                c["sets"].get(var).and_then(Value::as_str) == good.as_deref() && truthy(&c["ok"])
            });
            let good_failed = digest
                .env
                .iter()
                .any(|c| c["sets"].get(var).and_then(Value::as_str) == good.as_deref() && need(&c));
            let exercised = asked || digest.env.iter().any(|c| c["sets"].get(var).is_some());
            let fired = asked || injected.contains(&fact.id) && good_ok;
            Self::apply(
                fact,
                self.now,
                if good_failed {
                    "contradict"
                } else if good_ok {
                    "confirm"
                } else {
                    ""
                },
                fired,
                exercised,
                &files,
                &mut verdicts,
            );
        }
        let fetches: Vec<_> = obs.iter().filter(|o| o.intent == "fetch_remote").collect();
        let mut failed_hosts = Map::<String, Value>::new();
        let mut ok_hosts = BTreeSet::new();
        for o in &fetches {
            if o.outcome
                .as_deref()
                .is_some_and(|e| NETWORK_ERRORS.contains(&e))
            {
                for host in &o.hosts {
                    let n = failed_hosts.get(host).and_then(Value::as_u64).unwrap_or(0);
                    failed_hosts.insert(host.clone(), json!(n + 1));
                }
            }
            if o.outcome.as_deref() == Some("ran") {
                ok_hosts.extend(o.hosts.iter().cloned());
            }
        }
        let id = "boundary:fetch_remote";
        if !self.facts.contains_key(id) && !failed_hosts.is_empty() {
            self.facts.insert(
                id.into(),
                EnvFact::new(
                    id,
                    "boundary",
                    true,
                    json!({"hosts":{}}).as_object().unwrap().clone(),
                ),
            );
        }
        if let Some(fact) = self.facts.get_mut(id) {
            for (host, n) in &failed_hosts {
                increment(
                    fact.params.entry("hosts").or_insert(json!({})),
                    host,
                    n.as_u64().unwrap_or(0),
                )?;
            }
            let contradicted = fact
                .counts("hosts")
                .any(|(host, _)| ok_hosts.contains(host));
            let exercised = fetches.iter().any(|o| !o.hosts.is_empty());
            let fired = !failed_hosts.is_empty();
            Self::apply(
                fact,
                self.now,
                if contradicted {
                    "contradict"
                } else if fired {
                    "confirm"
                } else {
                    ""
                },
                fired,
                exercised,
                &files,
                &mut verdicts,
            );
        }
        let probes: Vec<_> = obs
            .iter()
            .filter(|o| {
                o.intent == "probe_vcs_refs"
                    && matches!(o.outcome.as_deref(), Some("hit" | "empty"))
            })
            .collect();
        let id = "empty_probe:vcs_refs";
        if !self.facts.contains_key(id) && !probes.is_empty() {
            self.facts.insert(
                id.into(),
                EnvFact::new(
                    id,
                    "empty_probe",
                    true,
                    json!({"kinds":{}}).as_object().unwrap().clone(),
                ),
            );
        }
        if let Some(fact) = self.facts.get_mut(id) {
            for o in &probes {
                increment(
                    fact.params.entry("kinds").or_insert(json!({})),
                    o.probe.as_deref().unwrap_or(""),
                    1,
                )?;
            }
            let verdict = if probes.iter().any(|o| o.outcome.as_deref() == Some("hit")) {
                "contradict"
            } else if !probes.is_empty() {
                "confirm"
            } else {
                ""
            };
            Self::apply(
                fact,
                self.now,
                verdict,
                !probes.is_empty(),
                !probes.is_empty(),
                &files,
                &mut verdicts,
            );
        }
        self.last_digest = Some(
            json!({"iid":iid,"resolved":resolved,"injected":injected,"verdicts":verdicts,"n_calls":digest.n_calls}),
        );
        lifecycle::drop_forgotten(&mut self.facts, &self.forgotten);
        self.last_injected.clear();
        self.now += 1;
        Ok(())
    }
    pub fn path(root: &Path, repo: &str) -> Result<PathBuf, String> {
        let directory = root.join(STATE_SUBDIR);
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        Ok(directory.join(repo_filename(repo) + ".json"))
    }
    pub fn save(&self, root: &Path) -> Result<(), String> {
        let path = Self::path(root, &self.repo)?;
        write_json_atomic(&path,&json!({"repo":self.repo,"now":self.now,"opt_tokens":0.0,"vocab":self.vocab.name,"facts":self.facts,"forgotten":self.forgotten}),false).map_err(|e| e.to_string())
    }
    pub fn load(
        repo: &str,
        root: &Path,
        vocab: Vocabulary,
        max_facts: Option<i64>,
        max_tokens: Option<i64>,
    ) -> Result<Self, String> {
        let mut distiller = Self::new(repo, vocab, max_facts, max_tokens);
        let path = Self::path(root, repo)?;
        if !path.exists() {
            return Ok(distiller);
        }
        let data: Value =
            serde_json::from_str(&fs::read_to_string(path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        distiller.now = data["now"].as_i64().unwrap_or(0);
        for (id, record) in data["facts"].as_object().into_iter().flatten() {
            distiller
                .facts
                .insert(id.clone(), EnvFact::from_record(record)?);
        }
        distiller.forgotten = data["forgotten"].as_object().cloned().unwrap_or_default();
        lifecycle::drop_forgotten(&mut distiller.facts, &distiller.forgotten);
        Ok(distiller)
    }
    pub fn debug_row(&self) -> Value {
        let mut row = Map::new();
        for (id, fact) in &self.facts {
            row.insert(id.clone(),json!({"state":fact.life.state,"fire":format!("{}/{}",fact.life.fire_num,fact.life.fire_den),"rate":round_decimal(fact.life.firing_rate(),2),"inject":fact.life.injectable(),"claim":fact.claim().chars().take(120).collect::<String>(),"family":fact.family}));
        }
        row.insert("_profile".into(), json!("envfacts"));
        row.insert("_vocab".into(), json!(self.vocab.name));
        if let Some(task) = &self.last_digest
            && truthy(task)
        {
            row.insert("_task".into(), task.clone());
        }
        Value::Object(row)
    }
}
