use serde_json::{Map, Value};

pub fn space(c: char) -> bool {
    matches!(c, '\u{0009}'..='\u{000d}' | ' ' | '\u{00a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}
pub fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|v| v != 0.0),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}
pub fn disabled(env: &Map<String, Value>) -> bool {
    ["TE_SIDECALL", "TE_DISABLE"].iter().any(|key| {
        env.get(*key).and_then(Value::as_str).is_some_and(|v| {
            ["1", "true", "yes", "on"].contains(&v.trim_matches(space).to_lowercase().as_str())
        })
    })
}
pub fn nullish<'a>(value: Option<&'a Value>, fallback: &'a Value) -> &'a Value {
    value.filter(|v| !v.is_null()).unwrap_or(fallback)
}
pub fn string(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => {
            let n = n.as_f64().expect("JSON number fits f64");
            if n == 0.0 {
                "0".into()
            } else if n.abs() >= 1e21 {
                format!("{n:e}").replacen('e', "e+", 1)
            } else if n.abs() < 1e-6 {
                format!("{n:e}")
            } else {
                n.to_string()
            }
        }
        Value::Array(values) => values
            .iter()
            .map(|v| {
                if v.is_null() {
                    String::new()
                } else {
                    string(v)
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".into(),
    }
}
