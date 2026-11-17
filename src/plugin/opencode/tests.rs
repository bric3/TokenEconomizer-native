use super::*;

struct ExitDuringStart<'a> {
    bridge: &'a Bridge,
    calls: Vec<String>,
    exports: Vec<Value>,
}
impl Runtime for ExitDuringStart<'_> {
    fn hook(&mut self, payload: &Value) -> Result<Value> {
        let event = payload["hook_event_name"].as_str().unwrap();
        self.calls.push(event.into());
        if event == "SessionStart" {
            let bridge = self.bridge;
            let exit = bridge.prepare("exit", &json!({}))?;
            bridge.run(exit, self)?;
            Ok(
                json!({"hookSpecificOutput":{"additionalContext":"late facts"},"systemMessage":"late notice"}),
            )
        } else {
            assert_eq!(event, "SessionEnd");
            assert_eq!(payload["reason"], "exit");
            Ok(json!({}))
        }
    }
    fn export(&mut self, _: &Id, _: &Id, records: Option<Value>) -> Result<String> {
        self.exports.push(records.unwrap());
        Ok("/export.json".into())
    }
    fn notice(&mut self, _: &str, _: bool) -> Result<()> {
        panic!("a completed exit cannot deliver a late notice")
    }
    fn apply(&mut self, _: Value) -> Result<()> {
        panic!("a completed exit cannot mutate SDK values")
    }
    fn mode(&mut self) -> Result<String> {
        panic!("an exit does not query the SDK mode")
    }
}

#[test]
fn exit_freezes_a_prompt_waiting_for_session_start() {
    let bridge = Bridge::new(2).unwrap();
    bridge
        .prepare("init", &json!({"directory":"/repo"}))
        .unwrap();
    let job = bridge
        .prepare(
            "prompt",
            &json!({"sessionID":"pending","prompt":{"text":"not yet recorded"}}),
        )
        .unwrap();
    let mut runtime = ExitDuringStart {
        bridge: &bridge,
        calls: Vec::new(),
        exports: Vec::new(),
    };
    assert_eq!(
        bridge.run(job, &mut runtime).unwrap(),
        json!({"actions":[]})
    );
    assert_eq!(runtime.calls, ["SessionStart", "SessionEnd"]);
    assert_eq!(
        runtime.exports[0],
        json!({"info":{"id":"pending"},"messages":[]})
    );
    assert!(
        bridge
            .prepare(
                "context",
                &json!({"sessionID":"pending","system_array":true})
            )
            .is_err()
    );
}

#[derive(Default)]
struct Capture {
    saved: IndexMap<String, Value>,
    changes: Vec<Value>,
    hooks: Vec<Value>,
}
impl Runtime for Capture {
    fn checkpoint(&mut self, changes: Vec<Value>) {
        for row in &changes {
            let key = row[0].as_str().unwrap().to_owned();
            if row.as_array().unwrap().len() == 1 {
                self.saved.shift_remove(&key);
            } else {
                self.saved.insert(key, row[1].clone());
            }
        }
        self.changes.extend(changes);
    }
    fn hook(&mut self, payload: &Value) -> Result<Value> {
        self.hooks.push(payload.clone());
        Ok(json!({"hookSpecificOutput":{"additionalContext":"facts"}}))
    }
    fn export(&mut self, _: &Id, _: &Id, _: Option<Value>) -> Result<String> {
        Ok(String::new())
    }
    fn notice(&mut self, _: &str, _: bool) -> Result<()> {
        Ok(())
    }
    fn apply(&mut self, _: Value) -> Result<()> {
        Ok(())
    }
    fn mode(&mut self) -> Result<String> {
        Ok("tui".into())
    }
}
fn call(bridge: &Bridge, runtime: &mut Capture, method: &str, body: Value) -> Value {
    let job = bridge.prepare(method, &body).unwrap();
    bridge.run(job, runtime).unwrap()
}
fn saved(runtime: &Capture) -> Value {
    json!(runtime.saved.iter().collect::<Vec<_>>())
}
fn recover(runtime: &Capture) -> Bridge {
    let bridge = Bridge::new(2).unwrap();
    bridge.restore(2, saved(runtime), &Value::Null).unwrap();
    bridge
}
fn stream(
    bridge: &Bridge,
    runtime: &mut Capture,
    kind: &str,
    session: &str,
    message: &str,
    extra: Value,
) {
    let mut body =
        json!({"type":kind,"properties":{"sessionID":session,"assistantMessageID":message}});
    body["properties"]
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    call(bridge, runtime, "event", body);
}

