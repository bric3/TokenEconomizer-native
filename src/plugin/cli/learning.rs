use super::{Error, Result, Runtime, field, text, whitespace, words};
use crate::{
    lifecycle::NO_RECORD,
    microskills::{observe::Vocabulary, stage},
    plugin::{context::LiveContext, fold, history, repomap, repos},
    value::{json_text, truthy},
};
use chrono::{Local, TimeZone};
use serde_json::{Value, json};

pub fn short(value: &Value, n: i64) -> String {
    let text = text(value)
        .split(whitespace)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let len = text.chars().count() as i128;
    if len <= i128::from(n) {
        return text;
    }
    let end = i128::from(n) - 1;
    let end = if end < 0 {
        (len + end).max(0)
    } else {
        end.min(len)
    };
    format!(
        "{}\u{2026}",
        text.chars().take(end as usize).collect::<String>()
    )
}

pub fn facts(args: &Value, runtime: &mut dyn Runtime) -> Result<i32> {
    if args["action"] == "forget" || args["action"] == "unforget" {
        return forget_facts(args, runtime);
    }
    let ids = words(&args["ids"])?;
    if !ids.is_empty() {
        runtime.say(&format!(
            "`microskills {}` takes no ids; did you mean `microskills forget {}`?",
            text(&args["action"]),
            ids.join(" ")
        ))?;
        return Ok(2);
    }
    let ctx = runtime.context(args)?;
    let inj = ctx.store.read("injection.json", Value::Null)?;
    if ctx.config["microskills"] == "off" {
        runtime.say(&format!("micro-skills is off here (the default). `config set microskills=on` turns it on: facts are then learned from finished sessions, and a model proposes more every {} sessions, through this agent",text(&ctx.config["microskills_propose_every"])))?;
        if !truthy(&inj) {
            return Ok(0);
        }
    }
    if truthy(&inj) && !inj.is_object() {
        return Err("saved injection must be an object".into());
    }
    let rendered = inj
        .get("rendered")
        .filter(|v| truthy(v))
        .unwrap_or(&Value::Null);
    let rendered = if rendered.is_null() {
        ""
    } else {
        rendered.as_str().ok_or("rendered facts must be text")?
    };
    let rendered = rendered.trim_end_matches(whitespace);
    runtime.say(if rendered.is_empty() {
        "(nothing injected yet: no fact has cleared its firing gate)"
    } else {
        rendered
    })?;
    let facts = inj.get("facts").filter(|v| truthy(v));
    if let Some(facts) = facts {
        let mut rows: Vec<_> = facts
            .as_object()
            .ok_or("saved facts must be an object")?
            .iter()
            .filter(|(_, v)| v.is_object() && v.get("state").is_some())
            .collect();
        rows.sort_by(|a, b| a.0.cmp(b.0));
        if !rows.is_empty() {
            runtime.say("\nfacts tracked:")?;
            let unknown = json!("?");
            for (key, value) in rows {
                let state = field(value, "state", &unknown)
                    .as_str()
                    .ok_or("fact state must be text")?;
                let fire = field(value, "fire", &unknown)
                    .as_str()
                    .ok_or("fact firing count must be text")?;
                runtime.say(&format!(
                    "  {state:9} fire={fire:>5} {} {key}",
                    if truthy(&value["inject"]) {
                        "INJECTED"
                    } else {
                        "        "
                    }
                ))?;
            }
        }
    }
    let gone = stage::forgotten_facts(
        &ctx.repo.label,
        &ctx.store.base.join("mechanisms"),
        &Vocabulary::default(),
    )?;
    if !gone.is_empty() {
        runtime.say("\nforgotten (`microskills unforget ID` brings one back):")?;
        let mut rows: Vec<_> = gone.iter().collect();
        rows.sort_by(|a, b| a.0.cmp(b.0));
        for (key, claim) in rows {
            runtime.say(&format!(
                "  {key}{}",
                if truthy(claim) {
                    format!("  ({})", short(claim, 80))
                } else {
                    String::new()
                }
            ))?;
        }
    }
    Ok(0)
}

