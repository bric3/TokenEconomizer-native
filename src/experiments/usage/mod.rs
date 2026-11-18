pub mod claude;
pub mod codex;
pub mod copilot;

#[cfg(test)]
mod tests;

use crate::value::{decimal_integer, number, python_repr, truthy};
use serde_json::{Map, Value, json};
use std::sync::LazyLock;

// shortcut: accounting integers must fit JSON's i64/u64 range; complete numeric parity before release.

fn kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
        Value::Number(_) if is_int(value) => "int",
        Value::Number(_) => "float",
    }
}
fn is_int(value: &Value) -> bool {
    matches!(value, Value::Bool(_)) || value.as_i64().is_some() || value.as_u64().is_some()
}
fn is_numeric(value: &Value) -> bool {
    value.is_number() || value.is_boolean()
}
fn integer(value: &Value) -> Option<i128> {
    match value {
        Value::Bool(v) => Some(i128::from(*v)),
        _ => value
            .as_i64()
            .map(i128::from)
            .or_else(|| value.as_u64().map(i128::from)),
    }
}
fn integer_value(value: i128) -> Result<Value, String> {
    i64::try_from(value)
        .map(|v| json!(v))
        .or_else(|_| u64::try_from(value).map(|v| json!(v)))
        .map_err(|_| "OverflowError: accounting integer exceeds the native JSON range".into())
}
fn int(value: &Value) -> Result<Value, String> {
    if !truthy(value) {
        return Ok(json!(0));
    }
    if let Some(n) = integer(value) {
        return integer_value(n);
    }
    if let Some(n) = value.as_f64() {
        return integer_value(n.trunc() as i128);
    }
    if let Some(s) = value.as_str() {
        return decimal_integer(s)
            .and_then(|v| v.parse::<i128>().ok())
            .ok_or_else(|| {
                format!(
                    "ValueError: invalid literal for int() with base 10: {}",
                    python_repr(value)
                )
            })
            .and_then(integer_value);
    }
    Err(format!(
        "TypeError: int() argument must be a string, a bytes-like object or a real number, not '{}'",
        kind(value)
    ))
}
fn arithmetic(a: &Value, b: &Value, subtract: bool) -> Result<Value, String> {
    if let (Some(a), Some(b)) = (integer(a), integer(b)) {
        return integer_value(if subtract { a - b } else { a + b });
    }
    if let (Some(a), Some(b)) = (number(a), number(b)) {
        return Ok(json!(if subtract { a - b } else { a + b }));
    }
    if !subtract {
        if let (Some(a), Some(b)) = (a.as_str(), b.as_str()) {
            return Ok(json!(format!("{a}{b}")));
        }
        if let (Some(a), Some(b)) = (a.as_array(), b.as_array()) {
            return Ok(json!(a.iter().chain(b).cloned().collect::<Vec<_>>()));
        }
        if a.is_string() || a.is_array() {
            return Err(format!(
                "TypeError: can only concatenate {} (not \"{}\") to {}",
                kind(a),
                kind(b),
                kind(a)
            ));
        }
    }
    Err(format!(
        "TypeError: unsupported operand type(s) for {}: '{}' and '{}'",
        if subtract { "-" } else { "+" },
        kind(a),
        kind(b)
    ))
}
fn add(a: &Value, b: &Value) -> Result<Value, String> {
    arithmetic(a, b, false)
}
fn subtract(a: &Value, b: &Value) -> Result<Value, String> {
    arithmetic(a, b, true)
}
fn nonnegative(value: Value) -> Value {
    if number(&value).is_some_and(|v| v < 0.0) {
        json!(0)
    } else {
        value
    }
}
fn fields<'a>(value: &'a Value, operation: &str) -> Result<&'a Map<String, Value>, String> {
    value.as_object().ok_or_else(|| {
        format!(
            "AttributeError: '{}' object has no attribute '{operation}'",
            kind(value)
        )
    })
}
fn fields_or_empty(value: &Value) -> Result<&Map<String, Value>, String> {
    static EMPTY: LazyLock<Map<String, Value>> = LazyLock::new(Map::new);
    if truthy(value) {
        fields(value, "get")
    } else {
        Ok(&EMPTY)
    }
}
fn get<'a>(map: &'a Map<String, Value>, key: &str) -> &'a Value {
    map.get(key).unwrap_or(&Value::Null)
}
fn or_zero(value: &Value) -> Value {
    if truthy(value) {
        value.clone()
    } else {
        json!(0)
    }
}
fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.split([
        '\n', '\r', '\u{b}', '\u{c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2028}',
        '\u{2029}',
    ])
    .map(|line| line.trim_matches(|c: char| c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}')))
    .filter(|line| !line.is_empty())
}
fn unhashable(value: &Value, operation: &str) -> String {
    format!(
        "TypeError: cannot use '{}' as a {operation} (unhashable type: '{}')",
        kind(value),
        kind(value)
    )
}
fn id_key(value: &Value) -> Result<String, String> {
    if value.is_array() || value.is_object() {
        return Err(unhashable(value, "dict key"));
    }
    if let Some(n) = integer(value) {
        return Ok(n.to_string());
    }
    if let Some(n) = value
        .as_f64()
        .filter(|n| n.fract() == 0.0 && *n >= i128::MIN as f64 && *n < i128::MAX as f64)
    {
        return Ok((n as i128).to_string());
    }
    Ok(value.to_string())
}
