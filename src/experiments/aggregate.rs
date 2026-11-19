//! Flatten benchmark results into the per-task and per-cell comparison indexes.
use crate::{
    ledger, pricing,
    python_json::Node,
    store::Store,
    value::{int_value, number, python_repr, round_decimal, scalar_text, truthy},
};
use indexmap::IndexMap;
use regex::Regex;
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
    sync::LazyLock,
    time::UNIX_EPOCH,
};

pub const FIELDS: &[&str] = &[
    "arm",
    "repo",
    "seed",
    "instance",
    "repo_full",
    "mode",
    "resolved",
    "resolved_p1",
    "resolved_p2",
    "webbench_attempts",
    "is_error",
    "aborted",
    "num_turns",
    "usage_incomplete",
    "usage_incomplete_reason",
    "output_tokens_missing",
    "not_graded",
    "not_graded_reason",
    "tokens_total",
    "input_tokens",
    "output_tokens",
    "cache_read_input_tokens",
    "cache_creation_input_tokens",
    "cost_usd",
    "vendor_cost_missing",
    "vendor_cost_missing_reason",
    "effective_tokens",
    "overhead_tokens",
    "cost_effective_tokens",
    "wall_clock_s",
    "patch_bytes",
    "model",
    "effort",
    "run_dir",
    "source_path",
];
pub const USD_PER_EFFECTIVE_TOKEN: f64 = 5.0 / 1_000_000.0;
pub const AGENTS_WITHOUT_VENDOR_USD: &[&str] = &["copilot", "codex"];

fn integer_sum(row: &Value, keys: &[&str]) -> Option<i128> {
    keys.iter().try_fold(0, |sum, key| {
        let value = &row[key];
        let integer = match value {
            Value::Number(value) => value
                .as_i64()
                .map(i128::from)
                .or_else(|| value.as_u64().map(i128::from))?,
            Value::Bool(value) => i128::from(*value),
            _ => 0,
        };
        Some(sum + integer)
    })
}

fn rounded_cost(value: f64, integer: Option<i128>) -> Result<Value, String> {
    if let Some(integer) = integer {
        serde_json::to_value(integer)
            .map_err(|_| "token sum exceeds the native JSON integer range".into())
    } else {
        Ok(json!(round_decimal(value, 1)))
    }
}

fn get<'a>(row: &'a Value, key: &str) -> &'a Value {
    static EMPTY: LazyLock<Value> = LazyLock::new(|| json!(""));
    row.get(key).unwrap_or(&EMPTY)
}

fn positive(row: &Value, key: &str) -> bool {
    number(&row[key]).is_some_and(|n| n > 0.0)
}

pub fn usage_incomplete(row: &Value) -> (bool, Value) {
    if truthy(&row["usage_incomplete"]) {
        let reason = &row["usage_incomplete_reason"];
        return (
            true,
            if truthy(reason) {
                reason.clone()
            } else {
                json!("harness_flagged")
            },
        );
    }
    if (truthy(&row["resolved"]) || positive(row, "patch_bytes"))
        && !truthy(&row["tokens_total"])
        && !truthy(&row["input_tokens"])
    {
        return (true, json!("legacy_zero_usage"));
    }
    (false, json!(""))
}

pub fn not_graded(row: &Value) -> (bool, Value) {
    let report = &row["eval_report"];
    if truthy(&report["eval_exec_failed"]) {
        let reason = &report["eval_exec_error"];
        return (
            true,
            if truthy(reason) {
                reason.clone()
            } else {
                json!(format!(
                    "eval exec rc={}",
                    scalar_text(&report["eval_exec_rc"])
                ))
            },
        );
    }
    if truthy(&report["grading_error"]) {
        return (
            true,
            json!(format!(
                "grading_error: {}",
                scalar_text(&report["grading_error"])
            )),
        );
    }
    (false, json!(""))
}

