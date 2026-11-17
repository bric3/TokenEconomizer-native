use super::{Result, archive, build};
use std::{ffi::OsString, path::PathBuf};

pub fn run(
    args: impl IntoIterator<Item = OsString>,
    mut source: PathBuf,
    mut binaries: Option<PathBuf>,
    default_out: PathBuf,
) -> Result<()> {
    let mut out = None;
    let mut droid = false;
    let mut zip = false;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let raw = arg.to_str().ok_or("option names must be UTF-8")?;
        if raw == "--" {
            if args.next().is_some() {
                return Err("unexpected positional argument".into());
            }
            break;
        }
        let (flag, inline) = raw
            .split_once('=')
            .map_or((raw, None), |(flag, value)| (flag, Some(value)));
        let options = [
            "--out",
            "--source",
            "--binaries",
            "--droid",
            "--zip",
            "--help",
        ];
        let mut matches = options
            .into_iter()
            .filter(|option| option.starts_with(flag));
        let flag = if flag == "-h" {
            "--help"
        } else if flag.starts_with("--") {
            let first = matches
                .next()
                .ok_or_else(|| format!("unrecognised argument: {flag}"))?;
            if matches.next().is_some() {
                return Err(format!("ambiguous argument: {flag}").into());
            }
            first
        } else {
            return Err(format!("unrecognised argument: {flag}").into());
        };
        if inline.is_some() && ["--droid", "--zip", "--help"].contains(&flag) {
            return Err(format!("{flag} does not take a value").into());
        }
        match flag {
            "--out" | "--source" | "--binaries" => {
                let value = inline
                    .map(OsString::from)
                    .or_else(|| args.next())
                    .ok_or("missing path argument")?;
                if value
                    .to_str()
                    .is_some_and(|value| value.starts_with("--") || value == "-h")
                    && inline.is_none()
                {
                    return Err("missing path argument".into());
                }
                match flag {
                    "--out" => out = Some(PathBuf::from(value)),
                    "--source" => source = value.into(),
                    _ => binaries = Some(value.into()),
                }
            }
            "--droid" => droid = true,
            "--zip" => zip = true,
            "--help" => {
                println!(
                    "usage: te-package [--out PATH] [--droid] [--zip] [--source PATH] [--binaries PATH]"
                );
                return Ok(());
            }
            _ => unreachable!(),
        }
    }
    let mut out = out.unwrap_or_else(|| default_out.join("marketplace"));
    if droid && out == default_out.join("marketplace") {
        out = default_out.join("tokeneconomizer-droid");
    }
    let plugin = build(&source, binaries.as_deref(), &out, droid)?;
    println!("plugin bundle: {}", plugin.display());
    println!("marketplace:   {}", plugin.parent().unwrap().display());
    if zip {
        println!("zip:           {}", archive(&plugin)?.display());
    }
    Ok(())
}
