use super::*;
use crate::{
    config::ToolingConfig,
    toolprune::stage::{ToolingContext, ToolingStage},
    trace::{codex, copilot},
};
use std::collections::BTreeMap;

fn counters(value: &Value) -> Result<BTreeMap<String, i64>> {
    if truthy(value) && !value.is_object() {
        return Err(Error::Value("expected a counter map".into()));
    }
    value
        .as_object()
        .into_iter()
        .flatten()
        .map(|(k, v)| number(v).map(|n| (k.clone(), n)))
        .collect()
}
pub fn enough_evidence(sessions: i64, prompts: i64, cfg: &Value, high_risk: bool) -> bool {
    let k = if high_risk {
        adapter::HIGH_RISK_FACTOR
    } else {
        1
    };
    (sessions >= cfg["toolprune_min_sessions"].as_i64().unwrap_or(0)
        && prompts >= cfg["toolprune_evidence"].as_i64().unwrap_or(0) * k)
        || sessions >= cfg["toolprune_sessions"].as_i64().unwrap_or(0) * k
}
#[allow(clippy::too_many_arguments)]
pub fn fold_toolprune(
    ctx: &LiveContext,
    adapter: &mut LiveAdapter,
    run: &Path,
    sid: &str,
    transcript: Option<&str>,
    workdir: Option<&Path>,
    prompts: Option<i64>,
    imported: bool,
    runtime: &mut dyn Runtime,
) -> Result<Value> {
    if !denylist::supports(&ctx.host) || ctx.config["toolprune"] == "off" {
        return Ok(Value::Null);
    }
    if ctx.host == "codex" {
        return fold_toolprune_codex(ctx, adapter, sid, transcript, workdir, prompts, runtime);
    }
    let st = read(ctx, "untool.json")?;
    let mut tc = ToolingContext {
        adapter: Some(adapter),
        ..Default::default()
    };
    let mut stage = ToolingStage::new(
        ToolingConfig {
            evidence: count(&ctx.config, "toolprune_min_sessions")?.max(1),
            ..Default::default()
        },
        &ctx.host,
        "agent",
        Some((&ctx.repo.label, &ctx.mech_dir()?)),
        true,
        &mut tc,
    )?;
    stage.used_tools = strings(&st["used_tools"]).into_iter().collect();
    stage.tool_unused_streaks = counters(&st["streaks"])?;
    let mut prompt_streaks = if truthy(&st["prompt_streaks"]) {
        counters(&st["prompt_streaks"])?
    } else {
        stage.tool_unused_streaks.clone()
    };
    stage.catalog_tools = strings(&st["catalog_tools"]).into_iter().collect();
    stage.catalog_desc = st["catalog_desc"].as_object().cloned().unwrap_or_default();
    let mut applied = denylist::applied_for(&ctx.store, &ctx.repo.root, &ctx.host)?;
    let kept = denylist::kept_for(&ctx.store, &ctx.repo.root, &ctx.host)?;
    stage.exclude_tools = Some(if imported {
        Vec::new()
    } else {
        applied.clone()
    });
    let protected = tc.adapter.as_mut().unwrap().protected_tools()?;
    let mut restored = Vec::new();
    if !imported && applied.iter().any(|t| protected.contains(t)) && !truthy(&ctx.config["shadow"])
    {
        match runtime.restore(
            ctx,
            &applied
                .iter()
                .filter(|t| protected.contains(t))
                .cloned()
                .collect::<Vec<_>>(),
        ) {
            Ok(items) => restored = items,
            Err(Error::Failed(_)) => {}
            Err(e) => return Err(e),
        }
    }
    if !restored.is_empty() {
        applied = denylist::applied_for(&ctx.store, &ctx.repo.root, &ctx.host)?;
    }
    stage.warned_no_catalog = true;
    let mut srow = json!({});
    let before = stage.tool_unused_streaks.clone();
    stage.after_task(
        &json!({"instance_id":sid,"untooled_excluded":stage.exclude_tools}),
        run,
        &mut srow,
        &mut tc,
    )?;
    for (tool, n) in &stage.tool_unused_streaks {
        if *n == 0 {
            prompt_streaks.insert(tool.clone(), 0);
        } else if *n == before.get(tool).copied().unwrap_or(0) + 1 {
            *prompt_streaks.entry(tool.clone()).or_default() += prompts.unwrap_or(1);
        }
    }
    let imported_n = count(&st, "imported_sessions")? + i64::from(imported);
    let mut live_streaks = counters(&st["live_streaks"])?;
    let mut live_prompts = counters(&st["live_prompt_streaks"])?;
    if !imported {
        for (tool, n) in &stage.tool_unused_streaks {
            if *n == 0 {
                live_streaks.insert(tool.clone(), 0);
                live_prompts.insert(tool.clone(), 0);
            } else if *n == before.get(tool).copied().unwrap_or(0) + 1 {
                *live_streaks.entry(tool.clone()).or_default() += 1;
                *live_prompts.entry(tool.clone()).or_default() += prompts.unwrap_or(1);
            }
        }
    }
    let sessions = count(&st, "sessions")? + 1;
    let raw: Vec<_> = stage
        .build_tools(
            &json!({"instance_id":format!("after-{sid}")}),
            sessions,
            &mut tc,
        )?
        .unwrap_or_default()
        .into_iter()
        .filter(|t| {
            enough_evidence(
                stage.tool_unused_streaks.get(t).copied().unwrap_or(0),
                prompt_streaks.get(t).copied().unwrap_or(0),
                &ctx.config,
                adapter::HIGH_RISK.contains(&t.as_str()),
            )
        })
        .collect();
    let mut doc = json!({
        "used_tools": stage.used_tools,
        "streaks": stage.tool_unused_streaks,
        "prompt_streaks": prompt_streaks,
        "catalog_tools": stage.catalog_tools,
        "catalog_desc": stage.catalog_desc,
        "catalog_source": srow.get("untool_catalog_source").filter(|v|truthy(v)).unwrap_or(&st["catalog_source"]),
        "sessions": sessions,
        "prompts": count(&st,"prompts")? + prompts.unwrap_or(0),
        "updated": history::now()
    });
    if imported_n != 0 {
        doc["imported_sessions"] = json!(imported_n);
        doc["imported_prompts"] = json!(
            count(&st, "imported_prompts")? + if imported { prompts.unwrap_or(0) } else { 0 }
        );
        doc["live_streaks"] = json!(live_streaks);
        doc["live_prompt_streaks"] = json!(live_prompts);
    }
    let proposal: Vec<_> = if ctx.host == "copilot" {
        let servers: Vec<_> = copilot::mcp_servers(&json!(ctx.env), &ctx.home)
            .into_iter()
            .collect();
        let proposal = denylist::copilot_server_proposal(
            &raw,
            &stage.catalog_tools.iter().cloned().collect::<Vec<_>>(),
            &stage.used_tools.iter().cloned().collect::<Vec<_>>(),
            &servers,
        );
        doc["servers"] = server_summary(&stage, &servers, &prompt_streaks);
        proposal
    } else {
        raw
    }
    .into_iter()
    .filter(|t| !applied.contains(t) && !kept.contains(t))
    .collect();
    doc["proposal"] = json!(proposal);
    let old = strings(&st["proposal"]);
    let mut new: Vec<_> = proposal
        .iter()
        .filter(|t| !old.contains(t))
        .cloned()
        .collect();
    new.sort();
    new.dedup();
    doc["new_in_proposal"] = json!(new);
    if imported {
        for key in ["auto_pending", "auto_apply_refused"] {
            if truthy(&st[key]) {
                doc[key] = st[key].clone();
            }
        }
        let held = strings(&doc["auto_pending"]["items"]);
        doc["needs_apply"] = json!(
            proposal
                .iter()
                .filter(|t| !held.contains(t))
                .collect::<Vec<_>>()
        );
    } else if ctx.config["toolprune"] == "auto"
        && !proposal.is_empty()
        && !truthy(&ctx.config["shadow"])
    {
        let mut held = proposal.clone();
        if imported_n != 0 {
            held.retain(|t| {
                enough_evidence(
                    live_streaks.get(t).copied().unwrap_or(0),
                    live_prompts.get(t).copied().unwrap_or(0),
                    &ctx.config,
                    adapter::HIGH_RISK.contains(&t.as_str()),
                )
            });
            doc["needs_apply"] = json!(
                proposal
                    .iter()
                    .filter(|t| !held.contains(t))
                    .collect::<Vec<_>>()
            );
        }
        if !held.is_empty() {
            hold(&mut doc, &st, &held, &ctx.repo.root);
        }
    }
    if !restored.is_empty() {
        doc["restored_protected"] = json!(restored);
    }
    ctx.store.write("untool.json", &doc)?;
    Ok(
        json!({"proposal":doc["proposal"],"pending":truthy(&doc["auto_pending"]),"restored":restored}),
    )
}
pub fn server_summary(
    stage: &ToolingStage,
    servers: &[String],
    prompt_streaks: &BTreeMap<String, i64>,
) -> Value {
    let mut out = json!({});
    for tool in &stage.catalog_tools {
        let Some(server) = copilot::mcp_server_of(tool, servers) else {
            continue;
        };
        if out.get(server).is_none() {
            out[server] =
                json!({"tools":0,"used":false,"unused_sessions":null,"unused_prompts":null});
        }
        let e = &mut out[server];
        e["tools"] = json!(e["tools"].as_i64().unwrap() + 1);
        e["used"] = json!(truthy(&e["used"]) || stage.used_tools.contains(tool));
        for (key, n) in [
            ("unused_sessions", stage.tool_unused_streaks.get(tool)),
            ("unused_prompts", prompt_streaks.get(tool)),
        ] {
            let n = n.copied().unwrap_or(0);
            e[key] = json!(e[key].as_i64().map_or(n, |prior| prior.min(n)));
        }
    }
    out
}
pub fn fold_toolprune_codex(
    ctx: &LiveContext,
    adapter: &mut LiveAdapter,
    sid: &str,
    transcript: Option<&str>,
    workdir: Option<&Path>,
    prompts: Option<i64>,
    runtime: &mut dyn Runtime,
) -> Result<Value> {
    let Some((transcript, workdir)) = transcript.filter(|s| !s.is_empty()).zip(workdir) else {
        return Ok(Value::Null);
    };
    let cap_dir = workdir.join("capabilities");
    fs::create_dir_all(&cap_dir)?;
    let cap_trace = cap_dir.join("trace.jsonl");
    if codex::capability_trace(Path::new(transcript), &cap_trace)?.is_none() {
        return Ok(Value::Null);
    }
    let run = run_dir(&cap_dir, &cap_trace)?;
    let protected = adapter.protected_tools();
    let mut tc = ToolingContext {
        adapter: Some(adapter),
        ..Default::default()
    };
    let mut stage = ToolingStage::new(
        ToolingConfig {
            codex_untool: true,
            ..Default::default()
        },
        "codex",
        "agent",
        Some((&ctx.repo.label, &ctx.mech_dir()?)),
        false,
        &mut tc,
    )?;
    stage.warned_no_catalog = true;
    stage.build_tools(&json!({"instance_id":sid}), 0, &mut tc)?;
    let mut applied = denylist::applied_for(&ctx.store, &ctx.repo.root, &ctx.host)?;
    let kept = denylist::kept_for(&ctx.store, &ctx.repo.root, &ctx.host)?;
    stage
        .codex_profile
        .as_mut()
        .unwrap()
        .pending
        .as_mut()
        .unwrap()["logical_capabilities"] = json!(applied);
    let mut restored = Vec::new();
    if applied.iter().any(|c| protected.contains(c))
        && !truthy(&ctx.config["shadow"])
        && !runtime.git_tracked(&ctx.repo.root, denylist::CODEX_CONFIG_REL)?
    {
        match runtime.restore(
            ctx,
            &applied
                .iter()
                .filter(|c| protected.contains(c))
                .cloned()
                .collect::<Vec<_>>(),
        ) {
            Ok(items) => restored = items,
            Err(Error::Failed(_) | Error::Value(_) | Error::Io(_)) => {}
            Err(e) => return Err(e),
        }
    }
    if !restored.is_empty() {
        applied = denylist::applied_for(&ctx.store, &ctx.repo.root, &ctx.host)?;
    }
    let before = counters(&stage.codex_profile.as_ref().unwrap().state["loaded_exposures"])?;
    stage.after_task(&json!({"instance_id":sid}), &run, &mut json!({}), &mut tc)?;
    let state = &stage.codex_profile.as_ref().unwrap().state;
    let ever_used = strings(&state["ever_completed"]);
    let loaded = counters(&state["loaded_exposures"])?;
    let prior = read(ctx, "untool.json")?;
    let mut loaded_prompts = if truthy(&prior["loaded_prompts"]) {
        counters(&prior["loaded_prompts"])?
    } else {
        before.clone()
    };
    for c in denylist::CODEX_LIVE_CAPABILITIES {
        if loaded.get(c).copied().unwrap_or(0) > before.get(c).copied().unwrap_or(0) {
            *loaded_prompts.entry(c.into()).or_default() += prompts.unwrap_or(1);
        }
    }
    let pruned = strings(&state["current_pruned"]);
    let proposal: Vec<String> = denylist::CODEX_LIVE_CAPABILITIES
        .into_iter()
        .filter(|c| {
            pruned.iter().any(|p| p == c)
                && !ever_used.iter().any(|p| p == c)
                && !applied.iter().any(|p| p == c)
                && !kept.iter().any(|p| p == c)
                && !protected.contains(c)
                && enough_evidence(
                    loaded.get(*c).copied().unwrap_or(0),
                    loaded_prompts.get(*c).copied().unwrap_or(0),
                    &ctx.config,
                    CODEX_HIGH_RISK.contains(c),
                )
        })
        .map(String::from)
        .collect();
    let capabilities = denylist::CODEX_LIVE_CAPABILITIES
        .into_iter()
        .map(|c| {
            Ok((
                c.to_string(),
                json!({
                    "loaded_sessions": loaded.get(c).copied().unwrap_or(0),
                    "loaded_prompts": loaded_prompts.get(c).copied().unwrap_or(0),
                    "unused_sessions": count(&state["zero_attempt_exposures"],c)?,
                    "used": ever_used.iter().any(|t|t==c)
                }),
            ))
        })
        .collect::<Result<serde_json::Map<String, Value>>>()?;
    let old = strings(&prior["proposal"]);
    let mut new: Vec<_> = proposal
        .iter()
        .filter(|t| !old.contains(t))
        .cloned()
        .collect();
    new.sort();
    new.dedup();
    let mut doc = json!({
        "sessions": count(state,"tasks_completed")?,
        "catalog_source": "capabilities",
        "prompts": count(&prior,"prompts")? + prompts.unwrap_or(0),
        "capabilities": capabilities,
        "loaded_prompts": loaded_prompts,
        "proposal": proposal,
        "updated": history::now(),
        "new_in_proposal": new
    });
    if ctx.config["toolprune"] == "auto" && !proposal.is_empty() && !truthy(&ctx.config["shadow"]) {
        hold(&mut doc, &prior, &proposal, &ctx.repo.root);
    }
    ctx.store.write("untool.json", &doc)?;
    Ok(json!({"proposal":proposal,"pending":truthy(&doc["auto_pending"]),"restored":restored}))
}
