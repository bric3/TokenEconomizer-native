use super::{
    Failure, Result, Runtime, Text, Value, apply, attr, binary, boolean, builtin, call, copy,
    fallback, field, formatted, function, invoke, json, literal, member, method, push, returned,
    text, truthy,
};
use crate::experiments::swebench::optimizer::code_points;
use regex::Regex;
use std::{collections::BTreeSet, sync::LazyLock};

fn matches(rt: &mut dyn Runtime, error: &Failure, names: &[&str]) -> Result<bool> {
    rt.call("exception_matches", json!([error, names]))
        .map(|v| truthy(&v))
}
fn caught(rt: &mut dyn Runtime, error: Option<&Failure>) -> Result<()> {
    rt.call("handler_exception", json!([error])).map(|_| ())
}
fn hfunction(rt: &mut dyn Runtime, name: &str) -> Result<()> {
    rt.call("handler_function", json!([name])).map(|_| ())
}
fn hcall(
    rt: &mut dyn Runtime,
    callable: &str,
    args: &[&str],
    kwargs: &[(&str, &str)],
    target: &str,
) -> Result<()> {
    copy(rt, "function", callable)?;
    rt.call("handler_invoke", json!([args, kwargs]))?;
    copy(rt, target, "called")
}
fn hformatted(rt: &mut dyn Runtime, slot: &str) -> Result<Text> {
    code_points(rt.call("handler_format", json!([slot]))?)
}
fn own(rt: &mut dyn Runtime, name: &str, args: &[&str], target: &str) -> Result<()> {
    member(rt, name, "function")?;
    invoke(rt, args, &[])?;
    copy(rt, target, "called")
}
fn tokens(rt: &mut dyn Runtime) -> Result<Value> {
    builtin(rt, "int")?;
    copy(rt, "integer", "function")?;
    function(rt, "os.environ.get")?;
    copy(rt, "get", "function")?;
    literal(rt, "key", json!("TE_REPOMAP_TOKENS"))?;
    function(rt, "DEFAULT_MAP_TOKENS")?;
    copy(rt, "default", "function")?;
    call(rt, "get", &["key", "default"], "budget")?;
    call(rt, "integer", &["budget"], "budget")?;
    Ok(returned("budget"))
}
fn word(rt: &mut dyn Runtime, value: Option<char>) -> Result<bool> {
    match value {
        None => Ok(false),
        Some(c) if c.is_ascii() => Ok(c.is_ascii_alphanumeric() || c == '_'),
        // The SDK supplies the pinned interpreter's Unicode alphanumeric property.
        Some(c) => rt
            .call("scaffold_alnum", json!([u32::from(c)]))
            .map(|v| truthy(&v)),
    }
}
fn idents(rt: &mut dyn Runtime) -> Result<Value> {
    static PATTERNS: LazyLock<[Regex; 3]> = LazyLock::new(|| {
        [
            Regex::new(r"`([A-Za-z_][A-Za-z0-9_.]*)`").unwrap(),
            Regex::new(r"([a-z_][a-z0-9_]*_[a-z0-9_]+)").unwrap(),
            Regex::new(r"([A-Z][a-zA-Z0-9]+[A-Z][a-zA-Z0-9]*)").unwrap(),
        ]
    });
    let mut ids = BTreeSet::new();
    for (index, pattern) in PATTERNS.iter().enumerate() {
        copy(rt, "candidate", "text")?;
        fallback(rt, "candidate", json!(""))?;
        let points = code_points(rt.call("scaffold_text", json!(["candidate"]))?)?;
        let input: String = points
            .0
            .iter()
            .map(|p| char::from_u32(*p).unwrap_or('\0'))
            .collect();
        for captures in pattern.captures_iter(&input) {
            let matched = captures.get(1).unwrap();
            if index != 0
                && (word(rt, input[..matched.start()].chars().next_back())?
                    || word(rt, input[matched.end()..].chars().next())?)
            {
                continue;
            }
            let ident = matched.as_str();
            if ident.len() > 2 {
                ids.insert(ident.rsplit('.').next().unwrap().to_owned());
            }
        }
    }
    Ok(json!(ids.into_iter().take(60).collect::<Vec<_>>()))
}
fn preflight(rt: &mut dyn Runtime) -> Result<Value> {
    if let Err(error) = rt.call("scaffold_import", json!(["aider.repomap"])) {
        if !matches(rt, &error, &["ImportError"])? {
            return Err(error);
        }
        caught(rt, Some(&error))?;
        hfunction(rt, "ScaffoldError")?;
        copy(rt, "error_class", "function")?;
        let mut message = Text::from(
            "the repomap scaffold needs aider importable by the interpreter running this lane, which is ",
        );
        hfunction(rt, "sys.executable")?;
        message.0.extend(hformatted(rt, "function")?.0);
        push(
            &mut message,
            ". It generates the map on the HOST because the container's Python is era-pinned. Two causes, likeliest first:\n  1. WRONG INTERPRETER -- aider is installed, but not here. The launchers run `${PYTHON:-python3}`, so an unactivated shell picks up the system Python. Fix: `source .venv/bin/activate`, or pass `PYTHON=/path/to/.venv/bin/python` to the launcher.\n  2. NOT INSTALLED -- `pip install -e '.[baselines]'` (or 'aider-chat') into THAT interpreter.\n(",
        );
        message.0.extend(hformatted(rt, "caught_error")?.0);
        push(&mut message, ")");
        text(rt, "message", message)?;
        hcall(rt, "error_class", &["message"], &[], "error")?;
        return rt.call("scaffold_raise", json!(["error", true]));
    }
    Ok(Value::Null)
}
fn fetch(rt: &mut dyn Runtime) -> Result<Value> {
    function(rt, "os.path.join")?;
    copy(rt, "join", "function")?;
    attr(rt, "ctx", "run_dir", "run_dir")?;
    literal(rt, "suffix", json!("_repomap_src"))?;
    call(rt, "join", &["run_dir", "suffix"], "dest")?;
    function(rt, "os.makedirs")?;
    literal(rt, "yes", json!(true))?;
    invoke(rt, &["dest"], &[("exist_ok", "yes")])?;
    attr(rt, "ctx", "read_bytes", "read")?;
    member(rt, "TAR_SCRIPT", "script")?;
    call(rt, "read", &["script"], "blob")?;
    if !boolean(rt, "blob")? {
        function(rt, "ScaffoldError")?;
        literal(
            rt,
            "message",
            json!("the repository stream out of the container was empty"),
        )?;
        invoke(rt, &["message"], &[])?;
        return rt.call("task_raise", json!(["called"]));
    }
    function(rt, "tarfile.open")?;
    copy(rt, "open", "function")?;
    apply(rt, "io.BytesIO", &["blob"], "buffer")?;
    copy(rt, "function", "open")?;
    invoke(rt, &[], &[("fileobj", "buffer")])?;
    rt.call("task_enter", json!(["tar"]))?;
    let body: Result<()> = (|| {
        method(rt, "tar_handle", "getmembers", &[], "members")?;
        literal(rt, "safe", json!([]))?;
        rt.call("workspace_iter", json!(["members", "members_iter"]))?;
        while truthy(&rt.call("workspace_next", json!(["members_iter", "entry"]))?) {
            method(rt, "entry", "isfile", &[], "regular")?;
            if !boolean(rt, "regular")? {
                continue;
            }
            attr(rt, "entry", "name", "name")?;
            attr(rt, "name", "startswith", "startswith")?;
            literal(rt, "slash", json!("/"))?;
            literal(rt, "parent", json!(".."))?;
            rt.call("eval_tuple", json!(["prefixes", ["slash", "parent"]]))?;
            call(rt, "startswith", &["prefixes"], "unsafe_prefix")?;
            if boolean(rt, "unsafe_prefix")? {
                continue;
            }
            literal(rt, "traversal", json!("/../"))?;
            attr(rt, "entry", "name", "name")?;
            if truthy(&rt.call("provider_contains", json!(["traversal", "name"]))?) {
                continue;
            }
            rt.call("eval_append", json!(["safe", "entry"]))?;
        }
        attr(rt, "tar_handle", "extractall", "function")?;
        invoke(rt, &["dest"], &[("members", "safe")])
    })();
    rt.call("task_exit", json!(["tar", body.as_ref().err()]))?;
    if let Err(error) = body
        && !boolean(rt, "exit_result")?
    {
        return Err(error);
    }
    Ok(returned("dest"))
}
fn generate(rt: &mut dyn Runtime) -> Result<Value> {
    function(rt, "subprocess.run")?;
    copy(rt, "run", "function")?;
    function(rt, "sys.executable")?;
    copy(rt, "python", "function")?;
    literal(
        rt,
        "generator",
        json!(
            std::env::current_exe()?
                .with_file_name(format!("te-repomap-gen{}", std::env::consts::EXE_SUFFIX))
        ),
    )?;
    literal(rt, "python_flag", json!("--python"))?;
    builtin(rt, "str")?;
    copy(rt, "string", "function")?;
    apply(rt, "map_tokens", &[], "budget")?;
    call(rt, "string", &["budget"], "budget")?;
    literal(rt, "separator", json!(","))?;
    method(rt, "separator", "join", &["idents"], "idents_arg")?;
    rt.call(
        "eval_list",
        json!([
            "args",
            [
                "generator",
                "python_flag",
                "python",
                "src_dir",
                "budget",
                "idents_arg"
            ]
        ]),
    )?;
    literal(rt, "yes", json!(true))?;
    literal(rt, "timeout", json!(600))?;
    copy(rt, "function", "run")?;
    invoke(
        rt,
        &["args"],
        &[
            ("capture_output", "yes"),
            ("text", "yes"),
            ("timeout", "timeout"),
        ],
    )?;
    copy(rt, "res", "called")?;
    attr(rt, "res", "returncode", "rc")?;
    literal(rt, "zero", json!(0))?;
    rt.call("web_compare", json!(["ne", "rc", "zero"]))?;
    literal(rt, "empty", json!(""))?;
    if boolean(rt, "comparison")? {
        attr(rt, "res", "stderr", "stderr")?;
        fallback(rt, "stderr", json!(""))?;
        rt.call("logger_slice", json!(["stderr", -300, null, "stderr"]))?;
        rt.call("eval_tuple", json!(["generated", ["empty", "stderr"]]))?;
    } else {
        attr(rt, "res", "stdout", "stdout")?;
        rt.call("eval_tuple", json!(["generated", ["stdout", "empty"]]))?;
    }
    Ok(returned("generated"))
}
fn cleanup(rt: &mut dyn Runtime, pending: Option<&Failure>) -> Result<()> {
    caught(rt, pending)?;
    if !truthy(&rt.call("scaffold_bool", json!(["src"]))?) {
        return Ok(());
    }
    hfunction(rt, "os.path.isdir")?;
    copy(rt, "isdir", "function")?;
    hcall(rt, "isdir", &["src"], &[], "is_dir")?;
    if truthy(&rt.call("scaffold_bool", json!(["is_dir"]))?) {
        hfunction(rt, "shutil.rmtree")?;
        copy(rt, "rmtree", "function")?;
        literal(rt, "yes", json!(true))?;
        hcall(
            rt,
            "rmtree",
            &["src"],
            &[("ignore_errors", "yes")],
            "removed",
        )?;
    }
    Ok(())
}
fn setup(rt: &mut dyn Runtime) -> Result<Value> {
    function(rt, "extract_idents")?;
    copy(rt, "extract", "function")?;
    attr(rt, "ctx", "task", "task")?;
    literal(rt, "key", json!("problem_statement"))?;
    literal(rt, "empty", json!(""))?;
    method(rt, "task", "get", &["key", "empty"], "problem")?;
    call(rt, "extract", &["problem"], "idents")?;
    literal(rt, "notes", json!({"arm_setup":"repomap"}))?;
    builtin(rt, "len")?;
    invoke(rt, &["idents"], &[])?;
    field(rt, "notes", "n_idents", "called")?;
    apply(rt, "map_tokens", &[], "budget")?;
    field(rt, "notes", "map_tokens_budget", "budget")?;
    literal(rt, "src", Value::Null)?;
    let result: Result<()> = (|| {
        own(rt, "fetch_sources", &["ctx"], "src")?;
        own(rt, "generate", &["src", "idents"], "generated")?;
        rt.call("scaffold_pair", json!(["generated", "map", "err"]))?;
        Ok(())
    })();
    let handled = if let Err(error) = result {
        (|| {
            if matches(rt, &error, &["ScaffoldError", "subprocess.TimeoutExpired"])? {
                caught(rt, Some(&error))?;
                builtin(rt, "str")?;
                copy(rt, "string", "function")?;
                hcall(rt, "string", &["caught_error"], &[], "err")?;
                rt.call("logger_slice", json!(["err", -300, null, "err"]))?;
                literal(rt, "map", json!(""))
            } else {
                Err(error)
            }
        })()
    } else {
        Ok(())
    };
    let pending = handled.err();
    cleanup(rt, pending.as_ref())?;
    if let Some(error) = pending {
        return Err(error);
    }
    method(rt, "map", "strip", &[], "stripped")?;
    if !boolean(rt, "stripped")? {
        attr(rt, "ctx", "log", "logger")?;
        let mut message = Text::from("repomap: generation produced no map (");
        message.0.extend(formatted(rt, "err", "")?.0);
        push(&mut message, ")");
        text(rt, "message", message)?;
        call(rt, "logger", &["message"], "logged")?;
        literal(rt, "extra", json!({"map_ok":false,"map_chars":0}))?;
        field(rt, "extra", "map_err", "err")?;
        method(rt, "notes", "update", &["extra"], "updated")?;
    } else {
        attr(rt, "ctx", "write_instructions", "writer")?;
        function(rt, "HEADER")?;
        copy(rt, "header", "function")?;
        binary(rt, "add", "header", "map", "instructions")?;
        call(rt, "writer", &["instructions"], "written")?;
        field(rt, "notes", "map_ok", "written")?;
        builtin(rt, "len")?;
        invoke(rt, &["map"], &[])?;
        field(rt, "notes", "map_chars", "called")?;
        attr(rt, "ctx", "profile", "profile")?;
        attr(rt, "profile", "instructions_file", "instructions_file")?;
        field(rt, "notes", "instructions_file", "instructions_file")?;
    }
    Ok(returned("notes"))
}
pub(super) fn dispatch(operation: &str, rt: &mut dyn Runtime) -> Result<Value> {
    match operation {
        "tokens" => tokens(rt),
        "idents" => idents(rt),
        "preflight" => preflight(rt),
        "fetch" => fetch(rt),
        "generate" => generate(rt),
        "setup" => setup(rt),
        _ => Err(Failure::new(
            "ValueError",
            format!("unknown repo-map scaffold operation: {operation}"),
        )),
    }
}
