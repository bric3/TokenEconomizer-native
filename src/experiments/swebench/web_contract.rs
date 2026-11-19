use super::{
    Result, Runtime,
    optimizer::code_points,
    providers::{boolean, copy, literal},
    runner_calls::{function, invoke, member},
};
use crate::{python_json::Text, value::truthy};
use serde_json::{Value, json};

const TASK: &str = include_str!("web-task-prompt.txt");
const ERROR: &str = include_str!("web-error-prompt.txt");

pub fn constants() -> Value {
    let mut constants = super::setup::constants()["web"].clone();
    constants.as_object_mut().unwrap().extend(json!({
        "TASK_TEMPLATE":TASK,"ERROR_TEMPLATE":ERROR,"attempt_pattern":super::web_output::ATTEMPT,
        "_MARK_OK":"✅","_MAX_FILE_BYTES":512*1024,
        "WEBBENCH_UNDICI_ABORT_S":300,"WEBBENCH_TASK_TIMEOUT_MAX":260,
        "_INIT_FORMAT_LINES":["Add filename after code block.",
            "The filename should appear on the line following the language specifier in your code block."],
        "_SKIP_DIRS":["node_modules",".git",".next",".nuxt",".svelte-kit",".parcel-cache",".turbo",
            "dist","build","out","coverage","test-results","playwright-report","eval","assets",
            "__pycache__",".pytest_cache",".claude",".copilot"],
        "_SKIP_FILES":["package-lock.json","pnpm-lock.yaml","yarn.lock",".DS_Store"],
        "classes":{
            "_AgentHandler":{"session":null},
            "WebBenchRunner":{"NET_GUARD_HOOK_KEY":"PreToolUse","NET_GUARD_PROBE":"echo https://github.com/probe"},
            "CopilotWebBenchRunner":{"NET_GUARD_HOOK_KEY":"preToolUse"},
            "CodexWebBenchRunner":{"_NS_PROBE":"echo https://github.com/probe","_net_guard_armed":false,
                "_AGENT_HOME_KEEP":["config.toml","instructions.md","AGENTS.md","RTK.md"]}
        }
    }).as_object().unwrap().clone());
    constants
}

fn builtin(rt: &mut dyn Runtime, name: &str) -> Result<()> {
    rt.call("eval_builtin", json!([name])).map(|_| ())
}
fn attribute(rt: &mut dyn Runtime, object: &str, name: &str, target: &str) -> Result<()> {
    rt.call("stdout_member", json!([object, name, target]))
        .map(|_| ())
}
fn call(rt: &mut dyn Runtime, function: &str, args: &[&str], target: &str) -> Result<()> {
    copy(rt, "function", function)?;
    invoke(rt, args, &[])?;
    copy(rt, target, "called")
}
fn apply(rt: &mut dyn Runtime, name: &str, args: &[&str], target: &str) -> Result<()> {
    function(rt, name)?;
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
    attribute(rt, object, name, "function")?;
    invoke(rt, args, &[])?;
    copy(rt, target, "called")
}
fn get(rt: &mut dyn Runtime, object: &str, key: &str, target: &str) -> Result<()> {
    rt.call("provider_method", json!([object, "get", [key], target]))
        .map(|_| ())
}
fn field(rt: &mut dyn Runtime, object: &str, key: &str, value: &str) -> Result<()> {
    rt.call("eval_field", json!([object, key, value]))
        .map(|_| ())
}
fn data(rt: &mut dyn Runtime, object: &str, key: &str, value: Value) -> Result<()> {
    literal(rt, "literal", value)?;
    field(rt, object, key, "literal")
}
fn iter(rt: &mut dyn Runtime, object: &str, target: &str) -> Result<()> {
    rt.call("workspace_iter", json!([object, target]))
        .map(|_| ())
}
fn next(rt: &mut dyn Runtime, iterator: &str, target: &str) -> Result<bool> {
    rt.call("workspace_next", json!([iterator, target]))
        .map(|v| truthy(&v))
}
fn append(rt: &mut dyn Runtime, object: &str, value: &str) -> Result<()> {
    rt.call("eval_append", json!([object, value])).map(|_| ())
}
fn fallback(rt: &mut dyn Runtime, object: &str, value: Value) -> Result<()> {
    if !boolean(rt, object)? {
        literal(rt, object, value)?;
    }
    Ok(())
}
fn is_none(rt: &mut dyn Runtime, object: &str) -> Result<bool> {
    rt.call("provider_is_none", json!([object]))
        .map(|v| truthy(&v))
}
fn contains(rt: &mut dyn Runtime, item: &str, object: &str) -> Result<bool> {
    rt.call("provider_contains", json!([item, object]))
        .map(|v| truthy(&v))
}
fn explicit_bool(rt: &mut dyn Runtime, object: &str, target: &str) -> Result<()> {
    builtin(rt, "bool")?;
    invoke(rt, &[object], &[])?;
    copy(rt, target, "called")
}
fn bool_get(rt: &mut dyn Runtime, key: &str, target: &str) -> Result<()> {
    builtin(rt, "bool")?;
    copy(rt, "bool", "function")?;
    get(rt, "run_kwargs", key, "raw_bool")?;
    call(rt, "bool", &["raw_bool"], target)
}
fn budget(rt: &mut dyn Runtime, target: &str) -> Result<()> {
    attribute(rt, "run_kwargs", "get", "budget_get")?;
    get(rt, "run_kwargs", "spiral_nudge", "nudge")?;
    let default = if boolean(rt, "nudge")? { 1 } else { 0 };
    literal(rt, "budget_default", json!(default))?;
    literal(rt, "budget_key", json!("spiral_nudge_budget"))?;
    call(rt, "budget_get", &["budget_key", "budget_default"], target)
}
fn accepts(rt: &mut dyn Runtime) -> Result<bool> {
    literal(rt, "never_abort", json!("never_abort"))?;
    apply(rt, "guard_accepts", &["never_abort"], "accepts")?;
    boolean(rt, "accepts")
}
fn verbose(rt: &mut dyn Runtime) -> Result<()> {
    rt.call("provider_env_get", json!(["verbosity", "TE_VERBOSE", "1"]))?;
    literal(rt, "one", json!("1"))?;
    rt.call("web_compare", json!(["eq", "verbosity", "one"]))?;
    copy(rt, "verbose", "comparison")
}

