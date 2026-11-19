//! Plan experiment cells and count the runnable tasks before launching a sweep.
use super::{
    CommandOutput, arms,
    swebench::{Failure, Result, official},
    workloads,
};
use crate::{
    plugin::cli::parser::{invalid_choice, is_option},
    value::{decimal_integer, number, python_repr, python_type, scalar_text, truthy},
};
use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const SWEBENCH_ARMS: &[&str] = &[
    "baseline",
    "baseline-medium",
    "baseline-low",
    "inject-only",
    "untool-only",
    "spiral-only",
    "trimech",
    "rtk",
    "serena",
    "repomap",
    "trimech-rtk",
    "trimech-repomap",
];
// WebBench's shorter streams use the frozen-envelope spiral configuration.
const WEBBENCH_ARMS: &[&str] = &[
    "baseline",
    "baseline-medium",
    "baseline-low",
    "trimech-web",
    "rtk",
];
pub const WEBBENCH_OUT_OF_SCOPE: &[&str] = &[
    "inject-only",
    "untool-only",
    "spiral-only-web",
    "trimech-rtk-web",
];
pub const WEBBENCH_UNAVAILABLE: &[&str] = &["serena", "repomap", "trimech-repomap"];
pub const SEEDS: [i64; 3] = [0, 1, 2];
pub const SEC_PER_TASK: f64 = 120.0;
pub const USD_PER_TASK: f64 = 0.34;

pub fn constants() -> Value {
    json!({"SWEEP_ARMS":{"swebench":SWEBENCH_ARMS,"webbench":WEBBENCH_ARMS},
        "SWEEP_ARMS_WEBBENCH_OUT_OF_SCOPE":WEBBENCH_OUT_OF_SCOPE,
        "SWEEP_ARMS_WEBBENCH_UNAVAILABLE":WEBBENCH_UNAVAILABLE,"SWEEP_SEEDS":SEEDS,
        "DEFAULT_SEC_PER_TASK":SEC_PER_TASK,"DEFAULT_USD_PER_TASK":USD_PER_TASK})
}

pub fn group_sizes(
    domain: &str,
    manifest: Option<&Path>,
    root: &Path,
    cwd: &Path,
) -> Result<IndexMap<String, usize>> {
    let default = root.join("workloads").join(format!("{domain}.json"));
    let path = manifest
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(&default);
    if cwd.join(path).exists() {
        let data = official::load(path, cwd)?;
        let data = data.as_object().ok_or_else(|| {
            Failure::new(
                "AttributeError",
                format!("'{}' object has no attribute 'get'", python_type(&data)),
            )
        })?;
        if let Some(groups) = data.get("groups").filter(|value| truthy(value)) {
            let groups = groups.as_object().ok_or_else(|| {
                Failure::new(
                    "AttributeError",
                    format!("'{}' object has no attribute 'items'", python_type(groups)),
                )
            })?;
            return groups
                .iter()
                .map(|(group, ids)| {
                    let size = match ids {
                        Value::Array(ids) => ids.len(),
                        Value::Object(ids) => ids.len(),
                        Value::String(ids) => ids.chars().count(),
                        _ => {
                            return Err(Failure::new(
                                "TypeError",
                                format!("object of type '{}' has no len()", python_type(ids)),
                            ));
                        }
                    };
                    Ok((group.clone(), size))
                })
                .collect();
        }
    }
    workloads::runnable_instances(domain)
        .map(|groups| {
            groups
                .into_iter()
                .map(|(group, ids)| (group, ids.len()))
                .collect()
        })
        .map_err(|e| Failure::new(e.kind(), e.to_string()))
}

pub fn cells(arms: &[String], repos: &[String], seeds: &[Value]) -> Vec<(String, String, Value)> {
    let mut cells = Vec::new();
    for seed in seeds {
        for repo in repos {
            for arm in arms {
                cells.push((arm.clone(), repo.clone(), seed.clone()));
            }
        }
    }
    cells
}

