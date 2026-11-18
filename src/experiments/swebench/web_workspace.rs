use super::{
    Result, Runtime,
    optimizer::code_points,
    providers::{boolean, copy, literal},
    runner_calls::{function, invoke, member},
};
use crate::toolprune::protected::fnmatch_points;
use crate::{python_json::Text, value::truthy};
use serde_json::{Value, json};

fn attribute(rt: &mut dyn Runtime, slot: &str, name: &str, target: &str) -> Result<()> {
    rt.call("stdout_member", json!([slot, name, target]))
        .map(|_| ())
}
fn apply(rt: &mut dyn Runtime, name: &str, args: &[&str], target: &str) -> Result<()> {
    function(rt, name)?;
    invoke(rt, args, &[])?;
    copy(rt, target, "called")
}
fn method(rt: &mut dyn Runtime, slot: &str, name: &str, args: &[&str], target: &str) -> Result<()> {
    attribute(rt, slot, name, "function")?;
    invoke(rt, args, &[])?;
    copy(rt, target, "called")
}
fn builtin(rt: &mut dyn Runtime, name: &str) -> Result<()> {
    rt.call("eval_builtin", json!([name])).map(|_| ())
}
fn constant(rt: &mut dyn Runtime, slot: &str, name: &str) -> Result<()> {
    rt.call("provider_constant", json!([slot, name]))
        .map(|_| ())
}
fn iter(rt: &mut dyn Runtime, slot: &str, target: &str) -> Result<()> {
    rt.call("workspace_iter", json!([slot, target])).map(|_| ())
}
fn next(rt: &mut dyn Runtime, iterator: &str, target: &str) -> Result<bool> {
    rt.call("workspace_next", json!([iterator, target]))
        .map(|v| truthy(&v))
}
fn contains(rt: &mut dyn Runtime, item: &str, collection: &str) -> Result<bool> {
    rt.call("provider_contains", json!([item, collection]))
        .map(|v| truthy(&v))
}
fn append(rt: &mut dyn Runtime, list: &str, value: &str) -> Result<()> {
    rt.call("eval_append", json!([list, value])).map(|_| ())
}
fn formatted(rt: &mut dyn Runtime, slot: &str) -> Result<Text> {
    code_points(rt.call("handler_format", json!([slot]))?)
}
fn text(rt: &mut dyn Runtime, slot: &str, value: Text) -> Result<()> {
    rt.call("logger_literal", json!([slot, value.0]))
        .map(|_| ())
}
fn suffix(rt: &mut dyn Runtime, suffix: &str) -> Result<()> {
    let mut message = formatted(rt, "rel")?;
    message.0.extend(suffix.chars().map(u32::from));
    text(rt, "skipped_entry", message)?;
    append(rt, "skipped", "skipped_entry")
}
fn compare(rt: &mut dyn Runtime, operation: &str, a: &str, b: &str) -> Result<bool> {
    rt.call("web_compare", json!([operation, a, b]))?;
    boolean(rt, "comparison")
}
fn fallback(rt: &mut dyn Runtime, source: &str, target: &str) -> Result<()> {
    if boolean(rt, source)? {
        copy(rt, target, source)
    } else {
        literal(rt, target, json!({}))
    }
}
fn makedirs(rt: &mut dyn Runtime, directory: &str) -> Result<()> {
    function(rt, "os.makedirs")?;
    literal(rt, "true", json!(true))?;
    invoke(rt, &[directory], &[("exist_ok", "true")])
}
fn opened(rt: &mut dyn Runtime, write: bool) -> Result<()> {
    builtin(rt, "open")?;
    literal(rt, "encoding", json!("utf-8"))?;
    if write {
        literal(rt, "write", json!("w"))?;
        invoke(rt, &["path", "write"], &[("encoding", "encoding")])?;
    } else {
        invoke(rt, &["path"], &[("encoding", "encoding")])?;
    }
    rt.call("eval_enter", json!([]))?;
    let result: Result<()> = (|| {
        if write {
            attribute(rt, "config_handle", "write", "write_function")?;
            builtin(rt, "isinstance")?;
            copy(rt, "isinstance", "function")?;
            builtin(rt, "str")?;
            copy(rt, "string_type", "function")?;
            copy(rt, "function", "isinstance")?;
            invoke(rt, &["content", "string_type"], &[])?;
            if boolean(rt, "called")? {
                copy(rt, "written", "content")?;
            } else {
                literal(rt, "written", json!(""))?;
            }
            copy(rt, "function", "write_function")?;
            invoke(rt, &["written"], &[])
        } else {
            method(rt, "config_handle", "read", &[], "content")
        }
    })();
    rt.call("eval_exit", json!([result.as_ref().err()]))?;
    if let Err(error) = result
        && !boolean(rt, "exit_result")?
    {
        return Err(error);
    }
    Ok(())
}

