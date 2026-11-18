use super::container_helper::quote;
use super::{
    Failure, Result, Runtime,
    codex_config::{integer, sequence},
    settings::field,
};
use crate::value::{base64, python_repr, python_type, scalar_text, truthy};
use serde_json::{Value, json};

mod native;

fn space(c: char) -> bool {
    c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}')
}
fn output<'a>(value: &'a Value, method: &str) -> Result<&'a str> {
    if !truthy(value) {
        return Ok("");
    }
    value.as_str().ok_or_else(|| {
        Failure::new(
            "AttributeError",
            format!(
                "'{}' object has no attribute '{method}'",
                python_type(value)
            ),
        )
    })
}
fn tail(text: &str, n: usize) -> String {
    text.chars()
        .skip(text.chars().count().saturating_sub(n))
        .collect()
}
fn member(value: &Value, text: &str) -> Result<bool> {
    if !truthy(value) {
        return Ok(false);
    }
    match value {
        Value::String(v) => Ok(v.contains(text)),
        Value::Array(v) => Ok(v.iter().any(|item| item == text)),
        Value::Object(v) => Ok(v.contains_key(text)),
        _ => Err(Failure::new(
            "TypeError",
            format!(
                "argument of type '{}' is not a container or iterable",
                python_type(value)
            ),
        )),
    }
}
fn sliced_message(value: &Value, n: usize) -> Result<String> {
    if !truthy(value) {
        return Ok(String::new());
    }
    match value {
        Value::String(text) => Ok(tail(text, n)),
        Value::Array(items) => Ok(scalar_text(&json!(&items[items.len().saturating_sub(n)..]))),
        Value::Object(_) => Err(Failure::new("KeyError", format!("slice(-{n}, None, None)"))),
        _ => Err(Failure::new(
            "TypeError",
            format!("'{}' object is not subscriptable", python_type(value)),
        )),
    }
}
fn exec(rt: &mut dyn Runtime, name: &Value, script: String) -> Result<Value> {
    rt.call("_exec", json!([name,script,{"timeout":60}]))
}
pub fn install_hosts(name: &Value, rt: &mut dyn Runtime) -> Result<Value> {
    if !truthy(&rt.call("config_function", json!(["network_tool_block", []]))?) {
        return Ok(json!(false));
    }
    let result = (|| -> Result<()> {
        let block = rt.call("net_guard_function", json!(["hosts_block", []]))?;
        let block = block.as_str().ok_or_else(|| {
            Failure::new(
                "AttributeError",
                format!("'{}' object has no attribute 'encode'", python_type(&block)),
            )
        })?;
        let blob = base64(block.as_bytes());
        let marker = rt.call("net_guard_constant", json!(["HOSTS_MARKER"]))?;
        let marker = marker
            .as_str()
            .ok_or_else(|| Failure::new("TypeError", "expected string or bytes-like object"))?;
        let marker = quote(marker);
        let result = exec(
            rt,
            name,
            format!(
                "grep -q {marker} /etc/hosts || echo {blob} | base64 -d >> /etc/hosts; grep -c {marker} /etc/hosts"
            ),
        )?;
        let stdout = output(&result["stdout"], "strip")?.trim_matches(space);
        if stdout
            .split(|c| {
                matches!(
                    c,
                    '\n' | '\r' | '\u{b}' | '\u{c}' | '\u{1c}'
                        ..='\u{1e}' | '\u{85}' | '\u{2028}' | '\u{2029}'
                )
            })
            .next_back()
            != Some("1")
        {
            let message = if truthy(&result["stderr"]) {
                &result["stderr"]
            } else {
                &result["stdout"]
            };
            return Err(Failure::new("RuntimeError", sliced_message(message, 200)?));
        }
        let probe = exec(
            rt,
            name,
            "getent hosts github.com | awk '{print $1}' | head -1".into(),
        )?;
        if !member(&probe["stdout"], "127.0.0.1")? {
            let stdout = output(&probe["stdout"], "strip")?;
            return Err(Failure::new(
                "RuntimeError",
                format!(
                    "hosts stanza written but github.com still resolves to {} -- this container's resolver is not reading /etc/hosts, so nothing is blocked",
                    python_repr(&json!(stdout.trim_matches(space)))
                ),
            ));
        }
        Ok(())
    })();
    match result {
        Ok(()) => Ok(json!(true)),
        Err(error) if error.is_a("builtins.Exception") => {
            rt.call(
                "log",
                json!([format!(
                    "answer-key hosts block FAILED ({}: {}) -- row runs UNGUARDED at the DNS layer",
                    error.kind, error.message
                )]),
            )?;
            Ok(json!(false))
        }
        Err(error) => Err(error),
    }
}
pub fn verify(name: &Value, python: &Value, rt: &mut dyn Runtime) -> Result<Value> {
    let first = field(rt, "runner", "NET_GUARD_PROBE", "json", None)?;
    let second = field(rt, "runner", "NET_GUARD_PROBE", "json", None)?;
    let payload = crate::value::json_text(
        &json!({"tool_name":"Bash","toolName":"bash","tool_input":{"command":first},"toolArgs":{"command":second}}),
    );
    let script = field(rt, "runner", "NET_GUARD_SCRIPT", "json", None)?;
    let result = exec(
        rt,
        name,
        format!(
            "printf '%s' {} | {} {}; echo rc=$?",
            quote(&payload),
            scalar_text(python),
            scalar_text(&script)
        ),
    )?;
    if !member(&result["stdout"], "rc=2")? {
        let stdout = output(&result["stdout"], "strip")?;
        return Err(Failure::new(
            "RuntimeError",
            format!(
                "guard installed but did NOT refuse its probe (expected rc=2, got {}) -- it would block nothing",
                python_repr(&json!(tail(stdout.trim_matches(space), 80)))
            ),
        ));
    }
    Ok(Value::Null)
}
pub fn blocks(name: &Value, int_max_digits: usize, rt: &mut dyn Runtime) -> Result<Value> {
    let result = (|| -> Result<String> {
        let path = field(rt, "runner", "NET_GUARD_LOG", "json", None)?;
        let result = exec(
            rt,
            name,
            format!("wc -l < {} 2>/dev/null || echo 0", scalar_text(&path)),
        )?;
        let stdout = if truthy(&result["stdout"]) {
            result["stdout"].clone()
        } else {
            json!("0")
        };
        let text = output(&stdout, "strip")?.trim_matches(space);
        let first = text
            .split(space)
            .find(|v| !v.is_empty())
            .ok_or_else(|| Failure::new("IndexError", "list index out of range"))?;
        let value = integer(&json!(first))?;
        if int_max_digits != 0
            && first
                .chars()
                .filter(|c| !matches!(c, '_' | '+' | '-'))
                .count()
                > int_max_digits
        {
            return Err(Failure::new(
                "ValueError",
                "integer string conversion limit exceeded",
            ));
        }
        Ok(value)
    })();
    match result {
        Ok(value) => Ok(json!(value)),
        Err(error) if error.is_a("builtins.Exception") => Ok(json!("0")),
        Err(error) => Err(error),
    }
}
pub fn graded_diff(name: &Value, rt: &mut dyn Runtime) -> Result<Value> {
    let mut diff = String::from("git -C /testbed diff");
    if truthy(&rt.call("guard_patch_excludes", json!([]))?) {
        let excludes = rt.call("guard_exclude_values", json!([]))?;
        let specs = sequence(&excludes)?
            .iter()
            .map(|e| format!("':(exclude,top){}'", scalar_text(e)))
            .collect::<Vec<_>>()
            .join(" ");
        diff.push_str(&format!(" -- . {specs}"));
    }
    let result = rt.call(
        "_exec",
        json!([
            name,
            format!("git -C /testbed add -A -N >/dev/null 2>&1; {diff}"),
            {}
        ]),
    )?;
    Ok(result["stdout"].clone())
}
pub fn dispatch(method: &str, args: &Value, rt: &mut dyn Runtime) -> Result<Value> {
    match method {
        "install" => native::install(args, rt),
        "hosts" => install_hosts(&args["name"], rt),
        "verify" => verify(&args["name"], &args["python"], rt),
        "blocks" => blocks(
            &args["name"],
            args["int_max_digits"].as_u64().unwrap_or(4300) as usize,
            rt,
        ),
        "graded_diff" => graded_diff(&args["name"], rt),
        _ => Err(Failure::new(
            "ValueError",
            format!("unknown guard operation: {method}"),
        )),
    }
}
