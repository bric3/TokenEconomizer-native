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
