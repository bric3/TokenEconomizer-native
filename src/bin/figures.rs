use std::{env, ffi::OsString, path::PathBuf};
use tokeneconomizer::figures::{self, Result};

fn run() -> Result<i32> {
    let mut args = env::args_os().skip(1);
    let Some(command) = args.next() else {
        return Err("expected figure name: results or framework".into());
    };
    if command == "--help" || command == "-h" {
        println!(
            "usage: te-figures <results|framework> [--out DIR] [--python PYTHON]\n\nRegenerate a pinned documentation figure as PNG and SVG.\nThe default output directory is ./docs/assets. Requires Matplotlib.\nTE_PYTHON selects the interpreter (default: python3)."
        );
        return Ok(0);
    }
    if command != "results" && command != "framework" {
        return Err(format!("unknown figure: {}", command.to_string_lossy()).into());
    }
    let mut out = PathBuf::from("docs/assets");
    let mut python = env::var_os("TE_PYTHON")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| OsString::from("python3"));
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--out") => {
                out = args
                    .next()
                    .filter(|value| {
                        !value.is_empty() && !value.as_encoded_bytes().starts_with(b"--")
                    })
                    .ok_or("--out requires a directory")?
                    .into()
            }
            Some("--python") => {
                python = args
                    .next()
                    .filter(|value| {
                        !value.is_empty() && !value.as_encoded_bytes().starts_with(b"--")
                    })
                    .ok_or("--python requires an interpreter")?
            }
            _ => return Err(format!("unrecognized argument: {}", arg.to_string_lossy()).into()),
        }
    }
    if command == "results" {
        figures::results(&out, &python)?;
    } else {
        figures::framework(&out, &python)?;
    }
    Ok(0)
}
fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("te-figures: {error}");
            std::process::exit(1);
        }
    }
}
