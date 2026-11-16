pub mod toml;

use regex::Regex;
use serde_json::{Map, Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::LazyLock,
    time::{SystemTime, UNIX_EPOCH},
};

use super::{applied, paths, process};
use crate::{
    store::Store,
    trace::copilot::mcp_server_of,
    value::{json_pretty, scalar_text, truthy},
};

pub const SETTINGS_REL: &str = ".claude/settings.local.json";
pub const CODEX_CONFIG_REL: &str = ".codex/config.toml";
pub const COPILOT_SETTINGS_REL: &str = ".github/copilot/settings.local.json";
pub const MARK: &str = "# tokeneconomizer";
pub const CODEX_LIVE_CAPABILITIES: [&str; 3] = ["collaboration", "goals", "image_media"];
pub type Toggle = (Option<&'static str>, &'static str, &'static str);
pub fn toggles(capability: &str) -> &'static [Toggle] {
    match capability {
        "collaboration" => &[
            (None, "include_collaboration_mode_instructions", "false"),
            (Some("agents"), "enabled", "false"),
            (Some("features"), "multi_agent", "false"),
        ],
        "goals" => &[(Some("features"), "goals", "false")],
        "image_media" => &[(Some("features"), "view_image", "false")],
        _ => &[],
    }
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Refused(String),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => e.fmt(f),
            Self::Refused(s) => s.fmt(f),
        }
    }
}
impl std::error::Error for Error {}
impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<String> for Error {
    fn from(e: String) -> Self {
        Self::Refused(e)
    }
}

pub trait Runtime {
    fn write_record(&mut self, store: &Store, doc: &Value) -> Result<(), Error> {
        store.write(applied::APPLIED, doc)?;
        Ok(())
    }
    fn write_json(&mut self, path: &Path, doc: &Value) -> Result<(), Error> {
        write_json(path, doc)
    }
    fn write_text(&mut self, path: &Path, text: &str) -> Result<(), Error> {
        write_text(path, text)
    }
    fn git_tracked(&mut self, root: &Path, relative: &str) -> Result<bool, Error> {
        Ok(git_tracked(root, relative))
    }
    fn codex_config_error(
        &mut self,
        root: &Path,
        cli: &str,
        timeout: f64,
    ) -> Result<Option<String>, Error> {
        Ok(codex_config_error(root, cli, timeout))
    }
}
pub struct NativeRuntime;
impl Runtime for NativeRuntime {}

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}
fn set(values: &[String]) -> BTreeSet<String> {
    values.iter().cloned().collect()
}
fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .map(scalar_text)
        .collect()
}
fn read_record(store: &Store) -> Result<Value, Error> {
    Ok(store.read(applied::APPLIED, Value::Null)?)
}
fn save_record(
    store: &Store,
    by: &Map<String, Value>,
    kept: Option<&Map<String, Value>>,
    runtime: &mut dyn Runtime,
) -> Result<(), Error> {
    let doc = applied::document(store, by, kept, now())?;
    runtime.write_record(store, &doc)
}
fn normcase(path: &Path) -> String {
    let path = path.to_string_lossy().into_owned();
    if cfg!(windows) {
        path.replace('/', "\\").to_lowercase()
    } else {
        path
    }
}
fn text_read(path: &Path) -> io::Result<String> {
    fs::read_to_string(path).map(|s| s.replace("\r\n", "\n").replace('\r', "\n"))
}

pub fn supports(host: &str) -> bool {
    ["claude", "codex", "copilot"].contains(&host)
}
pub fn unit(host: &str, plural: bool) -> String {
    let unit = match host {
        "codex" => "capability",
        "copilot" => "MCP server",
        _ => "tool",
    };
    if plural {
        unit.strip_suffix('y')
            .map_or_else(|| format!("{unit}s"), |s| format!("{s}ies"))
    } else {
        unit.into()
    }
}
pub fn settings_path(root: &Path, host: &str) -> PathBuf {
    root.join(match host {
        "codex" => CODEX_CONFIG_REL,
        "copilot" => COPILOT_SETTINGS_REL,
        _ => SETTINGS_REL,
    })
}

