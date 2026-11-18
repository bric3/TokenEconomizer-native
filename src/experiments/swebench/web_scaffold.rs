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
fn returned(slot: &str) -> Value {
    json!({"provider_ref":slot})
}
fn tuple(rt: &mut dyn Runtime, target: &str, items: &[&str]) -> Result<()> {
    let mut refs = Vec::new();
    for (i, value) in items.iter().enumerate() {
        let slot = format!("tuple_{i}");
        literal(rt, &slot, json!(value))?;
        refs.push(slot);
    }
    rt.call("eval_tuple", json!([target, refs])).map(|_| ())
}
fn closure(
    rt: &mut dyn Runtime,
    target: &str,
    kind: &str,
    operation: &str,
    captures: &[(&str, &str)],
) -> Result<()> {
    rt.call("eval_closure", json!([target, kind, operation, captures]))
        .map(|_| ())
}
fn profile(rt: &mut dyn Runtime, agent: &str) -> Result<Value> {
    function(rt, "scaffolds_pkg.AgentProfile")?;
    copy(rt, "constructor", "function")?;
    literal(rt, "kind", json!(agent))?;
    literal(
        rt,
        "instructions_file",
        json!(match agent {
            "claude" => "CLAUDE.md",
            "copilot" => ".github/copilot-instructions.md",
            _ => "AGENTS.md",
        }),
    )?;
    tuple(
        rt,
        "nav_tools",
        match agent {
            "claude" => &["Read", "Grep", "Glob"],
            "copilot" => &["view", "grep", "glob"],
            _ => &[],
        },
    )?;
    closure(
        rt,
        "mcp_args",
        "mcp",
        if agent == "claude" {
            "scaffold_mcp_claude"
        } else {
            "scaffold_mcp_empty"
        },
        &[],
    )?;
    tuple(
        rt,
        "patch_excludes",
        match agent {
            "claude" => &["CLAUDE.md"],
            "copilot" => &[".github/copilot-instructions.md", ".github/hooks"],
            _ => &["AGENTS.md", "RTK.md"],
        },
    )?;
    own(rt, "scaffold_bin_dir", &["home"], "bin_dir")?;
    let mut kwargs = vec![
        ("kind", "kind"),
        ("home", "home"),
        ("instructions_file", "instructions_file"),
        ("nav_tools", "nav_tools"),
        ("mcp_args", "mcp_args"),
        ("patch_excludes", "patch_excludes"),
        ("bin_dir", "bin_dir"),
    ];
    if agent == "codex" {
        literal(rt, "env", json!({}))?;
        field(rt, "env", "CODEX_HOME", "home")?;
        kwargs.push(("env", "env"));
    }
    copy(rt, "function", "constructor")?;
    invoke(rt, &[], &kwargs)?;
    Ok(returned("called"))
}
fn context(rt: &mut dyn Runtime) -> Result<Value> {
    closure(rt, "host", "host", "scaffold_host", &[])?;
    closure(
        rt,
        "exec",
        "exec",
        "scaffold_exec",
        &[
            ("self", "self"),
            ("workspace", "workspace"),
            ("home", "home"),
            ("run_dir_path", "run_dir_path"),
        ],
    )?;
    closure(rt, "place", "place", "scaffold_place", &[])?;
    closure(
        rt,
        "copy_in",
        "copy",
        "scaffold_copy",
        &[("self", "self"), ("home", "home"), ("place", "place")],
    )?;
    function(rt, "scaffolds_pkg.ScaffoldContext")?;
    copy(rt, "constructor", "function")?;
    own(rt, "scaffold_profile_for", &["home"], "profile")?;
    literal(rt, "none", Value::Null)?;
    function(rt, "_log")?;
    copy(rt, "log", "function")?;
    copy(rt, "function", "constructor")?;
    invoke(
        rt,
        &[],
        &[
            ("profile", "profile"),
            ("container", "none"),
            ("task", "task"),
            ("run_dir", "run_dir_path"),
            ("exec_", "exec"),
            ("copy_in", "copy_in"),
            ("host_run", "host"),
            ("read_bytes", "none"),
            ("log", "log"),
        ],
    )?;
    Ok(returned("called"))
}
fn host(rt: &mut dyn Runtime) -> Result<Value> {
    function(rt, "subprocess.run")?;
    literal(rt, "true", json!(true))?;
    invoke(
        rt,
        &["argv"],
        &[
            ("capture_output", "true"),
            ("text", "true"),
            ("timeout", "timeout"),
        ],
    )?;
    Ok(returned("called"))
}
fn exec(rt: &mut dyn Runtime) -> Result<Value> {
    builtin(rt, "dict")?;
    copy(rt, "dict", "function")?;
    function(rt, "os.environ")?;
    copy(rt, "environ", "function")?;
    call(rt, "dict", &["environ"], "e")?;
    field(rt, "e", "HOME", "home")?;
    function(rt, "os.pathsep")?;
    copy(rt, "pathsep", "function")?;
    attr(rt, "pathsep", "join", "join")?;
    own(rt, "scaffold_bin_dir", &["home"], "bin_dir")?;
    rt.call("provider_method", json!(["e", "get", ["PATH", ""], "path"]))?;
    rt.call("eval_list", json!(["paths", ["bin_dir", "path"]]))?;
    call(rt, "join", &["paths"], "path")?;
    field(rt, "e", "PATH", "path")?;
    attr(rt, "e", "update", "update")?;
    own(rt, "scaffold_exec_env", &["run_dir_path"], "extra")?;
    call(rt, "update", &["extra"], "updated")?;
    attr(rt, "e", "update", "update")?;
    copy(rt, "overrides", "env")?;
    fallback(rt, "overrides", json!({}))?;
    method(rt, "overrides", "items", &[], "items")?;
    rt.call("workspace_iter", json!(["items", "items_iter"]))?;
    literal(rt, "additions", json!({}))?;
    while truthy(&rt.call("workspace_next", json!(["items_iter", "item"]))?) {
        rt.call("workspace_pair", json!(["item", "key", "value"]))?;
        builtin(rt, "str")?;
        invoke(rt, &["value"], &[])?;
        rt.call("workspace_put", json!(["additions", "key", "called"]))?;
    }
    call(rt, "update", &["additions"], "updated")?;
    function(rt, "subprocess.run")?;
    literal(rt, "argv", json!(["bash", "-lc"]))?;
    rt.call("eval_append", json!(["argv", "script"]))?;
    literal(rt, "true", json!(true))?;
    invoke(
        rt,
        &["argv"],
        &[
            ("cwd", "workspace"),
            ("env", "e"),
            ("capture_output", "true"),
            ("text", "true"),
            ("timeout", "timeout"),
        ],
    )?;
    Ok(returned("called"))
}
fn handled_call(rt: &mut dyn Runtime, name: &str, args: &[&str], target: &str) -> Result<()> {
    rt.call("handler_function", json!([name]))?;
    rt.call("handler_invoke", json!([args, []]))?;
    copy(rt, target, "called")
}
fn place(rt: &mut dyn Runtime) -> Result<Value> {
    apply(rt, "os.path.lexists", &["dst"], "exists")?;
    if boolean(rt, "exists")? {
        apply(rt, "os.unlink", &["dst"], "removed")?;
    }
    match apply(rt, "os.link", &["src", "dst"], "linked") {
        Err(error) if catches(rt, &error, &["OSError"])? => {
            rt.call("handler_exception", json!([error]))?;
            handled_call(rt, "shutil.copy2", &["src", "dst"], "copied")?;
        }
        result => result?,
    }
    Ok(Value::Null)
}
fn mkdir(rt: &mut dyn Runtime, path: &str, handled: bool) -> Result<()> {
    if handled {
        rt.call("handler_function", json!(["os.makedirs"]))?;
    } else {
        function(rt, "os.makedirs")?;
    }
    literal(rt, "true", json!(true))?;
    if handled {
        rt.call("handler_invoke", json!([[path], [["exist_ok", "true"]]]))
            .map(|_| ())
    } else {
        invoke(rt, &[path], &[("exist_ok", "true")])
    }
}
fn formatted(rt: &mut dyn Runtime, slot: &str) -> Result<Text> {
    code_points(rt.call("handler_format", json!([slot]))?)
}
fn push(text: &mut Text, suffix: &str) {
    text.0.extend(suffix.chars().map(u32::from));
}
fn copy_in(rt: &mut dyn Runtime) -> Result<Value> {
    own(rt, "scaffold_bin_dir", &["home"], "bin_dir")?;
    let copied = (|| {
        function(rt, "os.makedirs")?;
        copy(rt, "makedirs", "function")?;
        apply(rt, "os.path.dirname", &["dest_path"], "parent")?;
        fallback(rt, "parent", json!("."))?;
        literal(rt, "true", json!(true))?;
        copy(rt, "function", "makedirs")?;
        invoke(rt, &["parent"], &[("exist_ok", "true")])?;
        call(rt, "place", &["host_path", "dest_path"], "placed")?;
        copy(rt, "dest", "dest_path")
    })();
    if let Err(error) = copied {
        if !catches(rt, &error, &["OSError"])? {
            return Err(error);
        }
        rt.call("handler_exception", json!([error]))?;
        rt.call("handler_function", json!(["os.path.join"]))?;
        copy(rt, "join", "function")?;
        handled_call(rt, "os.path.basename", &["dest_path"], "basename")?;
        copy(rt, "function", "join")?;
        rt.call("handler_invoke", json!([["bin_dir", "basename"], []]))?;
        copy(rt, "dest", "called")?;
        rt.call("task_caught_log", json!([error]))?;
        let mut message = Text::from("scaffold copy_in: ");
        message.0.extend(formatted(rt, "dest_path")?.0);
        push(&mut message, " not writable; using ");
        message.0.extend(formatted(rt, "dest")?.0);
        rt.call("logger_literal", json!(["message", message.0]))?;
        copy(rt, "function", "active_log")?;
        rt.call("handler_invoke", json!([["message"], []]))?;
        mkdir(rt, "bin_dir", true)?;
        copy(rt, "function", "place")?;
        rt.call("handler_invoke", json!([["host_path", "dest"], []]))?;
    }
    apply(rt, "os.chmod", &["dest", "mode"], "changed")?;
    function(rt, "subprocess.CompletedProcess")?;
    literal(rt, "empty", json!([]))?;
    literal(rt, "zero", json!(0))?;
    literal(rt, "text", json!(""))?;
    invoke(rt, &["empty", "zero", "text", "text"], &[])?;
    Ok(returned("called"))
}
fn setup(rt: &mut dyn Runtime) -> Result<Value> {
    member(rt, "scaffold", "scaffold")?;
    if is_none(rt, "scaffold")? {
        return Ok(json!({}));
    }
    function(rt, "os.makedirs")?;
    copy(rt, "makedirs", "function")?;
    own(rt, "scaffold_bin_dir", &["home"], "bin_dir")?;
    literal(rt, "true", json!(true))?;
    copy(rt, "function", "makedirs")?;
    invoke(rt, &["bin_dir"], &[("exist_ok", "true")])?;
    function(rt, "os.makedirs")?;
    copy(rt, "makedirs", "function")?;
    literal(rt, "claude", json!(".claude"))?;
    apply(rt, "os.path.join", &["home", "claude"], "claude_dir")?;
    copy(rt, "function", "makedirs")?;
    invoke(rt, &["claude_dir"], &[("exist_ok", "true")])?;
    own(rt, "prepare_agent_home", &["home"], "prepared")?;
    let installed = (|| {
        member(rt, "scaffold", "scaffold")?;
        attr(rt, "scaffold", "setup", "setup")?;
        own(
            rt,
            "_scaffold_context",
            &["workspace", "home", "task", "run_dir_path"],
            "context",
        )?;
        call(rt, "setup", &["context"], "notes")
    })();
    match installed {
        Ok(()) => Ok(returned("notes")),
        Err(error) if catches(rt, &error, &["Exception"])? => {
            rt.call("task_caught_log", json!([error]))?;
            rt.call("handler_item", json!(["task", "instance_id", "id"]))?;
            let mut message = formatted(rt, "id")?;
            push(&mut message, ": scaffold ");
            rt.call("handler_member", json!(["self", "scaffold", "scaffold"]))?;
            rt.call("handler_member", json!(["scaffold", "name", "name"]))?;
            message.0.extend(formatted(rt, "name")?.0);
            push(&mut message, " FAILED (");
            rt.call("handler_type", json!(["error", "error_type"]))?;
            message.0.extend(formatted(rt, "error_type")?.0);
            push(&mut message, ": ");
            message.0.extend(formatted(rt, "error")?.0);
            push(&mut message, ") -- task runs native, row flagged");
            rt.call("logger_literal", json!(["message", message.0]))?;
            copy(rt, "function", "active_log")?;
            rt.call("handler_invoke", json!([["message"], []]))?;
            literal(rt, "notes", json!({}))?;
            rt.call("handler_member", json!(["self", "scaffold", "scaffold"]))?;
            rt.call("handler_member", json!(["scaffold", "name", "name"]))?;
            field(rt, "notes", "arm_setup", "name")?;
            rt.call("handler_type", json!(["error", "error_type"]))?;
            let mut detail = formatted(rt, "error_type")?;
            push(&mut detail, ": ");
            detail.0.extend(formatted(rt, "error")?.0);
            detail.0.truncate(300);
            rt.call("logger_literal", json!(["detail", detail.0]))?;
            field(rt, "notes", "scaffold_error", "detail")?;
            Ok(returned("notes"))
        }
        Err(error) => Err(error),
    }
}
fn excludes(rt: &mut dyn Runtime) -> Result<Value> {
    member(rt, "scaffold", "scaffold")?;
    if is_none(rt, "scaffold")? {
        builtin(rt, "frozenset")?;
        invoke(rt, &[], &[])?;
        return Ok(returned("called"));
    }
    builtin(rt, "set")?;
    copy(rt, "set", "function")?;
    builtin(rt, "getattr")?;
    member(rt, "scaffold", "scaffold")?;
    literal(rt, "patch_attr", json!("patch_excludes"))?;
    tuple(rt, "empty", &[])?;
    invoke(rt, &["scaffold", "patch_attr", "empty"], &[])?;
    copy(rt, "patches", "called")?;
    if !boolean(rt, "patches")? {
        copy(rt, "patches", "empty")?;
    }
    call(rt, "set", &["patches"], "names")?;
    builtin(rt, "set")?;
    copy(rt, "set", "function")?;
    literal(rt, "placeholder", json!("/nonexistent"))?;
    own(rt, "scaffold_profile_for", &["placeholder"], "profile")?;
    attr(rt, "profile", "patch_excludes", "patches")?;
    call(rt, "set", &["patches"], "profile_names")?;
    binary(rt, "ior", "names", "profile_names", "names")?;
    builtin(rt, "frozenset")?;
    copy(rt, "frozenset", "function")?;
    rt.call(
        "eval_generator",
        json!(["kept", "names", "scaffold_name_element"]),
    )?;
    call(rt, "frozenset", &["kept"], "excluded")?;
    Ok(returned("excluded"))
}
pub fn dispatch(operation: &str, rt: &mut dyn Runtime) -> Result<Value> {
    match operation {
        "profile_claude" => profile(rt, "claude"),
        "profile_copilot" => profile(rt, "copilot"),
        "profile_codex" => profile(rt, "codex"),
        "mcp_claude" => {
            literal(rt, "flag", json!("--mcp-config"))?;
            rt.call("eval_list", json!(["args", ["flag", "cfg"]]))?;
            Ok(returned("args"))
        }
        "mcp_empty" => Ok(json!([])),
        "context" => context(rt),
        "host" => host(rt),
        "exec" => exec(rt),
        "place" => place(rt),
        "copy" => {
            literal(rt, "mode", json!(493))?;
            copy_in(rt)
        }
        "apply" => setup(rt),
        "excludes" => excludes(rt),
        "name_element" => {
            literal(rt, "star", json!("*"))?;
            if truthy(&rt.call("provider_contains", json!(["star", "element"]))?) {
                Ok(json!([]))
            } else {
                rt.call("eval_list", json!(["kept", ["element"]]))?;
                Ok(returned("kept"))
            }
        }
        _ => Err(Failure::new(
            "ValueError",
            format!("unknown Web-Bench scaffold helper: {operation}"),
        )),
    }
}
