use crate::value::python_string_repr;
use indexmap::IndexMap;
use serde_json::Value;
use std::fmt::Write;

// Python strings retain escaped surrogates and stdin's surrogate-escaped bytes.
#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub(super) struct Text(Vec<u32>);
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
    pub(super) fn record(&self) -> String {
        format!("{{\"command\": {}}}\n", self.json_prefix(400))
    }
    pub(super) fn json(&self) -> String {
        self.json_prefix(self.0.len())
    }
    fn json_prefix(&self, limit: usize) -> String {
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
fn float(value: f64) -> String {
    if value.is_nan() {
        return "nan".into();
    }
    if value.is_infinite() {
        return if value.is_sign_negative() {
            "-inf"
        } else {
            "inf"
        }
        .into();
    }
    if value == 0.0 {
        return if value.is_sign_negative() {
            "-0.0"
        } else {
            "0.0"
        }
        .into();
    }
    let raw = serde_json::Number::from_f64(value).unwrap().to_string();
    let (sign, raw) = raw
        .strip_prefix('-')
        .map_or(("", raw.as_str()), |v| ("-", v));
    let (mantissa, exponent) = raw
        .split_once(['e', 'E'])
        .map_or((raw, 0), |(m, e)| (m, e.parse::<i32>().unwrap()));
    let point = mantissa.find('.').unwrap_or(mantissa.len()) as i32;
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let zeros = digits.len() - digits.trim_start_matches('0').len();
    let exponent = exponent + point - 1 - zeros as i32;
    let digits = digits.trim_start_matches('0').trim_end_matches('0');
    if !(-4..16).contains(&exponent) {
        let tail = if digits.len() > 1 {
            format!(".{}", &digits[1..])
        } else {
            String::new()
        };
        format!(
            "{sign}{}{tail}e{}{:02}",
            &digits[..1],
            if exponent < 0 { "-" } else { "+" },
            exponent.abs()
        )
    } else {
        let point = exponent + 1;
        if point <= 0 {
            format!("{sign}0.{}{}", "0".repeat((-point) as usize), digits)
        } else if point as usize >= digits.len() {
            format!(
                "{sign}{}{}.0",
                digits,
                "0".repeat(point as usize - digits.len())
            )
        } else {
            format!(
                "{sign}{}.{}",
                &digits[..point as usize],
                &digits[point as usize..]
            )
        }
    }
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
                if n.is_i64() || n.is_u64() {
                    Self::Number(n.to_string(), n.as_f64() != Some(0.0))
                } else {
                    let n = n.as_f64().unwrap();
                    Self::Number(float(n), n != 0.0)
                }
            }
        }
    }
}
impl Node {
    fn field(&self, key: &str) -> Option<&Self> {
        match self {
            Self::Dict(items) => items.get(&Text::from(key)),
            _ => None,
        }
    }
    fn truthy(&self) -> bool {
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
                Self::String(v) => out.push_str(&v.json()),
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
                        stack.push(Part::Text(format!("{}: ", k.json())));
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
    fn scalar(&self) -> Text {
        match self {
            Self::String(v) => v.clone(),
            _ => Text::from(self.repr().as_str()),
        }
    }
    pub(super) fn as_command(&self) -> Text {
        match self {
            Self::String(v) => v.clone(),
            Self::List(v) => {
                let mut out = Vec::new();
                for (i, v) in v.iter().enumerate() {
                    if i > 0 {
                        out.push(32);
                    }
                    out.extend(v.scalar().0);
                }
                Text(out)
            }
            _ => Text::default(),
        }
    }
    pub(super) fn find_command(&self, depth: i64) -> Text {
        if depth < 0 {
            return Text::default();
        }
        match self {
            Self::Dict(items) => {
                if let Some(v) = self.field("command") {
                    let got = v.as_command();
                    if !got.0.is_empty() {
                        return got;
                    }
                }
                for value in items.values() {
                    let got = value.find_command(depth - 1);
                    if !got.0.is_empty() {
                        return got;
                    }
                }
            }
            Self::List(items) => {
                for value in items {
                    let got = value.find_command(depth - 1);
                    if !got.0.is_empty() {
                        return got;
                    }
                }
            }
            _ => (),
        }
        Text::default()
    }
    pub(super) fn hook_command(&self) -> Text {
        if !matches!(self, Self::Dict(_)) {
            return Text::default();
        }
        for key in ["tool_input", "toolArgs", "input", "arguments", "params"] {
            if let Some(args) = self.field(key)
                && let Some(command) = args.field("command")
                && command.truthy()
            {
                return command.as_command();
            }
        }
        if let Some(command) = self.field("command")
            && command.truthy()
        {
            return command.as_command();
        }
        self.find_command(4)
    }
}
// Parsing, formatting and destruction use heap stacks so unused deep metadata cannot disable the guard.
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
}
impl Parser {
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
        if !self.take(b'"') {
            return None;
        }
        let mut points = Vec::new();
        loop {
            let c = *self.text.get(self.at)?;
            self.at += 1;
            match c {
                34 => return Some(Text(points)),
                92 => {
                    let escape = *self.text.get(self.at)?;
                    self.at += 1;
                    let value = match escape {
                        34 | 47 | 92 => escape,
                        98 => 8,
                        102 => 12,
                        110 => 10,
                        114 => 13,
                        116 => 9,
                        117 => {
                            let high = self.hex()?;
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
                        _ => return None,
                    };
                    points.push(value);
                }
                0..=31 => return None,
                _ => points.push(c),
            }
        }
    }
    fn key(&mut self) -> Option<Text> {
        self.ws();
        let key = self.string()?;
        self.ws();
        self.take(b':').then_some(key)
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
                            return None;
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
                            return None;
                        }
                        *key = self.key()?;
                        break;
                    }
                }
            }
        }
    }
    fn atom(&mut self) -> Option<Node> {
        let c = *self.text.get(self.at)?;
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
                let first = *self.text.get(self.at)?;
                if !matches!(first, 48..=57) {
                    return None;
                }
                self.at += 1;
                if first != 48 {
                    while self.text.get(self.at).is_some_and(|v| matches!(v, 48..=57)) {
                        self.at += 1;
                    }
                }
                let mut fractional = false;
                if self.take(b'.') {
                    fractional = true;
                    let from = self.at;
                    while self.text.get(self.at).is_some_and(|v| matches!(v, 48..=57)) {
                        self.at += 1;
                    }
                    if from == self.at {
                        return None;
                    }
                }
                if self.take(b'e') || self.take(b'E') {
                    fractional = true;
                    if !self.take(b'+') {
                        self.take(b'-');
                    }
                    let from = self.at;
                    while self.text.get(self.at).is_some_and(|v| matches!(v, 48..=57)) {
                        self.at += 1;
                    }
                    if from == self.at {
                        return None;
                    }
                }
                let raw: String = self.text[start..self.at]
                    .iter()
                    .map(|c| char::from_u32(*c).unwrap())
                    .collect();
                if fractional {
                    let value = raw.parse::<f64>().ok()?;
                    Some(Node::Number(float(value), value != 0.0))
                } else {
                    if self.int_max_digits != 0
                        && raw.len() - usize::from(raw.starts_with('-')) > self.int_max_digits
                    {
                        return None;
                    }
                    let repr = if raw == "-0" { "0".into() } else { raw };
                    let nonzero = repr != "0";
                    Some(Node::Number(repr, nonzero))
                }
            }
            _ => None,
        }
    }
}
pub(super) fn parse(bytes: &[u8], int_max_digits: usize) -> Option<Node> {
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
    let mut parser = Parser {
        text,
        at: 0,
        int_max_digits,
    };
    let value = parser.value()?;
    parser.ws();
    if parser.at != parser.text.len() {
        return None;
    }
    Some(value)
}
pub(super) fn command(bytes: &[u8], int_max_digits: usize) -> Option<Text> {
    parse(bytes, int_max_digits).map(|v| v.hook_command())
}
