//! Paired cell reports, including mechanism costs and saved injection telemetry.
use super::{
    CommandOutput,
    swebench::{Failure, Result, official},
};
use crate::{
    ledger,
    microskills::envfacts::EnvFactsDistiller,
    pricing,
    python_json::Node,
    value::{number, python_repr, python_type, sum_floats, sum_numbers, truthy},
};
use num_rational::BigRational;
use num_traits::ToPrimitive;
use serde_json::{Map, Value, json};
use std::{
    cmp::Ordering,
    fs,
    path::{Path, PathBuf},
};

pub type Rows = Vec<(Value, Value)>;
pub type Cell = (Rows, Rows);

fn mapping(value: &Value) -> Result<&Map<String, Value>> {
    value.as_object().ok_or_else(|| {
        Failure::new(
            "AttributeError",
            format!("'{}' object has no attribute 'get'", python_type(value)),
        )
    })
}
fn key_eq(a: &Value, b: &Value) -> bool {
    if (a.is_boolean() || a.is_number()) && (b.is_boolean() || b.is_number()) {
        rational(a)
            .ok()
            .zip(rational(b).ok())
            .is_some_and(|(a, b)| a == b)
    } else {
        a == b
    }
}
fn put(rows: &mut Rows, key: Value, value: Value) {
    if let Some((_, existing)) = rows.iter_mut().find(|(id, _)| key_eq(id, &key)) {
        *existing = value;
    } else {
        rows.push((key, value));
    }
}
fn find<'a>(rows: &'a Rows, key: &Value) -> Option<&'a Value> {
    rows.iter().find(|(id, _)| key_eq(id, key)).map(|(_, v)| v)
}
fn entries(root: &Path, cwd: &Path) -> Vec<PathBuf> {
    fs::read_dir(cwd.join(root))
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok().map(|entry| root.join(entry.file_name())))
        .collect()
}
fn glob(pattern: &Path, cwd: &Path) -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::new()];
    for component in pattern.components() {
        let text = component.as_os_str().to_string_lossy();
        if text.contains(['*', '?', '[']) {
            paths = paths
                .into_iter()
                .flat_map(|base| entries(&base, cwd))
                .filter(|path| {
                    let name = path.file_name().unwrap().to_string_lossy();
                    (!name.starts_with('.') || text.starts_with('.'))
                        && crate::toolprune::protected::fnmatchcase(&name, &text)
                })
                .collect();
        } else {
            paths = paths
                .into_iter()
                .map(|path| path.join(component.as_os_str()))
                .filter(|path| fs::symlink_metadata(cwd.join(path)).is_ok())
                .collect();
        }
    }
    paths
}
pub fn load_cell(cell: &Path, cwd: &Path) -> Result<Cell> {
    let mut paths = glob(&cell.join("20*/row.json"), cwd);
    paths.sort();
    let (mut rows, mut configs) = (Vec::new(), Vec::new());
    for path in paths {
        let row = match official::load(&path, cwd) {
            Err(e)
                if matches!(
                    e.kind.as_str(),
                    "JSONDecodeError" | "UnicodeDecodeError" | "ValueError"
                ) =>
            {
                continue;
            }
            other => other?,
        };
        let object = mapping(&row)?;
        let id = object
            .get("instance")
            .filter(|v| truthy(v))
            .or_else(|| object.get("instance_id"))
            .unwrap_or(&Value::Null)
            .clone();
        if !truthy(&id) {
            continue;
        }
        if id.is_array() || id.is_object() {
            return Err(Failure::new(
                "TypeError",
                format!(
                    "cannot use '{}' as a dict key (unhashable type: '{}')",
                    python_type(&id),
                    python_type(&id)
                ),
            ));
        }
        put(&mut rows, id.clone(), row);
        let config = path.parent().unwrap().join("run_config.json");
        if cwd.join(&config).exists() {
            match official::load(&config, cwd) {
                Ok(value) => put(&mut configs, id, value),
                Err(e)
                    if matches!(
                        e.kind.as_str(),
                        "JSONDecodeError" | "UnicodeDecodeError" | "ValueError"
                    ) => {}
                Err(e) => return Err(e),
            }
        }
    }
    Ok((rows, configs))
}
fn text(value: &Value) -> String {
    Node::from(value).scalar().lossy()
}
fn numeric(value: &Value) -> Result<f64> {
    number(value).ok_or_else(|| {
        Failure::new(
            "TypeError",
            format!(
                "can't convert type '{}' to numerator/denominator",
                python_type(value)
            ),
        )
    })
}
fn integer(value: &Value) -> Option<i128> {
    if let Value::Bool(v) = value {
        return Some(i128::from(*v));
    }
    value
        .as_i64()
        .map(i128::from)
        .or_else(|| value.as_u64().map(i128::from))
}
fn int_value(value: i128) -> Result<Value> {
    serde_json::to_value(value).map_err(|_| {
        Failure::new(
            "OverflowError",
            "report integer exceeds the native JSON range",
        )
    })
}
fn rational(value: &Value) -> Result<BigRational> {
    if let Some(value) = integer(value) {
        return Ok(BigRational::from_integer(value.into()));
    }
    BigRational::from_float(numeric(value)?)
        .ok_or_else(|| Failure::new("ValueError", "report statistics require finite values"))
}
pub fn cost(row: &Value) -> Result<Value> {
    let execution = pricing::effective_tokens(row, None).map_err(|error| {
        for key in [
            "input_tokens",
            "cache_read_input_tokens",
            "cache_creation_input_tokens",
            "output_tokens",
        ] {
            if error.to_string() == format!("pricing: {key} must be numeric") {
                let value = &row[key];
                let message = if key == "cache_creation_input_tokens" {
                    format!(
                        "unsupported operand type(s) for -: '{}' and 'int'",
                        python_type(value)
                    )
                } else if value.is_string() || value.is_array() {
                    "can't multiply sequence by non-int of type 'float'".into()
                } else {
                    format!(
                        "unsupported operand type(s) for *: '{}' and 'float'",
                        python_type(value)
                    )
                };
                return Failure::new("TypeError", message);
            }
        }
        Failure::new("ValueError", error.to_string())
    })?;
    let keys = std::iter::once("tokens_total").chain(ledger::OVERHEAD_KEYS);
    if !pricing::has_breakdown(row) {
        let mut exact = 0;
        let mut all_int = true;
        for key in keys {
            let value = &row[key];
            if let Some(n) = integer(value) {
                exact += n;
            } else if value.is_number() {
                all_int = false;
            }
        }
        if all_int {
            return int_value(exact);
        }
    }
    Ok(json!(execution + ledger::overhead(row)))
}
fn sum(values: &[Value]) -> Result<Value> {
    if let Some(values) = values.iter().map(integer).collect::<Option<Vec<_>>>() {
        return int_value(values.into_iter().sum());
    }
    for value in values {
        numeric(value)?;
    }
    Ok(json!(sum_numbers(values)))
}
fn mean(values: &[Value]) -> Result<Value> {
    if let Some(integers) = values.iter().map(integer).collect::<Option<Vec<_>>>() {
        let total: i128 = integers.into_iter().sum();
        if total % (values.len() as i128) == 0 {
            return int_value(total / (values.len() as i128));
        }
    }
    // Python statistics.mean rounds the exact sum divided by n only once.
    let mut exact = BigRational::from_integer(0.into());
    for value in values {
        exact += rational(value)?;
    }
    exact /= BigRational::from_integer(values.len().into());
    Ok(json!(exact.to_f64().ok_or_else(|| Failure::new(
        "OverflowError",
        "mean exceeds float range"
    ))?))
}
fn median(values: &[Value]) -> Result<Value> {
    let mut ordered: Vec<_> = values
        .iter()
        .map(|value| Ok((rational(value)?, value)))
        .collect::<Result<_>>()?;
    ordered.sort_by(|a, b| a.0.cmp(&b.0));
    let n = ordered.len();
    if n % 2 == 1 {
        return Ok(ordered[n / 2].1.clone());
    }
    Ok(json!(
        numeric(&sum(&[
            ordered[n / 2 - 1].1.clone(),
            ordered[n / 2].1.clone()
        ])?)?
            / 2.0
    ))
}
fn display(value: f64, decimals: usize, comma: bool, sign: bool, width: usize) -> String {
    let raw = if sign {
        format!("{value:+.decimals$}")
    } else {
        format!("{value:.decimals$}")
    }
    .replace("NaN", "nan");
    let raw = if comma {
        let (sign, digits) = raw
            .strip_prefix(['-', '+'])
            .map_or(("", raw.as_str()), |v| (&raw[..1], v));
        let (whole, fraction) = digits
            .split_once('.')
            .map_or((digits, None), |(a, b)| (a, Some(b)));
        let mut grouped = String::from(sign);
        for (i, c) in whole.chars().enumerate() {
            if i > 0 && (whole.len() - i).is_multiple_of(3) {
                grouped.push(',');
            }
            grouped.push(c);
        }
        if let Some(fraction) = fraction {
            grouped.push('.');
            grouped.push_str(fraction);
        }
        grouped
    } else {
        raw
    };
    format!("{raw:>width$}")
}
fn display_value(value: &Value, width: usize) -> Result<String> {
    Ok(display(numeric(value)?, 0, true, false, width))
}
fn percentage(a: &Value, b: &Value, ratio_first: bool) -> Result<f64> {
    let (an, bn) = (numeric(a)?, numeric(b)?);
    if bn == 0.0 {
        return Err(Failure::new("ZeroDivisionError", "division by zero"));
    }
    Ok(if ratio_first {
        let ratio = if integer(a).is_some() && integer(b).is_some() {
            (rational(a)? / rational(b)?).to_f64().unwrap()
        } else {
            an / bn
        };
        100.0 * (ratio - 1.0)
    } else if let (Some(a), Some(b)) = (integer(a), integer(b)) {
        (BigRational::from_integer((100 * (a - b)).into()) / BigRational::from_integer(b.into()))
            .to_f64()
            .unwrap()
    } else {
        100.0 * (an - bn) / bn
    })
}
pub fn describe(rows: &Rows, configs: &Rows, label: &str) -> Result<String> {
    let empty = json!({});
    let row = rows.first().map_or(&empty, |(_, r)| r);
    let config = configs.first().map_or(&empty, |(_, r)| r);
    let routing = &mapping(config)?
        .get("routing")
        .filter(|v| truthy(v))
        .unwrap_or(&empty);
    let endpoint = mapping(routing)?.get("base_url").unwrap_or(&Value::Null);
    mapping(row)?;
    Ok(format!(
        "  {label:9} n={:3} agent={} model={} effort={} endpoint={} cli={}\n",
        rows.len(),
        text(&row["agent"]),
        text(&row["model"]),
        text(&row["effort"]),
        text(endpoint),
        text(&row["cli_version"])
    ))
}