pub fn vendor_usd(row: &Value) -> Value {
    ["cost_usd_vendor", "cost_usd"]
        .into_iter()
        .find_map(|key| row[key].is_number().then(|| row[key].clone()))
        .unwrap_or_else(|| json!(""))
}

pub fn vendor_cost_missing(row: &Value) -> (bool, Value) {
    let agent = if truthy(&row["agent"]) {
        scalar_text(&row["agent"])
    } else {
        String::new()
    };
    if AGENTS_WITHOUT_VENDOR_USD
        .iter()
        .any(|prefix| agent.starts_with(prefix))
        || vendor_usd(row) != ""
        || !(row["resolved"] == true
            || ["patch_bytes", "tokens_total", "input_tokens"]
                .into_iter()
                .any(|key| positive(row, key)))
    {
        return (false, json!(""));
    }
    (
        true,
        json!(format!(
            "{} reported no total_cost_usd",
            if agent.is_empty() { "agent" } else { &agent }
        )),
    )
}

pub fn normalize_repo(repo: &str, instance: &str) -> String {
    let repo = if repo.is_empty() {
        instance.rsplit_once('-').map_or(instance, |(repo, _)| repo)
    } else {
        repo
    };
    let repo = repo
        .replace("scikit-learn", "sklearn")
        .replace("pydata__xarray", "xarray");
    repo.split("__").next().unwrap_or("").into()
}

fn relative(path: &Path, base: &Path) -> io::Result<PathBuf> {
    let path = Store::new(path)?.base;
    let base = Store::new(base)?.base;
    let path: Vec<_> = path.components().collect();
    let base: Vec<_> = base.components().collect();
    let common = path.iter().zip(&base).take_while(|(a, b)| a == b).count();
    let mut result = PathBuf::new();
    for _ in common..base.len() {
        result.push("..");
    }
    for component in &path[common..] {
        result.push(component);
    }
    if result.as_os_str().is_empty() {
        result.push(".");
    }
    Ok(result)
}

pub fn cell_of(path: &Path, root: &Path, arm: &str) -> io::Result<(String, String)> {
    let path = relative(path, &root.join(arm))?;
    let cell = path
        .components()
        .next()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(cell.rsplit_once("_s").map_or_else(
        || (cell.clone(), String::new()),
        |(repo, seed)| (repo.into(), seed.into()),
    ))
}

// glob("**/row.json") skips hidden directories, follows directory symlinks and ignores scan errors.
fn row_paths(root: &Path, paths: &mut Vec<PathBuf>) {
    let row = root.join("row.json");
    if row.symlink_metadata().is_ok() {
        paths.push(row);
    }
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !entry.file_name().as_encoded_bytes().starts_with(b".") && path.is_dir() {
                row_paths(&path, paths);
            }
        }
    }
}

pub fn discover_arms(root: &Path) -> io::Result<Vec<String>> {
    let mut arms = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.path().is_dir() {
            let mut paths = Vec::new();
            row_paths(&entry.path(), &mut paths);
            if !paths.is_empty() {
                arms.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
    }
    arms.sort();
    Ok(arms)
}

pub fn run_order(path: &Path) -> (u8, String, String) {
    static STAMP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d{8}T\d{6}Z)__").unwrap());
    let name = path
        .parent()
        .and_then(Path::file_name)
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    if let Some(capture) = STAMP.captures(&name) {
        return (1, capture[1].into(), name);
    }
    let modified = fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .map(|time| match time.duration_since(UNIX_EPOCH) {
            Ok(value) => value.as_secs().to_string(),
            Err(error) => (-(error.duration().as_secs() as i128)).to_string(),
        })
        .unwrap_or_default();
    (0, modified, name)
}