pub fn materialize(rt: &mut dyn Runtime) -> Result<Value> {
    apply(rt, "os.path.isdir", &["workspace"], "is_directory")?;
    if boolean(rt, "is_directory")? {
        function(rt, "shutil.rmtree")?;
        literal(rt, "true", json!(true))?;
        invoke(rt, &["workspace"], &[("ignore_errors", "true")])?;
    }
    makedirs(rt, "workspace")?;
    fallback(rt, "files", "file_map")?;
    method(rt, "file_map", "items", &[], "items")?;
    iter(rt, "items", "items_iter")?;
    while next(rt, "items_iter", "entry")? {
        rt.call("workspace_pair", json!(["entry", "rel", "content"]))?;
        if !boolean(rt, "rel")? {
            continue;
        }
        literal(rt, "slash", json!("/"))?;
        method(rt, "rel", "startswith", &["slash"], "absolute")?;
        if boolean(rt, "absolute")? {
            continue;
        }
        method(rt, "rel", "split", &["slash"], "parts")?;
        literal(rt, "parent", json!(".."))?;
        if contains(rt, "parent", "parts")? {
            continue;
        }
        apply(rt, "os.path.join", &["workspace", "rel"], "path")?;
        function(rt, "os.makedirs")?;
        copy(rt, "makedirs", "function")?;
        apply(rt, "os.path.dirname", &["path"], "parent_path")?;
        copy(rt, "function", "makedirs")?;
        invoke(rt, &["parent_path"], &[("exist_ok", "true")])?;
        opened(rt, true)?;
    }
    Ok(Value::Null)
}

fn relative(rt: &mut dyn Runtime, path: &str, target: &str) -> Result<()> {
    apply(rt, "os.path.relpath", &[path, "workspace"], "relative")?;
    attribute(rt, "relative", "replace", "replace")?;
    function(rt, "os.sep")?;
    copy(rt, "separator", "function")?;
    literal(rt, "slash", json!("/"))?;
    copy(rt, "function", "replace")?;
    invoke(rt, &["separator", "slash"], &[])?;
    copy(rt, target, "called")
}
fn skip_name(rt: &mut dyn Runtime, constant_name: &str, slot: &str) -> Result<bool> {
    constant(rt, "skip_names", constant_name)?;
    if contains(rt, slot, "skip_names")? {
        return Ok(true);
    }
    literal(rt, "dot", json!("."))?;
    method(rt, slot, "startswith", &["dot"], "hidden")?;
    boolean(rt, "hidden")
}
fn directories(rt: &mut dyn Runtime) -> Result<()> {
    relative(rt, "root", "base")?;
    literal(rt, "dot", json!("."))?;
    if compare(rt, "eq", "base", "dot")? {
        literal(rt, "base", json!(""))?;
    }
    literal(rt, "kept", json!([]))?;
    iter(rt, "dirs", "dirs_iter")?;
    while next(rt, "dirs_iter", "directory")? {
        if boolean(rt, "base")? {
            let mut rel = formatted(rt, "base")?;
            rel.0.push(47);
            rel.0.extend(formatted(rt, "directory")?.0);
            text(rt, "directory_rel", rel)?;
        } else {
            copy(rt, "directory_rel", "directory")?;
        }
        if contains(rt, "directory_rel", "supplied_dirs")?
            || !skip_name(rt, "_SKIP_DIRS", "directory")?
        {
            append(rt, "kept", "directory")?;
        }
    }
    rt.call("workspace_dirs", json!([])).map(|_| ())
}
fn collect_file(rt: &mut dyn Runtime) -> Result<()> {
    rt.call("handler_exception", json!([null]))?;
    apply(rt, "os.path.join", &["root", "name"], "path")?;
    relative(rt, "path", "rel")?;
    if !contains(rt, "rel", "supplied")? {
        if skip_name(rt, "_SKIP_FILES", "name")? {
            return Ok(());
        }
        if contains(rt, "rel", "scaffold_files")? || contains(rt, "name", "scaffold_files")? {
            return append(rt, "skipped", "rel");
        }
        apply(rt, "_ignored", &["rel", "globs"], "ignored")?;
        if boolean(rt, "ignored")? {
            return suffix(rt, " (evaluator ignores it)");
        }
    }
    if let Err(error) = apply(rt, "os.path.getsize", &["path"], "size") {
        if error.is_a("builtins.OSError") {
            return Ok(());
        }
        return Err(error);
    }
    constant(rt, "cap", "_MAX_FILE_BYTES")?;
    if compare(rt, "gt", "size", "cap")? {
        let mut message = formatted(rt, "rel")?;
        message.0.extend(" (".chars().map(u32::from));
        message.0.extend(formatted(rt, "size")?.0);
        message.0.extend("B > cap)".chars().map(u32::from));
        text(rt, "skipped_entry", message)?;
        return append(rt, "skipped", "skipped_entry");
    }
    if let Err(error) = opened(rt, false) {
        if error.is_a("builtins.OSError") || error.is_a("builtins.UnicodeDecodeError") {
            rt.call("handler_exception", json!([error]))?;
            suffix(rt, " (unreadable/binary)")?;
            rt.call("handler_exception", json!([null]))?;
            return Ok(());
        }
        return Err(error);
    }
    method(rt, "seen", "add", &["rel"], "added")?;
    fallback(rt, "request_files", "request_map")?;
    method(rt, "request_map", "get", &["rel"], "before")?;
    rt.call("workspace_local", json!(["content", "after"]))?;
    if compare(rt, "ne", "before", "after")? {
        rt.call("workspace_put", json!(["out", "rel", "content"]))?;
    }
    Ok(())
}

