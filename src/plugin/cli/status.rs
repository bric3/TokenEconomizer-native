use super::{Result, Runtime, field, text, words};
use crate::{
    accounting,
    envelope::{self, EnvelopeTracker},
    plugin::{context::LiveContext, denylist, history, repos},
    value::{int_value, json_pretty, truthy},
};
use serde_json::{Value, json};

fn count(value: &Value, key: &str) -> Result<i64> {
    let Some(value) = value.get(key).filter(|v| truthy(v)) else {
        return Ok(0);
    };
    if let Some(n) = value.as_i64().or_else(|| {
        value
            .as_str()
            .and_then(|s| s.trim_matches(super::whitespace).parse().ok())
    }) {
        return Ok(n);
    }
    // shortcut: CLI counters use i64, extend their range in the production numeric audit.
    int_value(value)
        .filter(|n| *n >= i64::MIN as f64 && *n < -(i64::MIN as f64))
        .map(|n| n as i64)
        .ok_or_else(|| "invalid saved counter".into())
}

fn learned(value: &Value, key: &str, imported: i64) -> Result<i64> {
    count(value, key)?
        .checked_sub(imported)
        .ok_or_else(|| "saved session count exceeds i64".into())
}

pub fn status_explanations(
    ctx: &LiveContext,
    inj: &Value,
    unt: &Value,
    tracker: &EnvelopeTracker,
    applied: &Value,
    runtime: &mut dyn Runtime,
) -> Result<Value> {
    let cfg = &ctx.config;
    if !truthy(&cfg["enabled"]) {
        return Ok(json!({"plugin":"plugin is disabled; hooks record nothing"}));
    }
    let mut notes = json!({});
    if truthy(&cfg["shadow"]) {
        notes["plugin"] = json!("shadow mode: new decisions are recorded, not applied");
    }
    let facts_detail = "see microskills with the same --host/--cwd";
    let tools_detail = "see toolprune with the same --host/--cwd";
    let paused = truthy(&runtime.call(ctx, "paused", json!([]))?);
    let distiller = cfg["microskills"]
        .as_str()
        .is_some_and(|s| s.contains("distiller"));
    let facts = if cfg["microskills"] == "off" {
        "off; cached facts are not injected".into()
    } else if truthy(&inj["rendered"]) {
        let mut why = if truthy(&cfg["shadow"]) {
            "cached facts present; injection suppressed in shadow mode"
        } else {
            "cached facts are available for session start"
        }
        .to_string();
        if distiller && paused {
            why.push_str("; model proposer paused");
        }
        why
    } else if distiller && paused {
        format!("model proposer paused after a sign-in failure; {facts_detail}")
    } else if !truthy(inj) {
        "no learning snapshot recorded for this agent/repository yet".into()
    } else {
        let why = if let Some(facts) = inj["facts"].as_object() {
            let rows: Vec<_> = facts
                .iter()
                .filter(|(k, _)| {
                    !["_profile", "_vocab", "_task", "_extractors"].contains(&k.as_str())
                })
                .map(|(_, v)| v)
                .collect();
            if rows.iter().any(|v| {
                !v.is_object()
                    || !["CANDIDATE", "ACTIVE", "RETIRED"]
                        .iter()
                        .any(|s| v["state"] == *s)
            }) {
                "fact lifecycle details unavailable in the last learning snapshot".into()
            } else if rows.is_empty() {
                "no facts tracked in the last learning snapshot".into()
            } else {
                let counts = ["CANDIDATE", "ACTIVE", "RETIRED"]
                    .map(|state| rows.iter().filter(|v| v["state"] == state).count());
                format!(
                    "{} candidate, {} active, {} retired; no facts rendered",
                    counts[0], counts[1], counts[2]
                )
            }
        } else {
            "no fact details recorded in the last learning snapshot".to_string()
        };
        format!("{why}; {facts_detail}")
    };
    notes["microskills"] = json!(facts);
    notes["toolprune"] = json!(if !denylist::supports(&ctx.host) {
        format!("tool pruning is not available on {}", ctx.host)
    } else if cfg["toolprune"] == "off" {
        "off; previously applied rules are not restored automatically".into()
    } else {
        let proposal = runtime.call(ctx, "proposal", json!([unt]))?;
        let pending = unt["auto_pending"]["items"].as_array();
        let why = if truthy(&proposal) {
            if truthy(&cfg["shadow"]) {
                "proposal recorded in shadow mode"
            } else if cfg["toolprune"] == "propose" {
                "proposal awaiting approval"
            } else if truthy(&unt["auto_apply_refused"]) {
                "automatic apply was refused"
            } else if proposal.as_array().is_some_and(|items| {
                items
                    .iter()
                    .any(|item| pending.is_some_and(|p| p.contains(item)))
            }) {
                "proposal held for the idle wait"
            } else {
                "unapplied proposal recorded"
            }
        } else if truthy(applied) {
            "deny rules recorded as applied for this checkout"
        } else if !truthy(unt) {
            "no tool-usage snapshot recorded for this agent/repository yet"
        } else {
            "no eligible proposal recorded"
        };
        format!("{why}; {tools_detail}")
    });
    notes["loopguard"] = json!(if cfg["loopguard"] == "off" {
        "off".into()
    } else if tracker.envelope().is_none() {
        format!(
            "learning baseline: {}/{} ended-green turns",
            tracker.resolved_turns.len(),
            tracker.min_samples
        )
    } else if paused {
        "judge calls paused after a sign-in failure; backstop remains active".into()
    } else {
        "baseline ready; per-turn reasons cannot be inferred from these totals".to_string()
    });
    Ok(notes)
}