pub fn guard(rt: &mut dyn Runtime, codex: bool) -> Result<Value> {
    if codex {
        get(rt, "run_kwargs", "codex_spiral_policy", "policy")?;
        if is_none(rt, "policy")? {
            return Ok(Value::Null);
        }
        function(rt, "codex_spiral.CodexResumeGuard")?;
        copy(rt, "guard_constructor", "function")?;
        verbose(rt)?;
        copy(rt, "function", "guard_constructor")?;
        invoke(rt, &[], &[("policy", "policy"), ("verbose", "verbose")])?;
    } else {
        get(rt, "run_kwargs", "spiral_guard", "enabled")?;
        if !boolean(rt, "enabled")? {
            return Ok(Value::Null);
        }
        builtin(rt, "dict")?;
        copy(rt, "dict", "function")?;
        for (key, slot) in [
            ("abort_envelope", "envelope"),
            ("spiral_judge", "judge"),
            ("spiral_thr", "thr"),
        ] {
            get(rt, "run_kwargs", key, slot)?;
        }
        builtin(rt, "int")?;
        copy(rt, "int", "function")?;
        budget(rt, "budget")?;
        call(rt, "int", &["budget"], "nudge_budget")?;
        get(rt, "run_kwargs", "spiral_nudge_writer", "nudge_writer")?;
        verbose(rt)?;
        copy(rt, "function", "dict")?;
        invoke(
            rt,
            &[],
            &[
                ("envelope", "envelope"),
                ("judge", "judge"),
                ("thr", "thr"),
                ("nudge_budget", "nudge_budget"),
                ("nudge_writer", "nudge_writer"),
                ("verbose", "verbose"),
            ],
        )?;
        copy(rt, "guard_kwargs", "called")?;
        if accepts(rt)? {
            bool_get(rt, "spiral_never_abort", "never_abort_value")?;
            field(rt, "guard_kwargs", "never_abort", "never_abort_value")?;
        }
        function(rt, "NudgeGuard")?;
        rt.call("task_call_kwargs", json!([[], [], "guard_kwargs"]))?;
    }
    Ok(json!({"provider_ref":"called"}))
}