pub fn collect(rt: &mut dyn Runtime) -> Result<Value> {
    function(rt, "_load_ignore_globs")?;
    member(rt, "repo", "repo")?;
    invoke(rt, &["repo", "project_dir"], &[])?;
    copy(rt, "globs", "called")?;
    builtin(rt, "set")?;
    copy(rt, "set", "function")?;
    fallback(rt, "request_files", "request_map")?;
    copy(rt, "function", "set")?;
    invoke(rt, &["request_map"], &[])?;
    copy(rt, "supplied", "called")?;
    member(rt, "scaffold_answer_excludes", "function")?;
    invoke(rt, &[], &[])?;
    copy(rt, "scaffold_files", "called")?;
    iter(rt, "supplied", "supplied_iter")?;
    rt.call("workspace_set", json!(["supplied_dirs"]))?;
    while next(rt, "supplied_iter", "supplied_rel")? {
        apply(rt, "_ancestors", &["supplied_rel"], "ancestors")?;
        iter(rt, "ancestors", "ancestors_iter")?;
        while next(rt, "ancestors_iter", "ancestor")? {
            method(rt, "supplied_dirs", "add", &["ancestor"], "added")?;
        }
    }
    literal(rt, "out", json!({}))?;
    rt.call("workspace_set", json!(["seen"]))?;
    literal(rt, "skipped", json!([]))?;
    apply(rt, "os.walk", &["workspace"], "walk")?;
    iter(rt, "walk", "walk_iter")?;
    while next(rt, "walk_iter", "entry")? {
        rt.call("workspace_walk", json!([]))?;
        directories(rt)?;
        iter(rt, "names", "names_iter")?;
        while next(rt, "names_iter", "name")? {
            collect_file(rt)?;
        }
    }
    builtin(rt, "sorted")?;
    copy(rt, "sorted", "function")?;
    builtin(rt, "set")?;
    copy(rt, "set", "function")?;
    fallback(rt, "request_files", "request_map")?;
    copy(rt, "function", "set")?;
    invoke(rt, &["request_map"], &[])?;
    copy(rt, "remaining", "called")?;
    rt.call("web_binary", json!(["sub", "remaining", "seen", "deleted"]))?;
    copy(rt, "function", "sorted")?;
    invoke(rt, &["deleted"], &[])?;
    copy(rt, "deleted", "called")?;
    rt.call("provider_tuple", json!(["out", "deleted", "skipped"]))?;
    Ok(json!({"provider_ref":"tuple"}))
}

