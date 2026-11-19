use std::{env, ffi::OsString, path::PathBuf};
use tokeneconomizer::figures::{self, Result};

fn value(args: &mut impl Iterator<Item = OsString>, message: &str) -> Result<OsString> {
    args.next()
        .filter(|value| !value.is_empty() && !value.as_encoded_bytes().starts_with(b"--"))
        .ok_or_else(|| message.into())
}
fn run() -> Result<i32> {
    let mut args = env::args_os().skip(1);
    let Some(command) = args.next() else {
        return Err("expected figure name: results, framework or teaser".into());
    };
    if command == "--help" || command == "-h" {
        println!(
            "usage: te-figures <results|framework> [--out DIR] [--python PYTHON]\n       te-figures teaser --plex FONTS [--out DIR | --page DIR] [--still] [--python PYTHON]\n\nRegenerate the pinned documentation figures.\nThe default output directory is ./docs/assets.\nRequires Matplotlib for static figures; Pillow and IBM Plex fonts for the teaser.\nAnimation exports also require ffmpeg on PATH. --still writes only the teaser poster.\nTE_PYTHON selects the interpreter (default: python3)."
        );
        return Ok(0);
    }
    if command != "results" && command != "framework" && command != "teaser" {
        return Err(format!("unknown figure: {}", command.to_string_lossy()).into());
    }
    let mut out: Option<PathBuf> = None;
    let mut plex: Option<PathBuf> = None;
    let mut page: Option<PathBuf> = None;
    let mut still_only = false;
    let mut python = env::var_os("TE_PYTHON")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| OsString::from("python3"));
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--out") => out = Some(value(&mut args, "--out requires a directory")?.into()),
            Some("--python") => python = value(&mut args, "--python requires an interpreter")?,
            Some("--plex") if command == "teaser" => {
                plex = Some(value(&mut args, "--plex requires a fonts directory")?.into())
            }
            Some("--page") if command == "teaser" => {
                page = Some(value(&mut args, "--page requires a directory")?.into())
            }
            Some("--still") if command == "teaser" => still_only = true,
            _ => return Err(format!("unrecognized argument: {}", arg.to_string_lossy()).into()),
        }
    }
    if command == "teaser" {
        let plex = plex.ok_or("teaser requires --plex FONTS")?;
        if page.is_some() && out.is_some() {
            return Err("select --out or --page".into());
        }
        let is_page = page.is_some();
        let out = page.or(out).unwrap_or_else(|| PathBuf::from("docs/assets"));
        figures::teaser::render(&plex, &out, is_page, still_only, &python)?;
        return Ok(0);
    }
    let out = out.unwrap_or_else(|| PathBuf::from("docs/assets"));
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
