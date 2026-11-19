//! Find and remove unreferenced Python definitions from tracked repository text.
mod ast;
mod report;

use crate::value::python_space;
use ast::{Session, Tree};
use indexmap::IndexMap;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    ffi::OsStr,
    fmt, fs,
    path::Path,
    process::Command,
    sync::LazyLock,
};

pub use report::{counts, markdown, run};
pub const ALLOWLIST: &str = "scripts/dead_code_allowlist.txt";
pub const UNREFERENCED: &str = "unreferenced";
pub const TEST_ONLY: &str = "test-only";
const ROOTS: &[&str] = &["tokeneconomizer/", "experiments/", "plugin/", "scripts/"];
const PLAIN_DECORATORS: &[&str] = &[
    "staticmethod",
    "classmethod",
    "property",
    "cached_property",
    "lru_cache",
    "cache",
    "wraps",
    "dataclass",
    "total_ordering",
    "abstractmethod",
    "contextmanager",
];
static WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z_][A-Za-z0-9_]*").unwrap());
static CONSTANT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\A_?[A-Z][A-Z0-9_]*\z").unwrap());

#[derive(Debug, Serialize, Deserialize)]
pub struct Error {
    pub kind: String,
    pub message: String,
    #[serde(default)]
    pub bases: Vec<String>,
}
impl Error {
    fn new(kind: &str, message: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            message: message.into(),
            bases: Vec::new(),
        }
    }
    fn is_a(&self, kind: &str) -> bool {
        self.kind == kind || self.bases.iter().any(|base| base == kind)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        let kind = match error.kind() {
            std::io::ErrorKind::NotFound => "FileNotFoundError",
            std::io::ErrorKind::PermissionDenied => "PermissionError",
            std::io::ErrorKind::IsADirectory => "IsADirectoryError",
            _ => "OSError",
        };
        Self::new(kind, error.to_string())
    }
}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Serialize, Deserialize)]
pub struct Item {
    pub path: String,
    pub name: String,
    pub kind: String,
    pub line: usize,
    pub status: String,
    pub removable: bool,
    #[serde(rename = "_span")]
    pub span: (usize, usize),
    #[serde(rename = "_class")]
    pub class: Option<(usize, usize)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub removed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

pub fn is_test_path(path: &str) -> bool {
    let base = path.rsplit('/').next().unwrap_or(path);
    path.starts_with("tests/")
        || path.starts_with("experiments/tests/")
        || path.contains("/tests/")
        || base.starts_with("test_")
        || base == "conftest.py"
}
fn read_text(root: &Path, path: &str) -> Option<String> {
    let path = root.join(path);
    if !fs::symlink_metadata(&path).ok()?.is_file() {
        return None;
    }
    let bytes = fs::read(path).ok()?;
    if bytes.contains(&0) {
        return None;
    }
    String::from_utf8(bytes).ok()
}
fn tracked_files(root: &Path) -> Result<Vec<String>> {
    let output = Command::new("git")
        .args(["-C"])
        .arg(root)
        .args(["ls-files", "-z"])
        .output()?;
    if !output.status.success() {
        return Err(Error::new(
            "CalledProcessError",
            format!(
                "git ls-files failed: {}",
                String::from_utf8_lossy(&output.stderr).trim_end()
            ),
        ));
    }
    // shortcut: filenames must be UTF-8; extend native path handling in the OS-string audit.
    let files = String::from_utf8(output.stdout)
        .map_err(|error| Error::new("UnicodeDecodeError", error.to_string()))?;
    Ok(files
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect())
}
fn splitlines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((position, c)) = chars.next() {
        if matches!(
            c,
            '\n' | '\r'
                | '\x0b'
                | '\x0c'
                | '\x1c'
                | '\x1d'
                | '\x1e'
                | '\u{85}'
                | '\u{2028}'
                | '\u{2029}'
        ) {
            let mut end = position + c.len_utf8();
            if c == '\r' && chars.peek().is_some_and(|(_, c)| *c == '\n') {
                end = chars.next().unwrap().0 + 1;
            }
            lines.push(text[start..end].into());
            start = end;
        }
    }
    if start < text.len() {
        lines.push(text[start..].into());
    }
    lines
}