fn sort_ids(ids: &mut [Value]) -> Result<()> {
    let mut error = None;
    ids.sort_by(
        |a, b| match (a.as_str(), b.as_str(), number(a), number(b)) {
            (Some(a), Some(b), _, _) => a.cmp(b),
            (_, _, Some(_), Some(_)) => rational(a).unwrap().cmp(&rational(b).unwrap()),
            _ => {
                error = Some(Failure::new(
                    "TypeError",
                    format!(
                        "'<' not supported between instances of '{}' and '{}'",
                        python_type(a),
                        python_type(b)
                    ),
                ));
                Ordering::Equal
            }
        },
    );
    if let Some(error) = error {
        Err(error)
    } else {
        Ok(())
    }
}
fn resolved(row: &Value) -> bool {
    row["resolved"] == json!(true)
}
fn injection(configs: &Rows, id: &Value) -> Result<Value> {
    let empty = json!({});
    let config = find(configs, id).unwrap_or(&empty);
    let value = mapping(config)?
        .get("injection")
        .filter(|v| truthy(v))
        .unwrap_or(&empty);
    mapping(value)?;
    Ok(value.clone())
}
fn run(
    base_path: &Path,
    arm_path: &Path,
    out: Option<&Path>,
    cwd: &Path,
    stdout: &mut String,
) -> Result<()> {
    let (base, bcfg) = load_cell(base_path, cwd)?;
    let (arm, acfg) = load_cell(arm_path, cwd)?;
    stdout.push_str("== cells ==\n");
    stdout.push_str(&describe(&base, &bcfg, "baseline")?);
    stdout.push_str(&describe(&arm, &acfg, "arm")?);
    let mut common: Vec<_> = base
        .iter()
        .filter(|(id, _)| find(&arm, id).is_some())
        .map(|(id, _)| id.clone())
        .collect();
    sort_ids(&mut common)?;
    let only_b = base
        .iter()
        .filter(|(id, _)| find(&arm, id).is_none())
        .count();
    let only_a = arm
        .iter()
        .filter(|(id, _)| find(&base, id).is_none())
        .count();
    if only_b + only_a > 0 {
        stdout.push_str(&format!("  unpaired: {only_b} baseline-only, {only_a} arm-only (excluded from the paired figures)\n"));
    }
    let incomplete: Vec<_> = common
        .iter()
        .filter(|id| {
            truthy(&find(&base, id).unwrap()["usage_incomplete"])
                || truthy(&find(&arm, id).unwrap()["usage_incomplete"])
        })
        .cloned()
        .collect();
    if !incomplete.is_empty() {
        stdout.push_str(&format!(
            "  usage_incomplete on {} paired tasks (excluded): {}\n",
            incomplete.len(),
            python_repr(&json!(&incomplete[..incomplete.len().min(5)]))
        ));
    }
    let pairs: Vec<_> = common
        .into_iter()
        .filter(|id| !incomplete.contains(id))
        .collect();
    if pairs.is_empty() {
        return Err(Failure::new(
            "SystemExit",
            "no paired, complete tasks to compare",
        ));
    }
    let bc = pairs
        .iter()
        .map(|id| cost(find(&base, id).unwrap()))
        .collect::<Result<Vec<_>>>()?;
    let ac = pairs
        .iter()
        .map(|id| cost(find(&arm, id).unwrap()))
        .collect::<Result<Vec<_>>>()?;
    let deltas: Vec<_> = ac
        .iter()
        .zip(&bc)
        .map(|(a, b)| {
            Ok(json!(if numeric(b)? == 0.0 {
                0.0
            } else if let (Some(a), Some(b)) = (integer(a), integer(b)) {
                (BigRational::from_integer((a - b).into()) / BigRational::from_integer(b.into()))
                    .to_f64()
                    .unwrap()
            } else {
                (numeric(a)? - numeric(b)?) / numeric(b)?
            }))
        })
        .collect::<Result<_>>()?;
    let br = pairs
        .iter()
        .filter(|id| resolved(find(&base, id).unwrap()))
        .count();
    let ar = pairs
        .iter()
        .filter(|id| resolved(find(&arm, id).unwrap()))
        .count();
    let lost: Vec<_> = pairs
        .iter()
        .filter(|id| resolved(find(&base, id).unwrap()) && !resolved(find(&arm, id).unwrap()))
        .cloned()
        .collect();
    let won: Vec<_> = pairs
        .iter()
        .filter(|id| resolved(find(&arm, id).unwrap()) && !resolved(find(&base, id).unwrap()))
        .cloned()
        .collect();
    let oh = pairs
        .iter()
        .map(|id| ledger::overhead(find(&arm, id).unwrap()))
        .collect::<Vec<_>>();
    stdout.push_str(&format!("\n== paired comparison over {} tasks ==\n  {:22} {:>12} {:>12} {:>9}\n  {:22} {br:12} {ar:12} {:+9}\n",pairs.len(),"","baseline","arm","delta","resolved",(ar as i64)-(br as i64)));
    let (bm, am) = (mean(&bc)?, mean(&ac)?);
    let delta = percentage(&am, &bm, false)?;
    stdout.push_str(&format!(
        "  {:22} {} {} {}%\n",
        "mean eff tokens",
        display_value(&bm, 12)?,
        display_value(&am, 12)?,
        display(delta, 1, false, true, 8)
    ));
    let (bmed, amed) = (median(&bc)?, median(&ac)?);
    let delta = percentage(&amed, &bmed, false)?;
    stdout.push_str(&format!(
        "  {:22} {} {} {}%\n",
        "median eff tokens",
        display_value(&bmed, 12)?,
        display_value(&amed, 12)?,
        display(delta, 1, false, true, 8)
    ));
    let (bs, asum) = (sum(&bc)?, sum(&ac)?);
    let delta = percentage(&asum, &bs, false)?;
    stdout.push_str(&format!(
        "  {:22} {} {} {}%\n",
        "pooled eff tokens",
        display_value(&bs, 12)?,
        display_value(&asum, 12)?,
        display(delta, 1, false, true, 8)
    ));
    let turns = |rows: &Rows| {
        pairs
            .iter()
            .map(|id| {
                let row = find(rows, id).unwrap();
                let value = &row["num_turns"];
                if truthy(value) {
                    value.clone()
                } else {
                    json!(0)
                }
            })
            .collect::<Vec<_>>()
    };
    stdout.push_str(&format!(
        "  {:22} {} {}\n",
        "mean steps",
        display(numeric(&mean(&turns(&base))?)?, 1, false, false, 12),
        display(numeric(&mean(&turns(&arm))?)?, 1, false, false, 12)
    ));
    let (dm, dmed) = (mean(&deltas)?, median(&deltas)?);
    stdout.push_str(&format!(
        "  paired per-task delta: mean {}%, median {}%; arm cheaper on {}/{} tasks\n",
        display(100.0 * numeric(&dm)?, 1, false, true, 0),
        display(100.0 * numeric(&dmed)?, 1, false, true, 0),
        deltas
            .iter()
            .filter(|d| number(d).is_some_and(|v| v < 0.0))
            .count(),
        deltas.len()
    ));
    stdout.push_str(&format!(
        "  mechanism overhead charged to the arm: {} eff tokens\n",
        display(sum_floats(oh), 0, true, false, 0)
    ));
    stdout.push_str(&format!(
        "  resolve flips: lost {} {}  won {} {}\n",
        lost.len(),
        python_repr(&json!(&lost[..lost.len().min(6)])),
        won.len(),
        python_repr(&json!(&won[..won.len().min(6)]))
    ));
    let mut wp = Vec::new();
    let mut chars = Vec::new();
    for id in &pairs {
        let inj = injection(&acfg, id)?;
        if truthy(&inj["present"]) {
            wp.push(id.clone());
            chars.push(
                inj.get("chars")
                    .cloned()
                    .ok_or_else(|| Failure::new("KeyError", "chars"))?,
            );
        }
    }
    stdout.push_str("\n== injection telemetry (arm) ==\n");
    let chars_mean = if chars.is_empty() {
        json!(0)
    } else {
        mean(&chars)?
    };
    stdout.push_str(&format!(
        "  tasks with a prefix: {}/{}; prefix chars mean {} (~{} tokens)\n",
        wp.len(),
        pairs.len(),
        display_value(&chars_mean, 0)?,
        display(numeric(&chars_mean)? / 4.0, 0, true, false, 0)
    ));
    if !wp.is_empty() {
        for (name, group) in [
            ("with prefix", wp.clone()),
            (
                "without",
                pairs
                    .iter()
                    .filter(|id| !wp.contains(id))
                    .cloned()
                    .collect(),
            ),
        ] {
            if group.is_empty() {
                continue;
            }
            let means = |rows: &Rows| {
                mean(
                    &group
                        .iter()
                        .map(|id| cost(find(rows, id).unwrap()))
                        .collect::<Result<Vec<_>>>()?,
                )
            };
            let (a, b) = (means(&arm)?, means(&base)?);
            let delta = percentage(&a, &b, true)?;
            stdout.push_str(&format!("  {name:12} n={:3} arm mean={} baseline mean={} delta={}% resolved arm={} base={}\n",group.len(),display_value(&a,0)?,display_value(&b,0)?,display(delta,1,false,true,0),group.iter().filter(|id|resolved(find(&arm,id).unwrap())).count(),group.iter().filter(|id|resolved(find(&base,id).unwrap())).count()));
        }
    }
    for state in glob(&arm_path.join("envfacts_state/*.json"), cwd) {
        let d = official::load(&state, cwd)?;
        mapping(&d)?;
        stdout.push_str(&format!(
            "\n== learned facts ({}, {} tasks folded) ==\n",
            state.file_name().unwrap().to_string_lossy(),
            text(&d["now"])
        ));
        let empty = json!({});
        let raw_facts = d.get("facts").unwrap_or(&empty);
        let mut facts: Vec<_> = raw_facts
            .as_object()
            .ok_or_else(|| {
                Failure::new(
                    "AttributeError",
                    format!(
                        "'{}' object has no attribute 'items'",
                        python_type(raw_facts)
                    ),
                )
            })?
            .iter()
            .collect();
        facts.sort_by(|a, b| a.0.cmp(b.0));
        for (id, fact) in facts {
            let life = fact
                .get("life")
                .ok_or_else(|| Failure::new("KeyError", "life"))?;
            for key in ["state", "fire_num", "fire_den"] {
                if life.get(key).is_none() {
                    return Err(Failure::new("KeyError", key));
                }
            }
            stdout.push_str(&format!(
                "  {:9} fire={}/{} promoted@{} retired@{}  {id}\n",
                text(&life["state"]),
                text(&life["fire_num"]),
                text(&life["fire_den"]),
                text(&life["promoted_at"]),
                text(&life["retired_at"])
            ));
        }
        let repo = d["repo"]
            .as_str()
            .ok_or_else(|| Failure::new("KeyError", "repo"))?;
        let mut dist =
            EnvFactsDistiller::load(repo, &cwd.join(arm_path), Default::default(), None, None)
                .map_err(|e| Failure::new("ValueError", e))?;
        let block = dist.render();
        stdout.push_str(&format!(
            "  current block:{}\n",
            if block.is_empty() {
                " (none)".into()
            } else {
                format!("\n    {}", block.replace('\n', "\n    "))
            }
        ));
    }
    for stream in glob(&arm_path.join("_streams/*.jsonl"), cwd) {
        let mut first = None;
        // shortcut: whole-file decoding changes compound error order; stream if that fidelity is required.
        for (i, line) in official::load_text(&stream, cwd)?
            .split_terminator('\n')
            .enumerate()
        {
            let row = match official::parse(line) {
                Ok(v) => v,
                Err(e) if e.kind == "JSONDecodeError" => continue,
                Err(e) => return Err(e),
            };
            let empty = json!({});
            let mf = mapping(&row)?
                .get("manual_facts")
                .filter(|v| truthy(v))
                .unwrap_or(&empty);
            if first.is_none() {
                let values = mf.as_object().ok_or_else(|| {
                    Failure::new(
                        "AttributeError",
                        format!("'{}' object has no attribute 'values'", python_type(mf)),
                    )
                })?;
                if values
                    .values()
                    .any(|v| v.is_object() && truthy(&v["inject"]))
                {
                    first = Some(i);
                }
            }
        }
        stdout.push_str(&format!(
            "  first task index with an injectable fact (stream): {}\n",
            first.map_or("None".into(), |i| i.to_string())
        ));
    }
    if let Some(out) = out {
        let metrics = json!({"pairs":pairs.len(),"baseline_resolved":br,"arm_resolved":ar,"baseline_mean":bm,"arm_mean":am,"baseline_median":bmed,"arm_median":amed,"paired_delta_mean":dm,"paired_delta_median":dmed,"flips_lost":lost,"flips_won":won,"tasks_with_prefix":wp.len()});
        let encoded = Node::from(&metrics).json(true);
        let report = encoded
            .lines()
            .map(|line| {
                let spaces = line.len() - line.trim_start_matches(' ').len();
                format!("{}{}", " ".repeat(spaces / 2), &line[spaces..])
            })
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(cwd.join(out), report).map_err(|e| Failure::for_path(e, out))?;
    }
    Ok(())
}

