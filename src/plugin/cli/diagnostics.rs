use super::{Error, Result, Runtime, text, when};
use crate::{
    plugin::{
        hosts::{
            self,
            session::{self, Environment},
        },
        paths, repos, sidecall,
    },
    value::{json_text, python_repr, truthy},
};
use serde_json::{Value, json};
use std::{
    fs,
    net::{Ipv4Addr, Ipv6Addr},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

// Characters whose NFKC decomposition introduces a URL authority delimiter.
const NETLOC_DELIMITERS: &[char] = &[
    '\u{2047}', '\u{2048}', '\u{2049}', '\u{2100}', '\u{2101}', '\u{2105}', '\u{2106}', '\u{2a74}',
    '\u{fe13}', '\u{fe16}', '\u{fe55}', '\u{fe56}', '\u{fe5f}', '\u{fe6b}', '\u{ff03}', '\u{ff0f}',
    '\u{ff1a}', '\u{ff1f}', '\u{ff20}',
];

pub fn netloc(url: &str) -> Result<String> {
    let url: String = url
        .trim_start_matches(|c| c <= ' ')
        .chars()
        .filter(|c| !matches!(c, '\t' | '\r' | '\n'))
        .collect();
    let mut tail = url.as_str();
    if let Some((scheme, rest)) = url.split_once(':')
        && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    {
        tail = rest;
    }
    let Some(tail) = tail.strip_prefix("//") else {
        return Ok(String::new());
    };
    let authority = tail.split(['/', '?', '#']).next().unwrap_or("");
    if authority.contains('[') != authority.contains(']') {
        return Err(Error::new("ValueError", "Invalid IPv6 URL"));
    }
    if authority.contains('[') {
        let host_port = authority.rsplit('@').next().unwrap_or("");
        let host = if let Some((before, rest)) = host_port.split_once('[') {
            if !before.is_empty() {
                return Err(Error::new("ValueError", "Invalid IPv6 URL"));
            }
            let (host, port) = rest
                .split_once(']')
                .ok_or_else(|| Error::new("ValueError", "Invalid IPv6 URL"))?;
            if !port.is_empty() && !port.starts_with(':') {
                return Err(Error::new("ValueError", "Invalid IPv6 URL"));
            }
            host
        } else {
            host_port.split(':').next().unwrap_or("")
        };
        if host.starts_with(['v', 'V']) {
            let valid = host[1..].split_once('.').is_some_and(|(version, tail)| {
                !version.is_empty()
                    && version.chars().all(|c| c.is_ascii_hexdigit())
                    && !tail.is_empty()
            });
            if !valid {
                return Err(Error::new("ValueError", "IPvFuture address is invalid"));
            }
        } else {
            if host.parse::<Ipv4Addr>().is_ok() {
                return Err(Error::new(
                    "ValueError",
                    "An IPv4 address cannot be in brackets",
                ));
            }
            let (address, scope) = host
                .split_once('%')
                .map_or((host, None), |(a, s)| (a, Some(s)));
            if address.parse::<Ipv6Addr>().is_err()
                || scope.is_some_and(|s| s.is_empty() || s.contains('%'))
            {
                return Err(Error::new(
                    "ValueError",
                    format!(
                        "{} does not appear to be an IPv4 or IPv6 address",
                        python_repr(&json!(host))
                    ),
                ));
            }
        }
    }
    if authority.chars().any(|c| NETLOC_DELIMITERS.contains(&c)) {
        return Err(Error::new(
            "ValueError",
            format!("netloc '{authority}' contains invalid characters under NFKC normalization"),
        ));
    }
    Ok(authority.into())
}

pub fn rtk_hooks(home: &Path) -> Result<Vec<PathBuf>> {
    let mut candidates = vec![home.join(".claude").join("settings.json")];
    if let Ok(entries) = fs::read_dir(home.join(".copilot").join("hooks")) {
        for entry in entries {
            let Ok(entry) = entry else {
                break;
            };
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with('.')
                && if cfg!(windows) {
                    name.to_lowercase().ends_with(".json")
                } else {
                    name.ends_with(".json")
                }
            {
                candidates.push(entry.path());
            }
        }
    }
    let mut found = Vec::new();
    for path in candidates {
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };
        let text =
            std::str::from_utf8(&bytes).map_err(|e| Error::new("ValueError", e.to_string()))?;
        if text.contains("rtk") {
            found.push(path);
        }
    }
    Ok(found)
}
fn modified(path: &Path) -> Result<f64> {
    let time = fs::metadata(path)?.modified()?;
    Ok(time
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or_else(|e| -e.duration().as_secs_f64()))
}
pub fn recent_host(cwd: &Path, environment: &Environment) -> Result<Value> {
    // shortcut: repository-identity errors are strings, retain typed I/O errors in the path parity audit.
    let Ok(repo) =
        paths::repo_identity(&environment.path(cwd), &environment.env, &environment.home)
    else {
        return Ok(Value::Null);
    };
    let mut latest = None;
    for host in hosts::HOSTS {
        let dir = paths::repo_state_dir(&repo, host, &environment.env, &environment.home)?;
        if !dir.is_dir() {
            continue;
        }
        let mut time = modified(&dir)?;
        for entry in fs::read_dir(&dir)? {
            time = time.max(modified(&entry?.path())?);
        }
        if latest.as_ref().is_none_or(|(_, previous)| time > *previous) {
            latest = Some((host, time));
        }
    }
    Ok(latest.map_or(Value::Null, |(host, _)| json!(host)))
}
fn env<'a>(environment: &'a Environment, key: &str) -> &'a str {
    environment
        .env
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
}
fn package(settings: &Path) -> Result<Option<Value>> {
    let value = fs::read(settings)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|doc| doc.get("packages").cloned())
        .filter(truthy)
        .unwrap_or(json!([]));
    let entries: Vec<Value> = match value {
        Value::Array(entries) => entries,
        Value::Object(entries) => entries.into_iter().map(|(key, _)| json!(key)).collect(),
        Value::String(text) => text.chars().map(|c| json!(c.to_string())).collect(),
        _ => return Err(Error::new("TypeError", "packages must be iterable")),
    };
    Ok(entries
        .into_iter()
        .find(|entry| json_text(entry).to_lowercase().contains("tokeneconomizer")))
}

