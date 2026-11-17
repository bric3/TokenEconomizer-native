use super::hosts;
use crate::value::{decimal_integer, python_repr};
use regex::Regex;
use serde::Serialize;
use serde_json::{Value, json};
use std::{iter, mem, path::Path, sync::LazyLock};

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Help,
    Value,
    Flag,
    Append,
    Integer,
}
#[derive(Clone)]
struct OptionSpec {
    name: &'static str,
    kind: Kind,
    required: bool,
    help: Option<&'static str>,
}
impl OptionSpec {
    fn new(name: &'static str, kind: Kind) -> Self {
        Self {
            name,
            kind,
            required: false,
            help: None,
        }
    }
    fn dest(&self) -> String {
        if self.name == "--global" {
            "globally".into()
        } else {
            self.name.trim_start_matches('-').replace('-', "_")
        }
    }
    fn metavar(&self) -> String {
        if self.name == "--host" {
            choices(&host_names())
        } else {
            self.dest().to_uppercase()
        }
    }
    fn invocation(&self) -> String {
        if self.kind == Kind::Help {
            "-h, --help".into()
        } else if self.kind == Kind::Flag {
            self.name.into()
        } else {
            format!("{} {}", self.name, self.metavar())
        }
    }
    fn usage(&self) -> String {
        let inner = if self.kind == Kind::Help {
            "-h".into()
        } else {
            self.invocation()
        };
        if self.required {
            inner
        } else {
            format!("[{inner}]")
        }
    }
}
struct Command {
    name: &'static str,
    aliases: &'static [&'static str],
    actions: &'static [&'static str],
    default: &'static str,
    list: Option<(&'static str, &'static str)>,
    action_help: Option<&'static str>,
    list_help: Option<&'static str>,
    options: Vec<OptionSpec>,
}
fn commands() -> Vec<Command> {
    use Kind::*;
    let command = |name: &'static str,
                   aliases: &'static [&'static str],
                   actions: &'static [&'static str],
                   default: &'static str,
                   list: Option<(&'static str, &'static str)>,
                   options: Vec<OptionSpec>| Command {
        name,
        aliases,
        actions,
        default,
        list,
        options,
        action_help: None,
        list_help: None,
    };
    let mut explain = OptionSpec::new("--explain", Flag);
    explain.help = Some("briefly explain current settings and saved state (no model calls)");
    let mut session = OptionSpec::new("--session", Value);
    session.required = true;
    let mut job = OptionSpec::new("--job", Value);
    job.required = true;
    let mut dry = OptionSpec::new("--dry-run", Flag);
    dry.help = Some("say what would be imported, change nothing");
    let mut status = OptionSpec::new("--status", Flag);
    status.help = Some("what the import did here");
    let mut facts = command(
        "microskills",
        &["facts"],
        &["show", "forget", "unforget"],
        "show",
        Some(("ids", "ID")),
        vec![],
    );
    facts.action_help = Some(
        "show the facts (default); forget: stop injecting them and never learn them again; unforget: bring forgotten facts back",
    );
    facts.list_help = Some("fact ids, as `te microskills` lists them");
    vec![
        command(
            "status",
            &[],
            &[],
            "",
            None,
            vec![OptionSpec::new("--json", Flag), explain],
        ),
        facts,
        command(
            "toolprune",
            &["untool"],
            &[
                "show",
                "apply",
                "restore",
                "refresh-catalog",
                "measure",
                "apply-pending",
            ],
            "show",
            Some(("tools", "tools")),
            vec![],
        ),
        command(
            "config",
            &[],
            &["show", "set", "unset"],
            "show",
            Some(("pairs", "pairs")),
            vec![OptionSpec::new("--global", Flag)],
        ),
        command(
            "fold",
            &[],
            &[],
            "",
            None,
            vec![
                OptionSpec::new("--session", Append),
                OptionSpec::new("--verbose", Flag),
            ],
        ),
        command("judge", &[], &[], "", None, vec![session, job]),
        command("doctor", &[], &[], "", None, vec![]),
        command(
            "repos",
            &[],
            &["list", "migrate", "merge"],
            "list",
            Some(("keys", "keys")),
            vec![
                OptionSpec::new("--json", Flag),
                OptionSpec::new("--quiet", Flag),
            ],
        ),
        command(
            "history",
            &[],
            &["status", "import"],
            "status",
            None,
            vec![dry, status, OptionSpec::new("--auto", Flag)],
        ),
        command("repomap", &[], &["show", "refresh"], "show", None, vec![]),
        command(
            "probe",
            &[],
            &[],
            "",
            None,
            vec![
                OptionSpec::new("--sessions", Integer),
                OptionSpec::new("--model", Value),
                OptionSpec::new("--keep", Flag),
            ],
        ),
    ]
}
fn common() -> Vec<OptionSpec> {
    let mut help = OptionSpec::new("--help", Kind::Help);
    help.help = Some("show this help message and exit");
    vec![
        help,
        OptionSpec::new("--cwd", Kind::Value),
        OptionSpec::new("--host", Kind::Value),
    ]
}
fn host_names() -> Vec<String> {
    let mut names = hosts::HOSTS.map(String::from).to_vec();
    names.sort();
    names
}
pub fn command_names() -> Vec<String> {
    commands()
        .iter()
        .flat_map(|c| iter::once(c.name).chain(c.aliases.iter().copied()))
        .map(String::from)
        .collect()
}
fn choices(names: &[String]) -> String {
    format!("{{{}}}", names.join(","))
}
pub fn terminal_columns(value: Option<&str>) -> usize {
    value
        .and_then(decimal_integer)
        .filter(|s| !s.starts_with('-'))
        .map(|s| s.parse::<usize>().unwrap_or(usize::MAX))
        .filter(|n| *n > 0)
        .unwrap_or(80)
}
fn command_choices(command: &Command) -> String {
    choices(
        &command
            .actions
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
    )
}
fn spec(name: &str) -> Option<Command> {
    commands()
        .into_iter()
        .find(|c| c.name == name || c.aliases.contains(&name))
}
fn options(command: Option<&Command>) -> Vec<OptionSpec> {
    let mut options = common();
    if let Some(command) = command {
        options.extend(command.options.clone());
    }
    options
}
fn positions(command: Option<&Command>) -> Vec<(String, Option<&'static str>)> {
    let Some(c) = command else {
        return vec![(choices(&command_names()), None)];
    };
    let mut out = Vec::new();
    if !c.actions.is_empty() {
        out.push((command_choices(c), c.action_help));
    }
    if let Some((_, metavar)) = c.list {
        out.push((metavar.into(), c.list_help));
    }
    out
}
fn wrap_parts(
    parts: &[String],
    indent: usize,
    initial: Option<usize>,
    width: usize,
) -> Vec<String> {
    let mut out = Vec::new();
    let mut line = String::new();
    let mut length = initial.unwrap_or(indent) as i64 - 1;
    for part in parts {
        let n = part.chars().count() as i64;
        if !line.is_empty() && length + 1 + n > width as i64 {
            out.push(format!("{}{line}", " ".repeat(indent)));
            line.clear();
            length = indent as i64 - 1;
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(part);
        length += n + 1;
    }
    if !line.is_empty() {
        out.push(format!("{}{line}", " ".repeat(indent)));
    }
    if initial.is_some()
        && let Some(first) = out.first_mut()
    {
        first.drain(..indent);
    }
    out
}
pub fn usage(name: Option<&str>, columns: usize) -> String {
    let command = name.and_then(spec);
    let program = command
        .as_ref()
        .map(|c| format!("te {}", c.name))
        .unwrap_or("te".into());
    let opts = options(command.as_ref())
        .iter()
        .filter(|o| o.name != "--auto")
        .map(OptionSpec::usage)
        .collect::<Vec<_>>();
    let pos = if let Some(c) = &command {
        let mut out = Vec::new();
        if !c.actions.is_empty() {
            out.push(format!("[{}]", command_choices(c)));
        }
        if let Some((_, label)) = c.list {
            out.push(format!("[{label} ...]"));
        }
        out
    } else {
        vec![format!("{} ...", choices(&command_names()))]
    };
    let parts: Vec<_> = iter::once(program.clone())
        .chain(opts.iter().chain(&pos).cloned())
        .collect();
    let width = columns.saturating_sub(2).max(1);
    let text = parts.join(" ");
    if 7 + text.chars().count() <= width {
        return format!("usage: {text}\n");
    }
    let mut lines = if 7 + program.len() <= width * 3 / 4 {
        let indent = 7 + program.len() + 1;
        let mut out = wrap_parts(
            &iter::once(program.clone()).chain(opts).collect::<Vec<_>>(),
            indent,
            Some(7),
            width,
        );
        out.extend(wrap_parts(&pos, indent, None, width));
        out
    } else {
        let mut out = vec![program];
        let mut rest = opts;
        rest.extend(pos);
        out.extend(wrap_parts(&rest, 7, None, width));
        out
    };
    lines[0].insert_str(0, "usage: ");
    format!("{}\n", lines.join("\n"))
}
pub fn help(name: Option<&str>, columns: usize) -> String {
    let command = name.and_then(spec);
    let mut out = usage(name, columns);
    out.push('\n');
    if name.is_none() {
        out.push_str(include_str!("help.txt").trim_end());
        out.push_str("\n\n");
    }
    let pos = positions(command.as_ref());
    let opts = options(command.as_ref())
        .into_iter()
        .filter(|o| o.name != "--auto")
        .map(|o| (o.invocation(), o.help))
        .collect::<Vec<_>>();
    let width = columns.saturating_sub(2).max(1);
    let max_help = 24usize.min(width.saturating_sub(20).max(4));
    let help_position = pos
        .iter()
        .chain(&opts)
        .map(|(label, _)| label.chars().count() + 4)
        .max()
        .unwrap_or(4)
        .min(max_help);
    for (title, rows) in [("positional arguments", pos), ("options", opts)] {
        if rows.is_empty() {
            continue;
        }
        out.push_str(title);
        out.push_str(":\n");
        for (label, text) in rows {
            out.push_str("  ");
            out.push_str(&label);
            let Some(text) = text else {
                out.push('\n');
                continue;
            };
            let action_width = help_position.saturating_sub(4);
            if label.chars().count() > action_width {
                out.push('\n');
                out.push_str(&" ".repeat(help_position));
            } else {
                out.push_str(&" ".repeat(help_position - 2 - label.chars().count()));
            }
            let lines = wrap_help(text, width.saturating_sub(help_position).max(11));
            out.push_str(&lines.join(&format!("\n{}", " ".repeat(help_position))));
            out.push('\n');
        }
        out.push('\n');
    }
    out.truncate(out.trim_end_matches('\n').len());
    out.push('\n');
    out
}

fn wrap_help(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let mut rest = word.chars().collect::<Vec<_>>();
        while rest.len() > width {
            if !line.is_empty() {
                line.push(' ');
            }
            let available = width.saturating_sub(line.chars().count());
            line.extend(rest.drain(..available));
            lines.push(line.trim_end().to_string());
            line.clear();
        }
        if rest.is_empty() {
            continue;
        }
        if !line.is_empty() && line.chars().count() + 1 + rest.len() > width {
            lines.push(mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.extend(rest);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

#[derive(Serialize)]
pub struct Parsed {
    pub args: Value,
    pub exit: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}
impl Parsed {
    fn error(scope: Option<&str>, message: String, columns: usize) -> Self {
        let program = scope.map(|s| format!("te {s}")).unwrap_or("te".into());
        Self {
            args: Value::Null,
            exit: Some(2),
            stdout: String::new(),
            stderr: format!("{}{program}: error: {message}\n", usage(scope, columns)),
        }
    }
}
fn matching_characters(a: &[char], b: &[char]) -> usize {
    let mut previous = vec![0; b.len() + 1];
    let mut current = vec![0; b.len() + 1];
    let (mut start_a, mut start_b, mut size) = (0, 0, 0);
    for (i, left) in a.iter().enumerate() {
        for (j, right) in b.iter().enumerate() {
            current[j + 1] = if left == right { previous[j] + 1 } else { 0 };
            if current[j + 1] > size {
                (start_a, start_b, size) = (
                    i + 1 - current[j + 1],
                    j + 1 - current[j + 1],
                    current[j + 1],
                );
            }
        }
        mem::swap(&mut previous, &mut current);
    }
    if size == 0 {
        return 0;
    }
    matching_characters(&a[..start_a], &b[..start_b])
        + size
        + matching_characters(&a[start_a + size..], &b[start_b + size..])
}
fn invalid_choice(label: &str, value: &str, accepted: &[String]) -> String {
    let word = value.chars().collect::<Vec<_>>();
    // argparse 3.15 suggests the closest difflib match, breaking equal ratios by name.
    let suggestion = accepted
        .iter()
        .filter_map(|candidate| {
            let candidate_chars = candidate.chars().collect::<Vec<_>>();
            let total = candidate_chars.len() + word.len();
            if total == 0
                || 2.0 * candidate_chars.len().min(word.len()) as f64 / (total as f64) < 0.6
            {
                return None;
            }
            let ratio = 2.0 * matching_characters(&candidate_chars, &word) as f64 / total as f64;
            (ratio >= 0.6).then_some((ratio, candidate))
        })
        .max_by(|(a, left), (b, right)| a.total_cmp(b).then(left.cmp(right)));
    let hint = suggestion
        .map(|(_, name)| format!(", maybe you meant {}?", python_repr(&json!(name))))
        .unwrap_or_default();
    format!(
        "argument {label}: invalid choice: {}{hint} (choose from {})",
        python_repr(&json!(value)),
        accepted
            .iter()
            .map(|v| python_repr(&json!(v)))
            .collect::<Vec<_>>()
            .join(", ")
    )
}
fn is_option(value: &str) -> bool {
    static NEGATIVE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^-\.?\d").unwrap());
    value.starts_with('-') && value != "-" && !NEGATIVE.is_match(value) && !value.contains(' ')
}
fn optional(value: &str, options: &[OptionSpec]) -> bool {
    let name = value.split_once('=').map(|p| p.0).unwrap_or(value);
    name == "-h"
        || (name.starts_with("--") && options.iter().any(|o| o.name.starts_with(name)))
        || is_option(value)
}
fn integer(value: &str) -> Option<Value> {
    // shortcut: session counts are bounded by i64; include larger integers in the CLI numeric audit.
    decimal_integer(value)?
        .parse::<i64>()
        .ok()
        .map(|n| json!(n))
}
pub fn parse(argv: &[String], cwd: &Path, columns: usize) -> Parsed {
    let mut args = json!({"cwd":cwd,"host":null,"cmd":null});
    let mut command: Option<Command> = None;
    let mut name: Option<String> = None;
    let mut extras = Vec::new();
    let mut action_pending = true;
    let mut items_consumed = false;
    let mut ended = false;
    let mut index = 0;
    while index < argv.len() {
        let token = &argv[index];
        let scope = name.as_deref();
        if !ended && token == "--" {
            if command
                .as_ref()
                .is_some_and(|c| c.actions.is_empty() || items_consumed)
            {
                extras.push(token.clone());
            }
            ended = true;
            index += 1;
            continue;
        }
        if !ended && optional(token, &options(command.as_ref())) {
            let (option, explicit) = if token.starts_with("-h") && !token.starts_with("-h=") {
                ("-h", None)
            } else {
                token
                    .split_once('=')
                    .map(|(o, v)| (o, Some(v)))
                    .unwrap_or((token.as_str(), None))
            };
            let opts = options(command.as_ref());
            let matched = if option == "-h" {
                vec![&opts[0]]
            } else if let Some(exact) = opts.iter().find(|o| o.name == option) {
                vec![exact]
            } else if option.starts_with("--") {
                opts.iter().filter(|o| o.name.starts_with(option)).collect()
            } else {
                vec![]
            };
            if matched.len() > 1 {
                return Parsed::error(
                    scope,
                    format!(
                        "ambiguous option: {token} could match {}",
                        matched
                            .iter()
                            .map(|o| o.name)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    columns,
                );
            }
            let Some(option) = matched.first() else {
                extras.push(token.clone());
                index += 1;
                continue;
            };
            if matches!(option.kind, Kind::Flag | Kind::Help) {
                if let Some(value) = explicit {
                    return Parsed::error(
                        scope,
                        format!(
                            "argument {}: ignored explicit argument {}",
                            if option.kind == Kind::Help {
                                "-h/--help"
                            } else {
                                option.name
                            },
                            python_repr(&json!(value))
                        ),
                        columns,
                    );
                }
                if option.kind == Kind::Help {
                    return Parsed {
                        args: Value::Null,
                        exit: Some(0),
                        stdout: help(scope, columns),
                        stderr: String::new(),
                    };
                }
                args[option.dest()] = json!(true);
            } else {
                let value = if let Some(value) = explicit {
                    value
                } else {
                    index += 1;
                    let Some(value) = argv.get(index).filter(|v| !optional(v, &opts)) else {
                        return Parsed::error(
                            scope,
                            format!("argument {}: expected one argument", option.name),
                            columns,
                        );
                    };
                    value
                };
                let value = if option.name == "--host" {
                    let names = host_names();
                    if !names.iter().any(|n| n == value) {
                        return Parsed::error(
                            scope,
                            invalid_choice("--host", value, &names),
                            columns,
                        );
                    }
                    json!(value)
                } else if option.kind == Kind::Integer {
                    let Some(number) = integer(value) else {
                        return Parsed::error(
                            scope,
                            format!(
                                "argument {}: invalid int value: {}",
                                option.name,
                                python_repr(&json!(value))
                            ),
                            columns,
                        );
                    };
                    number
                } else {
                    json!(value)
                };
                if option.kind == Kind::Append {
                    let dest = option.dest();
                    if args[&dest].is_null() {
                        args[&dest] = json!([])
                    }
                    args[&dest].as_array_mut().unwrap().push(value);
                } else {
                    args[option.dest()] = value;
                }
            }
            index += 1;
            continue;
        }
        if command.is_none() {
            let Some(found) = spec(token) else {
                return Parsed::error(
                    None,
                    invalid_choice("cmd", token, &command_names()),
                    columns,
                );
            };
            args["cmd"] = json!(token);
            if !found.actions.is_empty() {
                args["action"] = json!(found.default);
            }
            if let Some((dest, _)) = found.list {
                args[dest] = json!([]);
            }
            for option in &found.options {
                args[option.dest()] = match option.kind {
                    Kind::Flag => json!(false),
                    Kind::Integer => json!(6),
                    _ => Value::Null,
                };
            }
            args["fn"] = json!(format!("cmd_{}", found.name));
            name = Some(found.name.to_string());
            command = Some(found);
            ended = false;
            index += 1;
            continue;
        }
        let found = command.as_ref().unwrap();
        if items_consumed || found.actions.is_empty() {
            extras.push(token.clone());
            index += 1;
            continue;
        }
        if action_pending && !found.actions.contains(&token.as_str()) {
            return Parsed::error(
                scope,
                invalid_choice(
                    "action",
                    token,
                    &found
                        .actions
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>(),
                ),
                columns,
            );
        }
        if action_pending {
            args["action"] = json!(token);
            index += 1;
            action_pending = false;
            items_consumed = found.list.is_none();
        }
        if let Some((dest, _)) = found.list {
            while let Some(value) = argv
                .get(index)
                .filter(|v| ended || v.as_str() == "--" || !optional(v, &options(command.as_ref())))
            {
                if !ended && value == "--" {
                    ended = true;
                    index += 1;
                    continue;
                }
                args[dest].as_array_mut().unwrap().push(json!(value));
                index += 1;
                items_consumed = true;
            }
        }
    }
    let Some(command) = command else {
        return Parsed::error(
            None,
            "the following arguments are required: cmd".into(),
            columns,
        );
    };
    let missing = command
        .options
        .iter()
        .filter(|o| o.required && args[o.dest()].is_null())
        .map(|o| o.name)
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Parsed::error(
            name.as_deref(),
            format!(
                "the following arguments are required: {}",
                missing.join(", ")
            ),
            columns,
        );
    }
    if !extras.is_empty() {
        return Parsed::error(
            None,
            format!("unrecognized arguments: {}", extras.join(" ")),
            columns,
        );
    }
    Parsed {
        args,
        exit: None,
        stdout: String::new(),
        stderr: String::new(),
    }
}
