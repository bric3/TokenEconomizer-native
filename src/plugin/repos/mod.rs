mod merge;
mod read;
pub use merge::*;
pub use read::*;

use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use chrono::Local;
use serde_json::{Map, Value, json};

use super::paths::{self, BY_REMOTE, RepoId};
use crate::{
    store::Store,
    value::{float_or_zero, scalar_text, truthy},
};

pub const INDEX: &str = "index.json";
pub const LAYOUT: i64 = 2;
pub const PIN_MAX_AGE_S: f64 = 30.0 * 86400.0;
pub const LOCK_WAIT_S: f64 = 2.0;
pub const MIGRATE_RESPAWN_S: f64 = 600.0;
pub const LEARNED: [&str; 3] = ["injection.json", "untool.json", "mechanisms"];

pub fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}
fn stamp() -> String {
    Local::now().format("%Y%m%d-%H%M%S").to_string()
}
pub fn repos_dir(root: &Path) -> PathBuf {
    root.join("repos")
}
pub fn repo_dir(root: &Path, key: &str) -> PathBuf {
    repos_dir(root).join(key)
}
fn store(root: &Path) -> Result<Store, String> {
    Store::new(root).map_err(|e| e.to_string())
}
fn read(store: &Store, name: &str) -> Result<Value, String> {
    store.read(name, Value::Null).map_err(|e| e.to_string())
}
fn object(value: &Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}
fn items(value: &Value) -> Vec<Value> {
    value.as_array().cloned().unwrap_or_default()
}
fn number(value: &Value) -> Result<f64, String> {
    float_or_zero(value)
}
fn names(value: &Map<String, Value>) -> BTreeSet<String> {
    value.keys().cloned().collect()
}
fn list(path: &Path) -> Vec<String> {
    fs::read_dir(path)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            entry
                .ok()
                .map(|e| e.file_name().to_string_lossy().into_owned())
        })
        .collect()
}
fn modified(path: &Path) -> Option<f64> {
    fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|t| t.as_secs_f64())
}

pub fn record_error(root: &Path, error: &str) {
    let _ = (|| -> std::io::Result<()> {
        fs::create_dir_all(root)?;
        let mut f = fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(root.join("errors.log"))?;
        writeln!(
            f,
            "--- {} pid={} (repos)\n{error}",
            Local::now().format("%Y-%m-%dT%H:%M:%S"),
            std::process::id()
        )
    })();
}

pub fn pin_store(root: &Path, host: &str) -> Result<Store, String> {
    store(&root.join("sessions").join(host))
}
pub fn pin_name(session: &Value) -> String {
    scalar_text(session)
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "._-".contains(c) {
                c
            } else {
                '_'
            }
        })
        .take(200)
        .collect::<String>()
        + ".json"
}
pub fn repo_from(doc: &Value) -> Option<RepoId> {
    if !doc.is_object() || !truthy(&doc["key"]) || !truthy(&doc["root"]) {
        return None;
    }
    let text = |key| {
        if truthy(&doc[key]) {
            scalar_text(&doc[key])
        } else {
            String::new()
        }
    };
    Some(RepoId {
        root: PathBuf::from(text("root")),
        key: text("key"),
        label: text("label"),
        remote: text("remote"),
        how: text("how"),
    })
}
pub fn pinned(root: &Path, host: &str, session: &Value) -> Result<Option<RepoId>, String> {
    let doc = read(&pin_store(root, host)?, &pin_name(session))?;
    let Some(mut repo) = repo_from(&doc) else {
        return Ok(None);
    };
    if repo_dir(root, &repo.key).is_dir() {
        return Ok(Some(repo));
    }
    let redirects = object(&read(&store(root)?, INDEX)?["redirects"]);
    let mut key = repo.key.clone();
    let since = number(&doc["at"])?;
    for _ in 0..8 {
        let Some(redirect) = redirects.get(&key) else {
            break;
        };
        if number(&redirect["at"])? < since {
            break;
        }
        key = redirect["to"].as_str().unwrap_or("").into();
    }
    if !key.is_empty() {
        repo.key = key;
    }
    Ok(Some(repo))
}
pub fn pin(
    root: &Path,
    host: &str,
    session: &Value,
    repo: &RepoId,
    cwd: &Path,
) -> Result<(), String> {
    let mut doc = serde_json::to_value(repo).unwrap();
    doc["cwd"] = json!(cwd);
    doc["at"] = json!(now());
    pin_store(root, host)?
        .write(&pin_name(session), &doc)
        .map_err(|e| e.to_string())?;
    Ok(())
}
pub fn prune_pins(
    root: &Path,
    host: &str,
    time: Option<f64>,
    max_age: f64,
) -> Result<usize, String> {
    let st = pin_store(root, host)?;
    let time = time.filter(|n| *n != 0.0).unwrap_or_else(now);
    let marker = st.base.join(".pruned");
    if modified(&marker).is_some_and(|m| time - m < 86400.0) {
        return Ok(0);
    }
    let mut n = 0;
    let _ = (|| -> std::io::Result<()> {
        for entry in fs::read_dir(&st.base)? {
            let entry = entry?;
            let p = entry.path();
            if entry.file_name().to_string_lossy().ends_with(".json")
                && modified(&p).is_some_and(|m| time - m > max_age)
            {
                fs::remove_file(p)?;
                n += 1;
            }
        }
        fs::write(marker, "")
    })();
    Ok(n)
}