pub fn read_settings(path: &Path) -> Result<Value, Error> {
    let text = match text_read(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(json!({})),
        Err(e) => {
            return Err(Error::Refused(format!(
                "cannot read {}: {e}",
                path.display()
            )));
        }
    };
    if text.trim().is_empty() {
        return Ok(json!({}));
    }
    let doc: Value = serde_json::from_str(&text).map_err(|e| {
        Error::Refused(format!(
            "refusing to edit {}: it is not valid JSON ({e}); fix it first",
            path.display()
        ))
    })?;
    if !doc.is_object() {
        return Err(Error::Refused(format!(
            "refusing to edit {}: it is not a JSON object",
            path.display()
        )));
    }
    Ok(doc)
}
pub fn guard_target(root: &Path, path: &Path) -> Result<(), Error> {
    let root = paths::realpath(root)?;
    let parent = paths::realpath(path.parent().unwrap_or(Path::new("")))?;
    if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
        || !parent.starts_with(root)
    {
        return Err(Error::Refused(format!(
            "refusing to write {}: it is, or goes through, a symlink that leads out of the repository",
            path.display()
        )));
    }
    Ok(())
}
pub fn write_json(path: &Path, doc: &Value) -> Result<(), Error> {
    write_text(path, &(json_pretty(doc, false) + "\n"))
}
pub fn write_text(path: &Path, text: &str) -> Result<(), Error> {
    fs::create_dir_all(
        path.parent()
            .ok_or_else(|| io::Error::other("missing settings directory"))?,
    )?;
    let temp = PathBuf::from(format!("{}.te.tmp", path.display()));
    fs::write(&temp, text)?;
    fs::rename(temp, path)?;
    Ok(())
}
pub fn exclude_from_git(root: &Path, relative: &str) -> Result<bool, Error> {
    let Some(gitdir) = paths::git_common_dir(root)? else {
        return Ok(false);
    };
    if root.join(".git").is_file() && !gitdir.join("HEAD").is_file() {
        return Ok(false);
    }
    let path = gitdir.join("info/exclude");
    if [path.parent().unwrap(), path.as_path()]
        .iter()
        .any(|p| fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink()))
    {
        return Ok(false);
    }
    let entry = format!("/{}", relative.replace(std::path::MAIN_SEPARATOR, "/"));
    let result = (|| -> io::Result<()> {
        let existing = if path.exists() {
            text_read(&path)?
        } else {
            String::new()
        };
        if existing.split_whitespace().any(|s| s == entry) {
            return Ok(());
        }
        fs::create_dir_all(path.parent().unwrap())?;
        let mut f = fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)?;
        write!(
            f,
            "{}# added by TokenEconomizer (per-user tool deny list)\n{entry}\n",
            if existing.is_empty() || existing.ends_with('\n') {
                ""
            } else {
                "\n"
            }
        )
    })();
    Ok(result.is_ok())
}

