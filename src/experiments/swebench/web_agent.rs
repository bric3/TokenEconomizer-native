use super::{
    Failure, Result, Runtime,
    optimizer::code_points,
    providers::{boolean, copy, literal},
    runner_calls::{function, invoke, member},
};
use crate::{python_json::Text, value::truthy};
use serde_json::{Value, json};

fn attr(rt: &mut dyn Runtime, object: &str, name: &str, target: &str) -> Result<()> {
    rt.call("stdout_member", json!([object, name, target]))
        .map(|_| ())
}
fn builtin(rt: &mut dyn Runtime, name: &str) -> Result<()> {
    rt.call("eval_builtin", json!([name])).map(|_| ())
}
fn call(rt: &mut dyn Runtime, callable: &str, args: &[&str], target: &str) -> Result<()> {
    copy(rt, "function", callable)?;
    invoke(rt, args, &[])?;
    copy(rt, target, "called")
}
fn apply(rt: &mut dyn Runtime, name: &str, args: &[&str], target: &str) -> Result<()> {
    function(rt, name)?;
    invoke(rt, args, &[])?;
    copy(rt, target, "called")
}
fn own(rt: &mut dyn Runtime, name: &str, args: &[&str], target: &str) -> Result<()> {
    member(rt, name, "function")?;
    invoke(rt, args, &[])?;
    copy(rt, target, "called")
}
fn method(
    rt: &mut dyn Runtime,
    object: &str,
    name: &str,
    args: &[&str],
    target: &str,
) -> Result<()> {
    attr(rt, object, name, "function")?;
    invoke(rt, args, &[])?;
    copy(rt, target, "called")
}
fn field(rt: &mut dyn Runtime, object: &str, key: &str, value: &str) -> Result<()> {
    rt.call("eval_field", json!([object, key, value]))
        .map(|_| ())
}
fn data(rt: &mut dyn Runtime, object: &str, key: &str, value: Value) -> Result<()> {
    literal(rt, "literal", value)?;
    field(rt, object, key, "literal")
}
fn binary(rt: &mut dyn Runtime, op: &str, a: &str, b: &str, target: &str) -> Result<()> {
    rt.call("web_binary", json!([op, a, b, target])).map(|_| ())
}
fn fallback(rt: &mut dyn Runtime, slot: &str, value: Value) -> Result<()> {
    if !boolean(rt, slot)? {
        literal(rt, slot, value)?;
    }
    Ok(())
}
fn is_none(rt: &mut dyn Runtime, slot: &str) -> Result<bool> {
    rt.call("provider_is_none", json!([slot]))
        .map(|v| truthy(&v))
}
fn catches(rt: &mut dyn Runtime, error: &Failure, names: &[&str]) -> Result<bool> {
    rt.call("exception_matches", json!([error, names]))
        .map(|v| truthy(&v))
}
fn returned(slot: &str) -> Result<Value> {
    Ok(json!({"provider_ref":slot}))
}
fn join(rt: &mut dyn Runtime, base: &str, suffix: &str, target: &str) -> Result<()> {
    literal(rt, "suffix", json!(suffix))?;
    apply(rt, "os.path.join", &[base, "suffix"], target)
}
fn mkdir(rt: &mut dyn Runtime, slot: &str) -> Result<()> {
    function(rt, "os.makedirs")?;
    literal(rt, "true", json!(true))?;
    invoke(rt, &[slot], &[("exist_ok", "true")])
}
fn home(rt: &mut dyn Runtime, codex: bool) -> Result<Value> {
    join(
        rt,
        "run_dir_path",
        if codex { ".codex-home" } else { ".agent-home" },
        "home",
    )?;
    returned("home")
}
fn bin_dir(rt: &mut dyn Runtime) -> Result<Value> {
    function(rt, "os.path.join")?;
    literal(rt, "local", json!(".local"))?;
    literal(rt, "bin", json!("bin"))?;
    invoke(rt, &["home", "local", "bin"], &[])?;
    returned("called")
}
fn exec_env(rt: &mut dyn Runtime, agent: &str) -> Result<Value> {
    literal(rt, "env", json!({}))?;
    if agent != "claude" {
        own(
            rt,
            if agent == "copilot" {
                "scaffold_copilot_home"
            } else {
                "agent_home"
            },
            &["run_dir_path"],
            "home",
        )?;
        field(
            rt,
            "env",
            if agent == "copilot" {
                "COPILOT_HOME"
            } else {
                "CODEX_HOME"
            },
            "home",
        )?;
    }
    returned("env")
}
fn copilot_home(rt: &mut dyn Runtime) -> Result<Value> {
    join(rt, "run_dir_path", ".copilot-home", "dest")?;
    apply(rt, "os.path.isdir", &["dest"], "exists")?;
    if boolean(rt, "exists")? {
        return returned("dest");
    }
    mkdir(rt, "dest")?;
    apply(rt, "copilot.copilot_config", &[], "src")?;
    for name in ["config.json", "settings.json", "mcp-config.json"] {
        literal(rt, "name", json!(name))?;
        apply(rt, "os.path.join", &["src", "name"], "path")?;
        apply(rt, "os.path.isfile", &["path"], "exists")?;
        if boolean(rt, "exists")? {
            function(rt, "shutil.copy2")?;
            copy(rt, "copy2", "function")?;
            apply(rt, "os.path.join", &["dest", "name"], "destination")?;
            call(rt, "copy2", &["path", "destination"], "copied")?;
        }
    }
    returned("dest")
}
fn prepare_copilot(rt: &mut dyn Runtime) -> Result<Value> {
    function(rt, "os.path.join")?;
    copy(rt, "join", "function")?;
    literal(rt, "tilde", json!("~"))?;
    apply(rt, "os.path.expanduser", &["tilde"], "real_home")?;
    literal(rt, "cache_name", json!(".cache"))?;
    literal(rt, "copilot_name", json!("copilot"))?;
    call(
        rt,
        "join",
        &["real_home", "cache_name", "copilot_name"],
        "real",
    )?;
    apply(rt, "os.path.isdir", &["real"], "exists")?;
    if !boolean(rt, "exists")? {
        return Ok(Value::Null);
    }
    join(rt, "home", ".cache", "cache")?;
    mkdir(rt, "cache")?;
    join(rt, "cache", "copilot", "link")?;
    apply(rt, "os.path.exists", &["link"], "exists")?;
    if !boolean(rt, "exists")? {
        match apply(rt, "os.symlink", &["real", "link"], "linked") {
            Err(error) if catches(rt, &error, &["OSError"])? => {
                rt.call("task_caught_log", json!([error]))?;
                let mut message = Text::from("could not link the copilot package cache (");
                message
                    .0
                    .extend(code_points(rt.call("handler_format", json!(["error"]))?)?.0);
                message
                    .0
                    .extend("); it will be re-downloaded into ".chars().map(u32::from));
                message
                    .0
                    .extend(code_points(rt.call("handler_format", json!(["cache"]))?)?.0);
                rt.call("logger_literal", json!(["message", message.0]))?;
                copy(rt, "function", "active_log")?;
                rt.call("handler_invoke", json!([["message"], []]))?;
            }
            result => result?,
        }
    }
    Ok(Value::Null)
}
fn write(rt: &mut dyn Runtime, path: &str, content: &str) -> Result<()> {
    literal(rt, "write_mode", json!("w"))?;
    literal(rt, "utf8", json!("utf-8"))?;
    invoke(rt, &[path, "write_mode"], &[("encoding", "utf8")])?;
    rt.call("task_enter", json!(["agentfile"]))?;
    let body = method(rt, "agentfile_handle", "write", &[content], "written");
    rt.call("task_exit", json!(["agentfile", body.as_ref().err()]))?;
    if let Err(error) = body
        && !boolean(rt, "exit_result")?
    {
        return Err(error);
    }
    Ok(())
}
fn config(rt: &mut dyn Runtime) -> Result<Value> {
    mkdir(rt, "home")?;
    function(rt, "codex.lean_instructions")?;
    copy(rt, "lean_instructions", "function")?;
    builtin(rt, "getattr")?;
    literal(rt, "base_prompt_name", json!("base_prompt"))?;
    literal(rt, "none", Value::Null)?;
    invoke(rt, &["self", "base_prompt_name", "none"], &[])?;
    copy(rt, "base_prompt", "called")?;
    member(rt, "_capabilities", "capabilities")?;
    call(
        rt,
        "lean_instructions",
        &["base_prompt", "capabilities"],
        "lean",
    )?;
    if boolean(rt, "lean")? {
        builtin(rt, "len")?;
        invoke(rt, &["lean"], &[])?;
        copy(rt, "lean_length", "called")?;
        literal(rt, "four", json!(4))?;
        binary(rt, "floordiv", "lean_length", "four", "tokens")?;
    } else {
        literal(rt, "tokens", json!(0))?;
    }
    rt.call("stream_set", json!(["_lean_instructions_tokens", "tokens"]))?;
    join(rt, "home", "instructions.md", "instr_path")?;
    function(rt, "codex.config_toml")?;
    copy(rt, "config_toml", "function")?;
    for (name, slot) in [
        ("routing", "routing"),
        ("model", "model"),
        ("effort", "effort"),
        ("_capabilities", "capabilities"),
        ("system_skills", "system_skills"),
    ] {
        member(rt, name, slot)?;
    }
    if boolean(rt, "lean")? {
        copy(rt, "instructions_path", "instr_path")?;
    } else {
        literal(rt, "instructions_path", Value::Null)?;
    }
    copy(rt, "function", "config_toml")?;
    invoke(
        rt,
        &[],
        &[
            ("routing", "routing"),
            ("model", "model"),
            ("effort", "effort"),
            ("project", "workspace"),
            ("capabilities", "capabilities"),
            ("system_skills", "system_skills"),
            ("instructions", "lean"),
            ("instructions_path", "instructions_path"),
        ],
    )?;
    copy(rt, "cfg", "called")?;
    builtin(rt, "open")?;
    copy(rt, "open", "function")?;
    join(rt, "home", "config.toml", "config_path")?;
    copy(rt, "function", "open")?;
    write(rt, "config_path", "cfg")?;
    if boolean(rt, "lean")? {
        builtin(rt, "open")?;
        write(rt, "instr_path", "lean")?;
    }
    returned("cfg")
}
fn get(rt: &mut dyn Runtime, object: &str, key: &str, target: &str) -> Result<()> {
    rt.call("provider_method", json!([object, "get", [key], target]))
        .map(|_| ())
}
fn merge(rt: &mut dyn Runtime, agent: &str) -> Result<Value> {
    apply(
        rt,
        match agent {
            "claude" => "_merge_usage",
            "copilot" => "copilot.merge_usage",
            _ => "codex.merge_usage",
        },
        &["a", "b"],
        "merged",
    )?;
    if agent == "copilot" {
        for (input, target) in [("a", "debug_a"), ("b", "debug_b")] {
            copy(rt, "mapping", input)?;
            fallback(rt, "mapping", json!({}))?;
            rt.call(
                "provider_method",
                json!(["mapping", "get", ["_debug_log", ""], target]),
            )?;
        }
        binary(rt, "add", "debug_a", "debug_b", "debug_log")?;
        field(rt, "merged", "_debug_log", "debug_log")?;
    }
    returned("merged")
}
fn usage(rt: &mut dyn Runtime, claude: bool) -> Result<Value> {
    if claude {
        copy(rt, "mapping", "result")?;
        fallback(rt, "mapping", json!({}))?;
        get(rt, "mapping", "usage", "usage")?;
        fallback(rt, "usage", json!({}))?;
    } else {
        literal(rt, "usage", json!({}))?;
        for key in [
            "input_tokens",
            "cache_read_input_tokens",
            "cache_creation_input_tokens",
            "output_tokens",
        ] {
            copy(rt, "mapping", "result")?;
            fallback(rt, "mapping", json!({}))?;
            get(rt, "mapping", key, "value")?;
            fallback(rt, "value", json!(0))?;
            field(rt, "usage", key, "value")?;
        }
    }
    returned("usage")
}
fn row(rt: &mut dyn Runtime, agent: &str) -> Result<Value> {
    if agent == "claude" {
        apply(rt, "_row_from_result", &["result"], "row")?;
        return returned("row");
    }
    builtin(rt, "dict")?;
    copy(rt, "dict", "function")?;
    copy(rt, "mapping", "result")?;
    fallback(rt, "mapping", json!({}))?;
    call(rt, "dict", &["mapping"], "row")?;
    rt.call(
        "provider_method",
        json!(["row", "setdefault", ["is_error", false], "default"]),
    )?;
    rt.call(
        "provider_method",
        json!(["row", "setdefault", ["cost_usd_vendor", null], "default"]),
    )?;
    get(rt, "row", "tokens_source", "source")?;
    if is_none(rt, "source")? {
        literal(rt, "incomplete", json!(false))?;
    } else {
        literal(
            rt,
            "expected_source",
            json!(if agent == "copilot" {
                "debug_log"
            } else {
                "stream_usage"
            }),
        )?;
        rt.call("web_compare", json!(["ne", "source", "expected_source"]))?;
        copy(rt, "incomplete", "comparison")?;
    }
    field(rt, "row", "usage_incomplete", "incomplete")?;
    get(rt, "row", "output_tokens", "output_tokens")?;
    let missing = is_none(rt, "output_tokens")?;
    data(rt, "row", "output_tokens_missing", json!(missing))?;
    data(rt, "row", "num_turns_recovered", json!(false))?;
    rt.call(
        "stream_item",
        json!(["row", "usage_incomplete", "row_incomplete"]),
    )?;
    if boolean(rt, "row_incomplete")? {
        attr(rt, "row", "setdefault", "setdefault")?;
        let mut reason = Text::from("tokens_source=");
        reason
            .0
            .extend(code_points(rt.call("task_format", json!(["source", ""]))?)?.0);
        rt.call("logger_literal", json!(["reason", reason.0]))?;
        literal(rt, "reason_key", json!("usage_incomplete_reason"))?;
        call(rt, "setdefault", &["reason_key", "reason"], "default")?;
    }
    returned("row")
}
pub fn dispatch(operation: &str, rt: &mut dyn Runtime) -> Result<Value> {
    match operation {
        "home_claude" => home(rt, false),
        "home_codex" => home(rt, true),
        "bin_dir" => bin_dir(rt),
        "exec_env_claude" => exec_env(rt, "claude"),
        "exec_env_copilot" => exec_env(rt, "copilot"),
        "exec_env_codex" => exec_env(rt, "codex"),
        "prepare_claude" => Ok(Value::Null),
        "prepare_copilot" => prepare_copilot(rt),
        "copilot_home" => copilot_home(rt),
        "config_codex" => config(rt),
        "merge_claude" => merge(rt, "claude"),
        "merge_copilot" => merge(rt, "copilot"),
        "merge_codex" => merge(rt, "codex"),
        "usage_claude" => usage(rt, true),
        "usage_copilot" | "usage_codex" => usage(rt, false),
        "row_claude" => row(rt, "claude"),
        "row_copilot" => row(rt, "copilot"),
        "row_codex" => row(rt, "codex"),
        _ => Err(Failure::new(
            "ValueError",
            format!("unknown Web-Bench agent helper: {operation}"),
        )),
    }
}
