use regex::Regex;
use regex_syntax::hir::{Class, ClassUnicode, ClassUnicodeRange, HirKind};
use serde::Serialize;
use serde_json::Value;
use serde_json::ser::Formatter;
use std::io::{self, Write};
use std::sync::LazyLock;

pub(crate) fn normalize_decimal_digits(text: &str) -> String {
    static DIGITS: LazyLock<ClassUnicode> = LazyLock::new(|| {
        let hir = regex_syntax::Parser::new().parse(r"\d").unwrap();
        let HirKind::Class(Class::Unicode(mut class)) = hir.into_kind() else {
            unreachable!("expected decimal digit class");
        };
        class.union(&ClassUnicode::new(
            crate::unicode17::DIGIT_ADDED
                .iter()
                .map(|&(first, last)| ClassUnicodeRange::new(first, last)),
        ));
        class
    });
    text.chars()
        .map(|c| {
            if c.is_ascii() {
                return c;
            }
            let ranges = DIGITS.ranges();
            let index = ranges.partition_point(|range| range.end() < c);
            ranges
                .get(index)
                .filter(|range| range.start() <= c)
                // Contiguous decimal blocks contain successive groups of ten digits.
                .map_or(c, |range| {
                    char::from(b'0' + ((u32::from(c) - u32::from(range.start())) % 10) as u8)
                })
        })
        .collect()
}

pub(crate) fn python_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                "int"
            } else {
                "float"
            }
        }
    }
}

pub fn decimal_integer(text: &str) -> Option<String> {
    static INTEGER: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^[+-]?[0-9]+(?:_[0-9]+)*$").unwrap());
    let text = normalize_decimal_digits(text.trim());
    if !INTEGER.is_match(&text) {
        return None;
    }
    Some(text.chars().filter(|c| *c != '_').collect())
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
        Value::String(text) => python_string_repr(text.chars().map(u32::from)),
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

pub(crate) fn python_string_repr(points: impl Iterator<Item = u32> + Clone) -> String {
    use std::fmt::Write;
    let quote = if points.clone().any(|c| c == 39) && !points.clone().any(|c| c == 34) {
        '\"'
    } else {
        '\''
    };
    let mut out = quote.to_string();
    for point in points {
        let Some(c) = char::from_u32(point) else {
            write!(out, "\\u{point:04x}").unwrap();
            continue;
        };
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
                match point {
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

pub fn round_decimal(value: f64, precision: usize) -> f64 {
    format!("{value:.precision$}").parse().unwrap()
}

// Python's float sum retains low-order terms across additions of different magnitudes.
pub(crate) fn sum_floats(values: impl IntoIterator<Item = f64>) -> f64 {
    let (mut total, mut correction) = (0.0_f64, 0.0);
    for value in values {
        let next = total + value;
        correction += if total.abs() >= value.abs() {
            (total - next) + value
        } else {
            (value - next) + total
        };
        total = next;
    }
    if correction.is_finite() {
        total + correction
    } else {
        total
    }
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
