use serde_json::{Value, json};

use crate::{
    pricing::runner_effective_or_approx,
    runner::Runner,
    value::{number, scalar_text, truthy},
};

pub const STUCK_JUDGE_SYSTEM: &str = concat!(
    "You monitor an AI coding agent that is partway through ONE task in a software repo. ",
    "A cheap heuristic flagged its recent process as a possible UNPRODUCTIVE LOOP. Read ",
    "the leakage-safe digest of its RECENT actions and judge whether it is genuinely ",
    "STUCK — repeating the same kind of edit/test/failure with no progress toward green ",
    "tests, unlikely to recover if it keeps going the same way — versus still making ",
    "real progress (changing failures, narrowing the problem, exploring new hypotheses).\n\n",
    "Judge ONLY the workflow from the digest; do NOT infer the task's solution or which ",
    "lines to change. Favor SPARING (stuck=false) when there is any sign of progress; ",
    "only return stuck=true with high confidence for a clear, repetitive dead-end.\n",
    "Return ONLY a JSON object: {\"stuck\": bool, \"confidence\": <0.0-1.0>, ",
    "\"reason\": \"<short leakage-safe justification>\"}."
);

pub const NUDGE_WRITER_SYSTEM: &str = concat!(
    "An AI coding agent is stuck in a REPETITIVE, UNPRODUCTIVE loop partway through one ",
    "task in a software repo. You are given a leakage-safe digest of its RECENT ACTIONS ",
    "only (tool names, shell commands, searched patterns, edited file BASENAMES, and ",
    "error SIGNATURES — never file contents or the task description). Write a SHORT ",
    "redirect (2-4 sentences) addressed to the agent that: (1) names the specific ",
    "repetitive pattern you actually see in the digest (e.g. re-editing the same file, ",
    "re-running the same test to the same failure); (2) proposes a MATERIALLY DIFFERENT ",
    "next step (a different file/root-cause to consider, re-reading the failing test's ",
    "expectation, verifying a new hypothesis before editing again).\n",
    "HARD CONSTRAINTS: Do NOT tell it to stop, give up, or reduce effort — only redirect. ",
    "Do NOT infer the task's solution, name specific line numbers to change, or reference ",
    "any code/identifier you cannot see in the digest. Output ONLY the redirect as plain ",
    "text — no preamble, no JSON, no headings."
);

pub fn llm_stuck_judge(digest: &str, runner: Option<&mut dyn Runner>, model: &str) -> Value {
    let Some(runner) = runner.filter(|_| !digest.is_empty()) else {
        return json!({"stuck": null, "confidence": 0.0, "reason": "no-runner", "_tokens": 0});
    };
    let prompt = format!(
        "{STUCK_JUDGE_SYSTEM}\n\nRecent process digest:\n{digest}\n\nReturn the JSON verdict:"
    );
    let (raw, real) = match runner.run(&prompt, model) {
        Ok(reply) => reply,
        Err(error) => {
            let reason: String = format!("runner-error:{error}").chars().take(60).collect();
            return json!({"stuck": null, "confidence": 0.0, "reason": reason, "_tokens": 0});
        }
    };
    let tokens = runner_effective_or_approx(
        &prompt,
        &raw,
        real,
        Some(|p: &str, r: &str| runner.estimate_effective(p, r)),
    );
    let mut verdict = json!({"stuck": null, "confidence": 0.0, "reason": "", "_tokens": tokens});
    if let (Some(start), Some(end)) = (raw.find('{'), raw.rfind('}'))
        && start <= end
        && let Ok(object) = serde_json::from_str::<Value>(&raw[start..=end])
    {
        verdict["stuck"] = if object["stuck"].is_null() {
            Value::Null
        } else {
            json!(truthy(&object["stuck"]))
        };
        let confidence = match &object["confidence"] {
            value if !truthy(value) => Some(0.0),
            Value::String(s) => s.trim().parse::<f64>().ok(),
            value => number(value),
        };
        // The upstream parser keeps `stuck` when decoding confidence fails.
        if let Some(confidence) = confidence {
            verdict["confidence"] = json!(1.0_f64.min(confidence).max(0.0));
            verdict["reason"] = json!(
                object
                    .get("reason")
                    .map(scalar_text)
                    .unwrap_or_default()
                    .chars()
                    .take(160)
                    .collect::<String>()
            );
        }
    }
    verdict
}

pub fn llm_nudge_writer(digest: &str, runner: Option<&mut dyn Runner>, model: &str) -> Value {
    let empty = || json!({"text": "", "_tokens": 0});
    let Some(runner) = runner.filter(|_| !digest.is_empty()) else {
        return empty();
    };
    let prompt =
        format!("{NUDGE_WRITER_SYSTEM}\n\nRecent process digest:\n{digest}\n\nWrite the redirect:");
    let Ok((raw, real)) = runner.run(&prompt, model) else {
        return empty();
    };
    let raw = raw.trim();
    if raw.is_empty() {
        return empty();
    }
    let tokens = runner_effective_or_approx(
        &prompt,
        raw,
        real,
        Some(|p: &str, r: &str| runner.estimate_effective(p, r)),
    );
    json!({"text": raw.chars().take(1200).collect::<String>(), "_tokens": tokens})
}