fn ignore_file(rt: &mut dyn Runtime) -> Result<()> {
    builtin(rt, "open")?;
    literal(rt, "encoding", json!("utf-8"))?;
    invoke(rt, &["path"], &[("encoding", "encoding")])?;
    rt.call("eval_enter", json!([]))?;
    let result: Result<()> = (|| {
        iter(rt, "config_handle", "lines_iter")?;
        while next(rt, "lines_iter", "line")? {
            super::web_output::strip(rt, false, "line")?;
            if !boolean(rt, "line")? {
                continue;
            }
            literal(rt, "comment", json!("#"))?;
            method(rt, "line", "startswith", &["comment"], "is_comment")?;
            if boolean(rt, "is_comment")? {
                continue;
            }
            literal(rt, "slash", json!("/"))?;
            method(rt, "line", "rstrip", &["slash"], "pattern")?;
            append(rt, "output", "pattern")?;
        }
        Ok(())
    })();
    rt.call("eval_exit", json!([result.as_ref().err()]))?;
    if let Err(error) = result
        && !boolean(rt, "exit_result")?
    {
        return Err(error);
    }
    Ok(())
}

pub fn load_ignore_globs(rt: &mut dyn Runtime) -> Result<Value> {
    literal(rt, "output", json!([]))?;
    for (index, (directory, filename)) in [
        ("project_dir", ".gitignore"),
        ("project_dir", ".npmignore"),
        ("repo", ".gitignore"),
        ("project_dir", ".evalignore"),
    ]
    .into_iter()
    .enumerate()
    {
        function(rt, "os.path.join")?;
        literal(rt, "filename", json!(filename))?;
        invoke(rt, &[directory, "filename"], &[])?;
        copy(rt, &format!("ignore_path{index}"), "called")?;
    }
    for index in 0..4 {
        copy(rt, "path", &format!("ignore_path{index}"))?;
        match ignore_file(rt) {
            Err(error) if error.is_a("builtins.OSError") => {}
            result => result?,
        }
    }
    Ok(json!({"provider_ref":"output"}))
}

pub fn ancestors(rt: &mut dyn Runtime) -> Result<Value> {
    literal(rt, "slash", json!("/"))?;
    method(rt, "rel", "split", &["slash"], "full_parts")?;
    rt.call("logger_slice", json!(["full_parts", null, -1, "parts"]))?;
    builtin(rt, "range")?;
    copy(rt, "range", "function")?;
    builtin(rt, "len")?;
    invoke(rt, &["parts"], &[])?;
    copy(rt, "length", "called")?;
    copy(rt, "function", "range")?;
    invoke(rt, &["length"], &[])?;
    iter(rt, "called", "indices")?;
    literal(rt, "output", json!([]))?;
    while next(rt, "indices", "index")? {
        literal(rt, "one", json!(1))?;
        rt.call("web_binary", json!(["add", "index", "one", "stop"]))?;
        rt.call("workspace_slice", json!(["parts", "stop", "prefix"]))?;
        method(rt, "slash", "join", &["prefix"], "ancestor")?;
        append(rt, "output", "ancestor")?;
    }
    Ok(json!({"provider_ref":"output"}))
}

fn matches(rt: &mut dyn Runtime) -> Result<bool> {
    let input = rt.call("workspace_match", json!([]))?;
    if let Some(input) = input.as_array() {
        let name = code_points(input[0].clone())?;
        let pattern = code_points(input[1].clone())?;
        return Ok(fnmatch_points(&name.0, &pattern.0));
    }
    copy(rt, "function", "match_function")?;
    invoke(rt, &["match_candidate", "match_pattern"], &[])?;
    boolean(rt, "called")
}

pub fn ignored(rt: &mut dyn Runtime) -> Result<Value> {
    rt.call("workspace_fnmatch_import", json!([]))?;
    literal(rt, "slash", json!("/"))?;
    method(rt, "rel", "split", &["slash"], "parts")?;
    iter(rt, "globs", "patterns_iter")?;
    while next(rt, "patterns_iter", "original_pattern")? {
        method(rt, "original_pattern", "lstrip", &["slash"], "pattern")?;
        if !boolean(rt, "pattern")? {
            continue;
        }
        iter(rt, "parts", "parts_iter")?;
        while next(rt, "parts_iter", "candidate")? {
            if matches(rt)? {
                return Ok(json!(true));
            }
        }
        copy(rt, "candidate", "rel")?;
        if matches(rt)? {
            return Ok(json!(true));
        }
    }
    Ok(json!(false))
}
