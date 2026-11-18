use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use serde_json::ser::Formatter;
use std::io::{self, Write};
use std::sync::LazyLock;

pub fn decimal_integer(text: &str) -> Option<String> {
    static INTEGER: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^[+-]?\p{Nd}+(?:_\p{Nd}+)*$").unwrap());
    static DIGIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\p{Nd}$").unwrap());
    let text = text.trim();
    if !INTEGER.is_match(text) {
        return None;
    }
    let mut normalized = String::new();
    for c in text.chars().filter(|c| *c != '_') {
        if c.is_ascii() {
            normalized.push(c);
            continue;
        }
        let code = c as u32;
        let mut start = code;
        // Decimal digit blocks contain consecutive groups of ten values.
        while char::from_u32(start - 1).is_some_and(|c| DIGIT.is_match(c.encode_utf8(&mut [0; 4])))
        {
            start -= 1;
        }
        normalized.push(char::from(b'0' + ((code - start) % 10) as u8));
    }
    Some(normalized)
}

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

pub(crate) fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::new();
    for chunk in bytes.chunks(3) {
        let value = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            output.push(if i > chunk.len() {
                '='
            } else {
                ALPHABET[((value >> (18 - i * 6)) & 63) as usize] as char
            });
        }
    }
    output
}

pub fn json_text(value: &Value) -> String {
    encode_json(value, true)
}

pub fn json_text_utf8(value: &Value) -> String {
    encode_json(value, false)
}

pub fn json_pretty(value: &Value, sort_keys: bool) -> String {
    json_indented(value, sort_keys, 2)
}

pub fn json_indented(value: &Value, sort_keys: bool, indent: usize) -> String {
    let mut value = value.clone();
    if sort_keys {
        value.sort_all_objects();
    }
    let padding = vec![b' '; indent];
    let mut encoded = Vec::new();
    value
        .serialize(&mut serde_json::Serializer::with_formatter(
            &mut encoded,
            serde_json::ser::PrettyFormatter::with_indent(&padding),
        ))
        .unwrap();
    let text = String::from_utf8(encoded).unwrap();
    let mut bytes = Vec::new();
    JsonFormatter { ascii: true }
        .write_string_fragment(&mut bytes, &text)
        .unwrap();
    String::from_utf8(bytes).unwrap()
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

pub(crate) fn python_repr(value: &Value) -> String {
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