#[derive(Deserialize)]
pub struct Sweep {
    pub domain: String,
    pub arms: Vec<String>,
    pub repos: Vec<String>,
    pub seeds: Vec<Value>,
    pub sec_per_task: f64,
    pub usd_per_task: Value,
    pub limit: i64,
    pub manifest: Option<PathBuf>,
}
impl Default for Sweep {
    fn default() -> Self {
        Self {
            domain: "swebench".into(),
            arms: Vec::new(),
            repos: Vec::new(),
            seeds: SEEDS.map(|s| json!(s)).to_vec(),
            sec_per_task: SEC_PER_TASK,
            usd_per_task: json!(USD_PER_TASK),
            limit: 0,
            manifest: None,
        }
    }
}
impl Sweep {
    pub fn estimate(&self, root: &Path, cwd: &Path) -> Result<Value> {
        let sizes = group_sizes(&self.domain, self.manifest.as_deref(), root, cwd)?;
        let mut per_repo = IndexMap::new();
        let mut unknown = Vec::new();
        for repo in &self.repos {
            if !sizes.contains_key(repo) {
                unknown.push(repo.clone());
            }
            let size = sizes.get(repo).copied().unwrap_or(0);
            per_repo.insert(
                repo,
                if self.limit > 0 {
                    size.min(self.limit as usize)
                } else {
                    size
                },
            );
        }
        let factor = self
            .arms
            .len()
            .checked_mul(self.seeds.len())
            .ok_or_else(|| {
                Failure::new(
                    "OverflowError",
                    "sweep cell count exceeds the native integer range",
                )
            })?;
        let tasks = per_repo
            .values()
            .sum::<usize>()
            .checked_mul(factor)
            .ok_or_else(|| {
                Failure::new(
                    "OverflowError",
                    "sweep task count exceeds the native integer range",
                )
            })?;
        let cells = factor.checked_mul(self.repos.len()).ok_or_else(|| {
            Failure::new(
                "OverflowError",
                "sweep cell count exceeds the native integer range",
            )
        })?;
        let usd = match &self.usd_per_task {
            Value::Number(n) if n.is_i64() || n.is_u64() => {
                let rate = n
                    .as_i64()
                    .map(i128::from)
                    .or_else(|| n.as_u64().map(i128::from))
                    .unwrap();
                let total = rate.checked_mul(tasks as i128).ok_or_else(|| {
                    Failure::new(
                        "OverflowError",
                        "sweep dollar estimate exceeds the native integer range",
                    )
                })?;
                serde_json::to_value(total)
                    .map_err(|e| Failure::new("OverflowError", e.to_string()))?
            }
            Value::Bool(rate) => json!(tasks * usize::from(*rate)),
            value => json!(
                number(value)
                    .ok_or_else(|| Failure::new("TypeError", "usd_per_task must be numeric"))?
                    * (tasks as f64)
            ),
        };
        if !self.sec_per_task.is_finite() || usd.is_null() {
            return Err(Failure::new("ValueError", "sweep rates must be finite"));
        }
        Ok(
            json!({"domain":self.domain,"n_arms":self.arms.len(),"n_repos":self.repos.len(),"n_seeds":self.seeds.len(),"n_cells":cells,"n_task_runs":tasks,
            "hours_serial":(tasks as f64)*self.sec_per_task/3600.0,"usd":usd,"unknown_repos":unknown,"limit":self.limit}),
        )
    }
}

