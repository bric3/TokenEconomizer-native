//! Mirror the selected WebBench projects while leaving their YAML authoritative.
use super::webbench::PROJECTS_ENV;
use crate::{
    experiments::{
        CommandOutput,
        swebench::{Failure, Result, Runtime, official},
        workloads,
    },
    plugin::cli::parser::is_option,
    value::{python_repr, truthy},
};
use serde_json::{Map, Value, json};
use std::{fs, path::Path};

pub const FIELDS: &[&str] = &["id", "date", "level", "description"];

pub fn projects_root(explicit: &Value, env: &Map<String, Value>) -> Result<Value> {
    let root = if truthy(explicit) {
        explicit
    } else {
        env.get(PROJECTS_ENV).unwrap_or(&Value::Null)
    };
    if truthy(root) {
        return Ok(root.clone());
    }
    Err(Failure::new(
        "SystemExit",
        format!("no Web-Bench projects root: set ${PROJECTS_ENV} or pass --projects-root"),
    ))
}

/// Stop at each yield so callers can change the project tree between iterations.
pub fn next_project(root: &Path, start: usize, cwd: &Path) -> Value {
    for (index, name) in workloads::groups("webbench")
        .expect("embedded groups")
        .keys()
        .enumerate()
        .skip(start)
    {
        if cwd.join(root).join(name).join("tasks.yml").is_file() {
            return json!({"name":name,"next":index+1});
        }
    }
    Value::Null
}

pub fn render(rt: &mut dyn Runtime, path: &Path) -> Result<String> {
    let nonempty = rt.call("yaml_load", json!(path))?;
    if !truthy(&nonempty) {
        return Ok(String::new());
    }
    rt.call("yaml_iter", Value::Null)?;
    let mut out = String::new();
    while truthy(&rt.call("yaml_next", Value::Null)?) {
        // YAML values stay in the SDK so date objects, aliases and codec errors retain their types.
        for field in FIELDS {
            let missing = rt.call("yaml_get", json!(field))?;
            if truthy(&missing) {
                rt.call("yaml_set", json!(""))?;
            } else if *field == "date" {
                rt.call("yaml_str", Value::Null)?;
            } else if *field == "description" {
                rt.call("yaml_strip", Value::Null)?;
            }
        }
        let mut record = Vec::new();
        for field in FIELDS {
            let encoded = rt.call("yaml_json", json!(field))?;
            let encoded = encoded
                .as_str()
                .ok_or_else(|| Failure::new("RuntimeError", "YAML codec must return text"))?;
            record.push(format!("\"{field}\":{encoded}"));
        }
        out.push_str(&format!("{{{}}}\n", record.join(",")));
    }
    Ok(out)
}

pub struct Options {
    root: Option<String>,
    projects: Vec<String>,
    all: bool,
    check: bool,
}

