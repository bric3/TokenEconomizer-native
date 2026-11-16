use serde::Serialize;
use serde_json::Value;
use std::io::{self, Write};

struct JsonFormatter {
    ascii: bool,
}

impl serde_json::ser::Formatter for JsonFormatter {
    fn begin_array_value<W: ?Sized + Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        if first {
            Ok(())
        } else {
            writer.write_all(b", ")
        }
    }

    fn begin_object_key<W: ?Sized + Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        if first {
            Ok(())
        } else {
            writer.write_all(b", ")
        }
    }

    fn begin_object_value<W: ?Sized + Write>(&mut self, writer: &mut W) -> io::Result<()> {
        writer.write_all(b": ")
    }

    fn write_string_fragment<W: ?Sized + Write>(
        &mut self,
        writer: &mut W,
        fragment: &str,
    ) -> io::Result<()> {
        if !self.ascii {
            return writer.write_all(fragment.as_bytes());
        }
        for c in fragment.chars() {
            if c <= '~' {
                writer.write_all(&[c as u8])?;
            } else {
                for unit in c.encode_utf16(&mut [0; 2]) {
                    write!(writer, "\\u{unit:04x}")?;
                }
            }
        }
        Ok(())
    }
}

pub fn json_text(value: &Value) -> String {
    encode_json(value, true)
}

pub fn json_text_utf8(value: &Value) -> String {
    encode_json(value, false)
}

fn encode_json(value: &Value, ascii: bool) -> String {
    let mut bytes = Vec::new();
    value
        .serialize(&mut serde_json::Serializer::with_formatter(
            &mut bytes,
            JsonFormatter { ascii },
        ))
        .unwrap();
    String::from_utf8(bytes).unwrap()
}

pub fn scalar_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::Array(_) | Value::Object(_) => python_repr(value),
        _ => value.to_string(),
    }
}

fn python_repr(value: &Value) -> String {
    match value {
        Value::String(text) => {
            let quote = if text.contains('\'') && !text.contains('"') {
                '"'
            } else {
                '\''
            };
            let mut out = quote.to_string();
            for c in text.chars() {
                if c == quote || c == '\\' {
                    out.push('\\');
                    out.push(c);
                } else if c == '\'' || c == '"' {
                    out.push(c);
                } else {
                    let escaped = c.escape_debug().to_string();
                    if escaped == "\\0" {
                        out.push_str("\\x00");
                    } else if escaped.starts_with("\\u{") {
                        use std::fmt::Write;
                        match c as u32 {
                            code @ ..=0xff => write!(out, "\\x{code:02x}").unwrap(),
                            code @ ..=0xffff => write!(out, "\\u{code:04x}").unwrap(),
                            code => write!(out, "\\U{code:08x}").unwrap(),
                        }
                    } else {
                        out.push_str(&escaped);
                    }
                }
            }
            out.push(quote);
            out
        }
        Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(python_repr)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Object(values) => format!(
            "{{{}}}",
            values
                .iter()
                .map(|(key, value)| format!(
                    "{}: {}",
                    python_repr(&Value::String(key.clone())),
                    python_repr(value)
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        scalar => scalar_text(scalar),
    }
}

pub fn round_decimal(value: f64, precision: usize) -> f64 {
    format!("{value:.precision$}").parse().unwrap()
}

pub fn count(value: &Value, field: &str) -> Result<f64, String> {
    integer(value).ok_or_else(|| format!("invalid {field} count"))
}

pub fn float_or_zero(value: &Value) -> Result<f64, String> {
    if !truthy(value) {
        return Ok(0.0);
    }
    if let Some(number) = number(value) {
        return Ok(number);
    }
    value
        .as_str()
        .and_then(|s| s.trim().parse().ok())
        .ok_or_else(|| "expected number".into())
}

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
    int_value(value)
}

pub fn int_value(value: &Value) -> Option<f64> {
    match value {
        Value::String(s) => s.trim().parse::<i128>().ok().map(|n| n as f64),
        _ => number(value).filter(|n| n.is_finite()).map(f64::trunc),
    }
}
