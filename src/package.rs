use crate::plugin::paths;
use crate::value::base64;
use chrono::{DateTime, Datelike, Local, Timelike};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fs::{self, File, FileTimes},
    io::{self, Write},
    path::{Path, PathBuf},
};
use zip::{CompressionMethod, DateTime as ZipTime, ZipWriter, write::SimpleFileOptions};

pub mod cli;

pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;
pub(crate) const NOTICES: &str = include_str!("package/THIRD-PARTY-NOTICES.txt");

pub(crate) fn validate_notices() -> Result<()> {
    let hash = format!("{:x}", Sha256::digest(include_bytes!("../Cargo.lock")));
    if !NOTICES.contains(&format!("\nCargo.lock SHA-256: {hash}\n")) {
        return Err(
            "third-party notices are stale; run jbang scripts/ThirdPartyNotices.java".into(),
        );
    }
    Ok(())
}

fn ignored(name: &str, droid: bool) -> bool {
    matches!(name, "__pycache__" | ".pytest_cache" | ".DS_Store" | "lib")
        || name.ends_with(".pyc")
        || droid
            && (matches!(
                name,
                ".claude-plugin"
                    | ".codex-plugin"
                    | ".cursor-plugin"
                    | ".github"
                    | ".antigravity-plugin"
                    | "hooks.json"
                    | "codex-skills"
                    | "hermes"
                    | "opencode"
                    | "pi"
                    | "slash-commands"
                    | "package.json"
            ) || name.ends_with(".md.bak"))
}

fn entries(path: &Path) -> io::Result<Vec<PathBuf>> {
    let mut entries = fs::read_dir(path)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<io::Result<Vec<_>>>()?;
    entries.sort();
    Ok(entries)
}

fn copy_tree(source: &Path, target: &Path, droid: bool) -> Result<()> {
    fs::create_dir_all(target)?;
    for path in entries(source)? {
        let name = path.file_name().ok_or("missing source filename")?;
        if ignored(&name.to_string_lossy(), droid) {
            continue;
        }
        let dest = target.join(name);
        if path.is_dir() {
            copy_tree(&path, &dest, droid)?;
        } else {
            fs::copy(&path, &dest)?;
            copy_metadata(&path, &dest)?;
        }
    }
    copy_metadata(source, target)
}

fn copy_metadata(source: &Path, target: &Path) -> Result<()> {
    let metadata = fs::metadata(source)?;
    File::open(target)?.set_times(
        FileTimes::new()
            .set_accessed(metadata.accessed()?)
            .set_modified(metadata.modified()?),
    )?;
    fs::set_permissions(target, metadata.permissions())?;
    Ok(())
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
    fs::create_dir_all(path.parent().ok_or("missing JSON directory")?)?;
    let mut file = File::create(path)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    writeln!(file)?;
    Ok(())
}

fn droid_marketplace() -> Value {
    json!({
        "name":"tokeneconomizer-droid",
        "description":"TokenEconomizer for Factory Droid: learned repository facts, priced sessions, and a failure-loop guard",
        "owner":{"name":"TokenEconomizer authors"},
        "plugins":[{"name":"tokeneconomizer","source":"./tokeneconomizer",
            "description":"Micro-skills injected at session start, session cost from Droid's own usage, and a guard for turns stuck in a failing loop.",
            "category":"productivity"}]
    })
}

