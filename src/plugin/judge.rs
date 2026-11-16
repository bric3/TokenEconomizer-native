use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    loopguard::{
        judge as loop_judge,
        turn::{BackgroundJudge, TurnGuard},
    },
    runner::Runner,
    store::Store,
    value::{float_or_zero, scalar_text, truthy},
};

pub trait Runtime {
    fn read(&mut self, store: &Store, name: &str) -> Result<Value, String> {
        store.read(name, Value::Null).map_err(|e| e.to_string())
    }
    fn write(&mut self, store: &Store, name: &str, value: &Value) -> Result<(), String> {
        store
            .write(name, value)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    fn append(&mut self, store: &Store, name: &str, value: &Value) -> Result<(), String> {
        store.append(name, value).map_err(|e| e.to_string())
    }
    fn paused(&mut self) -> Result<bool, String>;
    fn runner(&mut self, role: &str, local_only: bool) -> Result<Option<Box<dyn Runner>>, String>;
    fn judge(&mut self, digest: &str, runner: &mut dyn Runner) -> Result<Value, String> {
        Ok(loop_judge::llm_stuck_judge(digest, Some(runner), "agent"))
    }
    fn nudge(&mut self, digest: &str, runner: &mut dyn Runner) -> Result<Value, String> {
        Ok(loop_judge::llm_nudge_writer(digest, Some(runner), "agent"))
    }
    fn spawn_job(&mut self, session_id: &str, job_id: &str) -> Result<(), String>;
    fn spawn(&mut self, argv: &[String], log: &str) -> Result<Value, String>;
}

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}
fn request_name(sid: &str) -> String {
    format!("judge/{sid}.request.json")
}
fn result_name(sid: &str) -> String {
    format!("judge/{sid}.result.json")
}

pub struct AsyncJudge<'a> {
    pub store: &'a Store,
    pub session_id: &'a str,
    pub threshold: f64,
    pub runtime: &'a mut dyn Runtime,
}
impl<'a> AsyncJudge<'a> {
    pub fn new(
        store: &'a Store,
        session_id: &'a str,
        threshold: &Value,
        runtime: &'a mut dyn Runtime,
    ) -> Result<Self, String> {
        Ok(Self {
            store,
            session_id,
            threshold: float_or_zero(threshold)?,
            runtime,
        })
    }
}
impl BackgroundJudge for AsyncJudge<'_> {
    fn start(&mut self, guard: &mut TurnGuard, digest: &str) -> Result<Value, String> {
        let job = format!(
            "{}:{}:{}:{:.3}",
            guard.turn_key,
            guard.segment,
            guard.judge_calls,
            now()
        );
        self.runtime.write(
            self.store,
            &request_name(self.session_id),
            &json!({
                "id":job,"session_id":self.session_id,"turn_key":guard.turn_key,
                "segment":guard.segment,"digest":digest,"threshold":self.threshold,"at":now()
            }),
        )?;
        self.runtime.spawn_job(self.session_id, &job)?;
        Ok(json!(job))
    }
    fn poll(&mut self, guard: &mut TurnGuard) -> Result<(bool, Value), String> {
        let pending = &guard.judge_pending;
        if !truthy(&pending["id"]) {
            return Ok((false, Value::Null));
        }
        let result = self
            .runtime
            .read(self.store, &result_name(self.session_id))?;
        if result["id"] != pending["id"] {
            return Ok((false, Value::Null));
        }
        let verdict = result["verdict"].clone();
        Ok((
            true,
            if truthy(&verdict) {
                verdict
            } else {
                Value::Null
            },
        ))
    }
}

pub struct NoVerdict;
impl BackgroundJudge for NoVerdict {
    fn start(&mut self, _: &mut TurnGuard, _: &str) -> Result<Value, String> {
        Ok(Value::Null)
    }
    fn poll(&mut self, _: &mut TurnGuard) -> Result<(bool, Value), String> {
        Ok((false, Value::Null))
    }
}

pub fn run_job(
    store: &Store,
    host: &str,
    session_id: &str,
    job_id: &str,
    local_floor: f64,
    runtime: &mut dyn Runtime,
) -> Result<Value, String> {
    let req = runtime.read(store, &request_name(session_id))?;
    if req["id"] != job_id {
        return Ok(Value::Null);
    }
    let paused = runtime.paused()?;
    let runner = runtime.runner("judge", paused)?;
    let writer = runtime.runner("nudge", paused)?;
    let digest = req["digest"].as_str().unwrap_or("");
    let (mut verdict, mut written) = (Value::Null, Value::Null);
    if let Some(mut runner) = runner {
        verdict = runtime.judge(digest, &mut *runner).unwrap_or(Value::Null);
        if truthy(&verdict)
            && let Some(local) = runner.local_answer()
        {
            verdict["local"] = local;
            if truthy(&verdict["stuck"]) && float_or_zero(&verdict["confidence"])? < local_floor {
                verdict["stuck"] = json!(false);
                verdict["reason"] = json!(
                    format!(
                        "below local_judge_confidence: {}",
                        if truthy(&verdict["reason"]) {
                            scalar_text(&verdict["reason"])
                        } else {
                            String::new()
                        }
                    )
                    .chars()
                    .take(160)
                    .collect::<String>()
                );
            }
        }
        if truthy(&verdict)
            && truthy(&verdict["stuck"])
            && float_or_zero(&verdict["confidence"])? >= float_or_zero(&req["threshold"])?
        {
            if let Some(mut writer) = writer {
                written = runtime.nudge(digest, &mut *writer).unwrap_or(Value::Null);
            }
            if let Some(text) = written["text"].as_str().filter(|s| !s.trim().is_empty()) {
                verdict["_nudge"] = json!(text.trim());
            }
        }
    }
    let judge_tokens = float_or_zero(&verdict["_tokens"])?;
    let writer_tokens = float_or_zero(&written["_tokens"])?;
    if judge_tokens != 0.0 || writer_tokens != 0.0 {
        // Charge before publishing so a fold that sees the verdict also sees its cost.
        runtime.append(store, "ledger.jsonl", &json!({"kind":"overhead","session_id":session_id,"host":host,"job":job_id,"at":now(),
            "overhead":{"spiral_judge_tokens":judge_tokens,"spiral_nudge_writer_tokens":writer_tokens}}))?;
    }
    runtime.write(
        store,
        &result_name(session_id),
        &json!({"id":job_id,"verdict":verdict,"at":now()}),
    )?;
    Ok(verdict)
}

pub fn spawn_job(
    store: &Store,
    host: &str,
    session_id: &str,
    job_id: &str,
    local_floor: f64,
    synchronous: bool,
    runtime: &mut dyn Runtime,
) -> Result<Value, String> {
    if synchronous {
        run_job(store, host, session_id, job_id, local_floor, runtime)
    } else {
        runtime.spawn(
            &[
                "judge".into(),
                "--session".into(),
                session_id.into(),
                "--job".into(),
                job_id.into(),
            ],
            "judge.log",
        )
    }
}