#[test]
fn recovery_preserves_message_aliases_and_sends_only_changed_records() {
    let bridge = Bridge::new(2).unwrap();
    let mut runtime = Capture::default();
    call(&bridge, &mut runtime, "init", json!({"directory":"/repo"}));
    call(
        &bridge,
        &mut runtime,
        "prompt",
        json!({"sessionID":"a","prompt":{"text":"private prompt"}}),
    );
    stream(
        &bridge,
        &mut runtime,
        "session.step.started",
        "a",
        "shared",
        json!({"model":{"id":"model"}}),
    );
    call(
        &bridge,
        &mut runtime,
        "prompt",
        json!({"sessionID":"b","prompt":{"text":"another prompt"}}),
    );
    let restored = recover(&runtime);
    runtime.changes.clear();
    stream(
        &restored,
        &mut runtime,
        "session.step.ended",
        "b",
        "shared",
        json!({"cost":7,"tokens":{"input":3}}),
    );
    let state = restored.state().unwrap();
    assert_eq!(
        state.document(&Some(json!("a"))).unwrap()["messages"][1]["info"]["cost"],
        7
    );
    assert_eq!(
        state.document(&Some(json!("b"))).unwrap()["messages"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(runtime.changes.len(), 1);
    assert!(!json!(runtime.changes).to_string().contains("prompt"));
    assert_eq!(
        state.open.keys().cloned().collect::<Vec<_>>(),
        ["\"a\"", "\"b\""]
    );
    drop(state);
    // Reopening drops old prompt records while retained assistant references stay valid.
    call(
        &restored,
        &mut runtime,
        "event",
        json!({"type":"session.deleted","properties":{"sessionID":"a"}}),
    );
    call(
        &restored,
        &mut runtime,
        "prompt",
        json!({"sessionID":"a","prompt":{"text":"reopened"}}),
    );
    let restored = recover(&runtime);
    assert!(!saved(&runtime).to_string().contains("private prompt"));
    stream(
        &restored,
        &mut runtime,
        "session.step.ended",
        "a",
        "shared",
        json!({"cost":8}),
    );
    let state = restored.state().unwrap();
    assert_eq!(
        state.steps["\"shared\""].value.lock().unwrap()["info"]["cost"],
        8
    );
    assert_eq!(
        state.document(&Some(json!("a"))).unwrap()["messages"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        state.open.keys().cloned().collect::<Vec<_>>(),
        ["\"b\"", "\"a\""]
    );
}

#[test]
fn recovery_consumes_a_prompt_waiting_for_start_without_replaying_hooks() {
    let bridge = Bridge::new(2).unwrap();
    let mut runtime = Capture::default();
    call(&bridge, &mut runtime, "init", json!({"directory":"/repo"}));
    bridge
        .prepare(
            "prompt",
            &json!({"sessionID":"a","prompt":{"text":"interrupted"}}),
        )
        .unwrap();
    bridge.checkpoint(&mut runtime).unwrap();
    let restored = recover(&runtime);
    restored.checkpoint(&mut runtime).unwrap();
    assert!(runtime.hooks.is_empty());
    call(
        &restored,
        &mut runtime,
        "prompt",
        json!({"sessionID":"a","prompt":{"text":"next"}}),
    );
    assert_eq!(runtime.hooks.len(), 1);
    assert_eq!(runtime.hooks[0]["prompt_id"], "a:2");
    assert_eq!(
        restored
            .state()
            .unwrap()
            .document(&Some(json!("a")))
            .unwrap()["messages"][0]["parts"][0]["text"],
        "interrupted"
    );
    assert!(!saved(&runtime).to_string().contains("pending"));
}

#[test]
fn invalid_recovery_does_not_replace_the_bridge() {
    let bridge = Bridge::new(2).unwrap();
    for value in [
        json!({}),
        json!([["unknown", 1]]),
        json!([["\"config\"",{"version":1}]]),
        json!([["\"current\"", 1], ["\"current\"", 2]]),
        json!([["{\"step\":\"a\"}", 123]]),
    ] {
        assert!(bridge.restore(2, value, &Value::Null).is_err());
        assert!(!bridge.state().unwrap().initialised);
    }
    let mut runtime = Capture::default();
    call(&bridge, &mut runtime, "init", json!({"directory":"/repo"}));
    assert!(bridge.restore(2, saved(&runtime), &Value::Null).is_err());
}
