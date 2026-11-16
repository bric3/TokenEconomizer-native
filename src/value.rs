use serde_json::Value;

pub fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|n| n != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

pub fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Bool(b) => Some(u8::from(*b) as f64),
        _ => value.as_f64(),
    }
}

pub fn integer(value: &Value) -> Option<f64> {
    if !truthy(value) {
        return Some(0.0);
    }
    match value {
        Value::String(s) => s.trim().parse::<i128>().ok().map(|n| n as f64),
        _ => number(value).filter(|n| n.is_finite()).map(f64::trunc),
    }
}