struct Module {
    path: String,
    tree: Tree,
    imports: IndexMap<String, String>,
}
type Location = (usize, usize);
fn imports(tree: &Tree) -> IndexMap<String, String> {
    let mut imports = IndexMap::new();
    for node in tree.walk(0) {
        match tree.kind(node) {
            "Import" => {
                for alias in tree.list(node, "names") {
                    let name = tree.text(alias, "name");
                    let asname = tree.text(alias, "asname");
                    let first = name.split('.').next().unwrap();
                    imports
                        .entry(if asname.is_empty() { first } else { asname }.into())
                        .or_insert_with(|| {
                            if asname.is_empty() {
                                first.into()
                            } else {
                                name.into()
                            }
                        });
                }
            }
            "ImportFrom"
                if !tree.text(node, "module").is_empty()
                    && tree.nodes[node]["level"].as_u64().unwrap_or(0) == 0 =>
            {
                for alias in tree.list(node, "names") {
                    let name = tree.text(alias, "name");
                    let asname = tree.text(alias, "asname");
                    imports
                        .entry(if asname.is_empty() { name } else { asname }.into())
                        .or_insert_with(|| format!("{}:{name}", tree.text(node, "module")));
                }
            }
            _ => (),
        }
    }
    imports
}
fn base_name(tree: &Tree, node: usize) -> &str {
    match tree.kind(node) {
        "Name" => tree.text(node, "id"),
        "Attribute" => tree.text(node, "attr"),
        _ => "",
    }
}
fn registers(tree: &Tree, node: usize) -> bool {
    tree.list(node, "decorator_list")
        .into_iter()
        .any(|mut decorator| {
            if tree.kind(decorator) == "Call" {
                decorator = tree.child(decorator, "func").unwrap();
            }
            !PLAIN_DECORATORS.contains(&base_name(tree, decorator))
        })
}
fn dispatches(tree: &Tree, class: usize) -> bool {
    tree.walk(class).into_iter().any(|node| {
        let arguments = tree.list(node, "args");
        tree.kind(node) == "Call"
            && tree.child(node, "func").is_some_and(|function| {
                tree.kind(function) == "Name" && tree.text(function, "id") == "getattr"
            })
            && arguments.len() >= 2
            && tree.kind(arguments[0]) == "Name"
            && matches!(tree.text(arguments[0], "id"), "self" | "cls")
            && tree.kind(arguments[1]) != "Constant"
    })
}
fn resolve(
    session: &mut Session,
    tree: &Tree,
    mut expr: usize,
    imports: &IndexMap<String, String>,
) -> Result<Option<Value>> {
    let mut parts = Vec::new();
    while tree.kind(expr) == "Attribute" {
        parts.push(tree.text(expr, "attr").to_owned());
        expr = tree.child(expr, "value").unwrap();
    }
    if tree.kind(expr) != "Name" {
        return Ok(None);
    }
    let Some(import) = imports.get(tree.text(expr, "id")) else {
        return Ok(None);
    };
    let (module, attr) = import.split_once(':').unwrap_or((import, ""));
    if !session.stdlib.contains(module.split('.').next().unwrap()) {
        return Ok(None);
    }
    let mut dotted: Vec<_> = module.split('.').map(str::to_owned).collect();
    let minimum = dotted.len();
    if !attr.is_empty() {
        dotted.push(attr.into());
    }
    dotted.extend(parts.into_iter().rev());
    for cut in (minimum..=dotted.len()).rev() {
        let value: Result<Value> = (|| {
            let mut object = session.call("import", json!([dotted[..cut].join(".")]))?;
            for attr in &dotted[cut..] {
                object = session.call("getattr", json!([object, attr]))?;
            }
            Ok(object)
        })();
        match value {
            Ok(object) => {
                return Ok((session.call("is_type", json!([object]))? == true).then_some(object));
            }
            Err(error) if error.is_a("ImportError") || error.is_a("AttributeError") => (),
            Err(error) => return Err(error),
        }
    }
    Ok(None)
}
fn overrides(
    session: &mut Session,
    modules: &[Module],
    classes: &HashMap<String, Vec<Location>>,
    location: Location,
    name: &str,
    seen: &mut HashSet<Location>,
) -> Result<bool> {
    if !seen.insert(location) {
        return Ok(false);
    }
    let (module, class) = location;
    let tree = &modules[module].tree;
    let mut stdlib = Vec::new();
    for base in tree.list(class, "bases") {
        if let Some(resolved) = resolve(session, tree, base, &modules[module].imports)? {
            stdlib.push(resolved);
        } else if let Some(others) = classes.get(base_name(tree, base)) {
            for other in others {
                if overrides(session, modules, classes, *other, name, seen)? {
                    return Ok(true);
                }
            }
        }
    }
    if dispatches(tree, class) {
        return Ok(true);
    }
    for base in stdlib {
        if session.call("hasattr", json!([base, name]))? == true {
            return Ok(true);
        }
        for (module, class, prefixes) in [
            ("ast", "NodeVisitor", &["visit_"][..]),
            ("http.server", "BaseHTTPRequestHandler", &["do_"][..]),
            ("cmd", "Cmd", &["do_", "help_", "complete_"][..]),
        ] {
            let object = session.call("import", json!([module]))?;
            let target = session.call("getattr", json!([object, class]))?;
            if session.call("issubclass", json!([base, target]))? == true
                && prefixes.iter().any(|prefix| name.starts_with(prefix))
            {
                return Ok(true);
            }
        }
    }
    Ok(name.starts_with("visit_")
        && tree
            .list(class, "bases")
            .into_iter()
            .any(|base| matches!(base_name(tree, base), "NodeVisitor" | "NodeTransformer")))
}