pub fn strip_transport(rt: &mut dyn Runtime) -> Result<Value> {
    literal(rt, "kept", json!([]))?;
    method(rt, "description", "splitlines", &[], "lines")?;
    iter(rt, "lines", "lines_iter")?;
    while next(rt, "lines_iter", "line")? {
        super::web_output::strip(rt, false, "stripped")?;
        rt.call(
            "provider_constant",
            json!(["format_lines", "_INIT_FORMAT_LINES"]),
        )?;
        if !contains(rt, "stripped", "format_lines")? {
            append(rt, "kept", "line")?;
        }
    }
    literal(rt, "newline", json!("\n"))?;
    method(rt, "newline", "join", &["kept"], "line")?;
    super::web_output::strip(rt, false, "stripped")?;
    Ok(json!({"provider_ref":"stripped"}))
}
fn render(
    rt: &mut dyn Runtime,
    captured: Value,
    template: &str,
    callable: &str,
    fields: &[(&str, &str)],
    target: &str,
) -> Result<()> {
    if captured.is_array() && code_points(captured)? == Text::from(template) {
        // Only the pinned prompt assets reach this field expansion.
        let mut rest = template;
        let mut text = Text::default();
        while let Some(at) = rest.find('{') {
            text.0.extend(rest[..at].chars().map(u32::from));
            rest = &rest[at + 1..];
            let end = rest.find('}').expect("native prompt field closes");
            let slot = fields
                .iter()
                .find(|(key, _)| *key == &rest[..end])
                .expect("native prompt field exists")
                .1;
            text.0
                .extend(code_points(rt.call("task_format", json!([slot, ""]))?)?.0);
            rest = &rest[end + 1..];
        }
        text.0.extend(rest.chars().map(u32::from));
        rt.call("logger_literal", json!([target, text.0]))?;
    } else {
        copy(rt, "function", callable)?;
        invoke(rt, &[], fields)?;
        copy(rt, target, "called")?;
    }
    Ok(())
}
pub fn prompt(rt: &mut dyn Runtime) -> Result<Value> {
    function(rt, "strip_transport_instructions")?;
    copy(rt, "strip_transport", "function")?;
    get(rt, "request", "task", "description")?;
    if !boolean(rt, "description")? {
        get(rt, "task", "problem_statement", "description")?;
        fallback(rt, "description", json!(""))?;
    }
    method(rt, "description", "strip", &[], "description")?;
    call(rt, "strip_transport", &["description"], "description")?;
    get(rt, "request", "error", "error")?;
    fallback(rt, "error", json!(""))?;
    method(rt, "error", "strip", &[], "error")?;
    let body_template = rt.call(
        "contract_format_capture",
        json!(["TASK_TEMPLATE", "body_format"]),
    )?;
    if boolean(rt, "error")? {
        let error_template = rt.call(
            "contract_format_capture",
            json!(["ERROR_TEMPLATE", "error_format"]),
        )?;
        rt.call("logger_slice", json!(["error", null, 8000, "error_slice"]))?;
        render(
            rt,
            error_template,
            ERROR,
            "error_format",
            &[("error", "error_slice")],
            "error_block",
        )?;
    } else {
        literal(rt, "error_block", json!(""))?;
    }
    render(
        rt,
        body_template,
        TASK,
        "body_format",
        &[
            ("cwd", "workspace"),
            ("description", "description"),
            ("error_block", "error_block"),
        ],
        "body",
    )?;
    if boolean(rt, "injected")? {
        method(rt, "injected", "rstrip", &[], "manual")?;
        literal(rt, "gap", json!("\n\n"))?;
        rt.call("web_binary", json!(["add", "manual", "gap", "prefix"]))?;
        rt.call("web_binary", json!(["add", "prefix", "body", "body"]))?;
    }
    Ok(json!({"provider_ref":"body"}))
}