fn grouped(text: &str) -> String {
    let (sign, digits) = text.strip_prefix('-').map_or(("", text), |s| ("-", s));
    let mut out = String::from(sign);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

pub fn format_estimate(e: &Value, show_cost: bool) -> Result<String> {
    let field = |key: &str| e.get(key).ok_or_else(|| Failure::new("KeyError", key));
    let integer = |key: &str| -> Result<String> {
        let value = field(key)?;
        match value {
            Value::Number(n) if n.is_i64() || n.is_u64() => Ok(n.to_string()),
            Value::Bool(n) => Ok(u8::from(*n).to_string()),
            _ => Err(Failure::new(
                "ValueError",
                format!(
                    "Unknown format code 'd' for object of type '{}'",
                    python_type(value)
                ),
            )),
        }
    };
    let domain = field("domain")?
        .as_str()
        .ok_or_else(|| Failure::new("ValueError", "domain must be text"))?;
    let mut out = format!(
        "{domain:9} arms={:>2} repos={:>2} seeds={:>2}  cells={:>5}  task-runs={:>7}",
        integer("n_arms")?,
        integer("n_repos")?,
        integer("n_seeds")?,
        integer("n_cells")?,
        grouped(&integer("n_task_runs")?)
    );
    if show_cost {
        let hours = number(field("hours_serial")?)
            .ok_or_else(|| Failure::new("ValueError", "hours_serial must be numeric"))?;
        let usd = number(field("usd")?)
            .ok_or_else(|| Failure::new("ValueError", "usd must be numeric"))?;
        out.push_str(&format!(
            "  serial={:>8}h  ~${:>9}",
            grouped(&format!("{hours:.0}")),
            grouped(&format!("{usd:.0}"))
        ));
    }
    if e.get("limit").is_some_and(truthy) {
        out.push_str(&format!("  limit={}/cell", scalar_text(&e["limit"])));
    }
    Ok(out)
}

fn help(program: &str) -> String {
    // shortcut: retain the source help's default width until the CLI formatting audit.
    include_str!("sweep_help.txt").replacen("sweep_plan.py", program, 1)
}
fn usage(program: &str) -> String {
    format!("{}\n", help(program).split_once("\n\n").unwrap().0)
}

pub fn command(program: &str, args: &[String], root: &Path, cwd: &Path) -> Result<CommandOutput> {
    let fail = |message: &str| CommandOutput {
        exit: Some(2),
        stderr: format!("{}{program}: error: {message}\n", usage(program)),
        ..CommandOutput::default()
    };
    let mut sweep = Sweep::default();
    let mut format = String::from("estimate");
    let mut show_cost = false;
    let mut unknown = Vec::new();
    let mut position = 0;
    const OPTIONS: &[&str] = &[
        "--help",
        "--domain",
        "--arms",
        "--repos",
        "--seeds",
        "--limit",
        "--show-cost",
        "--sec-per-task",
        "--usd-per-task",
        "--workload",
        "--format",
    ];
    while position < args.len() {
        let arg = &args[position];
        position += 1;
        if arg == "--" {
            unknown.extend_from_slice(&args[position..]);
            break;
        }
        let (name, explicit) = arg
            .split_once('=')
            .map_or((arg.as_str(), None), |(a, b)| (a, Some(b)));
        let matches: Vec<_> = OPTIONS
            .iter()
            .copied()
            .filter(|option| {
                *option == name || (name.starts_with("--") && option.starts_with(name))
            })
            .collect();
        let name = if name == "-h" {
            "--help"
        } else if matches.len() == 1 {
            matches[0]
        } else if matches.len() > 1 {
            return Ok(fail(&format!(
                "ambiguous option: {name} could match {}",
                matches.join(", ")
            )));
        } else {
            unknown.push(arg.clone());
            continue;
        };
        if name == "--help" || name == "--show-cost" {
            if let Some(value) = explicit {
                return Ok(fail(&format!(
                    "argument {}: ignored explicit argument {}",
                    if name == "--help" { "-h/--help" } else { name },
                    python_repr(&json!(value))
                )));
            }
            if name == "--help" {
                return Ok(CommandOutput {
                    exit: Some(0),
                    stdout: help(program),
                    ..CommandOutput::default()
                });
            }
            show_cost = true;
            continue;
        }
        let multiple = matches!(name, "--arms" | "--repos" | "--seeds");
        let mut values = Vec::new();
        if let Some(value) = explicit {
            values.push(value.to_owned());
        } else {
            while position < args.len() && !is_option(&args[position]) && args[position] != "--" {
                values.push(args[position].clone());
                position += 1;
                if !multiple {
                    break;
                }
            }
        }
        if !multiple && values.is_empty() {
            return Ok(fail(&format!("argument {name}: expected one argument")));
        }
        match name {
            "--domain" | "--format" => {
                let accepted: Vec<String> = if name == "--domain" {
                    vec!["swebench".into(), "webbench".into()]
                } else {
                    vec![
                        "cells".into(),
                        "estimate".into(),
                        "arms".into(),
                        "counts".into(),
                    ]
                };
                if !accepted.contains(&values[0]) {
                    return Ok(fail(&invalid_choice(name, &values[0], &accepted)));
                }
                if name == "--domain" {
                    sweep.domain = values.remove(0);
                } else {
                    format = values.remove(0);
                }
            }
            "--arms" => sweep.arms = values,
            "--repos" => sweep.repos = values,
            "--seeds" | "--limit" => {
                let mut numbers = Vec::new();
                for value in values {
                    // shortcut: seeds use JSON integers and limits use i64; widen in the numeric audit.
                    let parsed = decimal_integer(&value)
                        .and_then(|s| s.parse::<i128>().ok())
                        .and_then(|n| serde_json::to_value(n).ok());
                    let Some(parsed) = parsed else {
                        return Ok(fail(&format!(
                            "argument {name}: invalid int value: {}",
                            python_repr(&json!(value))
                        )));
                    };
                    numbers.push(parsed);
                }
                if name == "--seeds" {
                    sweep.seeds = numbers;
                } else {
                    sweep.limit = numbers[0]
                        .as_i64()
                        .ok_or_else(|| Failure::new("OverflowError", "sweep limit exceeds i64"))?;
                }
            }
            "--sec-per-task" | "--usd-per-task" => {
                // shortcut: rates use finite Rust decimals; extend spellings in the CLI numeric audit.
                let Some(value) = values[0]
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|n| n.is_finite())
                else {
                    return Ok(fail(&format!(
                        "argument {name}: invalid float value: {}",
                        python_repr(&json!(values[0]))
                    )));
                };
                if name == "--sec-per-task" {
                    sweep.sec_per_task = value;
                } else {
                    sweep.usd_per_task = json!(value);
                }
            }
            "--workload" => sweep.manifest = Some(PathBuf::from(&values[0])),
            _ => unreachable!(),
        }
    }
    if !unknown.is_empty() {
        return Ok(fail(&format!(
            "unrecognized arguments: {}",
            unknown.join(" ")
        )));
    }
    if sweep.arms.is_empty() {
        sweep.arms = if sweep.domain == "webbench" {
            WEBBENCH_ARMS
        } else {
            SWEBENCH_ARMS
        }
        .iter()
        .map(|s| (*s).into())
        .collect();
    }
    if sweep.repos.is_empty() {
        sweep.repos = group_sizes(&sweep.domain, sweep.manifest.as_deref(), root, cwd)?
            .into_keys()
            .collect();
        sweep.repos.sort();
    }
    let bad: Vec<_> = sweep
        .arms
        .iter()
        .filter(|arm| !arms::ARM_ORDER.contains(&arm.as_str()))
        .cloned()
        .collect();
    if !bad.is_empty() {
        return Err(Failure::new(
            "SystemExit",
            format!("unknown arm(s): {}", bad.join(" ")),
        ));
    }
    let mut out = CommandOutput::default();
    match format.as_str() {
        "arms" => {
            for arm in &sweep.arms {
                out.stdout.push_str(&format!("{arm}\n"));
            }
        }
        "counts" => {
            let mut groups: Vec<_> =
                group_sizes(&sweep.domain, sweep.manifest.as_deref(), root, cwd)?
                    .into_iter()
                    .collect();
            groups.sort();
            for (group, size) in groups {
                out.stdout.push_str(&format!("{group}\t{size}\n"));
            }
        }
        "cells" | "estimate" => {
            let unavailable: Vec<_> = sweep
                .arms
                .iter()
                .filter(|arm| {
                    sweep.domain == "webbench" && WEBBENCH_UNAVAILABLE.contains(&arm.as_str())
                })
                .cloned()
                .collect();
            if format == "cells" && !unavailable.is_empty() {
                out.stderr.push_str(&format!(
                    "# SKIPPING scaffold arms not wired for webbench: {}\n",
                    unavailable.join(" ")
                ));
            }
            sweep.arms.retain(|arm| !unavailable.contains(arm));
            if format == "cells" {
                for (arm, repo, seed) in cells(&sweep.arms, &sweep.repos, &sweep.seeds) {
                    out.stdout
                        .push_str(&format!("{arm}\t{repo}\t{}\n", scalar_text(&seed)));
                }
            } else {
                let estimate = sweep.estimate(root, cwd)?;
                let unknown: Vec<_> = estimate["unknown_repos"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|s| s.as_str().unwrap())
                    .collect();
                if !unknown.is_empty() {
                    out.stderr.push_str(&format!(
                        "!! not in the {} workload record: {} (counted as 0 tasks)\n",
                        sweep.domain,
                        unknown.join(" ")
                    ));
                }
                out.stdout
                    .push_str(&format!("{}\n", format_estimate(&estimate, show_cost)?));
                if show_cost {
                    out.stdout.push_str(&format!("  at {:.0}s and ${:.2} per task-run (measured means, heavy-tailed -- an order of magnitude, not a quote)\n",sweep.sec_per_task,number(&sweep.usd_per_task).unwrap()));
                }
            }
        }
        _ => unreachable!(),
    }
    Ok(out)
}
