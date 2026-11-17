use super::{Result, Runtime, field, text, when, words};
use crate::{
    plugin::repos,
    value::{json_pretty, python_repr, truthy},
};
use serde_json::{Value, json};
use std::path::Path;

pub fn print_moves(done: &Value, runtime: &mut dyn Runtime) -> Result<usize> {
    if !truthy(done) {
        return Ok(0);
    }
    let mut count = 0;
    for result in done
        .as_array()
        .ok_or("migration results must be an array")?
    {
        let Some(moves) = result.get("moves").filter(|v| truthy(v)) else {
            continue;
        };
        for entry in moves.as_array().ok_or("migration moves must be an array")? {
            count += 1;
            let what = if result["into"] == entry["into"] {
                "all of its state".to_string()
            } else {
                format!("{} session(s)", text(&entry["moved"]))
            };
            let refold = if truthy(&entry["refold"]) {
                format!(", {} to be folded there again", text(&entry["refold"]))
            } else {
                String::new()
            };
            runtime.say(&format!(
                "  {} -> {} ({}): {what}{refold}",
                text(&result["key"]),
                text(&entry["into"]),
                text(&entry["host"])
            ))?;
        }
    }
    Ok(count)
}

pub fn repositories(args: &Value, runtime: &mut dyn Runtime) -> Result<i32> {
    let root = runtime.global("state_root", json!([]))?;
    let root = Path::new(root.as_str().ok_or("state root must be text")?);
    if (args["action"] == "migrate" || args["action"] == "merge")
        && truthy(&runtime.global("refuse_unwritable", json!([root]))?)
    {
        return Ok(1);
    }
    if args["action"] == "migrate" {
        let done = runtime.global("repos_run_migration", json!([root]))?;
        if done.is_null() {
            runtime.say("another migration holds the state root; try again in a minute")?;
            return Ok(1);
        }
        if !truthy(&args["quiet"]) && print_moves(&done, runtime)? == 0 {
            runtime
                .say("nothing to move: every session is filed under the repository it ran in")?;
        }
        return Ok(0);
    }
    if args["action"] == "merge" {
        let keys = words(&args["keys"])?;
        if keys.is_empty() || keys.len() > 2 {
            runtime.say("usage: te repos merge KEY [INTO] (keys as `te repos` lists them)")?;
            return Ok(2);
        }
        if let Some(bad) = keys.iter().find(|key| {
            key.is_empty()
                || [".", ".."].contains(&key.as_str())
                || key.contains(['/', '\\'])
                || Path::new(key.as_str())
                    .file_name()
                    .is_none_or(|name| name != key.as_str())
        }) {
            runtime.say(&format!(
                "not a state directory key: {} (keys as `te repos` lists them)",
                python_repr(&json!(bad))
            ))?;
            return Ok(2);
        }
        let src = &keys[0];
        let dir = repos::repos_dir(root);
        if !dir.join(src).is_dir() {
            runtime.say(&format!(
                "no state directory {} in {}",
                python_repr(&json!(src)),
                dir.display()
            ))?;
            return Ok(2);
        }
        let into = if let Some(key) = keys.get(1) {
            json!(key)
        } else {
            json!(runtime.context(args)?.repo)
        };
        let into_key = into
            .as_str()
            .unwrap_or_else(|| into["key"].as_str().unwrap_or(""));
        if into_key == src {
            runtime.say(&format!(
                "{src} is this repository's own state; name the one to merge into it"
            ))?;
            return Ok(2);
        }
        let res = runtime.global("repos_merge", json!([root, src, into]))?;
        if res.is_null() {
            runtime.say(
                "the state root or one of the two directories is busy; try again in a minute",
            )?;
            return Ok(1);
        }
        let mut rows: Vec<_> = res
            .as_object()
            .ok_or("merge result must be an object")?
            .iter()
            .collect();
        rows.sort_by(|a, b| a.0.cmp(b.0));
        let summary = rows
            .iter()
            .map(|(host, counts)| {
                let again = if truthy(&counts["refold"]) {
                    format!(" ({} to be folded again)", text(&counts["refold"]))
                } else {
                    String::new()
                };
                format!("{host} {} session(s){again}", text(&counts["moved"]))
            })
            .collect::<Vec<_>>()
            .join(", ");
        runtime.say(&format!(
            "merged {src} into {into_key}: {}",
            if summary.is_empty() {
                "no sessions"
            } else {
                &summary
            }
        ))?;
        runtime.say(&format!(
            "the old directory is kept under {}",
            root.join("merged").display()
        ))?;
        if !into.is_string() {
            let moves: Vec<_> = rows
                .iter()
                .map(|(host, counts)| -> Result<Value> {
                    let mut value = counts
                        .as_object()
                        .cloned()
                        .ok_or("merge counts must be an object")?;
                    value.insert("host".into(), json!(host));
                    value.insert("cwd".into(), into["root"].clone());
                    Ok(json!(value))
                })
                .collect::<Result<_>>()?;
            runtime.global("repos_refold", json!([[{"moves":moves}]]))?;
        }
        return Ok(0);
    }
    let current = runtime.context(args)?.repo;
    let listing = runtime.global("repos_listing", json!([root, current]))?;
    let listing = listing
        .as_array()
        .filter(|items| items.len() == 2)
        .ok_or("repository listing must contain rows and merge count")?;
    let (rows, merged) = (&listing[0], &listing[1]);
    if truthy(&args["json"]) {
        runtime.say(&json_pretty(&json!({"repos":rows,"merged":merged}), false))?;
        return Ok(0);
    }
    let rows = rows.as_array().ok_or("repository rows must be an array")?;
    if rows.is_empty() {
        runtime.say(&format!(
            "no state yet in {}",
            repos::repos_dir(root).display()
        ))?;
    }
    for row in rows {
        runtime.say(&format!(
            "{}  [{}]{}",
            text(&row["label"]),
            text(&row["key"]),
            if truthy(&row["current"]) {
                "  <- this directory"
            } else {
                ""
            }
        ))?;
        runtime.say(&format!(
            "  identified   {}",
            if truthy(&row["how"]) {
                repos::how_words(
                    row["how"]
                        .as_str()
                        .ok_or("repository identity method must be text")?,
                    row["remote"]
                        .as_str()
                        .ok_or("repository remote must be text")?,
                )
            } else {
                "recorded by an earlier version".into()
            }
        ))?;
        let imported = row
            .get("imported")
            .filter(|v| truthy(v))
            .unwrap_or(&Value::Null);
        let agents = row["agents"]
            .as_object()
            .ok_or("repository agent counts must be an object")?;
        let sessions = agents
            .iter()
            .map(|(host, n)| {
                let earlier = if truthy(&imported[host]) {
                    format!(" (+{} from earlier history)", text(&imported[host]))
                } else {
                    String::new()
                };
                format!("{host} {}{earlier}", text(n))
            })
            .collect::<Vec<_>>()
            .join(", ");
        runtime.say(&format!(
            "  sessions     {}; last {}",
            if sessions.is_empty() {
                "none"
            } else {
                &sessions
            },
            when(&row["last"])?
        ))?;
        if truthy(&row["checkouts"]) {
            runtime.say(&format!(
                "  checkouts    {}",
                words(&row["checkouts"])?.join(", ")
            ))?;
        } else if truthy(&row["cwds"]) {
            let cwds = words(&row["cwds"])?;
            let extra = if cwds.len() > 3 {
                format!(" (+{})", cwds.len() - 3)
            } else {
                String::new()
            };
            runtime.say(&format!(
                "  ran in       {}{extra}",
                cwds.iter().take(3).cloned().collect::<Vec<_>>().join(", ")
            ))?;
        }
        for note in words(field(row, "notes", &json!([])))? {
            runtime.say(&format!("  note         {note}"))?;
        }
    }
    if truthy(merged) {
        runtime.say(&format!(
            "\n{} earlier state director{} merged into the above, kept under {}",
            text(merged),
            if merged == &json!(1) { "y" } else { "ies" },
            root.join("merged").display()
        ))?;
    }
    Ok(0)
}