pub fn legacy_root(start: &Path, env: &Map<String, Value>) -> Result<Option<PathBuf>, String> {
    let mut ceilings = BTreeSet::new();
    for p in std::env::split_paths(
        env.get("GIT_CEILING_DIRECTORIES")
            .and_then(Value::as_str)
            .unwrap_or(""),
    ) {
        if !p.as_os_str().is_empty() {
            ceilings.insert(store(&p)?.base);
        }
    }
    let mut current = store(start)?.base;
    loop {
        if current.join(".git").exists() {
            return Ok(Some(current));
        }
        let Some(parent) = current.parent() else {
            return Ok(None);
        };
        if parent == current || ceilings.contains(parent) {
            return Ok(None);
        }
        current = parent.into();
    }
}
pub fn legacy_key(cwd: &Path, env: &Map<String, Value>) -> Result<String, String> {
    let root = legacy_root(cwd, env)?.unwrap_or(store(cwd)?.base);
    let remote = if root.join(".git").exists() {
        paths::normalize_remote(&paths::origin_url(&root)?.unwrap_or_default())
    } else {
        String::new()
    };
    let canonical = if remote.is_empty() {
        paths::realpath(&root)?.to_string_lossy().into_owned()
    } else {
        remote
    };
    Ok(format!(
        "{}-{}",
        paths::slug(root.file_name().and_then(|s| s.to_str()).unwrap_or("")),
        paths::digest(&canonical)
    ))
}

pub fn record_moves(idx: &mut Value, done: &[Value]) -> Result<(), String> {
    let moves: Vec<_> = done.iter().flat_map(|r| items(&r["moves"])).collect();
    if !moves.is_empty() {
        let old = &idx["migration"];
        let pending = !truthy(&old["noticed"]);
        let sessions = if pending {
            number(&old["sessions"])? as i64
        } else {
            0
        };
        let merged = if pending {
            number(&old["merged"])? as i64
        } else {
            0
        };
        idx["migration"] = json!({"at":now(),"noticed":false,"sessions":sessions+moves.iter().map(|m|m["moved"].as_i64().unwrap_or(0)).sum::<i64>(),"merged":merged+done.iter().filter(|r|truthy(&r["into"])).count() as i64});
    }
    Ok(())
}
pub fn candidates(
    root: &Path,
    repo: &RepoId,
    env: &Map<String, Value>,
    idx: &Value,
) -> Result<Vec<String>, String> {
    let mut keys = Vec::new();
    if idx["checked"].get(&repo.key).is_none() {
        keys.push(repo.key.clone());
    }
    keys.push(legacy_key(&repo.root, env)?);
    let real = paths::realpath(&repo.root)?.to_string_lossy().into_owned();
    if let Some(prev) = idx["checkouts"][&real]["key"]
        .as_str()
        .filter(|s| !s.is_empty())
    {
        keys.push(prev.into());
    }
    if repo.how == BY_REMOTE {
        let suffix = format!("-{}", repo.key.rsplit('-').next().unwrap_or(""));
        let mut extra: Vec<_> = list(&repos_dir(root))
            .into_iter()
            .filter(|n| n.ends_with(&suffix))
            .collect();
        extra.sort();
        keys.extend(extra);
    }
    let mut seen = BTreeSet::new();
    Ok(keys
        .into_iter()
        .filter(|k| seen.insert(k.clone()) && repo_dir(root, k).is_dir())
        .collect())
}
pub fn prune_index(idx: &mut Value, time: f64) -> Result<(), String> {
    let mut checkouts = Map::new();
    for (p, v) in object(&idx["checkouts"]) {
        if Path::new(&p).is_dir() || time - number(&v["seen"])? < PIN_MAX_AGE_S {
            checkouts.insert(p, v);
        }
    }
    idx["checkouts"] = json!(checkouts);
    Ok(())
}