pub fn doctor(args: &Value, environment: &Environment, runtime: &mut dyn Runtime) -> Result<i32> {
    let info = runtime.global("runtime", json!([]))?;
    let parts: Vec<u64> =
        serde_json::from_value(info["parts"].clone()).map_err(|e| e.to_string())?;
    let minimum: Vec<u64> =
        serde_json::from_value(info["minimum"].clone()).map_err(|e| e.to_string())?;
    let mut ok = parts >= minimum;
    let name = info["name"].as_str().ok_or("runtime name must be text")?;
    let version = info["version"]
        .as_str()
        .ok_or("runtime version must be text")?
        .split_whitespace()
        .next()
        .unwrap_or("");
    runtime.say(&format!(
        "{name:12}{version} {}",
        if ok {
            "ok".into()
        } else {
            format!("(needs >= {})", text(&info["required"]))
        }
    ))?;
    let root = runtime.global("state_root", json!([]))?;
    let root = Path::new(root.as_str().ok_or("state root must be text")?);
    let writable = (|| -> std::io::Result<()> {
        fs::create_dir_all(root)?;
        let probe = root.join(".write-test");
        fs::File::create(&probe)?;
        fs::remove_file(probe)
    })();
    match writable {
        Ok(()) => runtime.say(&format!("state root  {} (writable)", root.display()))?,
        Err(error) => {
            runtime.say(&format!(
                "state root  {} NOT writable: {error}",
                root.display()
            ))?;
            ok = false;
        }
    }
    let mut agents = Vec::new();
    for host in [
        "antigravity",
        "claude",
        "codex",
        "copilot",
        "droid",
        "grok",
        "hermes",
        "opencode",
        "pi",
    ] {
        let command = match host {
            "claude" => runtime.global("claude_cli", json!([]))?,
            "hermes" => runtime.global("hermes_cli", json!([]))?,
            "antigravity" => json!("agy"),
            _ => json!(host),
        };
        if truthy(&runtime.global("which", json!([command]))?) {
            agents.push(host);
        }
    }
    runtime.say(&format!("agents      {}",if agents.is_empty() {"none of antigravity (agy), claude, codex, copilot, droid, grok, hermes, opencode, pi on PATH".into()} else {agents.join(", ")}))?;
    let ctx = runtime.context(args)?;
    for host in &agents {
        let pause = sidecall::paused(host, root);
        if truthy(&pause) {
            runtime.say(&format!("{host:11} model calls paused since {}: `{}` said it is not signed in; `config set microskills=on` (or `loopguard=on`) tries again",when(&pause["at"])?,sidecall::call_name(host)))?;
        }
    }
    if agents.contains(&"claude") {
        let present = root
            .join("hosts")
            .join("claude")
            .join("catalog.json")
            .exists();
        runtime.say(&format!(
            "claude      catalog {}",
            if present { "present" } else { "not probed" }
        ))?;
    }
    if agents.contains(&"codex") {
        let trusted = runtime.call(&ctx, "codex_trusted", json!([]))?;
        if !trusted.is_null() {
            runtime.say(&format!("codex       this project is {} (an untrusted project's .codex/config.toml is ignored)",if truthy(&trusted) {"trusted"} else {"NOT trusted"}))?;
        }
    }
    if agents.contains(&"copilot") {
        runtime.say(&format!(
            "copilot     catalog {}",
            if root
                .join("hosts")
                .join("copilot")
                .join("catalog.json")
                .exists()
            {
                "present"
            } else {
                "not probed"
            }
        ))?;
        let base = env(environment, "COPILOT_PROVIDER_BASE_URL");
        let route = if base.is_empty() {
            "Copilot subscription".into()
        } else {
            let provider = env(environment, "COPILOT_PROVIDER_TYPE");
            let authority = netloc(base)?;
            format!(
                "BYOK {} at {}",
                if provider.is_empty() {
                    "openai"
                } else {
                    provider
                },
                if authority.is_empty() {
                    base
                } else {
                    &authority
                }
            )
        };
        let model = sidecall::copilot_model("", &environment.env);
        runtime.say(&format!(
            "copilot     {route}; side calls and `probe` use model {}",
            if model.is_empty() {
                "(Copilot default)"
            } else {
                &model
            }
        ))?;
        if !base.is_empty() {
            runtime.say("copilot     note: Copilot does not pass COPILOT_PROVIDER_* to the plugin's hooks, so its model calls (micro-skills' proposer, the loop guard's judge) cannot use this provider")?;
        }
        let trusted = runtime.call(&ctx, "copilot_folder_trusted", json!([args["cwd"]]))?;
        if !trusted.is_null() {
            runtime.say(&format!("copilot     this folder is {} (an untrusted folder's .github/copilot settings are ignored)",if truthy(&trusted) {"trusted"} else {"NOT trusted"}))?;
        }
    }
    if agents.contains(&"droid") {
        let sessions = env(environment, "DROID_SESSIONS_DIR");
        let sessions = if sessions.is_empty() {
            environment.home.join(".factory").join("sessions")
        } else {
            PathBuf::from(sessions)
        };
        let side = if truthy(&ctx.config["droid_sidecall"]) {
            "on (the `droid_sidecall` setting)"
        } else if sidecall::droid_enabled(&environment.env) {
            "on (TE_DROID_SIDECALL=1)"
        } else {
            "off (set `droid_sidecall=on` or TE_DROID_SIDECALL=1 to try them)"
        };
        runtime.say(&format!(
            "droid       sessions {}; side calls {side}; tool pruning is not available in the TUI",
            sessions.display()
        ))?;
    }
    if agents.contains(&"hermes") {
        let link = session::hermes_home(&environment.env, &environment.home)
            .join("plugins")
            .join("tokeneconomizer");
        runtime.say(&format!(
            "hermes      plugin {} (it also needs `hermes plugins enable tokeneconomizer`)",
            if environment.path(&link).exists() {
                format!("linked at {}", link.display())
            } else {
                format!("not in {}", link.display())
            }
        ))?;
    }
    if agents.contains(&"pi") {
        let dir = env(environment, "PI_CODING_AGENT_DIR");
        let settings = if dir.is_empty() {
            environment.home.join(".pi").join("agent")
        } else {
            PathBuf::from(dir)
        }
        .join("settings.json");
        let mine = package(&environment.path(&settings))?;
        runtime.say(&format!(
            "pi          package {}",
            if let Some(mine) = mine {
                format!("installed: {}", text(&mine))
            } else {
                format!("not in {}", settings.display())
            }
        ))?;
    }
    runtime.say(&format!(
        "repository  {}: {}; state {}",
        ctx.repo.root.display(),
        repos::describe(&ctx.repo),
        ctx.repo.key
    ))?;
    let home_git = paths::home_dir(&environment.env, &environment.home)?
        .unwrap_or_default()
        .join(".git");
    let cwd = args["cwd"]
        .as_str()
        .ok_or("working directory must be text")?;
    for path in repos::ignored_git_entries(&environment.path(Path::new(cwd)))? {
        if paths::realpath(&path)? != home_git {
            runtime.say(&format!(
                "repository  ignored {}: not a git repository (no HEAD, or a dangling gitdir)",
                path.display()
            ))?;
        }
    }
    if let Some(note) = repos::home_git_note(&environment.env, &environment.home)? {
        runtime.say(&format!("home        {note}"))?;
    }
    if repos::needs_migration(root)? {
        runtime.say("state       recorded by an earlier version and not yet migrated: `te repos migrate` (runs by itself at the next session start)")?;
    }
    let python = runtime.call(&ctx, "repomap_python", json!([]))?;
    runtime.say(&format!(
        "repomap     {}; {}",
        text(&ctx.config["repomap"]),
        if truthy(&python) {
            format!("aider found ({})", text(&python))
        } else {
            "no Python with aider found (pip install aider-chat, or set repomap_python)".into()
        }
    ))?;
    let rtk = runtime.global("which", json!(["rtk"]))?;
    let hooks = rtk_hooks(&environment.home)?;
    if truthy(&rtk) || !hooks.is_empty() {
        runtime.say(&format!("rtk         {}; hook in {}. Compatible. Note: `rtk pytest` prints nothing when pytest is missing, so the plugin cannot learn that fact from it",if truthy(&rtk) {text(&rtk)} else {"not on PATH".into()},if hooks.is_empty() {"no settings file".into()} else {hooks.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>().join(", ")}))?;
    }
    let log = root.join("errors.log");
    if log.exists() {
        let bytes = fs::read(log)?;
        let text = String::from_utf8_lossy(&bytes).replace("\r\n", "\n");
        let lines: Vec<_> = text
            .split_terminator([
                '\n', '\r', '\u{b}', '\u{c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2028}',
                '\u{2029}',
            ])
            .collect();
        runtime.say(&format!(
            "errors.log (tail):\n  {}",
            lines[lines.len().saturating_sub(6)..].join("\n  ")
        ))?;
    } else {
        runtime.say("errors.log  none")?;
    }
    Ok(i32::from(!ok))
}