pub fn forget_facts(args: &Value, runtime: &mut dyn Runtime) -> Result<i32> {
    let undo = args["action"] == "unforget";
    let ids = words(&args["ids"])?;
    if ids.is_empty() {
        runtime.say(&format!(
            "`microskills {}` needs the id of at least one fact; `microskills` lists them",
            text(&args["action"])
        ))?;
        return Ok(2);
    }
    let ctx = runtime.context(args)?;
    let refused = runtime.call(&ctx, "refuse_unwritable", json!([ctx.root]))?;
    if truthy(&refused) {
        return Ok(refused.as_i64().ok_or("invalid refusal code")? as i32);
    }
    let done = match fold::forget_microskills(&ctx, &ids, undo) {
        Ok(done) => done,
        Err(fold::Error::UnknownIds(unknown)) => {
            runtime.say(&format!("not changed: no learned fact here has the id {} ({}, {}); `microskills` lists the ids",unknown.join(", "),ctx.host,ctx.repo.label))?;
            return Ok(1);
        }
        Err(error @ fold::Error::Busy) => {
            runtime.say(&format!("not changed: {error}; try again in a moment"))?;
            return Ok(1);
        }
        Err(fold::Error::Io(error)) => return Err(error.into()),
        Err(fold::Error::Value(message)) => {
            return Err(Error {
                kind: "ValueError".into(),
                message,
            });
        }
        Err(error) => return Err(error.to_string().into()),
    };
    let done = done
        .as_array()
        .ok_or("forgetting returned invalid results")?;
    for row in done {
        let row = row
            .as_array()
            .filter(|row| row.len() == 3)
            .ok_or("forgetting returned an invalid fact")?;
        let (fid, changed, claim) = (text(&row[0]), &row[1], &row[2]);
        if changed == NO_RECORD {
            runtime.say(&format!("no longer forgotten, but nothing to bring back: {fid} had no saved record; a later fold may learn it again"))?;
            continue;
        }
        let what = match (undo, truthy(changed)) {
            (true, true) => "brought back",
            (true, false) => "not forgotten, nothing to bring back",
            (false, true) => "forgotten",
            (false, false) => "already forgotten",
        };
        runtime.say(&format!(
            "{what}: {fid}{}",
            if truthy(claim) {
                format!("  ({})", short(claim, 80))
            } else {
                String::new()
            }
        ))?;
    }
    if done.iter().any(|row| truthy(&row[1])) {
        runtime.say(&format!("from the next session, {}",if undo {"these facts are kept and injected under the same rules as any other fact"} else {"these facts are not injected, and are not learned again under the same id or from the same evidence (`microskills unforget ID` undoes this)"}))?;
    }
    Ok(0)
}

pub fn session_context(
    args: &Value,
    session: &Value,
    runtime: &mut dyn Runtime,
) -> Result<LiveContext> {
    let ctx = runtime.context(args)?;
    if truthy(session) && repos::pinned(&ctx.root, &ctx.host, session)?.is_some() {
        let pinned = runtime.pinned_context(args, session)?;
        if pinned
            .store
            .base
            .join("sessions")
            .join(format!("{}.json", text(session)))
            .exists()
        {
            return Ok(pinned);
        }
    }
    Ok(ctx)
}

pub fn fold_command(args: &Value, runtime: &mut dyn Runtime) -> Result<i32> {
    let sessions = args["session"].as_array();
    let ctx = session_context(
        args,
        sessions.and_then(|s| s.first()).unwrap_or(&Value::Null),
        runtime,
    )?;
    let sessions = if truthy(&args["session"]) {
        args["session"].clone()
    } else {
        runtime.call(&ctx, "pending_sessions", json!([]))?
    };
    for sid in sessions.as_array().ok_or("session ids must be an array")? {
        let rec = runtime.call(&ctx, "fold_session", json!([sid]))?;
        if !rec.is_null() && truthy(&args["verbose"]) {
            runtime.say(&json_text(&rec))?;
        }
    }
    Ok(0)
}

pub fn judge_command(args: &Value, runtime: &mut dyn Runtime) -> Result<i32> {
    let ctx = session_context(args, &args["session"], runtime)?;
    runtime.call(&ctx, "judge_job", json!([args["session"], args["job"]]))?;
    Ok(0)
}

pub fn when(ts: &Value) -> Result<String> {
    if !truthy(ts) {
        return Ok("-".into());
    }
    let ts = ts
        .as_f64()
        .or_else(|| ts.as_bool().map(|v| if v { 1.0 } else { 0.0 }))
        .filter(|n| n.is_finite())
        .ok_or("timestamp must be a finite number")?;
    // shortcut: CLI dates use Chrono's supported calendar range, reconcile extreme dates in the timestamp audit.
    if ts < i64::MIN as f64 || ts >= -(i64::MIN as f64) {
        return Err("timestamp is out of range".into());
    }
    Local
        .timestamp_opt(ts.floor() as i64, 0)
        .single()
        .map(|time| time.format("%Y-%m-%d %H:%M").to_string())
        .ok_or_else(|| "timestamp is out of range".into())
}

fn skipped(doc: &Value, runtime: &mut dyn Runtime) -> Result<()> {
    let mut skipped = doc["skipped"].as_object().cloned().unwrap_or_default();
    if truthy(&doc["over_limit"]) {
        skipped.insert("older_than_the_newest".into(), doc["over_limit"].clone());
    }
    if !skipped.is_empty() {
        let mut rows: Vec<_> = skipped.iter().collect();
        rows.sort_by(|a, b| a.0.cmp(b.0));
        runtime.say(&format!(
            "left out: {}",
            rows.into_iter()
                .map(|(key, value)| format!("{key} {}", text(value)))
                .collect::<Vec<_>>()
                .join(", ")
        ))?;
    }
    Ok(())
}
pub fn print_history(ctx: &LiveContext, doc: &Value, runtime: &mut dyn Runtime) -> Result<()> {
    if !truthy(doc) {
        runtime.say(&format!(
            "no import of earlier sessions recorded for {}{}",
            ctx.repo.label,
            if truthy(&ctx.config["history_import"]) {
                ""
            } else {
                " (history_import is off)"
            }
        ))?;
        return Ok(());
    }
    let description = history::describe(doc, &ctx.host);
    runtime.say(&format!(
        "{}: {}",
        ctx.repo.label,
        text(if truthy(&description) {
            &description
        } else {
            &doc["state"]
        })
    ))?;
    skipped(doc, runtime)
}