pub struct Analyzer {
    session: Session,
}
impl Analyzer {
    pub fn new(python: &OsStr) -> Result<Self> {
        Ok(Self {
            session: Session::new(python)?,
        })
    }
    pub fn stdout(&self) -> &str {
        &self.session.stdout
    }
    pub fn stderr(&self) -> &str {
        &self.session.stderr
    }
    pub fn scan(&mut self, root: &Path) -> Result<Vec<Item>> {
        let allowed: BTreeSet<_> = splitlines(&read_text(root, ALLOWLIST).unwrap_or_default())
            .into_iter()
            .filter_map(|line| {
                let entry = line.split('#').next().unwrap().trim_matches(python_space);
                entry.contains("::").then(|| entry.to_owned())
            })
            .collect();
        let mut used: HashMap<String, usize> = HashMap::new();
        let mut tested: HashMap<String, usize> = HashMap::new();
        let mut sources = BTreeMap::new();
        for path in tracked_files(root)? {
            if path == ALLOWLIST {
                continue;
            }
            let Some(text) = read_text(root, &path) else {
                continue;
            };
            let test = is_test_path(&path);
            let counts = if test { &mut tested } else { &mut used };
            for word in WORD.find_iter(&text) {
                *counts.entry(word.as_str().into()).or_default() += 1;
            }
            if !test
                && path.ends_with(".py")
                && ROOTS.iter().any(|prefix| path.starts_with(prefix))
                && !path.starts_with("experiments/tests/")
            {
                sources.insert(path, text);
            }
        }
        let mut modules = Vec::new();
        for (path, source) in sources {
            match self.session.parse(&source, &path) {
                Ok(tree) => {
                    let imports = imports(&tree);
                    modules.push(Module {
                        path,
                        tree,
                        imports,
                    });
                }
                Err(error) if error.is_a("SyntaxError") => (),
                Err(error) => return Err(error),
            }
        }
        let mut classes: HashMap<String, Vec<Location>> = HashMap::new();
        for (index, module) in modules.iter().enumerate() {
            for node in module.tree.list(0, "body") {
                if module.tree.kind(node) == "ClassDef" {
                    classes
                        .entry(module.tree.text(node, "name").into())
                        .or_default()
                        .push((index, node));
                }
            }
        }
        let mut items = Vec::new();
        for (index, module) in modules.iter().enumerate() {
            let tree = &module.tree;
            let mut exported = HashSet::new();
            for node in tree.list(0, "body") {
                if !matches!(tree.kind(node), "Assign" | "AugAssign" | "AnnAssign") {
                    continue;
                }
                let targets = if tree.kind(node) == "Assign" {
                    tree.list(node, "targets")
                } else {
                    tree.child(node, "target").into_iter().collect()
                };
                if targets.into_iter().any(|target| {
                    tree.kind(target) == "Name" && tree.text(target, "id") == "__all__"
                }) && let Some(value) = tree.child(node, "value")
                {
                    for node in tree.walk(value) {
                        if tree.kind(node) == "Constant"
                            && let Some(value) = tree.nodes[node]["value"].as_str()
                        {
                            exported.insert(value.to_owned());
                        }
                    }
                }
            }
            let mut candidates = Vec::new();
            for node in tree.list(0, "body") {
                let name = tree.text(node, "name");
                match tree.kind(node) {
                    "FunctionDef" | "AsyncFunctionDef" => {
                        if !exported.contains(name) && !registers(tree, node) {
                            candidates.push((
                                "function",
                                name.to_owned(),
                                name.to_owned(),
                                node,
                                None,
                            ));
                        }
                    }
                    "ClassDef" => {
                        if !exported.contains(name) && !registers(tree, node) {
                            candidates.push((
                                "class",
                                name.to_owned(),
                                name.to_owned(),
                                node,
                                None,
                            ));
                        }
                        for item in tree.list(node, "body") {
                            let method = tree.text(item, "name");
                            if matches!(tree.kind(item), "FunctionDef" | "AsyncFunctionDef")
                                && !registers(tree, item)
                                && !overrides(
                                    &mut self.session,
                                    &modules,
                                    &classes,
                                    (index, node),
                                    method,
                                    &mut HashSet::new(),
                                )?
                            {
                                candidates.push((
                                    "method",
                                    format!("{name}.{method}"),
                                    method.into(),
                                    item,
                                    Some(node),
                                ));
                            }
                        }
                    }
                    "Assign" | "AnnAssign" => {
                        let targets = if tree.kind(node) == "Assign" {
                            tree.list(node, "targets")
                        } else {
                            tree.child(node, "target").into_iter().collect()
                        };
                        if targets.len() == 1 && tree.kind(targets[0]) == "Name" {
                            let name = tree.text(targets[0], "id");
                            if CONSTANT.is_match(name) && !exported.contains(name) {
                                candidates.push(("constant", name.into(), name.into(), node, None));
                            }
                        }
                    }
                    _ => (),
                }
            }
            for (kind, qualname, name, node, class) in candidates {
                if (name.starts_with("__") && name.ends_with("__"))
                    || allowed.contains(&format!("{}::{qualname}", module.path))
                    || allowed.contains(&format!("{}::{name}", module.path))
                    || used.get(&name).copied().unwrap_or(0) > 1
                {
                    continue;
                }
                let mut span = tree.span(node);
                for decorator in tree.list(node, "decorator_list") {
                    span.0 = span.0.min(tree.span(decorator).0);
                }
                let removable = if kind == "constant" {
                    let value = tree.child(node, "value").ok_or_else(|| {
                        Error::new(
                            "AttributeError",
                            "'NoneType' object has no attribute '_fields'",
                        )
                    })?;
                    !tree
                        .walk(value)
                        .into_iter()
                        .any(|node| tree.kind(node) == "Call")
                } else {
                    true
                };
                items.push(Item {
                    path: module.path.clone(),
                    name: qualname,
                    kind: kind.into(),
                    line: span.0,
                    status: if tested.get(&name).copied().unwrap_or(0) == 0 {
                        UNREFERENCED
                    } else {
                        TEST_ONLY
                    }
                    .into(),
                    removable,
                    span,
                    class: class.map(|class| tree.span(class)),
                    removed: None,
                    note: None,
                    extra: Map::new(),
                });
            }
        }
        Ok(items)
    }
    pub fn fix(&mut self, root: &Path, items: &mut [Item]) -> Result<()> {
        let mut by_path: IndexMap<String, Vec<usize>> = IndexMap::new();
        for (index, item) in items.iter_mut().enumerate() {
            item.removed = Some(false);
            if item.status == UNREFERENCED && item.removable {
                by_path.entry(item.path.clone()).or_default().push(index);
            }
        }
        for (path, todo) in by_path {
            let full = root.join(&path);
            let source = fs::read_to_string(&full)?;
            let tree = self.session.parse(&source, "<unknown>")?;
            let lines = splitlines(&source);
            let classes: HashSet<_> = todo
                .iter()
                .filter(|index| items[**index].kind == "class")
                .map(|index| items[*index].span)
                .collect();
            let todo: Vec<_> = todo
                .into_iter()
                .filter(|index| {
                    !items[*index]
                        .class
                        .is_some_and(|span| classes.contains(&span))
                })
                .collect();
            let spans: Vec<_> = todo.iter().map(|index| items[*index].span).collect();
            let mut chosen = Vec::new();
            for index in todo {
                let (start, end) = items[index].span;
                // Cuts remove whole lines, including sibling statements after a semicolon.
                let shares_line = items[index].class.is_none()
                    && tree
                        .list(0, "body")
                        .into_iter()
                        .filter(|node| {
                            let span = tree.span(*node);
                            span.0 <= end && start <= span.1
                        })
                        .count()
                        > 1;
                if shares_line {
                    items[index].note = Some(
                        "kept: deleting it would remove another statement on the same line".into(),
                    );
                } else if items[index]
                    .class
                    .is_some_and(|class| class_would_empty(&tree, class, &spans))
                {
                    items[index].note = Some("kept: deleting it would empty its class".into());
                } else {
                    chosen.push(index);
                }
            }
            if chosen.is_empty() {
                continue;
            }
            chosen.sort_by_key(|index| std::cmp::Reverse(items[*index].span.0));
            let mut new = lines;
            for index in &chosen {
                let (mut start, end) = items[*index].span;
                if start == 0 || start > new.len() || end < start {
                    return Err(Error::new("ValueError", "invalid definition line span"));
                }
                let indent = indent(&new[start - 1]);
                while start > 1
                    && new[start - 2].trim_matches(python_space).starts_with('#')
                    && indent == self::indent(&new[start - 2])
                {
                    start -= 1;
                }
                cut(&mut new, start, end);
            }
            let text = new.concat();
            match self.session.parse(&text, &path) {
                Ok(_) => (),
                Err(error) if error.is_a("SyntaxError") => {
                    for index in chosen {
                        items[index].note =
                            Some("kept: the file would not parse without it".into());
                    }
                    continue;
                }
                Err(error) => return Err(error),
            }
            fs::write(full, text)?;
            for index in chosen {
                items[index].removed = Some(true);
            }
        }
        Ok(())
    }
    pub fn sweep(&mut self, root: &Path, max_passes: i64) -> Result<Vec<Item>> {
        if max_passes <= 0 {
            return Err(Error::new(
                "UnboundLocalError",
                "cannot access local variable 'items' where it is not associated with a value",
            ));
        }
        let mut removed = Vec::new();
        let mut last = Vec::new();
        for _ in 0..max_passes {
            let mut items = self.scan(root)?;
            self.fix(root, &mut items)?;
            let mut gone = Vec::new();
            last = Vec::new();
            for item in items {
                if item.removed == Some(true) {
                    gone.push(item);
                } else {
                    last.push(item);
                }
            }
            let finished = gone.is_empty();
            removed.extend(gone);
            if finished {
                break;
            }
        }
        removed.extend(last);
        Ok(removed)
    }
}
fn indent(line: &str) -> usize {
    line.chars().take_while(|c| python_space(*c)).count()
}
fn class_would_empty(tree: &Tree, class: (usize, usize), spans: &[(usize, usize)]) -> bool {
    for node in tree.list(0, "body") {
        if tree.kind(node) == "ClassDef" && tree.span(node) == class {
            let mut body = tree.list(node, "body");
            if body.first().is_some_and(|node| {
                tree.kind(*node) == "Expr"
                    && tree
                        .child(*node, "value")
                        .is_some_and(|value| tree.kind(value) == "Constant")
            }) {
                body.remove(0);
            }
            return body.into_iter().all(|node| {
                let span = tree.span(node);
                spans
                    .iter()
                    .any(|(start, end)| *start <= span.0 && span.1 <= *end)
            });
        }
    }
    false
}
fn cut(lines: &mut Vec<String>, mut start: usize, end: usize) {
    let mut before = 0;
    while before < start - 1
        && lines[start - 2 - before]
            .trim_matches(python_space)
            .is_empty()
    {
        before += 1;
    }
    let mut after = 0;
    while end + after < lines.len() && lines[end + after].trim_matches(python_space).is_empty() {
        after += 1;
    }
    let mut keep_after = if end + after < lines.len() {
        before.max(after) - before
    } else {
        0
    };
    let prefix = start - 1 - before;
    if prefix == 0
        || lines[prefix - 1]
            .trim_end_matches(python_space)
            .ends_with(':')
    {
        start -= before;
        keep_after = 0;
    } else if end + after >= lines.len() {
        start -= before;
    }
    lines.drain(start - 1..(end + after - keep_after).min(lines.len()));
}
