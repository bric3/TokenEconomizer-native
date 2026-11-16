use std::sync::LazyLock;

use fancy_regex::Regex as FancyRegex;
use regex::Regex;

use super::state::{SpiralState, basename};

static HEREDOC: LazyLock<FancyRegex> =
    LazyLock::new(|| FancyRegex::new(r#"(?<!<)<<(?!<)-?\s*(['"]?)([A-Za-z_]\w*)\1"#).unwrap());
static SOURCE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
    r"(?i)\.(?:py|pyi|pyx|pxd|js|jsx|mjs|cjs|ts|tsx|rs|go|java|kt|kts|scala|c|cc|cpp|cxx|h|hh|hpp|",
    r"rb|php|cs|swift|m|mm|sh|bash|zsh|lua|jl|ex|exs|erl|hs|ml|clj|sql|toml|ya?ml|json|cfg|ini|",
    r"conf|md|rst|html?|css|scss|vue|svelte|xml|gradle|properties|in)$")).unwrap()
});
static PYTHON: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|[\s/;&|(])python\d?(?:\.\d+)?\b").unwrap());
static PATCH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\*\*\* (?:Update|Add|Delete) File:\s*(.+?)\s*$").unwrap());
pub(crate) static SED_INPLACE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:-[a-zA-Z]*i|--in-place)").unwrap());

fn not_scratch(path: &str) -> bool {
    !path.is_empty()
        && !["/dev/", "/proc/", "/tmp/"]
            .iter()
            .any(|p| path.starts_with(p))
}

fn unquote(text: &str) -> &str {
    if text.len() >= 2
        && (text.starts_with('\'') && text.ends_with('\'')
            || text.starts_with('"') && text.ends_with('"'))
    {
        &text[1..text.len() - 1]
    } else {
        text
    }
}

pub fn split_heredocs(command: &str) -> Result<(String, Vec<String>), String> {
    let mut shell = Vec::new();
    let mut bodies = Vec::new();
    let mut tag: Option<String> = None;
    let mut body = Vec::new();
    for line in command.split('\n') {
        if let Some(end) = &tag {
            if line.trim() == end {
                bodies.push(body.join("\n"));
                tag = None;
                body.clear();
            } else {
                body.push(line);
            }
        } else {
            shell.push(line);
            if let Some(m) = HEREDOC.captures(line).map_err(|e| e.to_string())? {
                tag = Some(m[2].into());
            }
        }
    }
    if tag.is_some() {
        bodies.push(body.join("\n"));
    }
    Ok((shell.join("\n"), bodies))
}

// The reference uses shlex in two modes: shell edits preserve quotes and punctuation,
// while shell launchers use POSIX word splitting without comment removal.
pub fn tokens(text: &str, posix: bool) -> Result<Vec<String>, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    let mut word = String::new();
    let mut state = ' ';
    let mut escaped_state = ' ';
    let mut quoted = false;
    while i < chars.len() {
        let c = chars[i];
        let whitespace = if posix { " \t\r\n" } else { " \t\r" }.contains(c);
        let punctuation = !posix && "();<>|&\n".contains(c);
        if state == '\'' || state == '"' {
            quoted = true;
            if c == state {
                if posix {
                    state = 'a';
                } else {
                    word.push(c);
                    out.push(std::mem::take(&mut word));
                    state = ' ';
                    quoted = false;
                }
            } else if posix && c == '\\' && state == '"' {
                escaped_state = state;
                state = '\\';
            } else {
                word.push(c);
            }
        } else if state == '\\' {
            if escaped_state == '"' && c != '\\' && c != '"' {
                word.push('\\');
            }
            word.push(c);
            state = escaped_state;
        } else if whitespace {
            if !word.is_empty() || posix && quoted {
                out.push(std::mem::take(&mut word));
                quoted = false;
            }
            state = ' ';
        } else if !posix && c == '#' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if state == 'c' {
            if punctuation {
                word.push(c);
            } else {
                out.push(std::mem::take(&mut word));
                state = ' ';
                continue;
            }
        } else if posix && c == '\\' {
            escaped_state = 'a';
            state = '\\';
        } else if (posix || state == ' ') && "'\"".contains(c) {
            if !posix {
                word.push(c);
            }
            state = c;
        } else if punctuation {
            if !word.is_empty() {
                out.push(std::mem::take(&mut word));
            }
            word.push(c);
            state = 'c';
        } else {
            word.push(c);
            state = 'a';
        }
        i += 1;
    }
    if state == '\'' || state == '"' {
        return Err("No closing quotation".into());
    }
    if state == '\\' {
        return Err("No escaped character".into());
    }
    if !word.is_empty() || posix && quoted {
        out.push(word);
    }
    Ok(out)
}