fn usage(program: &str) -> String {
    // shortcut: retain the source help's default width until the CLI formatting audit.
    format!(
        "usage: {program} [-h] [--all] [--check]\n{}[--projects-root PROJECTS_ROOT]\n{}[project ...]\n",
        " ".repeat(program.len() + 8),
        " ".repeat(program.len() + 8)
    )
}
fn parser_error(program: &str, message: &str) -> CommandOutput {
    CommandOutput {
        exit: Some(2),
        stderr: format!("{}{program}: error: {message}\n", usage(program)),
        ..CommandOutput::default()
    }
}
pub fn parse(program: &str, args: &[String]) -> std::result::Result<Options, CommandOutput> {
    let mut options = Options {
        root: None,
        projects: Vec::new(),
        all: false,
        check: false,
    };
    let mut unknown = Vec::new();
    let mut position = 0;
    let mut positional_closed = false;
    while position < args.len() {
        let arg = &args[position];
        position += 1;
        if arg == "--" {
            if positional_closed {
                unknown.push(arg.clone());
                unknown.extend_from_slice(&args[position..]);
            } else {
                options.projects.extend_from_slice(&args[position..]);
            }
            break;
        }
        if !is_option(arg) {
            if positional_closed {
                unknown.push(arg.clone());
            } else {
                options.projects.push(arg.clone());
            }
            continue;
        }
        if !options.projects.is_empty() {
            positional_closed = true;
        }
        let (name, explicit) = arg
            .split_once('=')
            .map_or((arg.as_str(), None), |(a, b)| (a, Some(b)));
        let option = if name == "-h" || (name.starts_with("--h") && "--help".starts_with(name)) {
            "-h/--help"
        } else if name.starts_with("--a") && "--all".starts_with(name) {
            "--all"
        } else if name.starts_with("--c") && "--check".starts_with(name) {
            "--check"
        } else if name.starts_with("--p") && "--projects-root".starts_with(name) {
            "--projects-root"
        } else {
            unknown.push(arg.clone());
            continue;
        };
        if option == "--projects-root" {
            if let Some(value) = explicit {
                options.root = Some(value.into());
            } else if position < args.len() && args[position] != "--" && !is_option(&args[position])
            {
                options.root = Some(args[position].clone());
                position += 1;
            } else {
                return Err(parser_error(
                    program,
                    "argument --projects-root: expected one argument",
                ));
            }
        } else {
            if let Some(value) = explicit {
                return Err(parser_error(
                    program,
                    &format!(
                        "argument {option}: ignored explicit argument {}",
                        python_repr(&json!(value))
                    ),
                ));
            }
            match option {
                "--all" => options.all = true,
                "--check" => options.check = true,
                _ => {
                    return Err(CommandOutput {
                        exit: Some(0),
                        stdout: format!("{}{}", usage(program), include_str!("yaml_help.txt")),
                        ..CommandOutput::default()
                    });
                }
            }
        }
    }
    if !unknown.is_empty() {
        return Err(parser_error(
            program,
            &format!("unrecognized arguments: {}", unknown.join(" ")),
        ));
    }
    Ok(options)
}

pub fn command(
    rt: &mut dyn Runtime,
    program: &str,
    args: &[String],
    env: &Map<String, Value>,
    cwd: &Path,
) -> Result<CommandOutput> {
    let options = match parse(program, args) {
        Ok(options) => options,
        Err(output) => return Ok(output),
    };
    let root = projects_root(&json!(options.root), env)?;
    let root = Path::new(
        root.as_str()
            .ok_or_else(|| Failure::new("TypeError", "projects root must be a path"))?,
    );
    let mut names = options.projects;
    if options.all {
        names.clear();
        let mut start = 0;
        loop {
            let next = next_project(root, start, cwd);
            let Some(name) = next["name"].as_str() else {
                break;
            };
            names.push(name.to_owned());
            start = next["next"].as_u64().unwrap() as usize;
        }
    }
    if names.is_empty() {
        return Ok(parser_error(
            program,
            "name at least one project, or pass --all",
        ));
    }
    let mut stale = Vec::new();
    for name in names {
        let yml = root.join(&name).join("tasks.yml");
        if !cwd.join(&yml).is_file() {
            return Err(Failure::new(
                "SystemExit",
                format!("{name}: no tasks.yml at {}", yml.display()),
            ));
        }
        let jsonl = root.join(&name).join("tasks.jsonl");
        let want = render(rt, &yml)?;
        let have = if cwd.join(&jsonl).is_file() {
            Some(official::load_text(&jsonl, cwd)?)
        } else {
            None
        };
        let line = if have.as_ref() == Some(&want) {
            format!("  ok      {name}  ({} tasks)\n", want.matches('\n').count())
        } else if options.check {
            stale.push(name.clone());
            format!(
                "  STALE   {name}  ({})\n",
                if have.is_none() { "missing" } else { "differs" }
            )
        } else {
            fs::write(cwd.join(&jsonl), &want).map_err(|e| Failure::for_path(e, &jsonl))?;
            format!(
                "  wrote   {name}  ({} tasks) -> {}\n",
                want.matches('\n').count(),
                jsonl.display()
            )
        };
        rt.call("write_stdout", json!(line))?;
    }
    if !stale.is_empty() {
        return Err(Failure::new(
            "SystemExit",
            format!(
                "\n{} project(s) stale: {}\nregenerate with: python scripts/webbench_tasks_from_yaml.py --all",
                stale.len(),
                stale.join(" ")
            ),
        ));
    }
    Ok(CommandOutput::default())
}
