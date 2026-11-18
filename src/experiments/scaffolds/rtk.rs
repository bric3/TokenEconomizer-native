use super::{
    Failure, Result, Runtime, Text, Value, apply, attr, binary, boolean, builtin, call, copy,
    fallback, field, formatted, function, invoke, json, literal, member, method, push, returned,
    text, truthy,
};

fn own(rt: &mut dyn Runtime, name: &str, args: &[&str], target: &str) -> Result<()> {
    member(rt, name, "function")?;
    invoke(rt, args, &[])?;
    copy(rt, target, "called")
}
fn profile(rt: &mut dyn Runtime, name: &str, target: &str) -> Result<()> {
    attr(rt, "ctx", "profile", "profile")?;
    attr(rt, "profile", name, target)
}
fn eq_zero(rt: &mut dyn Runtime, result: &str, target: &str) -> Result<()> {
    attr(rt, result, "returncode", "returncode")?;
    literal(rt, "zero", json!(0))?;
    rt.call("web_compare", json!(["eq", "returncode", "zero"]))?;
    copy(rt, target, "comparison")
}
fn tail(rt: &mut dyn Runtime, object: &str, name: &str, size: i32, target: &str) -> Result<()> {
    attr(rt, object, name, target)?;
    fallback(rt, target, json!(""))?;
    rt.call("logger_slice", json!([target, -size, null, target]))?;
    Ok(())
}
fn exec(rt: &mut dyn Runtime, target: &str, timeout: i32, env: bool) -> Result<()> {
    literal(rt, "timeout", json!(timeout))?;
    copy(rt, "function", "exec")?;
    invoke(
        rt,
        &["command"],
        if env {
            &[("timeout", "timeout"), ("env", "env")]
        } else {
            &[("timeout", "timeout")]
        },
    )?;
    copy(rt, target, "called")
}
fn log(rt: &mut dyn Runtime, message: Text) -> Result<()> {
    text(rt, "message", message)?;
    call(rt, "logger", &["message"], "logged")
}
fn home_env(rt: &mut dyn Runtime) -> Result<()> {
    literal(rt, "env", json!({}))?;
    profile(rt, "home", "home")?;
    field(rt, "env", "HOME", "home")
}
fn compare_kind(rt: &mut dyn Runtime, kind: &str) -> Result<bool> {
    profile(rt, "kind", "kind")?;
    literal(rt, "expected_kind", json!(kind))?;
    rt.call("web_compare", json!(["eq", "kind", "expected_kind"]))?;
    boolean(rt, "comparison")
}
fn cache(rt: &mut dyn Runtime) -> Result<Value> {
    for (name, target) in [("TE_SCAFFOLD_CACHE", "explicit"), ("DATA", "data")] {
        function(rt, "os.environ.get")?;
        literal(rt, "key", json!(name))?;
        invoke(rt, &["key"], &[])?;
        copy(rt, target, "called")?;
        if boolean(rt, target)? {
            if target == "explicit" {
                return Ok(returned(target));
            }
            function(rt, "os.path.join")?;
            literal(rt, "suffix", json!("scaffolds"))?;
            invoke(rt, &["data", "suffix"], &[])?;
            return Ok(returned("called"));
        }
    }
    function(rt, "ScaffoldError")?;
    literal(
        rt,
        "message",
        json!(
            "no scaffold cache: set TE_SCAFFOLD_CACHE (or DATA) so the rtk binary can be staged on the host -- the sealed task container cannot reach GitHub. See the repo's .env.example"
        ),
    )?;
    invoke(rt, &["message"], &[])?;
    rt.call("task_raise", json!(["called"]))
}
fn bin_path(rt: &mut dyn Runtime) -> Result<Value> {
    function(rt, "os.path.join")?;
    copy(rt, "join", "function")?;
    apply(rt, "cache_dir", &[], "cache")?;
    literal(rt, "bin", json!("bin"))?;
    literal(rt, "rtk", json!("rtk"))?;
    call(rt, "join", &["cache", "bin", "rtk"], "dest")?;
    Ok(returned("dest"))
}
fn hook_binary(rt: &mut dyn Runtime) -> Result<Value> {
    attr(rt, "profile", "bin_dir", "bin_dir")?;
    let mut path = formatted(rt, "bin_dir", "")?;
    push(&mut path, "/rtk");
    text(rt, "hook_bin", path)?;
    Ok(returned("hook_bin"))
}
fn hook_config(rt: &mut dyn Runtime, copilot: bool) -> Result<Value> {
    attr(rt, "cls", "_hook_binary", "hook_binary")?;
    call(rt, "hook_binary", &["profile"], "hook_bin")?;
    let mut command = formatted(rt, "hook_bin", "")?;
    push(
        &mut command,
        if copilot {
            " hook copilot"
        } else {
            " hook claude"
        },
    );
    text(rt, "command", command)?;
    literal(rt, "hook", json!({"type":"command"}))?;
    field(rt, "hook", "command", "command")?;
    if copilot {
        literal(rt, "cwd", json!("."))?;
        field(rt, "hook", "cwd", "cwd")?;
        literal(rt, "timeout", json!(10))?;
        field(rt, "hook", "timeout", "timeout")?;
    }
    rt.call("eval_list", json!(["hooks", ["hook"]]))?;
    if !copilot {
        literal(rt, "matcher", json!({"matcher":"Bash"}))?;
        field(rt, "matcher", "hooks", "hooks")?;
        rt.call("eval_list", json!(["hooks", ["matcher"]]))?;
    }
    literal(rt, "events", json!({}))?;
    field(rt, "events", "PreToolUse", "hooks")?;
    literal(
        rt,
        "config",
        if copilot {
            json!({"version":1})
        } else {
            json!({})
        },
    )?;
    field(rt, "config", "hooks", "events")?;
    Ok(returned("config"))
}
fn access(rt: &mut dyn Runtime, target: &str) -> Result<()> {
    function(rt, "os.access")?;
    copy(rt, "access", "function")?;
    function(rt, "os.X_OK")?;
    copy(rt, "mode", "function")?;
    call(rt, "access", &["dest", "mode"], target)
}
fn stage(rt: &mut dyn Runtime) -> Result<Value> {
    own(rt, "bin_path", &[], "dest")?;
    apply(rt, "os.path.exists", &["dest"], "exists")?;
    if boolean(rt, "exists")? {
        access(rt, "accessible")?;
        if boolean(rt, "accessible")? {
            return Ok(json!(true));
        }
    }
    function(rt, "os.makedirs")?;
    copy(rt, "makedirs", "function")?;
    apply(rt, "os.path.dirname", &["dest"], "parent")?;
    literal(rt, "yes", json!(true))?;
    copy(rt, "function", "makedirs")?;
    invoke(rt, &["parent"], &[("exist_ok", "yes")])?;
    literal(rt, "suffix", json!(".tar.gz"))?;
    binary(rt, "add", "dest", "suffix", "tgz")?;
    attr(rt, "ctx", "host_run", "host")?;
    let mut command = Text::from("curl -fsSL -o ");
    command.0.extend(formatted(rt, "tgz", "")?.0);
    push(&mut command, " ");
    function(rt, "RTK_URL")?;
    command.0.extend(formatted(rt, "function", "")?.0);
    push(&mut command, " && tar xzf ");
    command.0.extend(formatted(rt, "tgz", "")?.0);
    push(&mut command, " -C ");
    apply(rt, "os.path.dirname", &["dest"], "parent")?;
    command.0.extend(formatted(rt, "parent", "")?.0);
    push(&mut command, " && chmod +x ");
    command.0.extend(formatted(rt, "dest", "")?.0);
    push(&mut command, " && rm -f ");
    command.0.extend(formatted(rt, "tgz", "")?.0);
    text(rt, "command", command)?;
    literal(rt, "bash", json!("bash"))?;
    literal(rt, "lc", json!("-lc"))?;
    rt.call("eval_list", json!(["args", ["bash", "lc", "command"]]))?;
    literal(rt, "timeout", json!(300))?;
    copy(rt, "function", "host")?;
    invoke(rt, &["args"], &[("timeout", "timeout")])?;
    copy(rt, "res", "called")?;
    eq_zero(rt, "res", "ok")?;
    if boolean(rt, "ok")? {
        access(rt, "ok")?;
    }
    if !boolean(rt, "ok")? {
        attr(rt, "ctx", "log", "logger")?;
        tail(rt, "res", "stderr", 200, "stderr")?;
        let mut message = Text::from("rtk: host staging failed (");
        message.0.extend(formatted(rt, "stderr", "")?.0);
        push(&mut message, ")");
        log(rt, message)?;
    }
    Ok(returned("ok"))
}
fn copilot_line(rt: &mut dyn Runtime) -> Result<()> {
    literal(rt, "marker", json!("RTK_HOOK_AT="))?;
    method(rt, "element", "startswith", &["marker"], "matches")?;
    if boolean(rt, "matches")? {
        literal(rt, "equals", json!("="))?;
        literal(rt, "one", json!(1))?;
        method(rt, "element", "split", &["equals", "one"], "parts")?;
        rt.call("stream_item", json!(["parts", 1, "path"]))?;
        rt.call("eval_append", json!(["at", "path"]))?;
    }
    Ok(())
}
fn copilot(rt: &mut dyn Runtime) -> Result<Value> {
    function(rt, "json.dumps")?;
    copy(rt, "dumps", "function")?;
    member(rt, "_copilot_hook_config", "config")?;
    attr(rt, "ctx", "profile", "profile")?;
    call(rt, "config", &["profile"], "config")?;
    literal(rt, "two", json!(2))?;
    copy(rt, "function", "dumps")?;
    invoke(rt, &["config"], &[("indent", "two")])?;
    copy(rt, "cfg", "called")?;
    member(rt, "COPILOT_HOOK_FILE", "name")?;
    let mut command = Text::from(
        "H=\"${COPILOT_HOME:-$HOME/.copilot}\"; mkdir -p \"$H/hooks\" && cat > \"$H/hooks/",
    );
    command.0.extend(formatted(rt, "name", "")?.0);
    push(&mut command, "\" <<'TE_EOF'\n");
    command.0.extend(formatted(rt, "cfg", "")?.0);
    push(&mut command, "\nTE_EOF\ntest -s \"$H/hooks/");
    command.0.extend(formatted(rt, "name", "")?.0);
    push(&mut command, "\" && echo \"RTK_HOOK_AT=$H/hooks/");
    command.0.extend(formatted(rt, "name", "")?.0);
    push(&mut command, "\"");
    text(rt, "command", command)?;
    attr(rt, "ctx", "exec_", "exec")?;
    home_env(rt)?;
    exec(rt, "res", 60, true)?;
    attr(rt, "res", "stdout", "stdout")?;
    fallback(rt, "stdout", json!(""))?;
    method(rt, "stdout", "splitlines", &[], "lines")?;
    literal(rt, "at", json!([]))?;
    rt.call("workspace_iter", json!(["lines", "iterator"]))?;
    while truthy(&rt.call("workspace_next", json!(["iterator", "element"]))?) {
        copilot_line(rt)?;
    }
    literal(rt, "notes", json!({}))?;
    builtin(rt, "bool")?;
    invoke(rt, &["at"], &[])?;
    field(rt, "notes", "rtk_hook_registered", "called")?;
    literal(rt, "channel", json!("copilot-hooks-dir"))?;
    field(rt, "notes", "rtk_hook_channel", "channel")?;
    if boolean(rt, "at")? {
        rt.call("stream_item", json!(["at", 0, "first"]))?;
    } else {
        literal(rt, "first", Value::Null)?;
    }
    field(rt, "notes", "rtk_hook_path", "first")?;
    attr(rt, "res", "stderr", "stderr")?;
    fallback(rt, "stderr", json!(""))?;
    attr(rt, "res", "stdout", "stdout")?;
    fallback(rt, "stdout", json!(""))?;
    binary(rt, "add", "stderr", "stdout", "log")?;
    rt.call("logger_slice", json!(["log", -200, null, "log"]))?;
    field(rt, "notes", "rtk_hook_log", "log")?;
    Ok(returned("notes"))
}
fn hook_probe(rt: &mut dyn Runtime) -> Result<Value> {
    attr(rt, "ctx", "exec_", "exec")?;
    function(rt, "shlex.quote")?;
    copy(rt, "quote", "function")?;
    member(rt, "_PROBE_PAYLOAD", "payload")?;
    call(rt, "quote", &["payload"], "quoted")?;
    let mut command = Text::from("printf '%s' ");
    command.0.extend(formatted(rt, "quoted", "")?.0);
    push(&mut command, " | ");
    member(rt, "_hook_binary", "hook_binary")?;
    attr(rt, "ctx", "profile", "profile")?;
    call(rt, "hook_binary", &["profile"], "hook_bin")?;
    command.0.extend(formatted(rt, "hook_bin", "")?.0);
    push(&mut command, " hook ");
    command.0.extend(formatted(rt, "agent", "")?.0);
    text(rt, "command", command)?;
    exec(rt, "res", 60, false)?;
    attr(rt, "res", "stdout", "stdout")?;
    fallback(rt, "stdout", json!(""))?;
    literal(rt, "marker", json!("rtk ls"))?;
    Ok(json!(truthy(
        &rt.call("provider_contains", json!(["marker", "stdout"]))?
    )))
}
fn codex(rt: &mut dyn Runtime) -> Result<Value> {
    builtin(rt, "dict")?;
    copy(rt, "dict", "function")?;
    profile(rt, "env", "profile_env")?;
    call(rt, "dict", &["profile_env"], "env")?;
    literal(rt, "codex_home", json!("CODEX_HOME"))?;
    method(rt, "env", "get", &["codex_home"], "home")?;
    if !boolean(rt, "home")? {
        function(rt, "ScaffoldError")?;
        literal(
            rt,
            "message",
            json!(
                "the Codex profile carries no CODEX_HOME in its env: rtk would write its instructions into a directory the CLI never reads"
            ),
        )?;
        invoke(rt, &["message"], &[])?;
        return rt.call("task_raise", json!(["called"]));
    }
    attr(rt, "env", "setdefault", "setdefault")?;
    literal(rt, "home_key", json!("HOME"))?;
    profile(rt, "home", "profile_home")?;
    call(rt, "setdefault", &["home_key", "profile_home"], "defaulted")?;
    attr(rt, "ctx", "exec_", "exec")?;
    literal(rt, "command", json!("rtk init -g --codex"))?;
    exec(rt, "init", 60, true)?;
    literal(rt, "notes", json!({"rtk_mode":"instructions"}))?;
    attr(rt, "init", "returncode", "init_rc")?;
    field(rt, "notes", "rtk_init_rc", "init_rc")?;
    literal(rt, "null", Value::Null)?;
    field(rt, "notes", "rtk_hook_registered", "null")?;
    member(rt, "RTK_RULE", "rule")?;
    attr(rt, "ctx", "exec_", "exec")?;
    let mut command = Text::from("H=");
    command.0.extend(formatted(rt, "home", "")?.0);
    push(
        &mut command,
        "; test -f $H/RTK.md && test -f $H/AGENTS.md && { grep -v -x -F \"@$H/RTK.md\" $H/AGENTS.md; printf '\\n'; cat $H/RTK.md; } > $H/AGENTS.md.tmp && mv $H/AGENTS.md.tmp $H/AGENTS.md; grep -c '",
    );
    command.0.extend(formatted(rt, "rule", "")?.0);
    push(&mut command, "' $H/AGENTS.md");
    text(rt, "command", command)?;
    exec(rt, "inline", 60, true)?;
    let parsed = (|| {
        builtin(rt, "int")?;
        copy(rt, "integer", "function")?;
        attr(rt, "inline", "stdout", "stdout")?;
        fallback(rt, "stdout", json!("0"))?;
        method(rt, "stdout", "strip", &[], "stripped")?;
        method(rt, "stripped", "splitlines", &[], "lines")?;
        rt.call("stream_item", json!(["lines", -1, "last"]))?;
        call(rt, "integer", &["last"], "present")
    })();
    if let Err(error) = parsed {
        if !truthy(&rt.call(
            "exception_matches",
            json!([error, ["ValueError", "IndexError"]]),
        )?) {
            return Err(error);
        }
        literal(rt, "present", json!(0))?;
    }
    eq_zero(rt, "init", "written")?;
    if boolean(rt, "written")? {
        rt.call("web_compare", json!(["gt", "present", "zero"]))?;
        copy(rt, "written", "comparison")?;
    }
    field(rt, "notes", "rtk_instructions_written", "written")?;
    rt.call("web_compare", json!(["gt", "present", "zero"]))?;
    field(rt, "notes", "rtk_agents_inlined", "comparison")?;
    attr(rt, "ctx", "exec_", "exec")?;
    member(rt, "_hook_binary", "hook_binary")?;
    attr(rt, "ctx", "profile", "profile")?;
    call(rt, "hook_binary", &["profile"], "hook_bin")?;
    let mut command = Text::from("env PATH=/usr/bin:/bin ");
    command.0.extend(formatted(rt, "hook_bin", "")?.0);
    push(&mut command, " --version");
    text(rt, "command", command)?;
    exec(rt, "probe", 60, false)?;
    eq_zero(rt, "probe", "exec_ok")?;
    field(rt, "notes", "rtk_hook_exec_ok", "exec_ok")?;
    rt.call(
        "stream_item",
        json!(["notes", "rtk_instructions_written", "written"]),
    )?;
    if !boolean(rt, "written")? {
        attr(rt, "ctx", "log", "logger")?;
        let mut message = Text::from("rtk: WARNING binary present but the rtk rule did NOT reach ");
        message.0.extend(formatted(rt, "home", "")?.0);
        push(
            &mut message,
            "/AGENTS.md -- this arm would run as native. (init rc=",
        );
        attr(rt, "init", "returncode", "init_rc")?;
        message.0.extend(formatted(rt, "init_rc", "")?.0);
        push(&mut message, ", inline: ");
        tail(rt, "inline", "stderr", 160, "stderr")?;
        message.0.extend(formatted(rt, "stderr", "")?.0);
        push(&mut message, ")");
        log(rt, message)?;
    }
    Ok(returned("notes"))
}
fn notes_update(rt: &mut dyn Runtime, name: &str) -> Result<()> {
    attr(rt, "notes", "update", "update")?;
    own(rt, name, &["ctx"], "extra")?;
    call(rt, "update", &["extra"], "updated")
}
fn setup(rt: &mut dyn Runtime) -> Result<Value> {
    literal(rt, "notes", json!({"arm_setup":"rtk"}))?;
    function(rt, "RTK_VERSION")?;
    field(rt, "notes", "rtk_pinned_version", "function")?;
    own(rt, "stage_host_binary", &["ctx"], "staged")?;
    builtin(rt, "bool")?;
    invoke(rt, &["staged"], &[])?;
    field(rt, "notes", "rtk_staged_host", "called")?;
    if boolean(rt, "staged")? {
        profile(rt, "bin_dir", "bin_dir")?;
        let mut dest = formatted(rt, "bin_dir", "")?;
        push(&mut dest, "/rtk");
        text(rt, "dest", dest)?;
        attr(rt, "ctx", "copy_in", "copy_in")?;
        own(rt, "bin_path", &[], "bin_path")?;
        call(rt, "copy_in", &["bin_path", "dest"], "copied")?;
        attr(rt, "ctx", "exec_", "exec")?;
        let mut command = Text::from("chmod +x ");
        command.0.extend(formatted(rt, "dest", "")?.0);
        push(&mut command, " && rtk --version");
        text(rt, "command", command)?;
        exec(rt, "ver", 60, false)?;
    } else {
        attr(rt, "ctx", "exec_", "exec")?;
        function(rt, "RTK_INSTALL_SH")?;
        let mut command = Text::from("(");
        command.0.extend(formatted(rt, "function", "")?.0);
        push(&mut command, ") && (cp ~/.local/bin/rtk ");
        profile(rt, "bin_dir", "bin_dir")?;
        command.0.extend(formatted(rt, "bin_dir", "")?.0);
        push(&mut command, "/ 2>/dev/null || cp /root/.cargo/bin/rtk ");
        profile(rt, "bin_dir", "bin_dir")?;
        command.0.extend(formatted(rt, "bin_dir", "")?.0);
        push(&mut command, "/ 2>/dev/null || true) && rtk --version");
        text(rt, "command", command)?;
        exec(rt, "ver", 300, false)?;
    }
    eq_zero(rt, "ver", "installed")?;
    field(rt, "notes", "rtk_installed", "installed")?;
    attr(rt, "ver", "stdout", "stdout")?;
    fallback(rt, "stdout", json!(""))?;
    method(rt, "stdout", "strip", &[], "version")?;
    rt.call("logger_slice", json!(["version", -40, null, "version"]))?;
    field(rt, "notes", "rtk_version", "version")?;
    if compare_kind(rt, "codex")? {
        notes_update(rt, "_codex_instructions")?;
        return Ok(returned("notes"));
    }
    attr(rt, "ctx", "exec_", "exec")?;
    literal(rt, "command", json!("rtk init -g"))?;
    home_env(rt)?;
    exec(rt, "init", 60, true)?;
    attr(rt, "init", "returncode", "init_rc")?;
    field(rt, "notes", "rtk_init_rc", "init_rc")?;
    member(rt, "_hook_binary", "hook_binary")?;
    attr(rt, "ctx", "profile", "profile")?;
    call(rt, "hook_binary", &["profile"], "hook_bin")?;
    literal(rt, "mode", json!("hook"))?;
    field(rt, "notes", "rtk_mode", "mode")?;
    field(rt, "notes", "rtk_hook_cmd", "hook_bin")?;
    if compare_kind(rt, "claude")? {
        attr(rt, "ctx", "merge_json_file", "merge")?;
        profile(rt, "home", "home")?;
        let mut path = formatted(rt, "home", "")?;
        push(&mut path, "/.claude/settings.json");
        text(rt, "path", path)?;
        member(rt, "_claude_hook", "config")?;
        attr(rt, "ctx", "profile", "profile")?;
        call(rt, "config", &["profile"], "config")?;
        call(rt, "merge", &["path", "config"], "registered")?;
        field(rt, "notes", "rtk_hook_registered", "registered")?;
    } else {
        notes_update(rt, "_install_copilot_hook")?;
    }
    attr(rt, "ctx", "exec_", "exec")?;
    let mut command = Text::from("env PATH=/usr/bin:/bin ");
    command.0.extend(formatted(rt, "hook_bin", "")?.0);
    push(&mut command, " --version");
    text(rt, "command", command)?;
    exec(rt, "probe", 60, false)?;
    eq_zero(rt, "probe", "exec_ok")?;
    field(rt, "notes", "rtk_hook_exec_ok", "exec_ok")?;
    member(rt, "_probe_hook_rewrites", "rewrite")?;
    profile(rt, "kind", "agent")?;
    call(rt, "rewrite", &["ctx", "agent"], "rewrites")?;
    field(rt, "notes", "rtk_hook_rewrites", "rewrites")?;
    literal(rt, "key", json!("rtk_hook_registered"))?;
    method(rt, "notes", "get", &["key"], "registered")?;
    let warn_rewrite = if boolean(rt, "registered")? {
        rt.call(
            "stream_item",
            json!(["notes", "rtk_hook_rewrites", "rewrites"]),
        )?;
        !boolean(rt, "rewrites")?
    } else {
        false
    };
    if warn_rewrite {
        attr(rt, "ctx", "log", "logger")?;
        log(
            rt,
            Text::from(
                "rtk: WARNING the hook is registered but does NOT rewrite this payload shape -- the arm would run as native with extra steps. Check the channel against baselines/rtk._install_copilot_hook.",
            ),
        )?;
    }
    method(rt, "notes", "get", &["key"], "registered")?;
    let warn_exec = if boolean(rt, "registered")? {
        rt.call(
            "stream_item",
            json!(["notes", "rtk_hook_exec_ok", "exec_ok"]),
        )?;
        !boolean(rt, "exec_ok")?
    } else {
        false
    };
    if warn_exec {
        attr(rt, "ctx", "log", "logger")?;
        let mut message = Text::from("rtk: WARNING hook registered but ");
        message.0.extend(formatted(rt, "hook_bin", "")?.0);
        push(
            &mut message,
            " is not executable by the agent's hook subprocess -- every bash call would raise a hook error and the arm would cost far MORE than native, not less.",
        );
        log(rt, message)?;
    }
    rt.call(
        "stream_item",
        json!(["notes", "rtk_installed", "installed"]),
    )?;
    if boolean(rt, "installed")? {
        method(rt, "notes", "get", &["key"], "registered")?;
        if !boolean(rt, "registered")? {
            attr(rt, "ctx", "log", "logger")?;
            log(
                rt,
                Text::from(
                    "rtk: WARNING binary present but hook NOT registered -- this arm would run as native. Check the agent's settings/plugin path.",
                ),
            )?;
        }
    }
    Ok(returned("notes"))
}
pub(super) fn dispatch(operation: &str, rt: &mut dyn Runtime) -> Result<Value> {
    match operation {
        "cache" => cache(rt),
        "bin_path" => bin_path(rt),
        "preflight" => {
            apply(rt, "cache_dir", &[], "cache")?;
            Ok(Value::Null)
        }
        "hook_binary" => hook_binary(rt),
        "claude_hook" => hook_config(rt, false),
        "copilot_config" => hook_config(rt, true),
        "stage" => stage(rt),
        "copilot" => copilot(rt),
        "probe" => hook_probe(rt),
        "codex" => codex(rt),
        "setup" => setup(rt),
        _ => Err(Failure::new(
            "ValueError",
            format!("unknown RTK scaffold operation: {operation}"),
        )),
    }
}