pub fn applied(store: &Store, path: Option<&Path>) -> Result<Vec<String>, Error> {
    let by = applied::applied_by_path(&read_record(store)?)?;
    if let Some(path) = path {
        let canonical = applied::canonical(&path.to_string_lossy())?;
        let values = by
            .get(&canonical)
            .filter(|v| truthy(v))
            .or_else(|| by.get(""));
        Ok(values.map(strings).unwrap_or_default())
    } else {
        Ok(by
            .values()
            .flat_map(strings)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect())
    }
}
pub fn applied_for(store: &Store, root: &Path, host: &str) -> Result<Vec<String>, Error> {
    applied(store, Some(&settings_path(root, host)))
}
pub fn kept_for(store: &Store, root: &Path, host: &str) -> Result<Vec<String>, Error> {
    let kept = applied::kept_by_path(&read_record(store)?)?;
    Ok(kept
        .get(&applied::canonical(
            &settings_path(root, host).to_string_lossy(),
        )?)
        .map(strings)
        .unwrap_or_default())
}
pub fn set_kept(
    store: &Store,
    root: &Path,
    host: &str,
    add: &[String],
    remove: &[String],
    runtime: &mut dyn Runtime,
) -> Result<(), Error> {
    let doc = read_record(store)?;
    let mut kept = applied::kept_by_path(&doc)?;
    let path = applied::canonical(&settings_path(root, host).to_string_lossy())?;
    let mut values = set(&kept.get(&path).map(strings).unwrap_or_default());
    values.extend(add.iter().cloned());
    for item in remove {
        values.remove(item);
    }
    kept.insert(path, json!(values));
    save_record(
        store,
        &applied::applied_by_path(&doc)?,
        Some(&kept),
        runtime,
    )
}
pub fn record(
    store: &Store,
    items: &[String],
    path: &Path,
    runtime: &mut dyn Runtime,
) -> Result<(), Error> {
    let mut by = applied::applied_by_path(&read_record(store)?)?;
    by.remove("");
    let mut items = items.to_vec();
    items.sort();
    by.insert(applied::canonical(&path.to_string_lossy())?, json!(items));
    save_record(store, &by, None, runtime)
}
pub fn json_list_keys(host: &str) -> (Option<&'static str>, &'static str) {
    if host == "claude" {
        (Some("permissions"), "deny")
    } else {
        (None, "disabledMcpServers")
    }
}
pub fn apply_json_list(
    root: &Path,
    items: &[String],
    store: &Store,
    host: &str,
    runtime: &mut dyn Runtime,
) -> Result<Vec<String>, Error> {
    let path = settings_path(root, host);
    guard_target(root, &path)?;
    let created = !path.exists();
    let mut data = read_settings(&path)?;
    let (outer, key) = json_list_keys(host);
    let holder = if let Some(outer) = outer {
        data.as_object_mut()
            .unwrap()
            .entry(outer)
            .or_insert(json!({}))
    } else {
        &mut data
    };
    let holder = holder
        .as_object_mut()
        .ok_or_else(|| Error::Refused("settings holder must be an object".into()))?;
    let list = holder
        .entry(key)
        .or_insert(json!([]))
        .as_array_mut()
        .ok_or_else(|| Error::Refused("settings deny list must be an array".into()))?;
    let added: Vec<_> = items
        .iter()
        .filter(|t| !list.contains(&json!(t)))
        .cloned()
        .collect();
    if added.is_empty() {
        return Ok(added);
    }
    list.extend(added.iter().map(|s| json!(s)));
    let owned = set(&applied(store, Some(&path))?)
        .union(&set(&added))
        .cloned()
        .collect::<Vec<_>>();
    // Record ownership first so a failed settings write can still be restored.
    record(store, &owned, &path, runtime)?;
    runtime.write_json(&path, &data)?;
    if created {
        exclude_from_git(
            root,
            match host {
                "copilot" => COPILOT_SETTINGS_REL,
                _ => SETTINGS_REL,
            },
        )?;
    }
    Ok(added)
}
pub fn restore_json_list(
    root: &Path,
    store: &Store,
    items: &[String],
    host: &str,
    runtime: &mut dyn Runtime,
) -> Result<Vec<String>, Error> {
    let mine = set(&applied_for(store, root, host)?);
    let target = if items.is_empty() {
        mine.clone()
    } else {
        mine.intersection(&set(items)).cloned().collect()
    };
    if target.is_empty() {
        return Ok(Vec::new());
    }
    let path = settings_path(root, host);
    guard_target(root, &path)?;
    let mut data = read_settings(&path)?;
    let (outer, key) = json_list_keys(host);
    let holder = if let Some(outer) = outer {
        data.get_mut(outer).filter(|v| truthy(v))
    } else {
        Some(&mut data)
    };
    let mut removed = Vec::new();
    let mut changed = false;
    let mut remove_outer = false;
    if let Some(holder) = holder {
        let current = holder
            .get(key)
            .filter(|v| truthy(v))
            .cloned()
            .unwrap_or(json!([]));
        let list = current
            .as_array()
            .ok_or_else(|| Error::Refused("settings deny list must be an array".into()))?;
        let contains = |v: &Value| v.as_str().is_some_and(|s| target.contains(s));
        let kept: Vec<_> = list.iter().filter(|v| !contains(v)).cloned().collect();
        removed = list
            .iter()
            .filter(|v| contains(v))
            .map(scalar_text)
            .collect();
        if let Some(map) = holder.as_object_mut().filter(|m| m.contains_key(key)) {
            if kept.is_empty() {
                map.remove(key);
            } else {
                map.insert(key.into(), json!(kept));
            }
            changed = true;
            remove_outer = outer.is_some() && map.is_empty();
        }
    }
    if changed {
        if remove_outer {
            data.as_object_mut().unwrap().remove(outer.unwrap());
        }
        runtime.write_json(&path, &data)?;
    }
    record(
        store,
        &mine.difference(&target).cloned().collect::<Vec<_>>(),
        &path,
        runtime,
    )?;
    Ok(removed)
}

