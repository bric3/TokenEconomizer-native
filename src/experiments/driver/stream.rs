use super::{
    Result, Runtime, Text, Value, append, attr, boolean, builtin, contains, copy, exit, formatted,
    function, invoke, json, literal, method, truthy,
};

fn call(rt: &mut dyn Runtime, name: &str, args: &[&str], target: &str) -> Result<()> {
    function(rt, name)?;
    invoke(rt, args, &[])?;
    copy(rt, target, "called")
}
fn argument(rt: &mut dyn Runtime, name: &str, options: Value, refs: &[(&str, &str)]) -> Result<()> {
    rt.call("driver_argument", json!([name, options, refs]))
        .map(|_| ())
}
fn parser(rt: &mut dyn Runtime) -> Result<()> {
    function(rt, "__doc__")?;
    copy(rt, "description", "function")?;
    function(rt, "argparse.RawDescriptionHelpFormatter")?;
    copy(rt, "formatter", "function")?;
    function(rt, "argparse.ArgumentParser")?;
    invoke(
        rt,
        &[],
        &[
            ("description", "description"),
            ("formatter_class", "formatter"),
        ],
    )?;
    copy(rt, "ap", "called")?;
    function(rt, "ARMS")?;
    copy(rt, "registry", "function")?;
    builtin(rt, "sorted", &["registry"], "choices")?;
    function(rt, "SCAFFOLD_ARMS")?;
    copy(rt, "scaffold_registry", "function")?;
    builtin(rt, "sorted", &["scaffold_registry"], "scaffolds")?;
    literal(rt, "separator", json!(", "))?;
    method(rt, "separator", "join", &["scaffolds"])?;
    let mut help = Text::from("arm to run; ");
    help.0.extend(formatted(rt, "called", "")?.0);
    append(
        &mut help,
        " additionally install an external tool scaffold (experiments/te_bench/baselines/)",
    );
    super::text(rt, "help", help)?;
    argument(
        rt,
        "--arm",
        json!({"required":true}),
        &[("choices", "choices"), ("help", "help")],
    )?;
    argument(
        rt,
        "--agent",
        json!({"default":"claude","choices":["claude","copilot","codex"],"help":"the sealed agent (selects transcript schema + generic-vs-codex mechanisms)"}),
        &[],
    )?;
    argument(
        rt,
        "--repo",
        json!({"default":"","help":"repo slug, e.g. pydata/xarray"}),
        &[],
    )?;
    for (name, options) in [
        ("--seed", json!({"default":0})),
        ("--limit", json!({"default":0,"help":"cap tasks (0 = all)"})),
    ] {
        rt.call("eval_builtin", json!(["int"]))?;
        copy(rt, "integer", "function")?;
        argument(rt, name, options, &[("type", "integer")])?;
    }
    argument(
        rt,
        "--adapter",
        json!({"default":"swebench","choices":["swebench","webbench","creddata","reposvul"]}),
        &[],
    )?;
    argument(
        rt,
        "--tasks-file",
        json!({"required":true,"help":"JSON list of {instance_id, problem_statement, repo} (a placeholder you point at your dataset export; see README.md)"}),
        &[],
    )?;
    argument(
        rt,
        "--workload",
        json!({"default":"","help":"workloads/<domain>.json manifest naming the exact evaluated instances; --tasks-file is filtered to it (and to --repo's group). Committed in this repo — the reproducible way to pin a run."}),
        &[],
    )?;
    argument(
        rt,
        "--results-root",
        json!({"required":true,"help":"<domain> results root; the arm subdir is created under it"}),
        &[],
    )?;
    argument(
        rt,
        "--runner",
        json!({"default":"stub","help":"'stub' (offline) or 'module:factory' returning an AgentRunner, e.g. te_harness.swebench:make_runner (the published harness)"}),
        &[],
    )?;
    argument(rt, "--model", json!({"default":"agent"}), &[])?;
    argument(
        rt,
        "--optimizer-model",
        json!({"default":"agent","help":"agent label whose model runs the between-task optimizer (agent|claude|copilot|codex...); NOT a model id"}),
        &[],
    )?;
    rt.call("eval_builtin", json!(["int"]))?;
    copy(rt, "integer", "function")?;
    argument(
        rt,
        "--max-turns",
        json!({"default":80}),
        &[("type", "integer")],
    )?;
    argument(
        rt,
        "--fresh",
        json!({"action":"store_true","help":"ignore any saved cross-task state (start the stream cold)"}),
        &[],
    )?;
    method(rt, "ap", "parse_args", &[])?;
    copy(rt, "args", "called")
}
fn args(rt: &mut dyn Runtime, name: &str) -> Result<()> {
    attr(rt, "args", name, name)
}
fn print(rt: &mut dyn Runtime, message: Text) -> Result<()> {
    rt.call("eval_builtin", json!(["print"]))?;
    super::text(rt, "message", message)?;
    invoke(rt, &["message"], &[])
}
fn arm_prefix(rt: &mut dyn Runtime) -> Result<Text> {
    let mut message = Text::from("[arm ");
    args(rt, "arm")?;
    message.0.extend(formatted(rt, "arm", "")?.0);
    append(&mut message, "] ");
    Ok(message)
}
fn configure(rt: &mut dyn Runtime) -> Result<()> {
    args(rt, "optimizer_model")?;
    literal(rt, "hyphen", json!("-"))?;
    if contains(rt, "hyphen", "optimizer_model")? {
        for prefix in ["claude-", "gpt-", "gemini-", "o1-", "o3-", "o4-"] {
            literal(rt, "prefix", json!(prefix))?;
            method(rt, "optimizer_model", "startswith", &["prefix"])?;
            if boolean(rt, "called")? {
                let mut message = Text::from("--optimizer-model ");
                message.0.extend(formatted(rt, "optimizer_model", "!r")?.0);
                append(
                    &mut message,
                    " looks like a model id, but this flag takes an AGENT LABEL. The between-task optimizer runs on the agent's own model so mechanism overhead is priced on the same basis as the task rows; pass --optimizer-model agent (or the agent's name).",
                );
                exit(rt, message)?;
            }
        }
    }
    function(rt, "ARMS")?;
    args(rt, "arm")?;
    rt.call("scaffold_item", json!(["function", "arm", "selected_arm"]))?;
    attr(rt, "selected_arm", "mode", "mode")?;
    attr(rt, "selected_arm", "cfg", "cfg_factory")?;
    attr(rt, "selected_arm", "env", "arm_env")?;
    method(rt, "arm_env", "items", &[])?;
    rt.call("workspace_iter", json!(["called", "iterator"]))?;
    while truthy(&rt.call("workspace_next", json!(["iterator", "entry"]))?) {
        rt.call("workspace_pair", json!(["entry", "key", "value"]))?;
        call(rt, "os.environ.get", &["key"], "prior")?;
        if !truthy(&rt.call("provider_is_none", json!(["prior"]))?) {
            rt.call("web_compare", json!(["ne", "prior", "value"]))?;
            if boolean(rt, "comparison")? {
                let mut message = arm_prefix(rt)?;
                message.0.extend(formatted(rt, "key", "")?.0);
                append(&mut message, ": overriding inherited ");
                message.0.extend(formatted(rt, "prior", "!r")?.0);
                append(&mut message, " with ");
                message.0.extend(formatted(rt, "value", "!r")?.0);
                print(rt, message)?;
            }
        }
        function(rt, "os.environ")?;
        rt.call("workspace_put", json!(["function", "key", "value"]))?;
    }
    attr(rt, "selected_arm", "env", "arm_env")?;
    if boolean(rt, "arm_env")? {
        let mut message = arm_prefix(rt)?;
        append(&mut message, "env: ");
        attr(rt, "selected_arm", "env", "arm_env")?;
        method(rt, "arm_env", "items", &[])?;
        builtin(rt, "sorted", &["called"], "entries")?;
        rt.call("workspace_iter", json!(["entries", "iterator"]))?;
        let mut first = true;
        while truthy(&rt.call("workspace_next", json!(["iterator", "entry"]))?) {
            rt.call("workspace_pair", json!(["entry", "key", "value"]))?;
            if !first {
                append(&mut message, " ");
            }
            first = false;
            message.0.extend(formatted(rt, "key", "")?.0);
            append(&mut message, "=");
            message.0.extend(formatted(rt, "value", "")?.0);
        }
        print(rt, message)?;
    }
    attr(rt, "selected_arm", "scaffold", "scaffold")?;
    if boolean(rt, "scaffold")? {
        let mut message = arm_prefix(rt)?;
        append(&mut message, "external tool scaffold: ");
        attr(rt, "selected_arm", "scaffold", "scaffold")?;
        message.0.extend(formatted(rt, "scaffold", "")?.0);
        print(rt, message)?;
    }
    copy(rt, "function", "cfg_factory")?;
    invoke(rt, &[], &[])?;
    copy(rt, "config", "called")?;
    call(rt, "needs_optimizer", &["config"], "paid")?;
    let paid = boolean(rt, "paid")?;
    literal(rt, "preflight", json!(if paid { "1" } else { "0" }))?;
    literal(rt, "key", json!("TE_OPTIMIZER_PREFLIGHT"))?;
    function(rt, "os.environ")?;
    rt.call("workspace_put", json!(["function", "key", "preflight"]))?;
    for name in ["adapter", "tasks_file", "repo"] {
        args(rt, name)?;
    }
    call(
        rt,
        "load_adapter",
        &["adapter", "tasks_file", "repo"],
        "adapter",
    )?;
    for name in ["runner", "agent", "model"] {
        args(rt, name)?;
    }
    attr(rt, "selected_arm", "scaffold", "scaffold")?;
    call(
        rt,
        "load_runner",
        &["runner", "agent", "model", "scaffold"],
        "runner",
    )?;
    for name in ["tasks_file", "repo", "limit", "workload"] {
        args(rt, name)?;
    }
    call(
        rt,
        "load_tasks",
        &["tasks_file", "repo", "limit", "workload"],
        "tasks",
    )?;
    if !boolean(rt, "tasks")? {
        exit(rt, Text::from("no tasks after filtering"))?;
    }
    Ok(())
}
fn directories(rt: &mut dyn Runtime) -> Result<()> {
    args(rt, "repo")?;
    literal(rt, "slash", json!("/"))?;
    method(rt, "repo", "split", &["slash"])?;
    rt.call("stream_item", json!(["called", -1, "tail"]))?;
    if !boolean(rt, "tail")? {
        literal(rt, "tail", json!("repo"))?;
    }
    literal(rt, "hyphen", json!("-"))?;
    literal(rt, "underscore", json!("_"))?;
    method(rt, "tail", "replace", &["hyphen", "underscore"])?;
    copy(rt, "repo_tag", "called")?;
    let mut cell = formatted(rt, "repo_tag", "")?;
    append(&mut cell, "_s");
    args(rt, "seed")?;
    cell.0.extend(formatted(rt, "seed", "")?.0);
    super::text(rt, "cell", cell)?;
    args(rt, "results_root")?;
    args(rt, "arm")?;
    call(
        rt,
        "os.path.join",
        &["results_root", "arm", "cell"],
        "arm_root",
    )?;
    literal(rt, "streams", json!("_streams"))?;
    call(rt, "os.path.join", &["arm_root", "streams"], "streams_dir")?;
    function(rt, "os.makedirs")?;
    literal(rt, "yes", json!(true))?;
    invoke(rt, &["streams_dir"], &[("exist_ok", "yes")])
}
fn construct(rt: &mut dyn Runtime) -> Result<()> {
    function(rt, "Economizer")?;
    copy(rt, "constructor", "function")?;
    args(rt, "agent")?;
    args(rt, "repo")?;
    if !boolean(rt, "repo")? {
        copy(rt, "repo", "repo_tag")?;
    }
    for name in ["fresh", "optimizer_model", "max_turns"] {
        args(rt, name)?;
    }
    copy(rt, "function", "constructor")?;
    invoke(
        rt,
        &["config"],
        &[
            ("agent", "agent"),
            ("repo", "repo"),
            ("results_root", "arm_root"),
            ("fresh", "fresh"),
            ("optimizer_model", "optimizer_model"),
            ("max_turns", "max_turns"),
            ("runner", "runner"),
            ("adapter", "adapter"),
        ],
    )?;
    copy(rt, "econ", "called")?;
    literal(rt, "begin_name", json!("begin_stream"))?;
    builtin(rt, "hasattr", &["runner", "begin_name"], "has_begin")?;
    if boolean(rt, "has_begin")? {
        attr(rt, "runner", "begin_stream", "function")?;
        copy(rt, "begin", "function")?;
        for name in ["arm", "repo", "seed"] {
            args(rt, name)?;
        }
        copy(rt, "function", "begin")?;
        invoke(
            rt,
            &["tasks"],
            &[
                ("results_root", "arm_root"),
                ("arm", "arm"),
                ("repo", "repo"),
                ("seed", "seed"),
            ],
        )?;
    }
    let mut filename = formatted(rt, "repo_tag", "")?;
    append(&mut filename, "__");
    filename.0.extend(formatted(rt, "mode", "")?.0);
    append(&mut filename, "__stream.jsonl");
    super::text(rt, "stream_name", filename)?;
    call(
        rt,
        "os.path.join",
        &["streams_dir", "stream_name"],
        "stream_path",
    )
}
fn task_directory(rt: &mut dyn Runtime) -> Result<()> {
    function(rt, "time.strftime")?;
    copy(rt, "strftime", "function")?;
    call(rt, "time.gmtime", &[], "utc")?;
    literal(rt, "stamp_format", json!("%Y%m%dT%H%M%SZ"))?;
    copy(rt, "function", "strftime")?;
    invoke(rt, &["stamp_format", "utc"], &[])?;
    let mut name = formatted(rt, "called", "")?;
    append(&mut name, "__");
    name.0.extend(formatted(rt, "i", "03d")?.0);
    append(&mut name, "__");
    args(rt, "agent")?;
    name.0.extend(formatted(rt, "agent", "")?.0);
    append(&mut name, "__");
    literal(rt, "key", json!("instance_id"))?;
    literal(rt, "default_id", json!("inst"))?;
    method(rt, "task", "get", &["key", "default_id"])?;
    name.0.extend(formatted(rt, "called", "")?.0);
    append(&mut name, "__");
    args(rt, "arm")?;
    name.0.extend(formatted(rt, "arm", "")?.0);
    append(&mut name, "__s");
    args(rt, "seed")?;
    name.0.extend(formatted(rt, "seed", "")?.0);
    append(&mut name, "__");
    name.0.extend(formatted(rt, "mode", "")?.0);
    super::text(rt, "run_name", name)?;
    call(rt, "os.path.join", &["arm_root", "run_name"], "run_dir")
}
fn row(rt: &mut dyn Runtime) -> Result<()> {
    literal(rt, "srow", json!({}))?;
    for key in ["resolved", "num_turns"] {
        literal(rt, "key", json!(key))?;
        method(rt, "row", "get", &["key"])?;
        rt.call("eval_field", json!(["srow", key, "called"]))?;
    }
    literal(rt, "key", json!("instance_id"))?;
    literal(rt, "empty", json!(""))?;
    method(rt, "task", "get", &["key", "empty"])?;
    rt.call("eval_field", json!(["srow", "instance", "called"]))?;
    method(
        rt,
        "econ",
        "after_task",
        &["task", "row", "saved_dir", "srow"],
    )?;
    rt.call("workspace_pair", json!(["called", "opt_tokens", "reason"]))?;
    literal(rt, "key", json!("seed"))?;
    args(rt, "seed")?;
    method(rt, "row", "setdefault", &["key", "seed"])?;
    literal(rt, "key", json!("run_dir"))?;
    method(rt, "row", "setdefault", &["key", "saved_dir"])?;
    literal(rt, "key", json!("optimizer_tokens"))?;
    literal(rt, "zero", json!(0))?;
    method(rt, "row", "get", &["key", "zero"])?;
    copy(rt, "prior_tokens", "called")?;
    copy(rt, "overhead", "opt_tokens")?;
    if !boolean(rt, "overhead")? {
        copy(rt, "overhead", "zero")?;
    }
    rt.call(
        "web_binary",
        json!(["add", "prior_tokens", "overhead", "combined_tokens"]),
    )?;
    rt.call(
        "eval_field",
        json!(["row", "optimizer_tokens", "combined_tokens"]),
    )?;
    call(rt, "lift_overhead", &["row", "srow"], "ignored")?;
    literal(rt, "filename", json!("row.json"))?;
    call(rt, "os.path.join", &["saved_dir", "filename"], "row_path")?;
    rt.call("eval_builtin", json!(["open"]))?;
    literal(rt, "write", json!("w"))?;
    literal(rt, "encoding", json!("utf-8"))?;
    invoke(rt, &["row_path", "write"], &[("encoding", "encoding")])?;
    rt.call("task_enter", json!(["row_file"]))?;
    let dumped = (|| {
        function(rt, "json.dump")?;
        literal(rt, "indent", json!(2))?;
        invoke(rt, &["row", "row_file_handle"], &[("indent", "indent")])
    })();
    match dumped {
        Ok(()) => {
            rt.call("task_exit", json!(["row_file", null]))?;
        }
        Err(error) => {
            rt.call("task_exit", json!(["row_file", error]))?;
            if !boolean(rt, "exit_result")? {
                return Err(error);
            }
        }
    }
    Ok(())
}
fn stream_row(rt: &mut dyn Runtime) -> Result<()> {
    literal(
        rt,
        "base_keys",
        json!([
            "input_tokens",
            "cache_read_input_tokens",
            "cache_creation_input_tokens",
            "output_tokens",
            "tokens_total",
            "optimizer_tokens",
            "codex_spiral_nudges"
        ]),
    )?;
    builtin(rt, "tuple", &["base_keys"], "base_keys")?;
    function(rt, "ledger.OVERHEAD_KEYS")?;
    rt.call(
        "web_binary",
        json!(["add", "base_keys", "function", "keys"]),
    )?;
    literal(rt, "fields", json!({}))?;
    rt.call("workspace_iter", json!(["keys", "keys_iter"]))?;
    while truthy(&rt.call("workspace_next", json!(["keys_iter", "key"]))?) {
        method(rt, "row", "get", &["key"])?;
        let present = !truthy(&rt.call("provider_is_none", json!(["called"]))?);
        if present || contains(rt, "key", "srow")? {
            method(rt, "row", "get", &["key"])?;
            rt.call("workspace_put", json!(["fields", "key", "called"]))?;
        }
    }
    literal(rt, "merged", json!({}))?;
    method(rt, "merged", "update", &["srow"])?;
    method(rt, "merged", "update", &["fields"])?;
    call(rt, "json.dumps", &["merged"], "document")?;
    literal(rt, "newline", json!("\n"))?;
    rt.call(
        "web_binary",
        json!(["add", "document", "newline", "document"]),
    )?;
    method(rt, "stream_handle", "write", &["document"])
}
fn tasks(rt: &mut dyn Runtime, resolved: &mut u64) -> Result<()> {
    builtin(rt, "enumerate", &["tasks"], "enumerated")?;
    rt.call("workspace_iter", json!(["enumerated", "task_iter"]))?;
    while truthy(&rt.call("workspace_next", json!(["task_iter", "entry"]))?) {
        rt.call("workspace_pair", json!(["entry", "i", "task"]))?;
        rt.call("eval_field", json!(["task", "_index", "i"]))?;
        method(rt, "econ", "injection_prefix", &["task"])?;
        copy(rt, "injected", "called")?;
        method(rt, "econ", "build_tools", &["task", "i"])?;
        copy(rt, "exclude_tools", "called")?;
        rt.call("eval_builtin", json!(["dict"]))?;
        copy(rt, "dictionary", "function")?;
        method(rt, "econ", "run_kwargs", &["task"])?;
        copy(rt, "options", "called")?;
        copy(rt, "function", "dictionary")?;
        invoke(rt, &["options"], &[])?;
        copy(rt, "run_kwargs", "called")?;
        literal(rt, "key", json!("model"))?;
        args(rt, "model")?;
        method(rt, "run_kwargs", "setdefault", &["key", "model"])?;
        task_directory(rt)?;
        attr(rt, "runner", "run_task", "function")?;
        copy(rt, "run", "function")?;
        args(rt, "arm")?;
        copy(rt, "function", "run")?;
        rt.call(
            "task_call_kwargs",
            json!([
                ["task"],
                [
                    ["arm", "arm"],
                    ["injected", "injected"],
                    ["exclude_tools", "exclude_tools"],
                    ["run_dir_path", "run_dir"]
                ],
                "run_kwargs"
            ]),
        )?;
        rt.call("workspace_pair", json!(["called", "row", "saved_dir"]))?;
        row(rt)?;
        stream_row(rt)?;
        literal(rt, "key", json!("resolved"))?;
        method(rt, "row", "get", &["key"])?;
        literal(rt, "yes", json!(true))?;
        rt.call("web_compare", json!(["is_", "called", "yes"]))?;
        if boolean(rt, "comparison")? {
            *resolved += 1;
        }
    }
    Ok(())
}
fn execute(rt: &mut dyn Runtime, resolved: &mut u64) -> Result<()> {
    rt.call("eval_builtin", json!(["open"]))?;
    literal(rt, "write", json!("w"))?;
    literal(rt, "encoding", json!("utf-8"))?;
    invoke(rt, &["stream_path", "write"], &[("encoding", "encoding")])?;
    rt.call("task_enter", json!(["stream"]))?;
    let ran = tasks(rt, resolved);
    match ran {
        Ok(()) => {
            rt.call("task_exit", json!(["stream", null]))?;
        }
        Err(error) => {
            rt.call("task_exit", json!(["stream", error]))?;
            if !boolean(rt, "exit_result")? {
                return Err(error);
            }
        }
    }
    Ok(())
}
pub(super) fn main(rt: &mut dyn Runtime) -> Result<Value> {
    call(rt, "load_env", &[], "ignored")?;
    parser(rt)?;
    configure(rt)?;
    directories(rt)?;
    construct(rt)?;
    let mut resolved = 0;
    let ran = execute(rt, &mut resolved);
    rt.call("handler_exception", json!([ran.as_ref().err()]))?;
    literal(rt, "end_name", json!("end_stream"))?;
    rt.call("eval_builtin", json!(["hasattr"]))?;
    rt.call("handler_invoke", json!([["runner", "end_name"], []]))?;
    if boolean(rt, "called")? {
        rt.call(
            "handler_member",
            json!(["runner", "end_stream", "function"]),
        )?;
        rt.call("handler_invoke", json!([[], []]))?;
    }
    ran?;
    let mut message = Text::from("arm=");
    for (name, prefix) in [("arm", ""), ("agent", " agent=")] {
        append(&mut message, prefix);
        args(rt, name)?;
        message.0.extend(formatted(rt, name, "")?.0);
    }
    append(&mut message, " repo=");
    args(rt, "repo")?;
    if !boolean(rt, "repo")? {
        copy(rt, "repo", "repo_tag")?;
    }
    message.0.extend(formatted(rt, "repo", "")?.0);
    append(&mut message, " seed=");
    args(rt, "seed")?;
    message.0.extend(formatted(rt, "seed", "")?.0);
    append(&mut message, " tasks=");
    builtin(rt, "len", &["tasks"], "count")?;
    message.0.extend(formatted(rt, "count", "")?.0);
    append(&mut message, &format!(" resolved={resolved} -> "));
    message.0.extend(formatted(rt, "arm_root", "")?.0);
    print(rt, message)?;
    let mut message = Text::from("stream -> ");
    message.0.extend(formatted(rt, "stream_path", "")?.0);
    print(rt, message)?;
    Ok(Value::Null)
}
