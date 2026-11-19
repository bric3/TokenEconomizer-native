use pcre2::bytes::{Regex as Engine, RegexBuilder};
use regex_syntax::{
    ParserBuilder,
    hir::{Class, ClassUnicode, ClassUnicodeRange, HirKind},
    utf8::Utf8Sequences,
};
use std::{collections::BTreeMap, sync::LazyLock};

pub struct Pattern {
    source: String,
    compiled: Engine,
}
impl Pattern {
    pub fn new(source: &str) -> Result<Option<Self>, String> {
        let mut compiler = Compiler::new(source);
        let valid = compiler.compile().is_some();
        if !valid {
            return Ok(None);
        }
        if compiler.incompatible_flags {
            return Err("ASCII and UNICODE flags are incompatible".into());
        }
        let Compiler {
            unicode,
            ascii_refs,
            ..
        } = compiler;
        // ASCII caseless backreferences compare bytes; encoded atoms retain code-point boundaries.
        let compiled = if ascii_refs {
            let mut compiler = Compiler::new(source);
            compiler.encode_bytes = true;
            compiler.compile();
            RegexBuilder::new().build(&format!(r"(?![\x80-\xbf]){}", compiler.bytes))
        } else {
            RegexBuilder::new().utf(true).ucp(true).build(&unicode)
        }
        .ok();
        Ok(compiled.map(|compiled| Self {
            source: source.into(),
            compiled,
        }))
    }
    pub fn as_str(&self) -> &str {
        &self.source
    }
    pub fn is_match(&self, text: &str) -> Result<bool, String> {
        self.compiled
            .is_match(text.as_bytes())
            .map_err(|error| error.to_string())
    }
}

#[derive(Clone, Copy, Default)]
struct Mode {
    ascii: bool,
    casei: bool,
    multiline: bool,
    dotall: bool,
    verbose: bool,
}