pub fn git_tracked(root: &Path, relative: &str) -> bool {
    process::run(
        &[
            "git".into(),
            "ls-files".into(),
            "--error-unmatch".into(),
            relative.into(),
        ],
        root,
        10.0,
        false,
    )
    .is_ok_and(|o| o.code == 0)
}
pub fn codex_config_error(root: &Path, cli: &str, timeout: f64) -> Option<String> {
    process::which(cli)?;
    let mut argv = process::program(cli).ok()?;
    argv.extend(["features".into(), "list".into()]);
    let out = process::run(&argv, root, timeout, true).ok()?;
    if out.code == 0 {
        None
    } else {
        let error = if out.stderr.is_empty() {
            out.stdout
        } else {
            out.stderr
        };
        let error = error.trim().chars().take(500).collect::<String>();
        Some(if error.is_empty() {
            format!("exit {}", out.code)
        } else {
            error
        })
    }
}
pub fn apply_codex(
    root: &Path,
    caps: &[String],
    store: &Store,
    runtime: &mut dyn Runtime,
) -> Result<Vec<String>, Error> {
    let path = settings_path(root, "codex");
    guard_target(root, &path)?;
    let created = !path.exists();
    if !created && runtime.git_tracked(root, CODEX_CONFIG_REL)? {
        return Err(Error::Refused(format!(
            "{} is committed in this repository; the plugin will not edit a shared file. Add the toggles by hand, or keep them in your own copy of it.",
            path.display()
        )));
    }
    let original = if created {
        String::new()
    } else {
        text_read(&path)?
    };
    let mut text = original.clone();
    let mut added = Vec::new();
    for cap in caps {
        if toggles(cap).is_empty() || applied(store, Some(&path))?.contains(cap) {
            continue;
        }
        let mut changed = false;
        for (table, key, value) in toggles(cap) {
            let (result, did) = toml::insert(&text, *table, key, value, &format!("{MARK}:{cap}"));
            text = result;
            changed |= did;
        }
        if changed {
            added.push(cap.clone());
        }
    }
    if added.is_empty() {
        return Ok(added);
    }
    if !toml::valid(&text) {
        return Err(Error::Refused(format!(
            "refusing to write {}: the result would not parse as TOML",
            path.display()
        )));
    }
    runtime.write_text(&path, &text)?;
    if let Some(error) = runtime.codex_config_error(root, "codex", 30.0)? {
        if created {
            fs::remove_file(&path)?;
        } else {
            runtime.write_text(&path, &original)?;
        }
        return Err(Error::Refused(format!(
            "Codex rejected the edited {}, so it was put back: {error}",
            path.display()
        )));
    }
    if created {
        exclude_from_git(root, CODEX_CONFIG_REL)?;
    }
    let owned = set(&applied(store, Some(&path))?)
        .union(&set(&added))
        .cloned()
        .collect::<Vec<_>>();
    record(store, &owned, &path, runtime)?;
    Ok(added)
}
pub fn restore_codex(
    root: &Path,
    store: &Store,
    caps: &[String],
    runtime: &mut dyn Runtime,
) -> Result<Vec<String>, Error> {
    let mine = set(&applied_for(store, root, "codex")?);
    let target = if caps.is_empty() {
        mine.clone()
    } else {
        mine.intersection(&set(caps)).cloned().collect()
    };
    if target.is_empty() {
        return Ok(Vec::new());
    }
    let path = settings_path(root, "codex");
    guard_target(root, &path)?;
    if path.exists() {
        let text = toml::remove(
            &text_read(&path)?,
            &target
                .iter()
                .map(|c| format!("{MARK}:{c}"))
                .collect::<Vec<_>>(),
        );
        if text.trim().is_empty() {
            fs::remove_file(&path)?;
        } else {
            runtime.write_text(&path, &text)?;
        }
    }
    record(
        store,
        &mine.difference(&target).cloned().collect::<Vec<_>>(),
        &path,
        runtime,
    )?;
    Ok(target.into_iter().collect())
}
pub fn apply(
    root: &Path,
    items: &[String],
    store: &Store,
    host: &str,
    runtime: &mut dyn Runtime,
) -> Result<Vec<String>, Error> {
    if host == "codex" {
        apply_codex(root, items, store, runtime)
    } else {
        apply_json_list(root, items, store, host, runtime)
    }
}
pub fn restore(
    root: &Path,
    store: &Store,
    items: &[String],
    host: &str,
    runtime: &mut dyn Runtime,
) -> Result<Vec<String>, Error> {
    if host == "codex" {
        restore_codex(root, store, items, runtime)
    } else {
        restore_json_list(root, store, items, host, runtime)
    }
}