/// Assemble the pinned assets. The test transport supplies no binaries to retain its reference payload.
pub fn build(source: &Path, binaries: Option<&Path>, out: &Path, droid: bool) -> Result<PathBuf> {
    let source = fs::canonicalize(source)?;
    let out = paths::realpath(out)?;
    let plugin = out.join("tokeneconomizer");
    for input in ["plugin", "tokeneconomizer", ".claude-plugin"] {
        let input = source.join(input);
        if plugin.starts_with(&input) || input.starts_with(&plugin) {
            return Err("bundle output overlaps its source".into());
        }
    }
    if !source.join("plugin").is_dir()
        || binaries.is_none() && !source.join("tokeneconomizer").is_dir()
    {
        return Err("missing plugin source payload".into());
    }
    let suffix = if let Some(binaries) = binaries {
        validate_notices()?;
        let binaries = fs::canonicalize(binaries)?;
        if binaries.starts_with(&plugin) {
            return Err("bundle output contains its binary inputs".into());
        }
        let suffix = if binaries.join("te.exe").is_file() {
            ".exe"
        } else {
            ""
        };
        for name in ["te", "te-hook", "te-repomap-gen"] {
            let path = binaries.join(format!("{name}{suffix}"));
            if !path.is_file() || !paths::executable(&path)? {
                return Err(format!("missing executable: {}", path.display()).into());
            }
        }
        suffix
    } else {
        ""
    };
    let mut market = if droid {
        droid_marketplace()
    } else {
        serde_json::from_slice(&fs::read(source.join(".claude-plugin/marketplace.json"))?)?
    };
    for entry in market["plugins"]
        .as_array_mut()
        .ok_or("marketplace plugins must be a list")?
    {
        if entry["name"] == "tokeneconomizer" {
            entry["source"] = json!("./tokeneconomizer");
        }
    }
    if fs::symlink_metadata(&plugin).is_ok() {
        if plugin.is_symlink() || plugin.is_file() {
            fs::remove_file(&plugin)?;
        } else {
            fs::remove_dir_all(&plugin)?;
        }
    }
    copy_tree(&source.join("plugin"), &plugin, droid)?;
    if droid {
        fs::copy(
            source.join("plugin/hooks/droid-hooks.json"),
            plugin.join("hooks/hooks.json"),
        )?;
        for name in ["droid", "codex", "copilot", "cursor"] {
            let path = plugin.join(format!("hooks/{name}-hooks.json"));
            if path.exists() {
                fs::remove_file(path)?;
            }
        }
        let commands = plugin.join("commands");
        fs::create_dir_all(&commands)?;
        for path in entries(&source.join("plugin/slash-commands"))? {
            if path.extension().is_some_and(|ext| ext == "md") {
                let text = fs::read_to_string(&path)?
                    .replace("${CLAUDE_PLUGIN_ROOT}", "${DROID_PLUGIN_ROOT}")
                    .replace("with the Bash tool", "with the Execute tool");
                fs::write(
                    commands.join(format!(
                        "tokeneconomizer-{}",
                        path.file_name().unwrap().to_string_lossy()
                    )),
                    text,
                )?;
            }
        }
    }
    if let Some(binaries) = binaries {
        native_payload(&plugin, binaries, suffix, droid)?;
        fs::copy(source.join("LICENSE"), plugin.join("LICENSE"))?;
    } else {
        copy_tree(
            &source.join("tokeneconomizer"),
            &plugin.join("lib/tokeneconomizer"),
            false,
        )?;
    }
    let manifest = if droid {
        ".factory-plugin"
    } else {
        ".claude-plugin"
    };
    write_json(&out.join(manifest).join("marketplace.json"), &market)?;
    Ok(plugin)
}