pub fn collect(root: &Path, arms: &[String], output: &mut dyn Write) -> Result<Vec<Value>, String> {
    let mut newest = BTreeMap::new();
    let mut superseded = 0;
    for arm in arms {
        let mut paths = Vec::new();
        row_paths(&root.join(arm), &mut paths);
        for path in paths {
            let decoded = fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|text| {
                    let text = text.replace("\r\n", "\n").replace('\r', "\n");
                    serde_json::from_str::<Value>(&text).map_err(|e| {
                        crate::python_json::parse_result(text.as_bytes(), 4300)
                            .err()
                            .map_or_else(|| e.to_string(), |e| e.message(&text))
                    })
                });
            let data = match decoded {
                Ok(data) => data,
                Err(error) => {
                    writeln!(output, "SKIP (unreadable): {} {error}", path.display())
                        .map_err(|e| e.to_string())?;
                    continue;
                }
            };
            if !data.is_object() {
                return Err("row.json must contain an object".into());
            }
            let (repo, seed) = cell_of(&path, root, arm).map_err(|e| e.to_string())?;
            let instance = if truthy(&data["instance"]) {
                data["instance"].clone()
            } else {
                get(&data, "instance_id").clone()
            };
            let repo = normalize_repo(&repo, &scalar_text(&instance));
            let run_dir = get(&data, "run_dir");
            let rd = run_dir.as_str().ok_or("run_dir must be a string")?;
            let mode = if rd.contains("manual4") {
                "manual4"
            } else if rd.contains("standalone") {
                "standalone"
            } else {
                ""
            };
            let eff = pricing::effective_tokens(&data, None).map_err(|e| e.to_string())?;
            let overhead = ledger::overhead(&data);
            let exec_integer = if pricing::has_breakdown(&data) {
                None
            } else {
                integer_sum(&data, &["tokens_total"])
            };
            let overhead_integer = integer_sum(&data, &ledger::OVERHEAD_KEYS);
            let (incomplete, reason) = usage_incomplete(&data);
            let (ungraded, ungraded_reason) = not_graded(&data);
            let (vendor_missing, vendor_reason) = vendor_cost_missing(&data);
            let mut row = json!({
                "arm":arm, "repo":repo, "seed":data.get("seed").cloned().unwrap_or_else(|| json!(seed)),
                "instance":instance,"repo_full":get(&data,"repo"),"mode":mode,
                "cost_usd":vendor_usd(&data),"vendor_cost_missing":vendor_missing,"vendor_cost_missing_reason":vendor_reason,
                "effective_tokens":rounded_cost(eff,exec_integer)?,"overhead_tokens":rounded_cost(overhead,overhead_integer)?,"cost_effective_tokens":rounded_cost(eff+overhead,exec_integer.zip(overhead_integer).map(|(a,b)|a+b))?,
                "usage_incomplete":incomplete,"usage_incomplete_reason":reason,"not_graded":ungraded,"not_graded_reason":ungraded_reason,
                "run_dir":run_dir,"source_path":relative(&path,root).map_err(|e| e.to_string())?.to_string_lossy()
            });
            for key in FIELDS {
                if row.get(key).is_none() {
                    row[key] = get(&data, key).clone();
                }
            }
            let key = [
                arm.clone(),
                repo,
                scalar_text(&row["seed"]),
                scalar_text(&instance),
            ];
            let order = run_order(&path);
            match newest.entry(key) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert((order, row));
                }
                std::collections::btree_map::Entry::Occupied(mut entry) => {
                    superseded += 1;
                    if order > entry.get().0 {
                        entry.insert((order, row));
                    }
                }
            }
        }
    }
    if superseded > 0 {
        writeln!(
            output,
            "superseded runs skipped: {superseded} (older attempts of a re-run task; still on disk)"
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(newest.into_values().map(|(_, row)| row).collect())
}

pub fn summarize(rows: &[Value]) -> Result<IndexMap<[String; 3], Value>, String> {
    let mut cells = IndexMap::new();
    for row in rows {
        let key = [
            scalar_text(&row["arm"]),
            scalar_text(&row["repo"]),
            scalar_text(&row["seed"]),
        ];
        let cell = cells.entry(key).or_insert_with(|| json!({"n":0,"graded":0,"resolved":0,"errors":0,"eff":0.0,"turns":0,"incomplete":0,"ungraded":0,"p1":0,"p2":0,"graded_pn":0,"vendor_missing":0}));
        increment(cell, "n");
        for (source, target) in [
            ("usage_incomplete", "incomplete"),
            ("vendor_cost_missing", "vendor_missing"),
            ("is_error", "errors"),
        ] {
            if row[source] == true {
                increment(cell, target);
            }
        }
        if row["not_graded"] == true {
            increment(cell, "ungraded");
        } else if row["resolved"] != "" {
            increment(cell, "graded");
            if row["resolved"] == true {
                increment(cell, "resolved");
            }
            let mut p1 = get(row, "resolved_p1").clone();
            let mut p2 = get(row, "resolved_p2").clone();
            let attempts = get(row, "webbench_attempts");
            if p1 == "" && p2 == "" {
                if attempts != "" {
                    let count = if truthy(attempts) {
                        int_value(attempts).ok_or("invalid webbench_attempts")?
                    } else {
                        0.0
                    };
                    if count == 0.0 {
                        p1 = Value::Null;
                        p2 = Value::Null;
                    } else {
                        p2 = json!(row["resolved"] == true);
                        p1 = json!(p2 == true && count == 1.0);
                    }
                } else {
                    p1 = row["resolved"].clone();
                    p2 = p1.clone();
                }
            }
            if !p1.is_null() && p1 != "" {
                increment(cell, "graded_pn");
                if p1 == true {
                    increment(cell, "p1");
                }
                if p2 == true {
                    increment(cell, "p2");
                }
            }
        }
        if let Some(eff) = number(&row["cost_effective_tokens"]).or_else(|| {
            row["cost_effective_tokens"]
                .as_str()
                .and_then(|s| s.trim().parse::<f64>().ok())
        }) {
            cell["eff"] = json!(cell["eff"].as_f64().unwrap() + eff);
        }
        if let Some(turns) = int_value(&row["num_turns"]) {
            cell["turns"] = json!(cell["turns"].as_i64().unwrap() + turns as i64);
        }
    }
    Ok(cells)
}

fn increment(cell: &mut Value, key: &str) {
    cell[key] = json!(cell[key].as_u64().unwrap() + 1);
}

fn csv_row(output: &mut dyn Write, fields: impl IntoIterator<Item = Value>) -> io::Result<()> {
    for (index, value) in fields.into_iter().enumerate() {
        if index > 0 {
            write!(output, ",")?;
        }
        let text = if value.is_null() {
            String::new()
        } else {
            Node::from(&value).scalar().lossy()
        };
        if text.contains([',', '"', '\r', '\n']) {
            write!(output, "\"{}\"", text.replace('"', "\"\""))?;
        } else {
            write!(output, "{text}")?;
        }
    }
    write!(output, "\r\n")
}

const SUMMARY_FIELDS: &[&str] = &[
    "arm",
    "repo",
    "seed",
    "n_tasks",
    "n_graded",
    "n_resolved",
    "resolve_rate",
    "n_attempted",
    "n_resolved_p1",
    "resolve_rate_p1",
    "n_resolved_p2",
    "resolve_rate_p2",
    "n_errors",
    "n_usage_incomplete",
    "n_not_graded",
    "n_vendor_cost_missing",
    "sum_effective_tokens",
    "sum_cost_usd",
    "sum_turns",
    "mean_effective_tokens",
    "mean_cost_usd",
];

pub fn write_indexes(
    root: &Path,
    arms: &[String],
    out: &Path,
    output: &mut dyn Write,
) -> Result<(), String> {
    fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let rows = collect(root, arms, output)?;
    let mut file =
        BufWriter::new(File::create(out.join("all_rows.csv")).map_err(|e| e.to_string())?);
    csv_row(&mut file, FIELDS.iter().map(|key| json!(key))).map_err(|e| e.to_string())?;
    for row in &rows {
        csv_row(&mut file, FIELDS.iter().map(|key| row[key].clone())).map_err(|e| e.to_string())?;
    }
    file.flush().map_err(|e| e.to_string())?;
    let cells = summarize(&rows)?;
    let mut file =
        BufWriter::new(File::create(out.join("summary_by_cell.csv")).map_err(|e| e.to_string())?);
    csv_row(&mut file, SUMMARY_FIELDS.iter().map(|key| json!(key))).map_err(|e| e.to_string())?;
    for ([arm, repo, seed], cell) in &cells {
        let n = cell["n"].as_f64().unwrap();
        let g = cell["graded"].as_f64().unwrap();
        let gp = cell["graded_pn"].as_f64().unwrap();
        let eff = cell["eff"].as_f64().unwrap();
        let cost = eff * USD_PER_EFFECTIVE_TOKEN;
        let rate = |key: &str, denominator: f64| {
            if denominator != 0.0 {
                json!(round_decimal(cell[key].as_f64().unwrap() / denominator, 4))
            } else {
                json!("")
            }
        };
        csv_row(
            &mut file,
            vec![
                json!(arm),
                json!(repo),
                json!(seed),
                cell["n"].clone(),
                cell["graded"].clone(),
                cell["resolved"].clone(),
                rate("resolved", g),
                cell["graded_pn"].clone(),
                cell["p1"].clone(),
                rate("p1", gp),
                cell["p2"].clone(),
                rate("p2", gp),
                cell["errors"].clone(),
                cell["incomplete"].clone(),
                cell["ungraded"].clone(),
                cell["vendor_missing"].clone(),
                json!(round_decimal(eff, 1)),
                json!(round_decimal(cost, 4)),
                cell["turns"].clone(),
                json!(round_decimal(eff / n, 1)),
                json!(round_decimal(cost / n, 6)),
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    file.flush().map_err(|e| e.to_string())?;
    writeln!(output, "arms: {}", python_repr(&json!(arms))).map_err(|e| e.to_string())?;
    for (key, description) in [
        (
            "usage_incomplete",
            "rows have token fields that are a LOWER BOUND, not a measurement -- their cost is understated and their cells' means with it",
        ),
        (
            "vendor_cost_missing",
            "rows RAN but carry no vendor dollar figure -- summing the cost_usd column under-counts those cells, while their effective_tokens are complete. Compare on cost_effective_tokens, or re-run those tasks",
        ),
        (
            "not_graded",
            "rows were NEVER GRADED -- their eval script did not run, so `resolved` is an absence, not a miss. They are excluded from n_graded/resolve_rate; re-run those cells",
        ),
    ] {
        let mut by_arm = BTreeMap::<&str, usize>::new();
        for row in &rows {
            if row[key] == true {
                *by_arm.entry(row["arm"].as_str().unwrap()).or_default() += 1;
            }
        }
        if !by_arm.is_empty() {
            writeln!(
                output,
                "!! {key}: {} of {} {description}",
                by_arm.values().sum::<usize>(),
                rows.len()
            )
            .map_err(|e| e.to_string())?;
            for (arm, count) in by_arm {
                writeln!(output, "     {arm}: {count}").map_err(|e| e.to_string())?;
            }
            writeln!(output, "   see the {key}_reason column in all_rows.csv")
                .map_err(|e| e.to_string())?;
        }
    }
    writeln!(
        output,
        "all_rows.csv: {} task-runs -> {}\nsummary_by_cell.csv: {} cells -> {}",
        rows.len(),
        out.display(),
        cells.len(),
        out.display()
    )
    .map_err(|e| e.to_string())
}

#[derive(Default, Serialize)]
pub struct Output {
    pub exit: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

pub fn command(program: &str, args: &[String]) -> Result<Output, String> {
    let usage = format!("usage: {program} [-h] [--arms [ARMS ...]] [--out OUT] root\n");
    let fail = |message: &str| Output {
        exit: Some(2),
        stdout: String::new(),
        stderr: format!("{usage}{program}: error: {message}\n"),
    };
    let mut root = None;
    let mut arms: Option<Vec<String>> = None;
    let mut out = None;
    let mut position = 0;
    let mut positional_only = false;
    while position < args.len() {
        let arg = &args[position];
        position += 1;
        if !positional_only && (arg == "--help" || arg == "-h") {
            // shortcut: help uses the pinned default width; add terminal-width wrapping in the CLI parity audit.
            return Ok(Output {
                stdout: format!("{usage}{}", include_str!("aggregate_help.txt")),
                exit: Some(0),
                ..Output::default()
            });
        }
        if !positional_only && arg == "--" {
            positional_only = true;
            continue;
        }
        if !positional_only && arg == "--arms" {
            let mut selected = Vec::new();
            while position < args.len() && !args[position].starts_with('-') {
                selected.push(args[position].clone());
                position += 1;
            }
            arms = Some(selected);
        } else if !positional_only && (arg == "--out" || arg.starts_with("--out=")) {
            if let Some(value) = arg.strip_prefix("--out=") {
                out = Some(value.into());
            } else if position < args.len() && !args[position].starts_with('-') {
                out = Some(args[position].clone());
                position += 1;
            } else {
                return Ok(fail("argument --out: expected one argument"));
            }
        } else if !positional_only && arg.starts_with("--arms=") {
            arms = Some(vec![arg[7..].into()]);
        } else if (!positional_only && arg.starts_with('-') && arg != "-")
            || root.replace(arg.clone()).is_some()
        {
            return Ok(fail(&format!("unrecognized arguments: {arg}")));
        }
    }
    let Some(root_arg) = root else {
        return Ok(fail("the following arguments are required: root"));
    };
    let root = Store::new(Path::new(&root_arg))
        .map_err(|e| e.to_string())?
        .base;
    let present = discover_arms(&root).map_err(|e| e.to_string())?;
    if present.is_empty() {
        return Ok(fail(&format!(
            "no arm dirs with row.json under {}",
            root.display()
        )));
    }
    let selected = arms
        .as_ref()
        .filter(|arms| !arms.is_empty())
        .unwrap_or(&present);
    if arms.as_ref().is_some_and(|arms| !arms.is_empty()) && out.is_none() {
        let dropped: Vec<_> = present
            .iter()
            .filter(|a| !selected.contains(a))
            .cloned()
            .collect();
        if !dropped.is_empty() {
            return Ok(fail(&format!(
                "--arms would rewrite the canonical index with a SUBSET of the tree.\n  index:   {}\n  keeping: {}\n  DROPPED: {}\n  The index is what compare_arms.py reads, so this would\n  silently discard those arms' rows. Either\n    aggregate the whole tree:  {program} {root_arg}\n    or write the subset apart: --out <dir>",
                root.join("index").display(),
                selected.join(" "),
                dropped.join(" ")
            )));
        }
    }
    let unknown: Vec<_> = selected
        .iter()
        .filter(|a| !present.contains(a))
        .cloned()
        .collect();
    let mut output = Vec::new();
    if !unknown.is_empty() {
        writeln!(
            output,
            "!! --arms named {} dir(s) with no row.json under {}: {}",
            unknown.len(),
            root.display(),
            unknown.join(" ")
        )
        .unwrap();
    }
    let out = out
        .filter(|s: &String| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("index"));
    write_indexes(&root, selected, &out, &mut output)?;
    Ok(Output {
        stdout: String::from_utf8(output).unwrap(),
        ..Output::default()
    })
}