pub fn adopt(
    root: &Path,
    repo: &RepoId,
    env: &Map<String, Value>,
    home: &Path,
    timeout: f64,
) -> Result<Vec<Value>, String> {
    let real = paths::realpath(&repo.root)?.to_string_lossy().into_owned();
    let st = store(root)?;
    let mut idx = json!(object(&read(&st, INDEX)?));
    if idx["checkouts"][&real]["key"] == repo.key {
        return Ok(Vec::new());
    }
    let lock = st.locked("index", timeout).map_err(|e| e.to_string())?;
    if !lock.acquired {
        return Ok(Vec::new());
    }
    idx = json!(object(&read(&st, INDEX)?));
    if idx["checkouts"][&real]["key"] == repo.key {
        return Ok(Vec::new());
    }
    let mut done = Vec::new();
    let mut ok = true;
    let mut cache = Map::new();
    for key in candidates(root, repo, env, &idx)? {
        let Some(result) =
            migrate_dir(root, &key, env, home, timeout, &mut cache, key == repo.key)?
        else {
            ok = false;
            continue;
        };
        done.push(result);
        idx = json!(object(&read(&st, INDEX)?));
        if repo_dir(root, &key).is_dir() {
            table(&mut idx, "checked")?.insert(key, json!(now()));
        }
        st.write(INDEX, &idx).map_err(|e| e.to_string())?;
    }
    note_repo(root, repo)?;
    if ok {
        idx = json!(object(&read(&st, INDEX)?));
        record_moves(&mut idx, &done)?;
        table(&mut idx, "checkouts")?.insert(real, json!({"key":repo.key,"seen":now()}));
        table(&mut idx, "checked")?.insert(repo.key.clone(), json!(now()));
        prune_index(&mut idx, now())?;
        st.write(INDEX, &idx).map_err(|e| e.to_string())?;
    }
    Ok(done)
}
fn table<'a>(doc: &'a mut Value, key: &str) -> Result<&'a mut Map<String, Value>, String> {
    if doc.get(key).is_none() {
        doc[key] = json!({});
    }
    doc[key]
        .as_object_mut()
        .ok_or_else(|| format!("expected {key} object"))
}