pub fn history_command(args: &Value, runtime: &mut dyn Runtime) -> Result<i32> {
    let ctx = runtime.context(args)?;
    if args["action"] == "status" || truthy(&args["status"]) {
        let doc = runtime.call(&ctx, "history_summary", json!([]))?;
        print_history(&ctx, &doc, runtime)?;
        return Ok(0);
    }
    if !ctx.keeps_history {
        runtime.say(&format!(
            "{}: no session history the plugin reads",
            ctx.host
        ))?;
        return Ok(1);
    }
    if truthy(&args["dry_run"]) {
        let plan = runtime.call(&ctx, "history_plan", json!([]))?;
        let sessions = plan["sessions"]
            .as_array()
            .ok_or("history plan has no session list")?;
        let dates = if sessions.is_empty() {
            String::new()
        } else {
            format!(
                ", oldest {}, newest {}",
                when(&sessions[0]["started"])?,
                when(&sessions.last().unwrap()["started"])?
            )
        };
        runtime.say(&format!(
            "{} earlier session(s) of {} would be read{dates}",
            sessions.len(),
            ctx.repo.label
        ))?;
        skipped(&plan, runtime)?;
        let why = runtime.call(&ctx, "history_refusal", json!([false]))?;
        if truthy(&why) {
            runtime.say(&format!("not importable now: {}", text(&why)))?;
        }
        return Ok(0);
    }
    if truthy(&runtime.call(&ctx, "refuse_unwritable", json!([ctx.store.base]))?) {
        return Ok(1);
    }
    let res = runtime.call(&ctx, "history_run", json!([truthy(&args["auto"])]))?;
    if truthy(&args["auto"]) {
        return Ok(0);
    }
    if ["refused", "skipped", "busy"]
        .iter()
        .any(|state| res["state"] == *state)
    {
        runtime.say(&format!("not imported: {}", text(&res["reason"])))?;
        return Ok(1);
    }
    let partial = if res["state"] == "partial" {
        format!(
            "; stopped at its time budget with {} to go (`te history import` resumes)",
            text(&res["left"])
        )
    } else {
        String::new()
    };
    runtime.say(&format!(
        "imported {} earlier session(s) of {}{partial}",
        text(&res["imported"]),
        ctx.repo.label
    ))?;
    Ok(0)
}

pub fn map_command(args: &Value, runtime: &mut dyn Runtime) -> Result<i32> {
    let ctx = runtime.context(args)?;
    let refresh = args["action"] == "refresh";
    let state = runtime.call(
        &ctx,
        if refresh {
            "repomap_refresh"
        } else {
            "repomap_state"
        },
        json!([]),
    )?;
    if !truthy(&state["map"]) {
        let error = if truthy(&state["error"]) {
            format!(": {}", text(&state["error"]))
        } else {
            String::new()
        };
        runtime.say(&format!("no map cached for {}{error}", ctx.repo.label))?;
        if ctx.config["repomap"] == "off" {
            runtime.say("the map is off; turn it on with `te config set repomap=on`")?;
        }
        return Ok(i32::from(refresh));
    }
    let current = state["commit"] == runtime.call(&ctx, "head_commit", json!([ctx.repo.root]))?;
    // shortcut: displayed map sizes use i64, extend their range in the production numeric audit.
    let chars = state["chars"]
        .as_i64()
        .ok_or("map character count must be an integer")?;
    let commit = text(
        state
            .get("commit")
            .filter(|v| truthy(v))
            .unwrap_or(&json!("?")),
    )
    .chars()
    .take(10)
    .collect::<String>();
    runtime.say(&format!(
        "{}: ~{} tokens (budget {}), commit {commit}{}, made in {}s with {}",
        ctx.repo.label,
        chars.div_euclid(4),
        text(&state["tokens_budget"]),
        if current { "" } else { " (an older commit)" },
        text(&state["secs"]),
        text(&state["python"])
    ))?;
    if truthy(&state["error"]) {
        runtime.say(&format!("last refresh failed: {}", text(&state["error"])))?;
    }
    let map = state["map"].as_str().ok_or("cached map must be text")?;
    runtime.say(&format!(
        "{}{}{}",
        repomap::HEADER,
        map.chars().take(2000).collect::<String>(),
        if map.chars().count() > 2000 {
            "\n..."
        } else {
            ""
        }
    ))?;
    Ok(0)
}