fn native_payload(plugin: &Path, binaries: &Path, suffix: &str, droid: bool) -> Result<()> {
    let scripts = plugin.join("scripts");
    fs::write(plugin.join("README.md"), include_str!("package/README.md"))?;
    fs::write(plugin.join("THIRD-PARTY-NOTICES.txt"), NOTICES)?;
    fs::remove_file(scripts.join("te_bootstrap.py"))?;
    for name in ["te", "te-hook", "te-repomap-gen"] {
        let source = binaries.join(format!("{name}{suffix}"));
        let target = scripts.join(format!("{name}{suffix}"));
        fs::copy(&source, &target)?;
        copy_metadata(&source, &target)?;
    }
    for name in ["te", "te-hook"] {
        if suffix == ".exe" {
            fs::write(
                scripts.join(name),
                format!(
                    "#!/bin/sh\ndir=$(CDPATH= cd -- \"${{0%/*}}\" && pwd)\nexec \"$dir/{name}.exe\" \"$@\"\n"
                ),
            )?;
        }
        fs::write(
            scripts.join(format!("{name}.cmd")),
            format!("@echo off\r\n\"%~dp0{name}.exe\" %*\r\nexit /b %ERRORLEVEL%\r\n"),
        )?;
    }
    if droid {
        return Ok(());
    }
    fs::write(
        plugin.join("pi/tokeneconomizer.ts"),
        include_str!("plugin/pi/sdk.ts"),
    )?;
    fs::write(
        plugin.join("opencode/tokeneconomizer.ts"),
        include_str!("plugin/opencode/sdk.ts"),
    )?;
    let hermes = plugin.join("hermes/__init__.py");
    fs::write(hermes, include_str!("plugin/hermes/sdk.py"))?;
    for host in ["codex", "copilot"] {
        let path = plugin.join(format!("hooks/{host}-hooks.json"));
        let mut hooks: Value = serde_json::from_slice(&fs::read(&path)?)?;
        let field = if host == "codex" {
            "commandWindows"
        } else {
            "windows"
        };
        let script = windows_hook(host);
        let command = if host == "codex" {
            // EncodedCommand avoids the different quoting rules of cmd.exe and PowerShell.
            format!(
                "powershell.exe -NoProfile -NonInteractive -EncodedCommand {}",
                base64(
                    &script
                        .encode_utf16()
                        .flat_map(u16::to_le_bytes)
                        .collect::<Vec<_>>()
                )
            )
        } else {
            format!("& {{ {script} }}")
        };
        for groups in hooks["hooks"]
            .as_object_mut()
            .ok_or("hook events must be an object")?
            .values_mut()
        {
            for group in groups.as_array_mut().ok_or("hook groups must be a list")? {
                for hook in group["hooks"]
                    .as_array_mut()
                    .ok_or("hooks must be a list")?
                {
                    hook[field] = json!(command);
                }
            }
        }
        write_json(&path, &hooks)?;
    }
    Ok(())
}

fn windows_hook(host: &str) -> String {
    let root = if host == "copilot" {
        "$root = $env:COPILOT_PLUGIN_ROOT; if (-not $root) { $root = $env:PLUGIN_ROOT }"
    } else {
        "$root = $env:PLUGIN_ROOT; if (-not $root) { $root = $env:CLAUDE_PLUGIN_ROOT }"
    };
    format!(
        "$OutputEncoding = [Console]::InputEncoding = [Console]::OutputEncoding = New-Object System.Text.UTF8Encoding; {root}; $payload = if ($MyInvocation.ExpectingInput) {{ $input -join \"`n\" }} else {{ [Console]::In.ReadToEnd() }}; $exe = Join-Path $root 'scripts/te-hook.exe'; if (-not (Test-Path -LiteralPath $exe)) {{ '{{}}'; exit 0 }}; $payload | & $exe --host {host}; exit $LASTEXITCODE"
    )
}

pub fn archive(plugin: &Path) -> Result<PathBuf> {
    let parent = plugin.parent().ok_or("missing marketplace directory")?;
    let path = parent
        .parent()
        .ok_or("missing archive directory")?
        .join("tokeneconomizer-plugin.zip");
    let mut zip = ZipWriter::new(File::create(&path)?);
    archive_tree(&mut zip, parent, plugin)?;
    zip.finish()?;
    Ok(path)
}

pub(crate) fn archive_tree(zip: &mut ZipWriter<File>, root: &Path, path: &Path) -> Result<()> {
    let name = path
        .strip_prefix(root)?
        .to_str()
        .ok_or("archive paths must be UTF-8")?
        .replace('\\', "/");
    let metadata = fs::metadata(path)?;
    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode()
    };
    #[cfg(not(unix))]
    let mode = if path.is_dir()
        || path
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|name| name == "scripts")
    {
        0o755
    } else {
        0o644
    };
    let modified: DateTime<Local> = metadata.modified()?.into();
    let stamp = ZipTime::from_date_and_time(
        modified.year().try_into()?,
        modified.month() as u8,
        modified.day() as u8,
        modified.hour() as u8,
        modified.minute() as u8,
        modified.second() as u8,
    )?;
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(mode)
        .last_modified_time(stamp)
        .large_file(metadata.len() >= u32::MAX as u64);
    if path.is_dir() {
        zip.add_directory(name, options)?;
        for child in entries(path)? {
            archive_tree(zip, root, &child)?;
        }
    } else {
        zip.start_file(name, options)?;
        io::copy(&mut File::open(path)?, zip)?;
    }
    Ok(())
}