struct Compiler<'a> {
    source: &'a str,
    position: usize,
    mode: Mode,
    stack: Vec<(Mode, Option<usize>)>,
    captures: usize,
    names: BTreeMap<String, usize>,
    unicode: String,
    bytes: String,
    start: bool,
    global_ascii: Option<bool>,
    ascii_refs: bool,
    unicode_refs: bool,
    incompatible_flags: bool,
    encode_bytes: bool,
}
impl<'a> Compiler<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            position: 0,
            mode: Mode::default(),
            stack: Vec::new(),
            captures: 0,
            names: BTreeMap::new(),
            unicode: String::new(),
            bytes: String::new(),
            start: true,
            global_ascii: None,
            ascii_refs: false,
            unicode_refs: false,
            incompatible_flags: false,
            encode_bytes: false,
        }
    }
    fn raw(&mut self, text: &str) {
        self.unicode.push_str(text);
        if self.encode_bytes {
            self.bytes.push_str(text);
        }
    }
    fn next(&mut self) -> Option<char> {
        let value = self.source[self.position..].chars().next()?;
        self.position += value.len_utf8();
        Some(value)
    }
    fn peek(&self) -> Option<char> {
        self.source[self.position..].chars().next()
    }
    fn literal(&self, first: char, last: char) -> Option<ClassUnicode> {
        if first > last {
            return None;
        }
        let mut class = ClassUnicode::new([ClassUnicodeRange::new(first, last)]);
        if self.mode.casei {
            if self.mode.ascii {
                for upper in b'A'..=b'Z' {
                    let lower = upper + 32;
                    if first <= char::from(upper) && char::from(upper) <= last {
                        class.push(ClassUnicodeRange::new(char::from(lower), char::from(lower)));
                    }
                    if first <= char::from(lower) && char::from(lower) <= last {
                        class.push(ClassUnicodeRange::new(char::from(upper), char::from(upper)));
                    }
                }
            } else {
                let turkish = ['I', 'i', 'İ', 'ı']
                    .into_iter()
                    .any(|c| first <= c && c <= last);
                class.case_fold_simple();
                if turkish {
                    for c in ['I', 'i', 'İ', 'ı'] {
                        class.push(ClassUnicodeRange::new(c, c));
                    }
                }
            }
        }
        Some(class)
    }
    fn atom(&mut self, class: &ClassUnicode) {
        let unicode = unicode_class(class);
        self.unicode.push_str(&format!("(?-i:{unicode})"));
        if self.encode_bytes {
            self.bytes.push_str(&format!("(?-i:{})", byte_class(class)));
        }
    }
    fn reference(&mut self, text: &str) {
        if self.mode.casei {
            if self.mode.ascii {
                self.ascii_refs = true;
            } else {
                self.unicode_refs = true;
            }
        }
        self.raw(text);
    }
    fn closed_reference(&self, id: usize) -> Option<String> {
        (id > 0 && id <= self.captures && !self.stack.iter().any(|(_, open)| *open == Some(id)))
            .then(|| format!(r"\g{{{id}}}"))
    }
    fn escape(&mut self, in_class: bool) -> Option<Escape> {
        let start = self.position - 1;
        let c = self.next()?;
        if matches!(c, 'w' | 'W' | 's' | 'S' | 'd' | 'D') {
            return Some(Escape::Class(category(c, self.mode.ascii)));
        }
        let literal = match c {
            'a' => '\x07',
            'f' => '\x0c',
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            'v' => '\x0b',
            'b' if in_class => '\x08',
            'x' | 'u' | 'U' => {
                let count = match c {
                    'x' => 2,
                    'u' => 4,
                    _ => 8,
                };
                let end = self.position.checked_add(count)?;
                let digits = self.source.get(self.position..end)?;
                let value = u32::from_str_radix(digits, 16).ok()?;
                self.position = end;
                char::from_u32(value)?
            }
            '0'..='9' => {
                let tail = &self.source[start + 1..];
                let octal = c == '0'
                    || tail.as_bytes().iter().take(3).count() == 3
                        && tail.as_bytes()[..3]
                            .iter()
                            .all(|b| matches!(b, b'0'..=b'7'));
                if in_class || octal {
                    let count = tail
                        .bytes()
                        .take(3)
                        .take_while(|b| matches!(b, b'0'..=b'7'))
                        .count();
                    if count == 0 {
                        return None;
                    }
                    let value = u32::from_str_radix(&tail[..count], 8).ok()?;
                    if value > 255 {
                        return None;
                    }
                    self.position = start + 1 + count;
                    char::from_u32(value)?
                } else {
                    if self.peek().is_some_and(|c| c.is_ascii_digit()) {
                        self.next();
                    }
                    let id = self.source[start + 1..self.position].parse().ok()?;
                    return Some(Escape::Raw(self.closed_reference(id)?, true));
                }
            }
            'b' | 'B' if !in_class => {
                let word = category('w', self.mode.ascii);
                let u = boundary(&unicode_class(&word), c == 'B');
                self.unicode.push_str(&u);
                if self.encode_bytes {
                    self.bytes.push_str(&boundary(&byte_class(&word), c == 'B'));
                }
                return Some(Escape::Done);
            }
            'A' if !in_class => return Some(Escape::Raw(r"\A".into(), false)),
            'Z' | 'z' if !in_class => return Some(Escape::Raw(r"\z".into(), false)),
            // shortcut: Unicode character names need a name table before the regex audit closes.
            c if c.is_ascii_alphabetic() => return None,
            c => c,
        };
        Some(Escape::Char(literal))
    }
    fn class_item(&mut self) -> Option<Escape> {
        match self.next()? {
            '\\' => self.escape(true),
            c => Some(Escape::Char(c)),
        }
    }
    fn class(&mut self) -> Option<()> {
        let negate = self.peek() == Some('^');
        if negate {
            self.next();
        }
        let mut result = ClassUnicode::new([]);
        let mut first = true;
        loop {
            if self.peek() == Some(']') && !first {
                self.next();
                break;
            }
            let item = self.class_item()?;
            first = false;
            if self.peek() == Some('-') {
                self.next();
                if self.peek() == Some(']') {
                    union(&mut result, item, self)?;
                    result.union(&self.literal('-', '-')?);
                    continue;
                }
                let end = self.class_item()?;
                let (Escape::Char(a), Escape::Char(b)) = (item, end) else {
                    return None;
                };
                result.union(&self.literal(a, b)?);
            } else {
                union(&mut result, item, self)?;
            }
        }
        if negate {
            result.negate();
        }
        self.atom(&result);
        Some(())
    }
    fn group(&mut self) -> Option<()> {
        if !self.source[self.position..].starts_with('?') {
            self.captures += 1;
            self.stack.push((self.mode, Some(self.captures)));
            self.raw("(");
            return Some(());
        }
        let start = self.position - 1;
        let rest = &self.source[self.position..];
        if rest.starts_with("?#") {
            let end = self.source[self.position..].find(')')? + self.position + 1;
            self.raw(&self.source[start..end]);
            self.position = end;
            return Some(());
        }
        if rest.starts_with("?P=") {
            let end = self.source[self.position..].find(')')? + self.position + 1;
            let name = &self.source[self.position + 3..end - 1];
            self.reference(&self.closed_reference(*self.names.get(name)?)?);
            self.position = end;
            return Some(());
        }
        let header = if rest.starts_with("?P<") {
            let end = self.source[self.position..].find('>')? + self.position + 1;
            let name = &self.source[self.position + 3..end - 1];
            static NAME: LazyLock<regex::Regex> = LazyLock::new(|| {
                regex::Regex::new(r"\A[_\p{XID_Start}]\p{XID_Continue}*\z").unwrap()
            });
            if !NAME.is_match(name) || self.names.contains_key(name) {
                return None;
            }
            self.captures += 1;
            self.names.insert(name.into(), self.captures);
            // Names remain in source metadata; engine references use validated group numbers.
            self.stack.push((self.mode, Some(self.captures)));
            self.raw("(");
            self.position = end;
            return Some(());
        } else if rest.starts_with("?(") {
            let end = self.source[self.position..].find(')')? + self.position + 1;
            let condition = &self.source[self.position + 2..end - 1];
            if !condition.bytes().all(|c| c.is_ascii_digit()) {
                let id = *self.names.get(condition)?;
                self.stack.push((self.mode, None));
                self.raw(&format!("(?({id})"));
                self.position = end;
                return Some(());
            }
            Some(end)
        } else {
            ["?:", "?=", "?!", "?<=", "?<!", "?>"]
                .into_iter()
                .find(|prefix| rest.starts_with(prefix))
                .map(|prefix| self.position + prefix.len())
        };
        if let Some(end) = header {
            self.stack.push((self.mode, None));
            self.raw(&self.source[start..end]);
            self.position = end;
            return Some(());
        }
        self.next();
        let mut mode = self.mode;
        let mut negative = false;
        let mut off = false;
        let mut flags = 0;
        let mut kind = None;
        let mut engine = String::new();
        let scoped = loop {
            let c = self.next()?;
            match c {
                ')' | ':' => break c == ':',
                '-' if !negative => {
                    negative = true;
                    engine.push(c);
                    continue;
                }
                'a' | 'u' if !negative => {
                    let ascii = c == 'a';
                    if kind.is_some_and(|value| value != ascii) {
                        return None;
                    }
                    kind = Some(ascii);
                    mode.ascii = ascii;
                }
                'i' | 'm' | 's' | 'x' => {
                    match c {
                        'i' => mode.casei = !negative,
                        'm' => mode.multiline = !negative,
                        's' => mode.dotall = !negative,
                        _ => mode.verbose = !negative,
                    }
                    engine.push(c);
                    if negative {
                        off = true;
                    }
                }
                _ => return None,
            }
            flags += 1;
        };
        if flags == 0 || negative && !off || !scoped && negative {
            return None;
        }
        if scoped {
            self.stack.push((self.mode, None));
            self.raw(&format!("(?{engine}:"));
        } else {
            if !self.start || !self.stack.is_empty() {
                return None;
            }
            if let Some(ascii) = kind {
                if self.global_ascii.is_some_and(|old| old != ascii) {
                    self.incompatible_flags = true;
                }
                self.global_ascii = Some(ascii);
            }
            if !engine.is_empty() {
                self.raw(&format!("(?{engine})"));
            }
        }
        self.mode = mode;
        Some(())
    }
    fn compile(&mut self) -> Option<()> {
        while let Some(c) = self.next() {
            if self.mode.verbose && (" \t\r\n\x0b\x0c".contains(c) || c == '#') {
                if c == '#' {
                    while self.peek().is_some_and(|c| c != '\n') {
                        self.next();
                    }
                }
                continue;
            }
            let was_start = self.start;
            match c {
                '(' => self.group()?,
                ')' => {
                    self.mode = self.stack.pop()?.0;
                    self.raw(")");
                }
                '[' => self.class()?,
                '\\' => match self.escape(false)? {
                    Escape::Char(c) => {
                        let class = self.literal(c, c)?;
                        self.atom(&class);
                    }
                    Escape::Class(class) => self.atom(&class),
                    Escape::Raw(raw, true) => self.reference(&raw),
                    Escape::Raw(raw, false) => self.raw(&raw),
                    Escape::Done => {}
                },
                '.' => {
                    let mut class = ClassUnicode::new([]);
                    if !self.mode.dotall {
                        class.push(ClassUnicodeRange::new('\n', '\n'));
                    }
                    class.negate();
                    self.atom(&class);
                }
                '$' if !self.mode.multiline => self.raw(r"(?=\n?\z)"),
                '^' | '$' | '*' | '+' | '?' | '|' => self.raw(&c.to_string()),
                '{' if self.source[self.position..].split_once('}').is_some_and(
                    |(count, _)| {
                        !count.is_empty() && count.chars().all(|c| c.is_ascii_digit() || c == ',')
                    },
                ) =>
                {
                    let end = self.source[self.position..].find('}')? + self.position + 1;
                    if self.source[self.position..].starts_with(',') {
                        self.raw(&format!("{{0{}", &self.source[self.position..end]));
                    } else {
                        self.raw(&self.source[self.position - 1..end]);
                    }
                    self.position = end;
                }
                c => {
                    let class = self.literal(c, c)?;
                    self.atom(&class);
                }
            }
            // Only leading global flags and comments leave the start position available.
            self.start = was_start && c == '(' && self.stack.is_empty();
        }
        // shortcut: mixed ASCII/Unicode caseless backreferences need per-reference VM modes.
        if !self.stack.is_empty() || self.ascii_refs && self.unicode_refs {
            return None;
        }
        Some(())
    }
}
enum Escape {
    Char(char),
    Class(ClassUnicode),
    Raw(String, bool),
    Done,
}
fn union(target: &mut ClassUnicode, item: Escape, compiler: &Compiler<'_>) -> Option<()> {
    match item {
        Escape::Char(c) => target.union(&compiler.literal(c, c)?),
        Escape::Class(class) => target.union(&class),
        _ => return None,
    };
    Some(())
}
fn parsed_class(pattern: &str) -> ClassUnicode {
    let hir = ParserBuilder::new().build().parse(pattern).unwrap();
    let HirKind::Class(Class::Unicode(class)) = hir.kind() else {
        panic!("expected Unicode class")
    };
    class.clone()
}
fn category(c: char, ascii: bool) -> ClassUnicode {
    static WORD: LazyLock<ClassUnicode> = LazyLock::new(|| parsed_class(r"[\p{L}\p{N}_]"));
    static DIGIT: LazyLock<ClassUnicode> = LazyLock::new(|| parsed_class(r"\d"));
    static SPACE: LazyLock<ClassUnicode> = LazyLock::new(|| parsed_class(r"[\s\x1c-\x1f]"));
    let mut class = match (c.to_ascii_lowercase(), ascii) {
        ('w', true) => parsed_class(r"[A-Za-z0-9_]"),
        ('d', true) => parsed_class(r"[0-9]"),
        ('s', true) => parsed_class(r"[\x09-\x0d ]"),
        ('w', false) => WORD.clone(),
        ('d', false) => DIGIT.clone(),
        _ => SPACE.clone(),
    };
    if c.is_ascii_uppercase() {
        class.negate();
    }
    class
}
fn unicode_class(class: &ClassUnicode) -> String {
    if class.ranges().is_empty() {
        return "(?!)".into();
    }
    let mut out = String::from("[");
    for range in class.iter() {
        out.push_str(&format!(
            r"\x{{{:x}}}-\x{{{:x}}}",
            u32::from(range.start()),
            u32::from(range.end())
        ));
    }
    out.push(']');
    out
}
fn byte_class(class: &ClassUnicode) -> String {
    let mut branches = Vec::new();
    for range in class.iter() {
        for sequence in Utf8Sequences::new(range.start(), range.end()) {
            let mut branch = String::new();
            for range in sequence.as_slice() {
                branch.push_str(&format!(r"[\x{:02x}-\x{:02x}]", range.start, range.end));
            }
            branches.push(branch);
        }
    }
    if branches.is_empty() {
        "(?!)".into()
    } else {
        format!("(?:{})", branches.join("|"))
    }
}
fn boundary(word: &str, negative: bool) -> String {
    let boundary = format!("(?:(?<={word})(?!{word})|(?<!{word})(?={word}))");
    if negative {
        format!("(?-i:(?!{boundary}))")
    } else {
        format!("(?-i:{boundary})")
    }
}
