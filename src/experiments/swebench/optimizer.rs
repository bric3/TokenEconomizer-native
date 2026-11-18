use super::{
    Failure, Result, Runtime,
    codex_config::integer_with_limit,
    providers::{boolean, copy, literal, read_json, trim},
};
use crate::{
    python_json::{self, Node, Text},
    value::truthy,
};
use serde_json::{Value, json};

fn object(items: Vec<(&str, Node)>) -> Node {
    Node::Dict(
        items
            .into_iter()
            .map(|(key, value)| (Text::from(key), value))
            .collect(),
    )
}
fn text(value: &str) -> Node {
    Node::String(Text::from(value))
}
fn number(value: String) -> Node {
    let nonzero = value != "0";
    Node::Number(value, nonzero)
}
fn parse_wire(value: &Value) -> Result<Node> {
    python_json::parse(value.as_str().unwrap_or("").as_bytes(), 0)
        .ok_or_else(|| Failure::new("ValueError", "invalid optimizer binding JSON"))
}
fn argument(rt: &mut dyn Runtime, slot: &str) -> Result<Node> {
    parse_wire(&rt.call("optimizer_json", json!([slot]))?)
}
fn formatted(rt: &mut dyn Runtime, slot: &str) -> Result<Text> {
    match argument_format(rt, slot)? {
        Node::String(ref value) => Ok(value.clone()),
        _ => unreachable!(),
    }
}
fn argument_format(rt: &mut dyn Runtime, slot: &str) -> Result<Node> {
    parse_wire(&rt.call("optimizer_format", json!([slot]))?)
}
fn append(target: &mut Text, value: &Text) {
    target.0.extend(&value.0);
}
fn suffix(mut value: Text, suffix: &str) -> Text {
    value.0.extend(suffix.chars().map(u32::from));
    value
}
fn blank(value: &Text) -> bool {
    value.0.iter().all(|point| {
        char::from_u32(*point)
            .is_some_and(|c| c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}'))
    })
}
fn get<'a>(value: &'a Node, key: &str) -> Result<Option<&'a Node>> {
    if !matches!(value, Node::Dict(_)) {
        return Err(Failure::new(
            "AttributeError",
            format!("'{}' object has no attribute 'get'", value.kind()),
        ));
    }
    Ok(value.field(key))
}
fn iterable(value: Option<&Node>, fallback: bool) -> Result<&[Node]> {
    match value {
        None => Ok(&[]),
        Some(value) if fallback && !value.truthy() => Ok(&[]),
        Some(Node::List(items)) => Ok(items),
        // Iterating JSON strings or object keys produces no object blocks.
        Some(Node::String(_) | Node::Dict(_)) => Ok(&[]),
        Some(value) => Err(Failure::new(
            "TypeError",
            format!("'{}' object is not iterable", value.kind()),
        )),
    }
}
fn is(value: Option<&Node>, expected: &str) -> bool {
    matches!(value,Some(Node::String(actual)) if actual==&Text::from(expected))
}
pub(super) fn response_text(data: &Node) -> Result<Text> {
    match get(data, "output_text")? {
        Some(Node::String(value)) if !blank(value) => return Ok(value.clone()),
        Some(Node::List(items)) => {
            let mut joined = Text::default();
            for item in items {
                if let Node::String(value) = item {
                    append(&mut joined, value);
                }
            }
            if !blank(&joined) {
                return Ok(joined);
            }
        }
        _ => {}
    }
    let mut output = Text::default();
    for item in iterable(get(data, "output")?, true)? {
        if !matches!(item, Node::Dict(_)) {
            continue;
        }
        for block in iterable(get(item, "content")?, true)? {
            if let Some(Node::String(value)) = block.field("text") {
                append(&mut output, value);
            }
        }
    }
    Ok(output)
}
pub fn response_text_call(rt: &mut dyn Runtime) -> Result<Value> {
    Ok(json!({"provider_encoded":response_text(&argument(rt,"data")?)?.json()}))
}
fn messages_text(data: &Node) -> Result<Text> {
    let mut output = Text::default();
    let mut count = 0_usize;
    for block in iterable(get(data, "content")?, false)? {
        if !is(block.field("type"), "text") {
            continue;
        }
        match block.field("text") {
            None => {}
            Some(Node::String(value)) => append(&mut output, value),
            Some(value) => {
                return Err(Failure::new(
                    "TypeError",
                    format!(
                        "sequence item {count}: expected str instance, {} found",
                        value.kind()
                    ),
                ));
            }
        }
        count += 1;
    }
    Ok(output)
}
fn capture_log(rt: &mut dyn Runtime) -> Result<()> {
    rt.call("optimizer_capture_log", json!([])).map(|_| ())
}
fn log(rt: &mut dyn Runtime, message: Text) -> Result<()> {
    rt.call("optimizer_log", json!([message.json()]))
        .map(|_| ())
}
fn starts_with(rt: &mut dyn Runtime, slot: &str, prefix: &str) -> Result<bool> {
    let wire = rt.call("optimizer_string", json!([slot]))?;
    if !wire.is_null() {
        if let Node::String(ref value) = parse_wire(&wire)? {
            return Ok(value
                .0
                .starts_with(&prefix.chars().map(u32::from).collect::<Vec<_>>()));
        }
        unreachable!();
    }
    rt.call(
        "provider_method",
        json!([slot, "startswith", [prefix], "prefix_match"]),
    )?;
    boolean(rt, "prefix_match")
}
fn request(rt: &mut dyn Runtime, codex: bool) -> Result<()> {
    if codex {
        copy(rt, "mid", "configured_model")?;
    } else {
        if boolean(rt, "model")? {
            copy(rt, "selected", "model")?;
        } else {
            literal(rt, "selected", json!(""))?;
        }
        let selected = if starts_with(rt, "selected", "claude-")? {
            "model"
        } else {
            "configured_model"
        };
        copy(rt, "mid", selected)?;
    }
    let mid = argument(rt, "mid")?;
    let body = if codex {
        let prompt = argument(rt, "prompt")?;
        let max = argument(rt, "max_tokens")?;
        object(vec![
            ("model", mid),
            (
                "input",
                Node::List(vec![object(vec![
                    ("role", text("user")),
                    (
                        "content",
                        Node::List(vec![object(vec![
                            ("type", text("input_text")),
                            ("text", prompt),
                        ])]),
                    ),
                ])]),
            ),
            ("max_output_tokens", max),
            ("store", Node::Bool(false)),
        ])
    } else {
        let max = argument(rt, "max_tokens")?;
        let prompt = argument(rt, "prompt")?;
        object(vec![
            ("model", mid),
            ("max_tokens", max),
            (
                "messages",
                Node::List(vec![object(vec![
                    ("role", text("user")),
                    ("content", prompt),
                ])]),
            ),
        ])
    };
    let mut headers = vec![json!(["content-type", "value", "application/json"])];
    if codex {
        rt.call("provider_routing_item", json!(["api_key", "api_key"]))?;
        let mut bearer = Text::from("Bearer ");
        append(&mut bearer, &formatted(rt, "api_key")?);
        headers.push(json!(["authorization", "encoded", bearer.json()]));
        rt.call("optimizer_capture_request", json!([]))?;
        rt.call("provider_routing_item", json!(["url", "base_url"]))?;
    } else {
        headers.push(json!(["anthropic-version", "value", "2023-06-01"]));
        if boolean(rt, "api_key")? {
            headers.push(json!(["x-api-key", "ref", "api_key"]));
        } else if boolean(rt, "bearer")? {
            let mut bearer = Text::from("Bearer ");
            append(&mut bearer, &formatted(rt, "bearer")?);
            headers.push(json!(["authorization", "encoded", bearer.json()]));
        }
        rt.call("optimizer_capture_request", json!([]))?;
        copy(rt, "url", "base_url")?;
        literal(rt, "timeout", json!(180))?;
    }
    trim(rt, "url", true)?;
    let Node::String(ref url) = argument(rt, "url")? else {
        return Err(Failure::new("TypeError", "optimizer URL must be a string"));
    };
    let url = suffix(
        url.clone(),
        if codex { "/responses" } else { "/v1/messages" },
    );
    rt.call(
        "optimizer_request",
        json!([url.json(), body.json(false), headers]),
    )?;
    Ok(())
}
fn byte_repr(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let quote = if bytes.contains(&b'\'') && !bytes.contains(&b'"') {
        b'"'
    } else {
        b'\''
    };
    let mut output = format!("b{}", char::from(quote));
    for &byte in bytes {
        match byte {
            b'\\' => output.push_str("\\\\"),
            b'\n' => output.push_str("\\n"),
            b'\r' => output.push_str("\\r"),
            b'\t' => output.push_str("\\t"),
            byte if byte == quote => {
                output.push('\\');
                output.push(char::from(byte));
            }
            32..=126 => output.push(char::from(byte)),
            _ => write!(output, "\\x{byte:02x}").unwrap(),
        }
    }
    output.push(char::from(quote));
    output
}
fn response(rt: &mut dyn Runtime, payload: &mut Option<Node>) -> Result<()> {
    rt.call("provider_http_enter", json!([]))?;
    let error = match read_json(rt) {
        Ok(value) => {
            *payload = Some(value);
            None
        }
        Err(error) => Some(error),
    };
    let suppressed = rt.call("provider_http_exit", json!([error]))?;
    if let Some(error) = error
        && !truthy(&suppressed)
    {
        return Err(error);
    }
    Ok(())
}
fn exchange(rt: &mut dyn Runtime, codex: bool) -> Result<Option<Node>> {
    let mut payload = None;
    for attempt in 0..3 {
        match response(rt, &mut payload) {
            Ok(()) => return Ok(payload),
            Err(error) if error.is_a("urllib.error.HTTPError") => {
                let code = rt.call("optimizer_error_code", json!([]))?;
                if code
                    .as_f64()
                    .is_some_and(|n| [429.0, 500.0, 502.0, 503.0, 529.0].contains(&n))
                    && attempt < 2
                {
                    rt.call("optimizer_sleep", json!([4 * (attempt + 1)]))?;
                    continue;
                }
                capture_log(rt)?;
                rt.call("optimizer_error_code", json!([]))?;
                let mut message = Text::from(if codex {
                    "codex optimizer_call HTTP "
                } else {
                    "optimizer_call HTTP "
                });
                append(&mut message, &formatted(rt, "error_code")?);
                message = suffix(message, ": ");
                let bytes: Vec<u8> = serde_json::from_value(rt.call(
                    "optimizer_error_body",
                    json!([if codex { 1200 } else { 200 }]),
                )?)
                .map_err(|e| Failure::new("TypeError", e.to_string()))?;
                message = suffix(message, &byte_repr(&bytes));
                log(rt, message)?;
                return Ok(None);
            }
            Err(error) if error.is_a("builtins.Exception") => {
                if attempt < 2 {
                    rt.call("optimizer_sleep", json!([4 * (attempt + 1)]))?;
                    continue;
                }
                capture_log(rt)?;
                log(
                    rt,
                    Text::from(
                        format!(
                            "{}optimizer_call failed: {}",
                            if codex { "codex " } else { "" },
                            error.message
                        )
                        .as_str(),
                    ),
                )?;
                return Ok(None);
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!()
}
fn integer(rt: &mut dyn Runtime, value: &Node) -> Result<String> {
    if !value.truthy() {
        return Ok("0".into());
    }
    if let Node::Number(raw, _) = value {
        if !raw.contains(['.', 'e']) && !matches!(raw.as_str(), "nan" | "inf" | "-inf") {
            return Ok(raw.clone());
        }
        let number = match raw.as_str() {
            "nan" => {
                return Err(Failure::new(
                    "ValueError",
                    "cannot convert float NaN to integer",
                ));
            }
            "inf" | "-inf" => {
                return Err(Failure::new(
                    "OverflowError",
                    "cannot convert float infinity to integer",
                ));
            }
            _ => raw
                .parse::<f64>()
                .map_err(|e| Failure::new("ValueError", e.to_string()))?,
        };
        let result = format!("{:.0}", number.trunc());
        return Ok(if result == "-0" { "0".into() } else { result });
    }
    match value {
        Node::String(value) => {
            let limit = rt
                .call("optimizer_integer_limit", json!([]))?
                .as_u64()
                .unwrap_or(0) as usize;
            let string = value
                .0
                .iter()
                .copied()
                .map(char::from_u32)
                .collect::<Option<String>>()
                .ok_or_else(|| {
                    Failure::new(
                        "ValueError",
                        format!(
                            "invalid literal for int() with base 10: {}",
                            crate::value::python_string_repr(value.0.iter().copied())
                        ),
                    )
                })?;
            integer_with_limit(&json!(string), limit)
        }
        Node::Bool(value) => Ok(if *value { "1" } else { "0" }.into()),
        other => Err(Failure::new(
            "TypeError",
            format!(
                "int() argument must be a string, a bytes-like object or a real number, not '{}'",
                other.kind()
            ),
        )),
    }
}
fn magnitude(a: &str, b: &str, add: bool) -> String {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut result = Vec::new();
    let mut carry = 0_i16;
    for offset in 0..a.len().max(b.len()) {
        let digit = |bytes: &[u8]| {
            bytes
                .len()
                .checked_sub(offset + 1)
                .map_or(0, |i| i16::from(bytes[i] - b'0'))
        };
        let total = digit(a) + if add { digit(b) } else { -digit(b) } + carry;
        result.push(u8::try_from(total.rem_euclid(10)).unwrap() + b'0');
        carry = total.div_euclid(10);
    }
    if carry > 0 {
        result.push(u8::try_from(carry).unwrap() + b'0');
    }
    while result.len() > 1 && result.last() == Some(&b'0') {
        result.pop();
    }
    result.reverse();
    String::from_utf8(result).unwrap()
}
fn nonnegative_difference(total: &str, cached: &str) -> String {
    let (t, c) = (
        total.trim_start_matches('-'),
        cached.trim_start_matches('-'),
    );
    match (total.starts_with('-'), cached.starts_with('-')) {
        (true, false) => "0".into(),
        (false, true) => magnitude(t, c, true),
        (negative, _) => {
            let (a, b) = if negative { (c, t) } else { (t, c) };
            if (a.len(), a) > (b.len(), b) {
                magnitude(a, b, false)
            } else {
                "0".into()
            }
        }
    }
}
fn usage_field(rt: &mut dyn Runtime, key: &str) -> Result<Node> {
    parse_wire(&rt.call("optimizer_usage_field", json!([key]))?)
}
fn record(rt: &mut dyn Runtime, text: &Text, stats: Value, stop: Option<&Node>) -> Result<()> {
    rt.call(
        "optimizer_record",
        json!([
            text.json(),
            stats,
            stop.map_or_else(|| "null".into(), |v| v.json(false))
        ]),
    )
    .map(|_| ())
}
fn finish(rt: &mut dyn Runtime, text: &Text, fields: Value) -> Result<Value> {
    rt.call("optimizer_pricing", fields)?;
    rt.call("optimizer_finish", json!([text.json()]))?;
    Ok(json!({"provider_ref":"optimizer_result"}))
}
fn messages(rt: &mut dyn Runtime, data: Node) -> Result<Value> {
    let answer = messages_text(&data)?;
    let empty = object(vec![]);
    let usage = get(&data, "usage")?
        .filter(|u| u.truthy())
        .unwrap_or(&empty);
    rt.call("optimizer_set_usage", json!([usage.json(false)]))?;
    if is(get(&data, "stop_reason")?, "max_tokens") {
        capture_log(rt)?;
        let mut message = Text::from("optimizer response hit max_tokens (");
        append(&mut message, &formatted(rt, "max_tokens")?);
        log(
            rt,
            suffix(
                message,
                ") and was TRUNCATED -- the caller will fail to parse it and act on nothing. Raise TE_OPTIMIZER_MAX_TOKENS.",
            ),
        )?;
    }
    record(rt, &answer, Value::Null, get(&data, "stop_reason")?)?;
    let mut fields = serde_json::Map::new();
    for key in [
        "input_tokens",
        "cache_read_input_tokens",
        "cache_creation_input_tokens",
        "output_tokens",
    ] {
        rt.call("optimizer_capture_usage", json!([key]))?;
        fields.insert(
            key.into(),
            if boolean(rt, key)? {
                json!(["ref", key])
            } else {
                json!(["value", 0])
            },
        );
    }
    finish(rt, &answer, json!(fields))
}
fn responses(rt: &mut dyn Runtime, data: Node) -> Result<Value> {
    let processed = rt.call("optimizer_response_text", json!([data.json(false)]))?;
    let data = parse_wire(&processed["data"])?;
    let answer = match parse_wire(&processed["text"])? {
        Node::String(ref text) => text.clone(),
        other => {
            return Err(Failure::new(
                "AttributeError",
                format!("'{}' object has no attribute 'strip'", other.kind()),
            ));
        }
    };
    rt.call("optimizer_usage_from_response", json!([]))?;
    if !boolean(rt, "usage")? {
        literal(rt, "usage", json!({}))?;
    }
    let usage = get(&data, "usage")?;
    let empty = object(vec![]);
    let usage = usage.filter(|u| u.truthy()).unwrap_or(&empty);
    let details = get(usage, "input_tokens_details")?
        .filter(|v| v.truthy())
        .unwrap_or(&empty);
    let cached = integer(rt, get(details, "cached_tokens")?.unwrap_or(&Node::Null))?;
    let total = integer(rt, get(usage, "input_tokens")?.unwrap_or(&Node::Null))?;
    let input = nonnegative_difference(&total, &cached);
    if is(get(&data, "status")?, "incomplete") || blank(&answer) {
        let details = get(&data, "incomplete_details")?
            .filter(|v| v.truthy())
            .unwrap_or(&empty);
        let reason = get(details, "reason")?.filter(|v| v.truthy());
        capture_log(rt)?;
        let mut message = Text::from("codex optimizer response unusable (");
        append(
            &mut message,
            &reason.map_or_else(
                || {
                    Text::from(if blank(&answer) {
                        "empty response"
                    } else {
                        "unknown"
                    })
                },
                Node::scalar,
            ),
        );
        message = suffix(message, ") at max_output_tokens=");
        append(&mut message, &formatted(rt, "max_tokens")?);
        log(
            rt,
            suffix(
                message,
                ": the caller will act on nothing while the call still bills. Raise TE_OPTIMIZER_MAX_TOKENS.",
            ),
        )?;
    }
    let raw_output = usage_field(rt, "output_tokens")?;
    let output = integer(rt, &raw_output)?;
    let stats = object(vec![
        ("input_tokens", number(input.clone())),
        ("cache_read_input_tokens", number(cached.clone())),
        ("output_tokens", number(output)),
    ]);
    record(rt, &answer, json!(stats.json(false)), get(&data, "status")?)?;
    // The log receives a new dictionary; billing reads output_tokens from the original usage again.
    let raw_output = usage_field(rt, "output_tokens")?;
    let output = integer(rt, &raw_output)?;
    finish(
        rt,
        &answer,
        json!({"input_tokens":["integer",input],"cache_read_input_tokens":["integer",cached],"cache_creation_input_tokens":["value",0],"output_tokens":["integer",output]}),
    )
}
pub fn run(codex: bool, rt: &mut dyn Runtime) -> Result<Value> {
    request(rt, codex)?;
    let Some(data) = exchange(rt, codex)? else {
        // A suppressed decoding error leaves Messages' local unset, but Responses initialises it to None.
        if !codex && truthy(&rt.call("optimizer_suppressed", json!([]))?) {
            return Err(Failure::new(
                "UnboundLocalError",
                "cannot access local variable 'd' where it is not associated with a value",
            ));
        }
        return Ok(json!(["", 0.0]));
    };
    if codex {
        if matches!(data, Node::Null) {
            Ok(json!(["", 0.0]))
        } else {
            responses(rt, data)
        }
    } else {
        messages(rt, data)
    }
}

pub fn copilot(rt: &mut dyn Runtime) -> Result<Value> {
    if !boolean(rt, "binary")? {
        rt.call("cli_binary", json!([]))?;
    }
    rt.call("cli_temp", json!(["neutral", "te_optimizer_cwd_"]))?;
    rt.call("cli_temp", json!(["logdir", "te_optimizer_log_"]))?;
    let mut argv = vec![
        json!(["ref", "binary"]),
        json!(["value", "-p"]),
        json!(["ref", "prompt"]),
        json!(["value", "-C"]),
        json!(["ref", "neutral"]),
    ];
    for item in [
        "--output-format",
        "json",
        "--no-color",
        "--log-level",
        "all",
        "--log-dir",
    ] {
        argv.push(json!(["value", item]));
    }
    argv.push(json!(["ref", "logdir"]));
    argv.extend([
        json!(["value", "--no-custom-instructions"]),
        json!(["value", "--disable-builtin-mcps"]),
    ]);
    for (constant, flag) in [
        ("OPTIMIZER_DENY_TOOLS", "--deny-tool"),
        ("OPTIMIZER_EXCLUDE_TOOLS", "--excluded-tools"),
    ] {
        let count = rt
            .call("cli_tools", json!([constant]))?
            .as_u64()
            .unwrap_or(0);
        for index in 0..count {
            argv.push(json!(["value", flag]));
            argv.push(json!(["tool", constant, index]));
        }
    }
    if boolean(rt, "model")? {
        rt.call("cli_stringify", json!(["model", "model_string"]))?;
        let Node::String(ref value) = argument(rt, "model_string")? else {
            unreachable!()
        };
        if !matches!(value.lossy().to_lowercase().as_str(), "agent" | "copilot") {
            rt.call("cli_stringify", json!(["model", "selected_model"]))?;
            argv.extend([
                json!(["value", "--model"]),
                json!(["ref", "selected_model"]),
            ]);
        }
    }
    let launched: Result<()> = (|| {
        rt.call("cli_capture_run", json!([]))?;
        rt.call("cli_run_env", json!([]))?;
        rt.call("cli_run", json!([argv]))?;
        if !boolean(rt, "stdout")? {
            literal(rt, "stdout", json!(""))?;
        }
        Ok(())
    })();
    // The source's finally removes the neutral directory even when environment setup fails.
    rt.call("cli_remove", json!(["neutral"]))?;
    if let Err(error) = launched {
        if error.is_a("subprocess.TimeoutExpired")
            || error.is_a("builtins.OSError")
            || error.is_a("subprocess.SubprocessError")
        {
            return Ok(json!(["", null]));
        }
        return Err(error);
    }
    let lines = parse_wire(&rt.call("cli_lines", json!([]))?)?;
    let limit = rt
        .call("optimizer_integer_limit", json!([]))?
        .as_u64()
        .unwrap_or(0) as usize;
    let mut answer = text("").json(false);
    for line in iterable(Some(&lines), false)? {
        let Node::String(line) = line else { continue };
        let Ok(event) = python_json::parse_points(line.0.clone(), limit) else {
            continue;
        };
        if is(get(&event, "type")?, "assistant.message") {
            let empty = object(vec![]);
            let data = get(&event, "data")?
                .filter(|v| v.truthy())
                .unwrap_or(&empty);
            if let Some(content) = get(data, "content")?.filter(|v| v.truthy()) {
                answer = content.json(false);
            }
        }
    }
    rt.call("cli_set_text", json!([answer]))?;
    rt.call("cli_capture_sum", json!([]))?;
    rt.call("cli_read_log", json!([]))?;
    rt.call("cli_sum", json!([]))?;
    rt.call("cli_remove", json!(["logdir"]))?;
    let observed = (|| {
        rt.call("cli_capture_record", json!([]))?;
        copy(rt, "observed_usage", "usage")?;
        if !boolean(rt, "observed_usage")? {
            literal(rt, "observed_usage", json!({}))?;
        }
        rt.call("cli_record", json!([])).map(|_| ())
    })();
    if let Err(error) = observed
        && !error.is_a("builtins.Exception")
    {
        return Err(error);
    }
    if boolean(rt, "usage")? {
        rt.call("cli_pricing", json!([]))?;
    } else {
        literal(rt, "effective", Value::Null)?;
    }
    rt.call("cli_finish", json!([]))?;
    Ok(json!({"provider_ref":"optimizer_result"}))
}