pub fn migrate_all(
    root: &Path,
    env: &Map<String, Value>,
    home: &Path,
    timeout: f64,
) -> Result<Option<Vec<Value>>, String> {
    let st = store(root)?;
    let lock = st.locked("index", timeout).map_err(|e| e.to_string())?;
    if !lock.acquired {
        return Ok(None);
    }
    let mut keys = list(&repos_dir(root));
    keys.sort();
    let mut done = Vec::new();
    let mut cache = Map::new();
    let mut complete = true;
    for key in keys {
        let mut idx = json!(object(&read(&st, INDEX)?));
        if idx["checked"].get(&key).is_some() || !repo_dir(root, &key).is_dir() {
            continue;
        }
        let Some(result) = migrate_dir(root, &key, env, home, timeout, &mut cache, false)? else {
            complete = false;
            continue;
        };
        done.push(result);
        idx = json!(object(&read(&st, INDEX)?));
        if repo_dir(root, &key).is_dir() {
            table(&mut idx, "checked")?.insert(key, json!(now()));
        }
        st.write(INDEX, &idx).map_err(|e| e.to_string())?;
    }
    let mut idx = json!(object(&read(&st, INDEX)?));
    record_moves(&mut idx, &done)?;
    if complete {
        idx["layout"] = json!(LAYOUT);
    }
    st.write(INDEX, &idx).map_err(|e| e.to_string())?;
    Ok(Some(done))
}
pub fn merge(
    root: &Path,
    src: &str,
    destination: &Value,
    timeout: f64,
) -> Result<Option<Value>, String> {
    let st = store(root)?;
    let lock = st.locked("index", timeout).map_err(|e| e.to_string())?;
    if !lock.acquired {
        return Ok(None);
    }
    let (key, label) = if let Some(key) = destination.as_str() {
        (key.to_string(), String::new())
    } else {
        let repo = repo_from(destination).ok_or("expected destination repository")?;
        note_repo(root, &repo)?;
        (repo.key, repo.label)
    };
    merge_whole(root, src, &key, &label, timeout)
}
pub fn needs_migration(root: &Path) -> Result<bool, String> {
    if number(&read(&store(root)?, INDEX)?["layout"])? >= LAYOUT as f64 {
        return Ok(false);
    }
    Ok(list(&repos_dir(root))
        .iter()
        .any(|k| repo_dir(root, k).is_dir()))
}
pub fn migration_notice(root: &Path, command: &str) -> Result<Option<String>, String> {
    let st = store(root)?;
    let doc = read(&st, INDEX)?;
    let m = &doc["migration"];
    if !truthy(m) || truthy(&m["noticed"]) {
        return Ok(None);
    }
    let lock = st.locked("index", LOCK_WAIT_S).map_err(|e| e.to_string())?;
    if !lock.acquired {
        return Ok(None);
    }
    let mut idx = json!(object(&read(&st, INDEX)?));
    if truthy(&idx["migration"]["noticed"]) {
        return Ok(None);
    }
    idx["migration"]["noticed"] = json!(true);
    st.write(INDEX, &idx).map_err(|e| e.to_string())?;
    let m = &idx["migration"];
    Ok(Some(format!(
        "TokenEconomizer: state recorded by an earlier version is now filed by repository ({} session(s) moved, {} directory(ies) merged; nothing deleted) -- see `{command}`.",
        m.get("sessions").map_or("0".into(), scalar_text),
        m.get("merged").map_or("0".into(), scalar_text)
    )))
}
pub fn settle(
    root: &Path,
    host: &str,
    repo: &RepoId,
    cwd: &Path,
    env: &Map<String, Value>,
    home: &Path,
    session: Option<&Value>,
) -> Result<bool, String> {
    let moved = match adopt(root, repo, env, home, LOCK_WAIT_S) {
        Ok(done) => done,
        Err(error) => {
            record_error(root, &error);
            Vec::new()
        }
    };
    if let Some(session) = session.filter(|s| truthy(s)) {
        pin(root, host, session, repo, cwd)?;
    }
    Ok(moved.iter().any(|r| truthy(&r["moves"])))
}

pub fn refold_targets(done: &[Value]) -> Vec<(String, PathBuf)> {
    done.iter()
        .flat_map(|r| items(&r["moves"]))
        .filter(|m| truthy(&m["refold"]) && truthy(&m["cwd"]))
        .filter_map(|m| {
            Some((
                m["host"].as_str()?.into(),
                PathBuf::from(m["cwd"].as_str()?),
            ))
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub fn finish_refolds(
    replies: &[Value],
    env: &Map<String, Value>,
    home: &Path,
) -> Result<(), String> {
    for reply in replies {
        if let Some(error) = reply.get("error").and_then(Value::as_str) {
            record_error(&paths::state_root(env, home)?, error);
        }
    }
    Ok(())
}

#[derive(serde::Serialize)]
pub struct MigrationOutcome {
    pub done: Option<Vec<Value>>,
    pub targets: Vec<(String, PathBuf)>,
}
pub fn run_migration(
    root: &Path,
    env: &Map<String, Value>,
    home: &Path,
) -> Result<MigrationOutcome, String> {
    let done = migrate_all(root, env, home, 60.0)?;
    if done.is_some() {
        let _ = fs::remove_file(root.join(".migrating"));
    }
    let targets = refold_targets(done.as_deref().unwrap_or_default());
    Ok(MigrationOutcome { done, targets })
}

#[derive(serde::Serialize)]
#[serde(tag = "action", rename_all = "lowercase")]
pub enum StartMigration {
    Skip,
    Inline(MigrationOutcome),
    Spawn {
        command: Vec<&'static str>,
        log: &'static str,
    },
}
pub fn start_migration(
    root: &Path,
    env: &Map<String, Value>,
    home: &Path,
) -> Result<StartMigration, String> {
    if !needs_migration(root)? {
        return Ok(StartMigration::Skip);
    }
    if env.get("TE_FOLD_SYNC").and_then(Value::as_str) == Some("1") {
        return Ok(StartMigration::Inline(run_migration(root, env, home)?));
    }
    let marker = root.join(".migrating");
    if modified(&marker).is_some_and(|time| now() - time < MIGRATE_RESPAWN_S) {
        return Ok(StartMigration::Skip);
    }
    fs::write(marker, "").map_err(|e| e.to_string())?;
    Ok(StartMigration::Spawn {
        command: vec!["repos", "migrate", "--quiet"],
        log: "migrate.log",
    })
}