pub fn status(args: &Value, runtime: &mut dyn Runtime) -> Result<i32> {
    let ctx = runtime.context(args)?;
    let cfg = &ctx.config;
    let mut config = cfg.clone();
    config
        .as_object_mut()
        .ok_or("expected configuration")?
        .shift_remove("warnings");
    let mut out = json!({"repo":ctx.repo.label,"repo_root":ctx.repo.root,"repo_key":ctx.repo.key,"identified_by":ctx.repo.how,"host":ctx.host,"state":ctx.store.base,"config":config,"warnings":field(cfg,"warnings",&json!([])),"session_note":runtime.call(&ctx,"session_note",json!([]))?});
    let read = |name| -> Result<Value> {
        let value = ctx.store.read(name, Value::Null)?;
        if truthy(&value) && !value.is_object() {
            return Err("saved snapshot must be an object".into());
        }
        Ok(if truthy(&value) { value } else { json!({}) })
    };
    let inj = read("injection.json")?;
    let unt = read("untool.json")?;
    let tracker = envelope::load_envelope(
        &ctx.store,
        cfg["loopguard_aggr"].as_str().unwrap_or("default"),
        &cfg["loopguard_thresholds"],
    )?;
    let ledger = accounting::summarize(&ctx.store.read_lines("ledger.jsonl", None)?)?;
    let inj_imported = count(&inj, "imported_sessions")?;
    let unt_imported = count(&unt, "imported_sessions")?;
    let rendered = inj
        .get("rendered")
        .filter(|v| truthy(v))
        .unwrap_or(&Value::Null);
    let rendered = if rendered.is_null() {
        ""
    } else {
        rendered.as_str().ok_or("rendered facts must be text")?
    };
    out["microskills"] = json!({"profile":cfg["microskills"],"injected_lines":rendered.split(['\n','\r','\u{b}','\u{c}','\u{1c}','\u{1d}','\u{1e}','\u{85}','\u{2028}','\u{2029}']).filter(|s| s.starts_with("- ")).count(),"sessions_folded":learned(&inj,"sessions",inj_imported)?,"imported_sessions":inj_imported});
    out["toolprune"] = json!({"mode":cfg["toolprune"],"sessions":learned(&unt,"sessions",unt_imported)?,"prompts":learned(&unt,"prompts",count(&unt,"imported_prompts")?)?,"imported_sessions":unt_imported,"imported_prompts":count(&unt,"imported_prompts")?,"catalog_source":unt["catalog_source"],"proposal":field(&unt,"proposal",&json!([])),"applied":runtime.call(&ctx,"applied",json!([]))?});
    out["loopguard"] = json!({"mode":cfg["loopguard"],"envelope":tracker.envelope(),"ended_green_turns":tracker.resolved_turns.len()});
    out["history"] = runtime.call(&ctx, "history_summary", json!([]))?;
    out["ledger"] = ledger.clone();
    if truthy(&args["explain"]) {
        out["explanations"] = status_explanations(
            &ctx,
            &inj,
            &unt,
            &tracker,
            &out["toolprune"]["applied"],
            runtime,
        )?;
    }
    if truthy(&args["json"]) {
        runtime.say(&json_pretty(&out, false))?;
        return Ok(0);
    }
    runtime.say(&format!(
        "TokenEconomizer -- {} ({})",
        ctx.repo.label, ctx.host
    ))?;
    runtime.say(&format!(
        "  repository   {}, {}",
        ctx.repo.root.display(),
        repos::describe(&ctx.repo)
    ))?;
    runtime.say(&format!("  state        {}", ctx.store.base.display()))?;
    runtime.say(&format!(
        "  config       microskills={} toolprune={} loopguard={} repomap={} mode={} shadow={}",
        text(&cfg["microskills"]),
        text(&cfg["toolprune"]),
        text(&cfg["loopguard"]),
        text(&cfg["repomap"]),
        text(&cfg["mode"]),
        text(&cfg["shadow"])
    ))?;
    if cfg["repomap"] != "off" {
        let rm = runtime.call(&ctx, "repomap_state", json!([]))?;
        let info = if truthy(&rm["map"]) {
            format!(
                "{} tokens cached for {}",
                count(&rm, "chars")?.div_euclid(4),
                text(
                    rm.get("commit")
                        .filter(|v| truthy(v))
                        .unwrap_or(&json!("?"))
                )
                .chars()
                .take(10)
                .collect::<String>()
            )
        } else {
            format!(
                "none yet ({})",
                text(
                    rm.get("error")
                        .filter(|v| truthy(v))
                        .unwrap_or(&json!("made after the first session"))
                )
            )
        };
        runtime.say(&format!("  repomap      {info}"))?;
    }
    if let Some(warnings) = cfg["warnings"].as_array() {
        for warning in warnings {
            runtime.say(&format!("  warning      {}", text(warning)))?;
        }
    }
    if truthy(&out["session_note"]) {
        runtime.say(&format!("  note         {}", text(&out["session_note"])))?;
    }
    let earlier = if truthy(&ledger["imported_sessions"]) {
        format!(
            " (+{} from earlier history)",
            text(&ledger["imported_sessions"])
        )
    } else {
        String::new()
    };
    runtime.say(&format!(
        "  sessions     {} folded{earlier}, {} effective tokens, plugin overhead {} ({:.1}%)",
        text(&ledger["sessions"]),
        accounting::fmt_tokens(ledger["effective_tokens"].as_f64()),
        accounting::fmt_tokens(ledger["overhead_tokens"].as_f64()),
        ledger["overhead_frac"].as_f64().unwrap_or(0.0) * 100.0
    ))?;
    let hist = history::describe(&out["history"], &ctx.host);
    if truthy(&hist) {
        runtime.say(&format!("  history      {}", text(&hist)))?;
    }
    let i = &out["microskills"];
    let earlier = if truthy(&i["imported_sessions"]) {
        format!(", +{} from earlier history", text(&i["imported_sessions"]))
    } else {
        String::new()
    };
    runtime.say(&format!(
        "  microskills  {} fact(s) injected at session start ({} sessions learned from{earlier})",
        text(&i["injected_lines"]),
        text(&i["sessions_folded"])
    ))?;
    if !denylist::supports(&ctx.host) {
        runtime.say("  toolprune    not available on this host")?;
    } else {
        let u = &out["toolprune"];
        let earlier = if truthy(&u["imported_sessions"]) {
            format!(
                " (+{} from earlier history, {} prompts)",
                text(&u["imported_sessions"]),
                text(&u["imported_prompts"])
            )
        } else {
            String::new()
        };
        let proposal = words(&u["proposal"])?.join(", ");
        let applied = words(&u["applied"])?.join(", ");
        runtime.say(&format!("  toolprune    {} sessions ({} prompts) observed{earlier}, catalog: {}; proposed {}: {}; applied: {}",text(&u["sessions"]),text(&u["prompts"]),text(u.get("catalog_source").filter(|v| truthy(v)).unwrap_or(&json!("none yet"))),denylist::unit(&ctx.host,true),if proposal.is_empty() {"-"} else {&proposal},if applied.is_empty() {"-"} else {&applied}))?;
    }
    let env_s = tracker
        .envelope()
        .filter(|v| *v != 0.0)
        .map(|v| format!("{v:.0} tool calls"))
        .unwrap_or("not learned yet".into());
    runtime.say(&format!("  loopguard    envelope {env_s} ({} ended-green turns); {} redirect(s), {} still-stuck notice(s) over {} turns",tracker.resolved_turns.len(),text(&ledger["nudges"]),text(&ledger["aborts"]),text(&ledger["turns"])))?;
    if let Some(explanations) = out["explanations"].as_object() {
        for (name, why) in explanations {
            runtime.say(&format!("  explain      {name}: {}", text(why)))?;
        }
    }
    Ok(0)
}