fn env<'a>(env: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    env.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}
pub fn copilot_trusted_folders(
    environment: &Map<String, Value>,
    home: &Path,
) -> Option<Vec<String>> {
    let root = env(environment, "COPILOT_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".copilot"));
    let text = text_read(&root.join("config.json")).ok()?;
    let text = toml::lines(&text)
        .into_iter()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let doc: Value = serde_json::from_str(&text).ok()?;
    Some(
        doc["trustedFolders"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(String::from)
            .collect(),
    )
}
pub fn copilot_folder_trusted(
    path: &Path,
    environment: &Map<String, Value>,
    home: &Path,
) -> Result<Option<bool>, Error> {
    let Some(folders) = copilot_trusted_folders(environment, home) else {
        return Ok(None);
    };
    let real = normcase(&paths::realpath(path)?);
    for folder in folders {
        let folder = normcase(&paths::realpath(Path::new(&folder))?);
        if real == folder
            || real.starts_with(
                &(folder
                    .trim_end_matches(std::path::MAIN_SEPARATOR)
                    .to_string()
                    + std::path::MAIN_SEPARATOR_STR),
            )
        {
            return Ok(Some(true));
        }
    }
    Ok(Some(false))
}
pub fn codex_home(environment: &Map<String, Value>, home: &Path) -> PathBuf {
    env(environment, "CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".codex"))
}
pub fn codex_project_trusted(
    root: &Path,
    environment: &Map<String, Value>,
    home: &Path,
) -> Result<Option<bool>, Error> {
    static PROJECT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"^\s*\[projects\.(?:"([^"]+)"|'([^']+)')\]\s*$"#).unwrap());
    static HEADER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*\[").unwrap());
    static TRUST: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"^\s*trust_level\s*=\s*"([^"]*)""#).unwrap());
    let Ok(text) = text_read(&codex_home(environment, home).join("config.toml")) else {
        return Ok(None);
    };
    let (mut levels, mut current) = (BTreeMap::new(), None);
    for line in toml::lines(&text) {
        if let Some(m) = PROJECT.captures(&line) {
            current = Some(normcase(Path::new(
                m.get(1).or_else(|| m.get(2)).unwrap().as_str(),
            )));
            continue;
        }
        if HEADER.is_match(&line) {
            current = None;
            continue;
        }
        if let (Some(current), Some(m)) = (&current, TRUST.captures(&line)) {
            levels.insert(current.clone(), m[1].to_string());
        }
    }
    let mut roots = vec![root.to_path_buf()];
    if let Some(main) = paths::main_checkout(root)? {
        roots.push(main);
    }
    for root in roots {
        for path in [Store::new(&root)?.base, paths::realpath(&root)?] {
            if let Some(level) = levels.get(&normcase(&path)) {
                return Ok(Some(level == "trusted"));
            }
        }
    }
    Ok(Some(false))
}
pub fn copilot_server_proposal(
    raw: &[String],
    catalog: &[String],
    used: &[String],
    servers: &[String],
) -> Vec<String> {
    let mut by = BTreeMap::<String, BTreeSet<String>>::new();
    for tool in catalog {
        if let Some(server) = mcp_server_of(tool, servers) {
            by.entry(server.into()).or_default().insert(tool.clone());
        }
    }
    let raw = set(raw);
    let used = set(used);
    by.into_iter()
        .filter(|(_, tools)| !tools.is_empty() && tools.is_subset(&raw) && tools.is_disjoint(&used))
        .map(|(server, _)| server)
        .collect()
}
