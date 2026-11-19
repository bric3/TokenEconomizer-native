use super::{
    Failure, Result, Runtime,
    optimizer::code_points,
    providers::{boolean, copy, literal},
    runner_calls::{function, invoke, member},
};
use crate::{
    python_json::Text,
    value::{python_string_repr, truthy},
};
use serde_json::{Value, json};

const LAUNCHER: &str =
    "# te-net-guard launcher v1\nimport json, shlex, subprocess, sys\nentry = json.loads(";

fn append(text: &mut Text, suffix: &str) {
    text.0.extend(suffix.chars().map(u32::from));
}
fn text(rt: &mut dyn Runtime, slot: &str, value: Text) -> Result<()> {
    rt.call("logger_literal", json!([slot, value.0]))
        .map(|_| ())
}
fn formatted(rt: &mut dyn Runtime, slot: &str) -> Result<Text> {
    code_points(rt.call("task_format", json!([slot, ""]))?)
}
fn attr(rt: &mut dyn Runtime, object: &str, name: &str, target: &str) -> Result<()> {
    rt.call("stdout_member", json!([object, name, target]))
        .map(|_| ())
}
fn call(rt: &mut dyn Runtime, function_slot: &str, args: &[&str], target: &str) -> Result<()> {
    copy(rt, "function", function_slot)?;
    invoke(rt, args, &[])?;
    copy(rt, target, "called")
}
fn field(rt: &mut dyn Runtime, object: &str, key: &str, slot: &str) -> Result<()> {
    rt.call("eval_field", json!([object, key, slot]))
        .map(|_| ())
}
fn own(rt: &mut dyn Runtime, name: &str, args: &[&str], target: &str) -> Result<()> {
    member(rt, name, "function")?;
    invoke(rt, args, &[])?;
    copy(rt, target, "called")
}
fn returned(slot: &str) -> Value {
    json!({"provider_ref":slot})
}
fn fallback(rt: &mut dyn Runtime, slot: &str) -> Result<()> {
    if !boolean(rt, slot)? {
        literal(rt, slot, json!(""))?;
    }
    Ok(())
}
fn home(rt: &mut dyn Runtime, suffix: &str, target: &str) -> Result<()> {
    member(rt, "scaffold_profile", "profile")?;
    attr(rt, "profile", "home", "home")?;
    let mut path = formatted(rt, "home")?;
    append(&mut path, suffix);
    text(rt, target, path)
}
fn program(rt: &mut dyn Runtime, event: &str) -> Result<()> {
    // Resolve the outer encoder before the inner call can replace the module attribute.
    function(rt, "json.dumps")?;
    copy(rt, "outer_dumps", "function")?;
    function(rt, "json.dumps")?;
    invoke(rt, &["entry"], &[])?;
    copy(rt, "encoded", "called")?;
    call(rt, "outer_dumps", &["encoded"], "encoded_literal")?;
    let mut result = Text::from(LAUNCHER);
    result.0.extend(formatted(rt, "encoded_literal")?.0);
    append(&mut result, ")\nhelper = ");
    let helper = std::env::current_exe()?
        .with_file_name(format!("te-net-guard{}", std::env::consts::EXE_SUFFIX));
    append(
        &mut result,
        &python_string_repr(helper.to_string_lossy().chars().map(u32::from)),
    );
    append(
        &mut result,
        "\npath = sys.argv[1]\nif len(sys.argv) > 3 and path == '--native-helper':\n    helper, path = sys.argv[2:4]\n    args = [helper, '--install-hook', ",
    );
    let agent = if event == "preToolUse" {
        "copilot"
    } else {
        "claude"
    };
    append(
        &mut result,
        &python_string_repr(agent.chars().map(u32::from)),
    );
    append(
        &mut result,
        ", path, shlex.quote(helper)]\nelse:\n    args = [helper, '--install-entry', ",
    );
    append(
        &mut result,
        &python_string_repr(event.chars().map(u32::from)),
    );
    append(
        &mut result,
        ", path]\nraise SystemExit(subprocess.run(args, input=json.dumps(entry).encode()).returncode)\n",
    );
    text(rt, "program", result)
}
fn entry(rt: &mut dyn Runtime, copilot: bool) -> Result<Text> {
    let mut command = formatted(rt, "python")?;
    append(&mut command, " ");
    member(rt, "NET_GUARD_SCRIPT", "script")?;
    let script = formatted(rt, "script")?;
    command.0.extend(&script.0);
    text(rt, "command", command)?;
    literal(
        rt,
        "entry",
        json!({"matcher":if copilot {"bash"} else {"Bash"}}),
    )?;
    if copilot {
        literal(rt, "command_type", json!("command"))?;
        field(rt, "entry", "type", "command_type")?;
        field(rt, "entry", "command", "command")?;
    } else {
        literal(rt, "hook", json!({"type":"command"}))?;
        field(rt, "hook", "command", "command")?;
        rt.call("eval_list", json!(["hooks", ["hook"]]))?;
        field(rt, "entry", "hooks", "hooks")?;
    }
    Ok(script)
}
fn quote(value: &Text) -> Text {
    if !value.0.is_empty()
        && value.0.iter().all(|point| {
            u8::try_from(*point)
                .is_ok_and(|byte| byte.is_ascii_alphanumeric() || b"_@%+=:,./-".contains(&byte))
        })
    {
        return value.clone();
    }
    let mut result = Text::from("'");
    for point in &value.0 {
        if *point == 39 {
            append(&mut result, "'\"'\"'");
        } else {
            result.0.push(*point);
        }
    }
    append(&mut result, "'");
    result
}
fn execute(rt: &mut dyn Runtime, helper: Text) -> Result<Value> {
    member(rt, "_exec", "exec")?;
    let program = formatted(rt, "program")?;
    let native = program
        .0
        .starts_with(&LAUNCHER.chars().map(u32::from).collect::<Vec<_>>());
    let mut script = Text::from("cat > /tmp/te_net_guard_install.py <<'TE_EOF'\n");
    script.0.extend(program.0);
    append(&mut script, "\nTE_EOF\n");
    script.0.extend(formatted(rt, "python")?.0);
    append(&mut script, " /tmp/te_net_guard_install.py ");
    // Custom merge programs keep the original argv contract.
    if native {
        append(&mut script, "--native-helper ");
        script.0.extend(quote(&helper).0);
        append(&mut script, " ");
    }
    script.0.extend(formatted(rt, "path")?.0);
    text(rt, "install_script", script)?;
    literal(rt, "timeout", json!(60))?;
    copy(rt, "function", "exec")?;
    invoke(rt, &["name", "install_script"], &[("timeout", "timeout")])?;
    copy(rt, "result", "called")?;
    attr(rt, "result", "returncode", "rc")?;
    literal(rt, "zero", json!(0))?;
    rt.call("web_compare", json!(["ne", "rc", "zero"]))?;
    let mut failed = boolean(rt, "comparison")?;
    if !failed {
        attr(rt, "result", "stdout", "stdout")?;
        fallback(rt, "stdout")?;
        literal(rt, "armed", json!("armed"))?;
        failed = !truthy(&rt.call("provider_contains", json!(["armed", "stdout"]))?);
    }
    if failed {
        attr(rt, "result", "stderr", "stderr")?;
        fallback(rt, "stderr")?;
        attr(rt, "result", "stdout", "stdout")?;
        fallback(rt, "stdout")?;
        rt.call("web_binary", json!(["add", "stderr", "stdout", "message"]))?;
        rt.call("logger_slice", json!(["message", -200, null, "message"]))?;
        rt.call("eval_builtin", json!(["RuntimeError"]))?;
        invoke(rt, &["message"], &[])?;
        rt.call("task_raise", json!(["called"]))?;
    }
    Ok(json!(true))
}
pub fn run(operation: &str, rt: &mut dyn Runtime) -> Result<Value> {
    match operation {
        "script" | "log" => {
            home(
                rt,
                if operation == "script" {
                    "/net_guard.py"
                } else {
                    "/net_guard_blocks.jsonl"
                },
                "path",
            )?;
            Ok(returned("path"))
        }
        "attempts" => {
            function(rt, "answer_key_attempts")?;
            invoke(rt, &["run_dir_path"], &[])?;
            Ok(returned("called"))
        }
        "program" => {
            program(rt, "PreToolUse")?;
            Ok(returned("program"))
        }
        "write" => {
            let helper = entry(rt, false)?;
            own(rt, "_net_guard_merge_program", &["entry"], "program")?;
            execute(rt, helper)
        }
        "register_copilot" => {
            let helper = entry(rt, true)?;
            home(rt, "/.copilot/settings.json", "path")?;
            program(rt, "preToolUse")?;
            execute(rt, helper)
        }
        "register_claude" | "register_codex" => {
            member(rt, "_write_hook_entry", "writer")?;
            if operation == "register_claude" {
                home(rt, "/.claude/settings.json", "path")?;
            } else {
                function(rt, "codex.C_HOME")?;
                let mut path = formatted(rt, "function")?;
                append(&mut path, "/hooks.json");
                text(rt, "path", path)?;
            }
            call(rt, "writer", &["name", "python", "path"], "written")?;
            if operation == "register_codex" {
                rt.call("stream_set", json!(["_guard_python", "python"]))?;
                Ok(json!(true))
            } else {
                Ok(returned("written"))
            }
        }
        _ => Err(Failure::new(
            "ValueError",
            format!("unknown guard convenience operation: {operation}"),
        )),
    }
}