fn exclusions(rt: &mut dyn Runtime) -> Result<()> {
    copy(rt, "excluded", "exclude_tools")?;
    fallback(rt, "excluded", json!([]))?;
    iter(rt, "excluded", "tools_iter")?;
    literal(rt, "drop", json!([]))?;
    while next(rt, "tools_iter", "tool")? {
        member(rt, "protected_tools", "protected")?;
        if !contains(rt, "tool", "protected")? {
            append(rt, "drop", "tool")?;
        }
    }
    apply(rt, "network_tool_block", &[], "network_tools")?;
    iter(rt, "network_tools", "tools_iter")?;
    literal(rt, "network_drop", json!([]))?;
    while next(rt, "tools_iter", "tool")? {
        if !contains(rt, "tool", "drop")? {
            append(rt, "network_drop", "tool")?;
        }
    }
    rt.call("web_binary", json!(["add", "drop", "network_drop", "drop"]))?;
    Ok(())
}
fn requested(rt: &mut dyn Runtime) -> Result<()> {
    builtin(rt, "list")?;
    copy(rt, "list", "function")?;
    copy(rt, "excluded", "exclude_tools")?;
    fallback(rt, "excluded", json!([]))?;
    call(rt, "list", &["excluded"], "requested")
}
fn finish_file(rt: &mut dyn Runtime, result: Result<()>) -> Result<()> {
    rt.call("task_exit", json!(["config", result.as_ref().err()]))?;
    if let Err(error) = result
        && !boolean(rt, "exit_result")?
    {
        return Err(error);
    }
    Ok(())
}
pub fn config(rt: &mut dyn Runtime) -> Result<Value> {
    exclusions(rt)?;
    literal(rt, "cfg", json!({}))?;
    for key in ["instance_id", "repo"] {
        get(rt, "task", key, "value")?;
        field(rt, "cfg", key, "value")?;
    }
    field(rt, "cfg", "arm", "arm")?;
    data(rt, "cfg", "domain", json!("webbench"))?;
    for name in ["agent", "model", "effort"] {
        member(rt, name, "value")?;
        field(rt, "cfg", name, "value")?;
    }
    field(rt, "cfg", "max_turns", "max_turns")?;
    for name in ["cli_version", "agent_bin"] {
        member(rt, name, "value")?;
        field(rt, "cfg", name, "value")?;
    }
    literal(rt, "routing", json!({}))?;
    for (name, key) in [
        ("routing_base", "base_url"),
        ("optimizer_model", "optimizer_model"),
    ] {
        member(rt, name, "value")?;
        field(rt, "routing", key, "value")?;
    }
    builtin(rt, "getattr")?;
    literal(rt, "attribute", json!("optimizer_transport"))?;
    literal(rt, "api", json!("api"))?;
    invoke(rt, &["self", "attribute", "api"], &[])?;
    field(rt, "routing", "optimizer_transport", "called")?;
    field(rt, "cfg", "routing", "routing")?;
    literal(rt, "tools", json!({}))?;
    requested(rt)?;
    field(rt, "tools", "excluded_requested", "requested")?;
    field(rt, "tools", "excluded_applied", "drop")?;
    builtin(rt, "sorted")?;
    copy(rt, "sorted", "function")?;
    member(rt, "protected_tools", "protected")?;
    call(rt, "sorted", &["protected"], "protected_sorted")?;
    field(rt, "tools", "protected", "protected_sorted")?;
    field(rt, "cfg", "tools", "tools")?;
    member(rt, "scaffold", "scaffold")?;
    if is_none(rt, "scaffold")? {
        literal(rt, "scaffold_config", Value::Null)?;
    } else {
        literal(rt, "scaffold_config", json!({}))?;
        member(rt, "scaffold", "scaffold")?;
        attribute(rt, "scaffold", "name", "scaffold_name")?;
        field(rt, "scaffold_config", "name", "scaffold_name")?;
        builtin(rt, "sorted")?;
        copy(rt, "sorted", "function")?;
        member(rt, "scaffold_answer_excludes", "function")?;
        invoke(rt, &[], &[])?;
        copy(rt, "scaffold_excludes", "called")?;
        call(rt, "sorted", &["scaffold_excludes"], "scaffold_excludes")?;
        field(
            rt,
            "scaffold_config",
            "answer_excludes",
            "scaffold_excludes",
        )?;
    }
    field(rt, "cfg", "scaffold", "scaffold_config")?;
    literal(rt, "injection", json!({}))?;
    explicit_bool(rt, "injected", "present")?;
    field(rt, "injection", "present", "present")?;
    builtin(rt, "len")?;
    copy(rt, "len", "function")?;
    copy(rt, "manual", "injected")?;
    fallback(rt, "manual", json!(""))?;
    call(rt, "len", &["manual"], "length")?;
    field(rt, "injection", "chars", "length")?;
    if boolean(rt, "injected")? {
        function(rt, "hashlib.sha256")?;
        copy(rt, "sha256", "function")?;
        method(rt, "injected", "encode", &[], "bytes")?;
        call(rt, "sha256", &["bytes"], "hash")?;
        method(rt, "hash", "hexdigest", &[], "digest")?;
        rt.call("logger_slice", json!(["digest", null, 16, "sha"]))?;
    } else {
        literal(rt, "sha", Value::Null)?;
    }
    field(rt, "injection", "sha256", "sha")?;
    field(rt, "cfg", "injection", "injection")?;
    literal(rt, "spiral", json!({}))?;
    bool_get(rt, "spiral_guard", "enabled")?;
    field(rt, "spiral", "guard", "enabled")?;
    for (key, name) in [("abort_envelope", "envelope"), ("spiral_thr", "thresholds")] {
        get(rt, "run_kwargs", key, "value")?;
        field(rt, "spiral", name, "value")?;
    }
    for (key, name) in [
        ("spiral_judge", "judge"),
        ("spiral_nudge_writer", "nudge_writer_llm"),
    ] {
        get(rt, "run_kwargs", key, "value")?;
        let present = !is_none(rt, "value")?;
        data(rt, "spiral", name, json!(present))?;
    }
    budget(rt, "budget")?;
    field(rt, "spiral", "nudge_budget", "budget")?;
    if accepts(rt)? {
        bool_get(rt, "spiral_never_abort", "never_abort_value")?;
    } else {
        literal(rt, "never_abort_value", json!(false))?;
    }
    field(rt, "spiral", "never_abort", "never_abort_value")?;
    data(rt, "spiral", "guard_per_attempt", json!(true))?;
    field(rt, "cfg", "spiral", "spiral")?;
    literal(rt, "webbench", json!({}))?;
    field(rt, "webbench", "project", "project")?;
    member(rt, "repo", "value")?;
    field(rt, "webbench", "evaluator_repo", "value")?;
    for (name, key) in [
        ("config_path", "evaluator_config"),
        ("log_path", "evaluator_log"),
        ("hash", "evaluator_hash"),
    ] {
        attribute(rt, "sess", name, "value")?;
        field(rt, "webbench", key, "value")?;
    }
    for (name, key) in [
        ("retry", "retry"),
        ("max_workers", "playwright_workers"),
        ("node", "node"),
    ] {
        member(rt, name, "value")?;
        field(rt, "webbench", key, "value")?;
    }
    field(rt, "cfg", "webbench", "webbench")?;
    literal(rt, "timeouts", json!({}))?;
    for (name, key) in [
        ("task_timeout", "task_s"),
        ("verdict_timeout", "verdict_s"),
        ("start_timeout", "start_s"),
    ] {
        member(rt, name, "value")?;
        field(rt, "timeouts", key, "value")?;
    }
    field(rt, "cfg", "timeouts", "timeouts")?;
    let mut grading = Text::from("@web-bench/");
    grading
        .0
        .extend(code_points(rt.call("task_format", json!(["project", ""]))?)?.0);
    grading
        .0
        .extend(" playwright suite (cumulative)".chars().map(u32::from));
    rt.call("logger_literal", json!(["grading", grading.0]))?;
    field(rt, "cfg", "grading_source", "grading")?;
    builtin(rt, "open")?;
    copy(rt, "open", "function")?;
    literal(rt, "filename", json!("run_config.json"))?;
    apply(
        rt,
        "os.path.join",
        &["run_dir_path", "filename"],
        "config_path",
    )?;
    literal(rt, "write", json!("w"))?;
    literal(rt, "utf8", json!("utf-8"))?;
    copy(rt, "function", "open")?;
    invoke(rt, &["config_path", "write"], &[("encoding", "utf8")])?;
    rt.call("task_enter", json!(["config"]))?;
    let written: Result<()> = (|| {
        function(rt, "json.dump")?;
        copy(rt, "dump", "function")?;
        literal(rt, "indent", json!(2))?;
        builtin(rt, "str")?;
        copy(rt, "default", "function")?;
        copy(rt, "function", "dump")?;
        invoke(
            rt,
            &["cfg", "config_handle"],
            &[("indent", "indent"), ("default", "default")],
        )
    })();
    finish_file(rt, written)?;
    Ok(Value::Null)
}