pub fn command(program: &str, args: &[String], cwd: &Path) -> Result<CommandOutput> {
    let usage = format!("usage: {program} [-h] --baseline BASELINE --arm ARM [--json JSON]\n");
    let fail = |message: &str| CommandOutput {
        exit: Some(2),
        stderr: format!("{usage}{program}: error: {message}\n"),
        ..CommandOutput::default()
    };
    let mut fields = Map::new();
    let mut unknown = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        i += 1;
        let (name, explicit) = arg
            .split_once('=')
            .map_or((arg.as_str(), None), |(a, b)| (a, Some(b)));
        let flag = if name == "-h" || (name.starts_with("--h") && "--help".starts_with(name)) {
            "--help"
        } else if name.starts_with("--b") && "--baseline".starts_with(name) {
            "--baseline"
        } else if name.starts_with("--a") && "--arm".starts_with(name) {
            "--arm"
        } else if name.starts_with("--j") && "--json".starts_with(name) {
            "--json"
        } else {
            unknown.push(arg.clone());
            continue;
        };
        if flag == "--help" {
            if let Some(value) = explicit {
                return Ok(fail(&format!(
                    "argument -h/--help: ignored explicit argument {}",
                    python_repr(&json!(value))
                )));
            }
            return Ok(CommandOutput {
                exit: Some(0),
                stdout: format!("{usage}{}", include_str!("compare_help.txt")),
                ..CommandOutput::default()
            });
        }
        let value = if let Some(value) = explicit {
            value.to_owned()
        } else if i < args.len()
            && !crate::plugin::cli::parser::is_option(&args[i])
            && args[i] != "--"
        {
            i += 1;
            args[i - 1].clone()
        } else {
            return Ok(fail(&format!("argument {flag}: expected one argument")));
        };
        fields.insert(flag.into(), json!(value));
    }
    let missing: Vec<_> = ["--baseline", "--arm"]
        .into_iter()
        .filter(|f| !fields.contains_key(*f))
        .collect();
    if !missing.is_empty() {
        return Ok(fail(&format!(
            "the following arguments are required: {}",
            missing.join(", ")
        )));
    }
    if !unknown.is_empty() {
        return Ok(fail(&format!(
            "unrecognized arguments: {}",
            unknown.join(" ")
        )));
    }
    let mut output = CommandOutput::default();
    if let Err(mut error) = run(
        Path::new(fields["--baseline"].as_str().unwrap()),
        Path::new(fields["--arm"].as_str().unwrap()),
        fields
            .get("--json")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(Path::new),
        cwd,
        &mut output.stdout,
    ) {
        error.stdout = Some(Box::new(json!(output.stdout)));
        return Err(error);
    }
    Ok(output)
}
