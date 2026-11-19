use super::{
    Failure, Result, Runtime, Value, attr, boolean, copy, equal, function, invoke, json, literal,
    method, returned,
};

fn member(rt: &mut dyn Runtime, name: &str, target: &str) -> Result<()> {
    attr(rt, "self", name, target)
}
fn default_model(rt: &mut dyn Runtime) -> Result<()> {
    member(rt, "_codex", "codex")?;
    let codex = boolean(rt, "codex")?;
    literal(
        rt,
        "billed",
        json!(if codex {
            "gpt-5.6-sol"
        } else {
            "claude-opus-4-8"
        }),
    )
}
fn transcript(rt: &mut dyn Runtime) -> Result<Value> {
    function(rt, "os.path.join")?;
    literal(rt, "filename", json!("transcript.jsonl"))?;
    invoke(rt, &["run_dir_path", "filename"], &[])?;
    copy(rt, "path", "called")?;
    member(rt, "_codex", "codex")?;
    let lines = if boolean(rt, "codex")? {
        json!([
            {"type":"item.completed","item":{"id":"i0","status":"completed","type":"command_execution","command":"python -m pytest -q"}},
            {"type":"item.completed","item":{"id":"i1","status":"completed","type":"command_execution","command":"sed -n '1,40p' setup.py"}}
        ])
    } else {
        json!([
            {"message":{"content":[{"type":"tool_use","id":"t0","name":"bash","input":{"command":"python -m pytest -q"}}]}},
            {"message":{"content":[{"type":"tool_result","tool_use_id":"t0","content":"ok"}]}},
            {"message":{"content":[{"type":"tool_use","id":"t1","name":"read","input":{"file_path":"setup.py"}}]}},
            {"message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":"..."}]}}
        ])
    };
    literal(rt, "lines", lines)?;
    literal(rt, "write", json!("w"))?;
    literal(rt, "encoding", json!("utf-8"))?;
    rt.call("eval_builtin", json!(["open"]))?;
    invoke(rt, &["path", "write"], &[("encoding", "encoding")])?;
    rt.call("task_enter", json!(["transcript"]))?;
    let write = (|| {
        literal(rt, "encoded", json!([]))?;
        rt.call("workspace_iter", json!(["lines", "iterator"]))?;
        while super::truthy(&rt.call("workspace_next", json!(["iterator", "line"]))?) {
            function(rt, "json.dumps")?;
            invoke(rt, &["line"], &[])?;
            rt.call("eval_append", json!(["encoded", "called"]))?;
        }
        literal(rt, "newline", json!("\n"))?;
        method(rt, "newline", "join", &["encoded"])?;
        copy(rt, "document", "called")?;
        method(rt, "transcript_handle", "write", &["document"])
    })();
    match write {
        Ok(()) => {
            rt.call("task_exit", json!(["transcript", null]))?;
        }
        Err(error) => {
            rt.call("task_exit", json!(["transcript", error]))?;
            if !boolean(rt, "exit_result")? {
                return Err(error);
            }
        }
    }
    Ok(Value::Null)
}
fn task(rt: &mut dyn Runtime) -> Result<Value> {
    function(rt, "os.makedirs")?;
    literal(rt, "yes", json!(true))?;
    invoke(rt, &["run_dir_path"], &[("exist_ok", "yes")])?;
    member(rt, "_write_transcript", "function")?;
    invoke(rt, &["run_dir_path"], &[])?;
    rt.call("eval_builtin", json!(["int"]))?;
    copy(rt, "integer", "function")?;
    literal(rt, "key", json!("_index"))?;
    literal(rt, "zero", json!(0))?;
    method(rt, "task", "get", &["key", "zero"])?;
    copy(rt, "index_value", "called")?;
    copy(rt, "function", "integer")?;
    invoke(rt, &["index_value"], &[])?;
    copy(rt, "idx", "called")?;
    literal(rt, "key", json!("model"))?;
    method(rt, "run_kwargs", "get", &["key"])?;
    copy(rt, "billed", "called")?;
    if !boolean(rt, "billed")? {
        default_model(rt)?;
    }
    if equal(rt, "billed", "agent")? {
        default_model(rt)?;
    }
    literal(rt, "three", json!(3))?;
    rt.call("web_binary", json!(["mod", "idx", "three", "remainder"]))?;
    rt.call("web_compare", json!(["ne", "remainder", "zero"]))?;
    literal(rt, "base_turns", json!(24))?;
    rt.call("web_binary", json!(["add", "base_turns", "idx", "turns"]))?;
    literal(
        rt,
        "row",
        json!({"input_tokens":900,"cache_read_input_tokens":7000,"cache_creation_input_tokens":400,"output_tokens":350,"tokens_total":8650,"resolved":null,"num_turns":null,"is_error":false,"instance":null,"repo":null,"model":null,"codex_spiral_nudges":0}),
    )?;
    rt.call("eval_field", json!(["row", "resolved", "comparison"]))?;
    rt.call("eval_field", json!(["row", "num_turns", "turns"]))?;
    literal(rt, "empty", json!(""))?;
    for (field, key) in [("instance", "instance_id"), ("repo", "repo")] {
        literal(rt, "key", json!(key))?;
        method(rt, "task", "get", &["key", "empty"])?;
        rt.call("eval_field", json!(["row", field, "called"]))?;
    }
    rt.call("eval_field", json!(["row", "model", "billed"]))?;
    rt.call("provider_tuple", json!(["row", "run_dir_path"]))?;
    Ok(returned("tuple"))
}
pub(super) fn run(name: &str, rt: &mut dyn Runtime) -> Result<Value> {
    match name {
        "new" => {
            rt.call("stream_set", json!(["agent", "agent"]))?;
            literal(rt, "codex", json!("codex"))?;
            rt.call("web_compare", json!(["eq", "agent", "codex"]))?;
            rt.call("stream_set", json!(["_codex", "comparison"]))?;
            Ok(Value::Null)
        }
        "optimizer" => {
            literal(rt, "answer", json!("{\"extractors\": [], \"facts\": []}"))?;
            literal(rt, "tokens", json!(40.0))?;
            rt.call("provider_tuple", json!(["answer", "tokens"]))?;
            Ok(returned("tuple"))
        }
        "transcript" => transcript(rt),
        "task" => task(rt),
        _ => Err(Failure::new(
            "ValueError",
            format!("unknown stub runner operation: {name}"),
        )),
    }
}