const OPS: &[&str] = &["&&", "||", ";", ";;", "|", "|&", "&", "\n", "(", ")"];
const OUT: &[&str] = &[">", ">>", ">|", "&>", "&>>"];
const SKIP: &[&str] = &["<", "<<", "<<<", "<&", ">&", "<>"];

fn commands(shell: &str) -> (Vec<Vec<String>>, Vec<String>) {
    let streams = tokens(shell, false).map(|t| vec![t]).unwrap_or_else(|_| {
        shell
            .split('\n')
            .filter_map(|line| {
                tokens(line, false).ok().map(|mut t| {
                    t.push("\n".into());
                    t
                })
            })
            .collect()
    });
    let mut commands = Vec::new();
    let mut writes = Vec::new();
    for stream in streams {
        let mut current: Vec<String> = Vec::new();
        let mut i = 0;
        while i < stream.len() {
            let t = stream[i].as_str();
            if OPS.contains(&t) {
                if !current.is_empty() {
                    commands.push(std::mem::take(&mut current));
                }
            } else if OUT.contains(&t) || SKIP.contains(&t) {
                if current
                    .last()
                    .is_some_and(|s| !s.is_empty() && s.chars().all(char::is_numeric))
                {
                    current.pop();
                }
                if OUT.contains(&t)
                    && let Some(next) = stream.get(i + 1).filter(|s| !OPS.contains(&s.as_str()))
                {
                    writes.push(unquote(next).into());
                }
                i += 1;
            } else {
                current.push(unquote(t).into());
            }
            i += 1;
        }
        if !current.is_empty() {
            commands.push(current);
        }
    }
    (commands, writes)
}

fn short_script_flag(token: &str, script: &str, stop: &str) -> Option<bool> {
    if token.starts_with("--") || token.len() < 2 {
        return None;
    }
    for (i, c) in token.char_indices().skip(1) {
        if stop.contains(c) {
            return None;
        }
        if script.contains(c) {
            return Some(i + c.len_utf8() < token.len());
        }
    }
    None
}

fn inplace_files(tokens: &[String], sed: bool) -> Vec<String> {
    static PERL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^-[a-zA-Z]*i").unwrap());
    let inplace = if sed { &*SED_INPLACE } else { &*PERL };
    if !tokens[1..].iter().any(|t| inplace.is_match(t)) {
        return Vec::new();
    }
    let mut files = Vec::new();
    let mut have_script = false;
    let mut skip = false;
    for t in &tokens[1..] {
        if skip {
            skip = false;
            have_script = true;
        } else if t.is_empty() || t.starts_with('-') {
            let script = if sed && ["--expression", "--file"].contains(&t.as_str()) {
                Some(false)
            } else if sed && (t.starts_with("--expression=") || t.starts_with("--file=")) {
                Some(true)
            } else if sed {
                short_script_flag(t, "ef", "i")
            } else {
                short_script_flag(t, "eE", "iIMmxdDC")
            };
            match script {
                Some(true) => have_script = true,
                Some(false) => skip = true,
                None => {}
            }
        } else if !have_script {
            have_script = true;
        } else {
            files.push(t.clone());
        }
    }
    files
}

