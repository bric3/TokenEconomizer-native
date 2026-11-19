use crate::value::{python_float, python_string_repr};
use indexmap::IndexMap;
use serde_json::Value;
use std::fmt::Write;

// Python strings retain escaped surrogates and surrogate-escaped UTF-8 bytes.
#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct Text(pub(super) Vec<u32>);
impl From<&str> for Text {
    fn from(value: &str) -> Self {
        Self(value.chars().map(u32::from).collect())
    }
}
impl Text {
    pub(super) fn lossy(&self) -> String {
        self.0
            .iter()
            .map(|c| char::from_u32(*c).unwrap_or('\u{fffd}'))
            .collect()
    }
    fn repr(&self) -> String {
        python_string_repr(self.0.iter().copied())
    }
    pub(super) fn json(&self) -> String {
        self.json_prefix(self.0.len())
    }
    pub(super) fn json_prefix(&self, limit: usize) -> String {
        self.encode_json(limit, true)
    }
    fn encode_json(&self, limit: usize, ascii: bool) -> String {
        let mut out = String::from("\"");
        for point in self.0.iter().take(limit).copied() {
            match point {
                8 => out.push_str("\\b"),
                9 => out.push_str("\\t"),
                10 => out.push_str("\\n"),
                12 => out.push_str("\\f"),
                13 => out.push_str("\\r"),
                34 => out.push_str("\\\""),
                92 => out.push_str("\\\\"),
                32.. if !ascii && char::from_u32(point).is_some() => {
                    out.push(char::from_u32(point).unwrap());
                }
                32..=126 => out.push(char::from_u32(point).unwrap()),
                ..=0xffff => write!(out, "\\u{point:04x}").unwrap(),
                _ => {
                    let value = point - 0x10000;
                    write!(
                        out,
                        "\\u{:04x}\\u{:04x}",
                        0xd800 + (value >> 10),
                        0xdc00 + (value & 1023)
                    )
                    .unwrap();
                }
            }
        }
        out.push('"');
        out
    }
}
#[derive(PartialEq)]
pub(super) enum Node {
    Null,
    Bool(bool),
    Number(String, bool),
    String(Text),
    List(Vec<Node>),
    Dict(IndexMap<Text, Node>),
}
impl From<&Value> for Node {
    fn from(value: &Value) -> Self {
        match value {
            Value::Null => Self::Null,
            Value::Bool(v) => Self::Bool(*v),
            Value::String(v) => Self::String(Text::from(v.as_str())),
            Value::Array(v) => Self::List(v.iter().map(Self::from).collect()),
            Value::Object(v) => Self::Dict(
                v.iter()
                    .map(|(k, v)| (Text::from(k.as_str()), Self::from(v)))
                    .collect(),
            ),
            Value::Number(n) => {
                if !n.is_f64() {
                    Self::Number(n.to_string(), n.as_u64() != Some(0))
                } else {
                    let n = n.as_f64().unwrap();
                    Self::Number(python_float(n), n != 0.0)
                }
            }
        }
    }
}
impl Node {
    pub(super) fn field(&self, key: &str) -> Option<&Self> {
        match self {
            Self::Dict(items) => items.get(&Text::from(key)),
            _ => None,
        }
    }
    pub(super) fn truthy(&self) -> bool {
        match self {
            Self::Null => false,
            Self::Bool(v) => *v,
            Self::Number(_, v) => *v,
            Self::String(v) => !v.0.is_empty(),
            Self::List(v) => !v.is_empty(),
            Self::Dict(v) => !v.is_empty(),
        }
    }
    fn repr(&self) -> String {
        enum Part<'a> {
            Node(&'a Node),
            Key(&'a Text),
            Literal(&'static str),
        }
        let mut stack = vec![Part::Node(self)];
        let mut out = String::new();
        while let Some(part) = stack.pop() {
            let node = match part {
                Part::Literal(text) => {
                    out.push_str(text);
                    continue;
                }
                Part::Key(text) => {
                    out.push_str(&text.repr());
                    continue;
                }
                Part::Node(node) => node,
            };
            match node {
                Self::String(v) => out.push_str(&v.repr()),
                Self::Null => out.push_str("None"),
                Self::Bool(true) => out.push_str("True"),
                Self::Bool(false) => out.push_str("False"),
                Self::Number(v, _) => out.push_str(v),
                Self::List(items) => {
                    out.push('[');
                    stack.push(Part::Literal("]"));
                    for (i, value) in items.iter().enumerate().rev() {
                        stack.push(Part::Node(value));
                        if i > 0 {
                            stack.push(Part::Literal(", "));
                        }
                    }
                }
                Self::Dict(items) => {
                    out.push('{');
                    stack.push(Part::Literal("}"));
                    for (i, (key, value)) in items.iter().enumerate().rev() {
                        stack.push(Part::Node(value));
                        stack.push(Part::Literal(": "));
                        stack.push(Part::Key(key));
                        if i > 0 {
                            stack.push(Part::Literal(", "));
                        }
                    }
                }
            }
        }
        out
    }
    pub(super) fn json(&self, pretty: bool) -> String {
        self.encode_json(pretty, true)
    }
    pub(super) fn json_utf8(&self, pretty: bool) -> String {
        self.encode_json(pretty, false)
    }
    fn encode_json(&self, pretty: bool, ascii: bool) -> String {
        enum Part<'a> {
            Node(&'a Node, usize),
            Text(String),
        }
        let mut stack = vec![Part::Node(self, 0)];
        let mut out = String::new();
        while let Some(part) = stack.pop() {
            let (node, level) = match part {
                Part::Node(node, level) => (node, level),
                Part::Text(text) => {
                    out.push_str(&text);
                    continue;
                }
            };
            match node {
                Self::Null => out.push_str("null"),
                Self::Bool(true) => out.push_str("true"),
                Self::Bool(false) => out.push_str("false"),
                Self::String(v) => out.push_str(&v.encode_json(v.0.len(), ascii)),
                Self::Number(v, _) => out.push_str(match v.as_str() {
                    "nan" => "NaN",
                    "inf" => "Infinity",
                    "-inf" => "-Infinity",
                    _ => v,
                }),
                Self::List(items) => {
                    if items.is_empty() {
                        out.push_str("[]");
                        continue;
                    }
                    out.push('[');
                    if pretty {
                        out.push('\n');
                    }
                    stack.push(Part::Text(if pretty {
                        format!("\n{}]", "  ".repeat(level))
                    } else {
                        String::from("]")
                    }));
                    for (i, v) in items.iter().enumerate().rev() {
                        stack.push(Part::Node(v, level + 1));
                        if pretty {
                            stack.push(Part::Text("  ".repeat(level + 1)));
                        }
                        if i > 0 {
                            stack.push(Part::Text(if pretty {
                                String::from(",\n")
                            } else {
                                String::from(", ")
                            }));
                        }
                    }
                }
                Self::Dict(items) => {
                    if items.is_empty() {
                        out.push_str("{}");
                        continue;
                    }
                    out.push('{');
                    if pretty {
                        out.push('\n');
                    }
                    stack.push(Part::Text(if pretty {
                        format!("\n{}}}", "  ".repeat(level))
                    } else {
                        String::from("}")
                    }));
                    for (i, (k, v)) in items.iter().enumerate().rev() {
                        stack.push(Part::Node(v, level + 1));
                        stack.push(Part::Text(format!("{}: ", k.encode_json(k.0.len(), ascii))));
                        if pretty {
                            stack.push(Part::Text("  ".repeat(level + 1)));
                        }
                        if i > 0 {
                            stack.push(Part::Text(if pretty {
                                String::from(",\n")
                            } else {
                                String::from(", ")
                            }));
                        }
                    }
                }
            }
        }
        out
    }
    pub(super) fn kind(&self) -> &'static str {
        match self {
            Self::Null => "NoneType",
            Self::Bool(_) => "bool",
            Self::String(_) => "str",
            Self::List(_) => "list",
            Self::Dict(_) => "dict",
            Self::Number(v, _) => {
                if v.contains(['.', 'e']) || matches!(v.as_str(), "nan" | "inf" | "-inf") {
                    "float"
                } else {
                    "int"
                }
            }
        }
    }
    pub(super) fn scalar(&self) -> Text {
        match self {
            Self::String(v) => v.clone(),
            _ => Text::from(self.repr().as_str()),
        }
    }
}
// Parsing, formatting and destruction use heap stacks for deeply nested JSON.
impl Drop for Node {
    fn drop(&mut self) {
        fn children(node: &mut Node, pending: &mut Vec<Node>) {
            match node {
                Node::List(items) => pending.append(items),
                Node::Dict(items) => pending.extend(std::mem::take(items).into_values()),
                _ => (),
            }
        }
        let mut pending = Vec::new();
        children(self, &mut pending);
        while let Some(mut node) = pending.pop() {
            children(&mut node, &mut pending);
        }
    }
}
struct Parser {
    text: Vec<u32>,
    at: usize,
    int_max_digits: usize,
    error: Option<ParseError>,
}
impl Parser {
    fn fail<T>(&mut self, message: &'static str, position: usize) -> Option<T> {
        self.error = Some(ParseError::Syntax { message, position });
        None
    }
    fn ws(&mut self) {
        while self
            .text
            .get(self.at)
            .is_some_and(|v| matches!(v, 9 | 10 | 13 | 32))
        {
            self.at += 1;
        }
    }
    fn take(&mut self, c: u8) -> bool {
        if self.text.get(self.at) == Some(&u32::from(c)) {
            self.at += 1;
            true
        } else {
            false
        }
    }
    fn literal(&mut self, value: &str) -> bool {
        let end = self.at + value.len();
        if self
            .text
            .get(self.at..end)
            .is_some_and(|s| s.iter().copied().eq(value.bytes().map(u32::from)))
        {
            self.at = end;
            true
        } else {
            false
        }
    }
    fn hex(&mut self) -> Option<u32> {
        let mut value = 0;
        for _ in 0..4 {
            value = value * 16 + char::from_u32(*self.text.get(self.at)?)?.to_digit(16)?;
            self.at += 1;
        }
        Some(value)
    }
    fn string(&mut self) -> Option<Text> {
        let start = self.at;
        if !self.take(b'"') {
            return None;
        }
        let mut points = Vec::new();
        loop {
            let Some(&c) = self.text.get(self.at) else {
                return self.fail("Unterminated string starting at", start);
            };
            self.at += 1;
            match c {
                34 => return Some(Text(points)),
                92 => {
                    let Some(&escape) = self.text.get(self.at) else {
                        return self.fail("Unterminated string starting at", start);
                    };
                    self.at += 1;
                    let value = match escape {
                        34 | 47 | 92 => escape,
                        98 => 8,
                        102 => 12,
                        110 => 10,
                        114 => 13,
                        116 => 9,
                        117 => {
                            let unicode_position = self.at - 1;
                            let Some(high) = self.hex() else {
                                return self.fail("Invalid \\uXXXX escape", unicode_position);
                            };
                            if (0xd800..=0xdbff).contains(&high) {
                                let saved = self.at;
                                if self.literal("\\u")
                                    && let Some(low) = self.hex()
                                    && (0xdc00..=0xdfff).contains(&low)
                                {
                                    0x10000 + ((high - 0xd800) << 10) + (low - 0xdc00)
                                } else {
                                    self.at = saved;
                                    high
                                }
                            } else {
                                high
                            }
                        }
                        _ => return self.fail("Invalid \\escape", self.at - 2),
                    };
                    points.push(value);
                }
                0..=31 => return self.fail("Invalid control character at", self.at - 1),
                _ => points.push(c),
            }
        }
    }
    fn key(&mut self) -> Option<Text> {
        self.ws();
        if self.text.get(self.at) != Some(&34) {
            return self.fail("Expecting property name enclosed in double quotes", self.at);
        }
        let key = self.string()?;
        self.ws();
        if !self.take(b':') {
            return self.fail("Expecting ':' delimiter", self.at);
        }
        Some(key)
    }
    fn value(&mut self) -> Option<Node> {
        enum Frame {
            List(Vec<Node>),
            Dict(IndexMap<Text, Node>, Text),
        }
        let mut stack = Vec::new();
        loop {
            self.ws();
            let mut node = if self.take(b'[') {
                self.ws();
                if self.take(b']') {
                    Node::List(Vec::new())
                } else {
                    stack.push(Frame::List(Vec::new()));
                    continue;
                }
            } else if self.take(b'{') {
                self.ws();
                if self.take(b'}') {
                    Node::Dict(IndexMap::new())
                } else {
                    stack.push(Frame::Dict(IndexMap::new(), self.key()?));
                    continue;
                }
            } else {
                self.atom()?
            };
            loop {
                match stack.last_mut() {
                    None => return Some(node),
                    Some(Frame::List(items)) => {
                        items.push(node);
                        self.ws();
                        if self.take(b']') {
                            let Frame::List(items) = stack.pop()? else {
                                unreachable!()
                            };
                            node = Node::List(items);
                            continue;
                        }
                        if !self.take(b',') {
                            return self.fail("Expecting ',' delimiter", self.at);
                        }
                        let comma = self.at - 1;
                        self.ws();
                        if self.text.get(self.at) == Some(&93) {
                            return self.fail("Illegal trailing comma before end of array", comma);
                        }
                        break;
                    }
                    Some(Frame::Dict(items, key)) => {
                        items.insert(std::mem::take(key), node);
                        self.ws();
                        if self.take(b'}') {
                            let Frame::Dict(items, _) = stack.pop()? else {
                                unreachable!()
                            };
                            node = Node::Dict(items);
                            continue;
                        }
                        if !self.take(b',') {
                            return self.fail("Expecting ',' delimiter", self.at);
                        }
                        let comma = self.at - 1;
                        self.ws();
                        if self.text.get(self.at) == Some(&125) {
                            return self.fail("Illegal trailing comma before end of object", comma);
                        }
                        *key = self.key()?;
                        break;
                    }
                }
            }
        }
    }
    fn atom(&mut self) -> Option<Node> {
        let Some(&c) = self.text.get(self.at) else {
            return self.fail("Expecting value", self.at);
        };
        match c {
            34 => self.string().map(Node::String),
            110 if self.literal("null") => Some(Node::Null),
            116 if self.literal("true") => Some(Node::Bool(true)),
            102 if self.literal("false") => Some(Node::Bool(false)),
            78 if self.literal("NaN") => Some(Node::Number("nan".into(), true)),
            73 if self.literal("Infinity") => Some(Node::Number("inf".into(), true)),
            45 if self.literal("-Infinity") => Some(Node::Number("-inf".into(), true)),
            45 | 48..=57 => {
                let start = self.at;
                self.take(b'-');
                let Some(&first) = self.text.get(self.at) else {
                    return self.fail("Expecting value", start);
                };
                if !matches!(first, 48..=57) {
                    return self.fail("Expecting value", start);
                }
                self.at += 1;
                if first != 48 {
                    while self.text.get(self.at).is_some_and(|v| matches!(v, 48..=57)) {
                        self.at += 1;
                    }
                }
                let mut fractional = false;
                let decimal_position = self.at;
                if self.take(b'.') {
                    let from = self.at;
                    while self.text.get(self.at).is_some_and(|v| matches!(v, 48..=57)) {
                        self.at += 1;
                    }
                    if from == self.at {
                        self.at = decimal_position;
                    } else {
                        fractional = true;
                    }
                }
                let exponent_position = self.at;
                if self.take(b'e') || self.take(b'E') {
                    if !self.take(b'+') {
                        self.take(b'-');
                    }
                    let from = self.at;
                    while self.text.get(self.at).is_some_and(|v| matches!(v, 48..=57)) {
                        self.at += 1;
                    }
                    if from == self.at {
                        self.at = exponent_position;
                    } else {
                        fractional = true;
                    }
                }
                let raw: String = self.text[start..self.at]
                    .iter()
                    .map(|c| char::from_u32(*c).unwrap())
                    .collect();
                if fractional {
                    let value = raw.parse::<f64>().ok()?;
                    Some(Node::Number(python_float(value), value != 0.0))
                } else {
                    let digits = raw.len() - usize::from(raw.starts_with('-'));
                    if self.int_max_digits != 0 && digits > self.int_max_digits {
                        self.error = Some(ParseError::IntegerLimit {
                            limit: self.int_max_digits,
                            digits,
                        });
                        return None;
                    }
                    let repr = if raw == "-0" { "0".into() } else { raw };
                    let nonzero = repr != "0";
                    Some(Node::Number(repr, nonzero))
                }
            }
            _ => self.fail("Expecting value", self.at),
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub(super) enum ParseError {
    Syntax {
        message: &'static str,
        position: usize,
    },
    IntegerLimit {
        limit: usize,
        digits: usize,
    },
}
impl ParseError {
    pub(super) fn message(self, document: &str) -> String {
        match self {
            Self::Syntax { message, position } => {
                let mut line = 1_usize;
                let mut column = 1_usize;
                for point in document.chars().take(position) {
                    if point == '\n' {
                        line += 1;
                        column = 1;
                    } else {
                        column += 1;
                    }
                }
                format!("{message}: line {line} column {column} (char {position})")
            }
            Self::IntegerLimit { limit, digits } => format!(
                "Exceeds the limit ({limit} digits) for integer string conversion: value has {digits} digits; use sys.set_int_max_str_digits() to increase the limit"
            ),
        }
    }
}
pub(super) fn parse(bytes: &[u8], int_max_digits: usize) -> Option<Node> {
    parse_result(bytes, int_max_digits).ok()
}
pub(super) fn parse_result(bytes: &[u8], int_max_digits: usize) -> Result<Node, ParseError> {
    let mut text = Vec::new();
    let mut rest = bytes;
    while !rest.is_empty() {
        match std::str::from_utf8(rest) {
            Ok(valid) => {
                text.extend(valid.chars().map(u32::from));
                break;
            }
            Err(error) => {
                let n = error.valid_up_to();
                text.extend(
                    std::str::from_utf8(&rest[..n])
                        .unwrap()
                        .chars()
                        .map(u32::from),
                );
                let end = n + error.error_len().unwrap_or(rest.len() - n);
                text.extend(rest[n..end].iter().map(|b| 0xdc00 + u32::from(*b)));
                rest = &rest[end..];
            }
        }
    }
    parse_points(text, int_max_digits)
}
pub(super) fn parse_points(text: Vec<u32>, int_max_digits: usize) -> Result<Node, ParseError> {
    let mut parser = Parser {
        text,
        at: 0,
        int_max_digits,
        error: None,
    };
    if parser.text.first() == Some(&0xfeff) {
        return Err(ParseError::Syntax {
            message: "Unexpected UTF-8 BOM (decode using utf-8-sig)",
            position: 0,
        });
    }
    let value = parser.value().ok_or_else(|| {
        parser.error.take().unwrap_or(ParseError::Syntax {
            message: "Expecting value",
            position: parser.at,
        })
    })?;
    parser.ws();
    if parser.at != parser.text.len() {
        return Err(ParseError::Syntax {
            message: "Extra data",
            position: parser.at,
        });
    }
    Ok(value)
}
