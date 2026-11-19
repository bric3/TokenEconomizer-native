mod backrefs;

use regex_syntax::{
    ParserBuilder,
    hir::{Class, ClassUnicode, ClassUnicodeRange, HirKind},
};
use std::{collections::BTreeMap, ops::Range, sync::LazyLock};

use super::unicode17::{CASE_FOLD_ADDED, DIGIT_ADDED, WORD_ADDED};
use backrefs::{Matcher, Reference};

pub struct Pattern {
    source: String,
    compiled: Matcher,
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
        let compiled = Matcher::new(&compiler.unicode, compiler.references);
        Ok(compiled.map(|compiled| Self {
            source: source.into(),
            compiled,
        }))
    }
    pub fn as_str(&self) -> &str {
        &self.source
    }
    pub fn is_match(&self, text: &str) -> Result<bool, String> {
        self.compiled.is_match(text)
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

// Fixed character widths keep backreferences valid inside Python lookbehinds.
#[derive(Clone, Copy)]
struct Width {
    prefix: Option<usize>,
    atom: Option<usize>,
    branch: Option<usize>,
    alternative: bool,
}
impl Default for Width {
    fn default() -> Self {
        Self {
            prefix: Some(0),
            atom: Some(0),
            branch: Some(0),
            alternative: false,
        }
    }
}
impl Width {
    fn append(&mut self, atom: Option<usize>) {
        self.prefix = self
            .prefix
            .zip(self.atom)
            .and_then(|(a, b)| a.checked_add(b));
        self.atom = atom;
    }
    fn repeat(&mut self, low: usize, high: Option<usize>) {
        self.atom = if high == Some(low) {
            if low == 0 {
                Some(0)
            } else {
                self.atom.and_then(|n| n.checked_mul(low))
            }
        } else if self.atom == Some(0) {
            Some(0)
        } else {
            None
        };
    }
    fn alternate(&mut self) {
        let width = self
            .prefix
            .zip(self.atom)
            .and_then(|(a, b)| a.checked_add(b));
        if !self.alternative {
            self.branch = width;
        } else if self.branch != width {
            self.branch = None;
        }
        self.alternative = true;
        self.prefix = Some(0);
        self.atom = Some(0);
    }
    fn finish(self) -> Option<usize> {
        let width = self
            .prefix
            .zip(self.atom)
            .and_then(|(a, b)| a.checked_add(b));
        if self.alternative && self.branch != width {
            None
        } else {
            width
        }
    }
}
struct Frame {
    mode: Mode,
    capture: Option<usize>,
    width: Width,
    assertion: bool,
    conditional: bool,
}

struct Compiler<'a> {
    source: &'a str,
    position: usize,
    mode: Mode,
    stack: Vec<Frame>,
    captures: usize,
    widths: Vec<Option<usize>>,
    width: Width,
    names: BTreeMap<String, usize>,
    unicode: String,
    references: Vec<(Range<usize>, Reference)>,
    start: bool,
    global_ascii: Option<bool>,
    incompatible_flags: bool,
}
impl<'a> Compiler<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            position: 0,
            mode: Mode::default(),
            stack: Vec::new(),
            captures: 0,
            widths: vec![None],
            width: Width::default(),
            names: BTreeMap::new(),
            unicode: String::new(),
            references: Vec::new(),
            start: true,
            global_ascii: None,
            incompatible_flags: false,
        }
    }
    fn raw(&mut self, text: &str) {
        self.unicode.push_str(text);
    }
    fn open(&mut self, capture: Option<usize>, assertion: bool, conditional: bool) {
        self.stack.push(Frame {
            mode: self.mode,
            capture,
            width: std::mem::take(&mut self.width),
            assertion,
            conditional,
        });
    }
    fn close(&mut self) -> Option<()> {
        let frame = self.stack.pop()?;
        let mut width = self.width.finish();
        if frame.conditional && !self.width.alternative && width != Some(0) {
            width = None;
        }
        if let Some(id) = frame.capture {
            self.widths[id] = width;
        }
        self.width = frame.width;
        self.width
            .append(if frame.assertion { Some(0) } else { width });
        self.mode = frame.mode;
        Some(())
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
                for &group in CASE_FOLD_ADDED {
                    if group.iter().any(|&c| first <= c && c <= last) {
                        for &c in group {
                            class.push(ClassUnicodeRange::new(c, c));
                        }
                    }
                }
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
        self.width.append(Some(1));
        let unicode = unicode_class(class);
        self.unicode.push_str(&format!("(?-i:{unicode})"));
    }
    fn reference(&mut self, id: usize) {
        let width = self.widths[id];
        self.width.append(width);
        let start = self.unicode.len();
        self.raw(&format!(r"\g{{{id}}}"));
        if self.mode.casei {
            self.references.push((
                start..self.unicode.len(),
                Reference {
                    group: id,
                    ascii: self.mode.ascii,
                    width,
                },
            ));
        }
    }
    fn closed_reference(&self, id: usize) -> Option<usize> {
        (id > 0 && id <= self.captures && !self.stack.iter().any(|frame| frame.capture == Some(id)))
            .then_some(id)
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
            'N' => {
                if self.next()? != '{' {
                    return None;
                }
                let end = self.source[self.position..].find('}')? + self.position;
                let character = named_character(&self.source[self.position..end])?;
                self.position = end + 1;
                character
            }
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
                    return Some(Escape::Reference(self.closed_reference(id)?));
                }
            }
            'b' | 'B' if !in_class => {
                let word = category('w', self.mode.ascii);
                let u = boundary(&unicode_class(&word), c == 'B');
                self.unicode.push_str(&u);
                self.width.append(Some(0));
                return Some(Escape::Done);
            }
            'A' if !in_class => return Some(Escape::Raw(r"\A".into())),
            'Z' | 'z' if !in_class => return Some(Escape::Raw(r"\z".into())),
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
            self.widths.push(None);
            self.open(Some(self.captures), false, false);
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
            self.reference(self.closed_reference(*self.names.get(name)?)?);
            self.position = end;
            return Some(());
        }
        let header = if rest.starts_with("?P<") {
            let end = self.source[self.position..].find('>')? + self.position + 1;
            let name = &self.source[self.position + 3..end - 1];
            let mut characters = name.chars();
            if characters
                .next()
                .is_none_or(|c| c != '_' && !unicode_ident::is_xid_start(c))
                || !characters.all(unicode_ident::is_xid_continue)
                || self.names.contains_key(name)
            {
                return None;
            }
            self.captures += 1;
            self.widths.push(None);
            self.names.insert(name.into(), self.captures);
            // Names remain in source metadata; engine references use validated group numbers.
            self.open(Some(self.captures), false, false);
            self.raw("(");
            self.position = end;
            return Some(());
        } else if rest.starts_with("?(") {
            let end = self.source[self.position..].find(')')? + self.position + 1;
            let condition = &self.source[self.position + 2..end - 1];
            if !condition.bytes().all(|c| c.is_ascii_digit()) {
                let id = *self.names.get(condition)?;
                self.open(None, false, true);
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
            self.open(
                None,
                rest.starts_with("?=") || rest.starts_with("?!") || rest.starts_with("?<"),
                rest.starts_with("?("),
            );
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
            self.open(None, false, false);
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
        let mut quantified = false;
        while let Some(c) = self.next() {
            if self.mode.verbose && (" \t\r\n\x0b\x0c".contains(c) || c == '#') {
                if c == '#' {
                    while self.peek().is_some_and(|c| c != '\n') {
                        self.next();
                    }
                }
                continue;
            }
            let modifier = quantified && matches!(c, '?' | '+');
            quantified = false;
            let was_start = self.start;
            match c {
                '(' => self.group()?,
                ')' => {
                    self.close()?;
                    self.raw(")");
                }
                '[' => self.class()?,
                '\\' => match self.escape(false)? {
                    Escape::Char(c) => {
                        let class = self.literal(c, c)?;
                        self.atom(&class);
                    }
                    Escape::Class(class) => self.atom(&class),
                    Escape::Reference(id) => self.reference(id),
                    Escape::Raw(raw) => {
                        self.width.append(Some(0));
                        self.raw(&raw);
                    }
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
                '$' if !self.mode.multiline => {
                    self.width.append(Some(0));
                    self.raw(r"(?=\n?\z)");
                }
                '^' | '$' => {
                    self.width.append(Some(0));
                    self.raw(&c.to_string());
                }
                '|' => {
                    self.width.alternate();
                    self.raw("|");
                }
                '*' | '+' | '?' => {
                    if !modifier {
                        self.width
                            .repeat(usize::from(c == '+'), (c == '?').then_some(1));
                    }
                    self.raw(&c.to_string());
                    quantified = !modifier;
                }
                '{' if self.source[self.position..].split_once('}').is_some_and(
                    |(count, _)| {
                        !count.is_empty() && count.chars().all(|c| c.is_ascii_digit() || c == ',')
                    },
                ) =>
                {
                    let end = self.source[self.position..].find('}')? + self.position + 1;
                    let count = &self.source[self.position..end - 1];
                    let parts = count.split(',').collect::<Vec<_>>();
                    let bounds = match parts.as_slice() {
                        [n] => n.parse::<usize>().ok().map(|n| (n, Some(n))),
                        [low, high] => {
                            let low = if low.is_empty() {
                                Some(0)
                            } else {
                                low.parse().ok()
                            };
                            low.map(|low| (low, high.parse().ok()))
                        }
                        _ => None,
                    };
                    if let Some((low, high)) = bounds {
                        self.width.repeat(low, high);
                        quantified = true;
                    } else {
                        self.width.append(Some(count.len() + 2));
                    }
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
        if !self.stack.is_empty() {
            return None;
        }
        Some(())
    }
}
enum Escape {
    Char(char),
    Class(ClassUnicode),
    Raw(String),
    Reference(usize),
    Done,
}
fn named_character(name: &str) -> Option<char> {
    // UnicodeData stores Tangut ranges without the names that Python derives from code points.
    if let Some((prefix, hex)) = name.split_once('-')
        && prefix.eq_ignore_ascii_case("TANGUT IDEOGRAPH")
        && hex.len() == 5
        && let Ok(code) = u32::from_str_radix(hex, 16)
        && matches!(code, 0x17000..=0x187ff | 0x18d00..=0x18d1e)
    {
        return char::from_u32(code);
    }
    // Python preserves spaces and hyphens in names; the database accepts looser spellings.
    if let Some(character) = unicode_names2::character(name)
        && unicode_names2::name(character)
            .is_some_and(|canonical| canonical.to_string().eq_ignore_ascii_case(name))
    {
        return Some(character);
    }
    include_str!("data/NameAliases-17.0.0.txt")
        .lines()
        .find_map(|line| {
            let (code, rest) = line.split_once(';')?;
            let (alias, _) = rest.split_once(';')?;
            if !alias.eq_ignore_ascii_case(name) {
                return None;
            }
            char::from_u32(u32::from_str_radix(code, 16).ok()?)
        })
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
fn updated_class(pattern: &str, added: &[(char, char)]) -> ClassUnicode {
    let mut class = parsed_class(pattern);
    class.union(&ClassUnicode::new(
        added
            .iter()
            .map(|&(first, last)| ClassUnicodeRange::new(first, last)),
    ));
    class
}
fn category(c: char, ascii: bool) -> ClassUnicode {
    static WORD: LazyLock<ClassUnicode> =
        LazyLock::new(|| updated_class(r"[\p{L}\p{N}_]", WORD_ADDED));
    static DIGIT: LazyLock<ClassUnicode> = LazyLock::new(|| updated_class(r"\d", DIGIT_ADDED));
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
fn boundary(word: &str, negative: bool) -> String {
    let boundary = format!("(?:(?<={word})(?!{word})|(?<!{word})(?={word}))");
    if negative {
        format!("(?-i:(?!{boundary}))")
    } else {
        format!("(?-i:{boundary})")
    }
}