fn py_writes(text: &str) -> Result<Vec<String>, String> {
    static OPEN_LITERAL: LazyLock<FancyRegex> = LazyLock::new(|| {
        FancyRegex::new(r#"open\(\s*(['"])([^'"\n]+)\1\s*,\s*(['"])([^'"]*)\3"#).unwrap()
    });
    static OPEN_VARIABLE: LazyLock<FancyRegex> = LazyLock::new(|| {
        FancyRegex::new(r#"open\(\s*([A-Za-z_]\w*)\s*,\s*(['"])([^'"]*)\2"#).unwrap()
    });
    static WRITE_LITERAL: LazyLock<FancyRegex> = LazyLock::new(|| {
        FancyRegex::new(r#"(?:pathlib\.)?Path\(\s*(['"])([^'"\n]+)\1\s*\)\.write_(?:text|bytes)\("#)
            .unwrap()
    });
    static WRITE_VARIABLE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\b([A-Za-z_]\w*)\.write_(?:text|bytes)\(").unwrap());
    static QUOTED_PATH: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"['"]([^'"\n]+)['"]"#).unwrap());
    let mut out = Vec::new();
    let mut variables = Vec::new();
    for m in OPEN_LITERAL.captures_iter(text) {
        let m = m.map_err(|e| e.to_string())?;
        if m[4].contains(['w', 'a', 'x', '+']) {
            out.push(m[2].into());
        }
    }
    for m in OPEN_VARIABLE.captures_iter(text) {
        let m = m.map_err(|e| e.to_string())?;
        if m[3].contains(['w', 'a', 'x', '+']) {
            variables.push(m[1].to_owned());
        }
    }
    for m in WRITE_LITERAL.captures_iter(text) {
        let m = m.map_err(|e| e.to_string())?;
        out.push(m[2].into());
    }
    variables.extend(WRITE_VARIABLE.captures_iter(text).map(|m| m[1].to_owned()));
    let mut seen = Vec::new();
    for variable in variables {
        if seen.contains(&variable) {
            continue;
        }
        seen.push(variable.clone());
        let variable = regex::escape(&variable);
        let assignment = FancyRegex::new(&format!(
            r#"\b{variable}\s*=\s*(?:(?:pathlib\.)?Path\(\s*)?(['"])([^'"\n]+)\1"#
        ))
        .map_err(|e| e.to_string())?;
        let mut paths = Vec::new();
        for m in assignment.captures_iter(text) {
            paths.push(m.map_err(|e| e.to_string())?[2].to_owned());
        }
        let loop_pattern = Regex::new(&format!(r"\bfor\s+{variable}\s+in\s+[\[(]([^\])]*)[\])]"))
            .map_err(|e| e.to_string())?;
        if let Some(m) = loop_pattern.captures(text) {
            paths.extend(QUOTED_PATH.captures_iter(&m[1]).map(|m| m[1].to_owned()));
        }
        if paths.is_empty() {
            paths.push("(script)".into());
        }
        out.extend(paths);
    }
    Ok(out)
}

fn git_args(tokens: &[String]) -> &[String] {
    if tokens[0].rsplit('/').next() != Some("git") {
        return &[];
    }
    let mut i = 1;
    while i < tokens.len() && tokens[i].starts_with('-') {
        i += if [
            "-C",
            "-c",
            "--git-dir",
            "--work-tree",
            "--namespace",
            "--config-env",
            "--attr-source",
        ]
        .contains(&tokens[i].as_str())
        {
            2
        } else {
            1
        };
    }
    &tokens[i.min(tokens.len())..]
}

fn restored(tokens: &[String]) -> Vec<String> {
    let args = git_args(tokens);
    if args.len() < 2 || !["checkout", "restore"].contains(&args[0].as_str()) {
        return Vec::new();
    }
    let rest = if let Some(i) = args.iter().position(|s| s == "--") {
        &args[i + 1..]
    } else if args[0] == "restore" {
        &args[1..]
    } else {
        &[]
    };
    rest.iter()
        .filter(|s| !s.starts_with('-'))
        .cloned()
        .collect()
}

fn wrapper_options(head: &str) -> Option<&'static [&'static str]> {
    Some(match head {
        "sudo" => &[
            "-u",
            "-g",
            "-p",
            "-C",
            "-D",
            "-R",
            "-r",
            "-t",
            "-T",
            "-U",
            "--user",
            "--group",
            "--prompt",
            "--close-from",
            "--chdir",
            "--chroot",
            "--role",
            "--type",
            "--command-timeout",
            "--other-user",
        ],
        "env" => &["-u", "-C", "-S", "--unset", "--chdir", "--split-string"],
        "timeout" => &["-s", "-k", "--signal", "--kill-after"],
        "nice" => &["-n", "--adjustment"],
        "time" => &["-f", "-o", "--format", "--output"],
        "command" | "nohup" => &[],
        _ => return None,
    })
}

fn bare(tokens: &[String]) -> &[String] {
    static ASSIGNMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\w+=").unwrap());
    let mut rest = tokens;
    while let Some(first) = rest.first() {
        if let Some(options) = wrapper_options(first) {
            let mut i = 1;
            while i < rest.len() && rest[i].starts_with('-') {
                let token = &rest[i];
                i += 1;
                if token == "--" {
                    break;
                }
                if token.starts_with("--") {
                    if options.contains(&token.as_str()) {
                        i += 1;
                    }
                } else {
                    for (n, c) in token.char_indices().skip(1) {
                        if options.contains(&format!("-{c}").as_str()) {
                            if n + c.len_utf8() == token.len() {
                                i += 1;
                            }
                            break;
                        }
                    }
                }
            }
            if first == "timeout" {
                i += 1;
            }
            rest = &rest[i.min(rest.len())..];
        } else if ASSIGNMENT.is_match(first) {
            rest = &rest[1..];
        } else {
            break;
        }
    }
    rest
}

fn names(paths: Vec<String>) -> Vec<String> {
    let mut names = Vec::new();
    for path in paths {
        let name = if path.starts_with('(') {
            path
        } else if not_scratch(&path) {
            basename(&path)
        } else {
            continue;
        };
        if !name.is_empty() && !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

pub fn shell_revert_targets(command: &str) -> Result<Vec<String>, String> {
    if command.trim().is_empty() {
        return Ok(Vec::new());
    }
    let (shell, _) = split_heredocs(command)?;
    let (commands, _) = commands(&shell);
    Ok(names(
        commands
            .iter()
            .map(|c| bare(c))
            .filter(|c| !c.is_empty())
            .flat_map(restored)
            .filter(|p| not_scratch(p))
            .collect(),
    ))
}

pub fn shell_edit_targets(command: &str) -> Result<Vec<String>, String> {
    if command.trim().is_empty() {
        return Ok(Vec::new());
    }
    let (shell, bodies) = split_heredocs(command)?;
    let (commands, writes) = commands(&shell);
    let mut found: Vec<String> = writes.into_iter().filter(|w| SOURCE.is_match(w)).collect();
    for tokens in commands.iter().map(|c| bare(c)).filter(|c| !c.is_empty()) {
        let head = tokens[0].rsplit('/').next().unwrap_or("");
        match head {
            "sed" | "perl" => found.extend(inplace_files(tokens, head == "sed")),
            "tee" => found.extend(
                tokens[1..]
                    .iter()
                    .filter(|s| !s.starts_with('-') && SOURCE.is_match(s))
                    .cloned(),
            ),
            "apply_patch" => {
                let mut text = bodies.clone();
                text.push(tokens[1..].join(" "));
                let text = text.join("\n");
                let mut paths: Vec<String> =
                    PATCH.captures_iter(&text).map(|m| m[1].into()).collect();
                if paths.is_empty() {
                    paths.push("(patch)".into());
                }
                found.extend(paths);
            }
            _ if head == "patch" || git_args(tokens).first().is_some_and(|s| s == "apply") => {
                found.push("(patch)".into())
            }
            _ => found.extend(restored(tokens)),
        }
    }
    if PYTHON.is_match(&shell) {
        let mut text = vec![shell];
        text.extend(bodies);
        found.extend(py_writes(&text.join("\n"))?);
    }
    Ok(names(found))
}

pub fn fold_shell_edits(state: &mut SpiralState, names: &[String]) {
    for name in names {
        state.edit_order.push(name.clone());
        let count = state
            .edit_counts
            .get(name)
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            .saturating_add(1);
        state.edit_counts.insert(name.clone(), count.into());
    }
    if !names.is_empty()
        && let Some(recent) = state.recent.last_mut()
    {
        let shown = names[..names.len().min(4)].join(", ");
        let suffix = if names.len() > 4 { ", ..." } else { "" };
        recent.push_str(&format!("  [edits {shown}{suffix}]"));
    }
}
