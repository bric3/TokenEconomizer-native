use super::*;
use crate::trace::jsonl::TextReader;
use regex::Regex;
use std::sync::LazyLock;

pub const HOSTS: [&str; 10] = [
    "claude",
    "codex",
    "copilot",
    "droid",
    "pi",
    "opencode",
    "cursor",
    "hermes",
    "antigravity",
    "grok",
];

pub fn host_dirs(directory: &Path) -> Vec<String> {
    let mut hosts: Vec<_> = list(directory)
        .into_iter()
        .filter(|h| HOSTS.contains(&h.as_str()) && directory.join(h).is_dir())
        .collect();
    hosts.sort();
    hosts
}
pub fn sessions(st: &Store) -> Result<Map<String, Value>, String> {
    let mut out = Map::new();
    let mut filenames = list(&st.base.join("sessions"));
    filenames.sort();
    for name in filenames.iter().filter(|n| n.ends_with(".json")) {
        let rec = read(st, &format!("sessions/{name}"))?;
        out.insert(name[..name.len() - 5].into(), json!(object(&rec)));
    }
    for sid in object(&read(st, "folded.json")?).keys() {
        out.entry(sid.clone()).or_insert(json!({}));
    }
    Ok(out)
}
pub fn transcript_cwd(path: &Path) -> Option<Value> {
    static CWD: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#""cwd"\s*:\s*("(?:[^"\\]|\\.)*")"#).unwrap());
    let mut reader = TextReader::open(path).ok()?;
    let mut text = String::new();
    let mut remaining = 262144;
    while remaining > 0 {
        let line = reader.readline(remaining).ok()?;
        if line.is_empty() {
            break;
        }
        remaining -= line.chars().count();
        text.push_str(&line);
    }
    CWD.captures(&text)
        .and_then(|c| serde_json::from_str::<Value>(&c[1]).ok())
}
pub fn transcript_of(rec: &Value, folded: &Value) -> Value {
    if truthy(&rec["transcript"]) {
        rec["transcript"].clone()
    } else if truthy(&folded["transcript"]) {
        folded["transcript"].clone()
    } else {
        json!("")
    }
}
pub fn session_cwd(rec: &Value, folded: &Value) -> Option<Value> {
    transcript_cwd(Path::new(transcript_of(rec, folded).as_str().unwrap_or("")))
        .filter(truthy)
        .or_else(|| rec.get("cwd").filter(|v| truthy(v)).cloned())
}
pub fn imported(entry: &Value) -> bool {
    entry.is_object() && truthy(&entry["imported"])
}
pub fn has_learned(st: &Store) -> Result<bool, String> {
    for name in LEARNED {
        let p = st.base.join(name);
        if p.is_file() || p.is_dir() && fs::read_dir(p).map_err(|e| e.to_string())?.next().is_some()
        {
            return Ok(true);
        }
    }
    Ok(false)
}
pub fn dir_label(directory: &Path) -> Result<String, String> {
    let doc = read(&store(directory)?, "repo.json")?;
    if truthy(&doc["label"]) {
        return Ok(scalar_text(&doc["label"]));
    }
    for host in host_dirs(directory) {
        let st = store(&directory.join(host))?;
        for sub in ["envfacts_state", "manual_state"] {
            if let Some(name) = list(&st.base.join("mechanisms").join(sub))
                .into_iter()
                .find(|n| n.ends_with(".json"))
            {
                return Ok(name[..name.len() - 5].replace("__", "/"));
            }
        }
        for rec in st
            .read_lines("ledger.jsonl", None)
            .map_err(|e| e.to_string())?
            .iter()
            .rev()
        {
            if truthy(&rec["repo"]) {
                return Ok(scalar_text(&rec["repo"]));
            }
        }
    }
    Ok(String::new())
}
pub fn note_repo(root: &Path, repo: &RepoId) -> Result<(), String> {
    let st = store(&repo_dir(root, &repo.key))?;
    if !st.base.is_dir() {
        return Ok(());
    }
    let mut doc = json!(object(&read(&st, "repo.json")?));
    let want = json!({"key":repo.key,"label":repo.label,"remote":repo.remote,"how":repo.how});
    if want
        .as_object()
        .unwrap()
        .iter()
        .any(|(key, value)| doc.get(key) != Some(value))
    {
        doc.as_object_mut().unwrap().extend(object(&want));
        doc["updated"] = json!(now());
        st.write("repo.json", &doc).map_err(|e| e.to_string())?;
    }
    Ok(())
}
pub fn how_words(how: &str, remote: &str) -> String {
    match how {
        paths::BY_REMOTE => format!("by its origin remote ({remote})"),
        paths::BY_PATH => "by its path (a git repository with no remote)".into(),
        _ => "by its directory (not a git repository)".into(),
    }
}
pub fn describe(repo: &RepoId) -> String {
    format!("identified {}", how_words(&repo.how, &repo.remote))
}
pub fn ignored_git_entries(cwd: &Path) -> Result<Vec<PathBuf>, String> {
    let mut current = store(cwd)?.base;
    let mut out = Vec::new();
    loop {
        let dot = current.join(".git");
        if paths::is_git_entry(&dot)? {
            return Ok(out);
        }
        if fs::symlink_metadata(&dot).is_ok() {
            out.push(dot);
        }
        let Some(parent) = current.parent() else {
            return Ok(out);
        };
        if parent == current {
            return Ok(out);
        }
        current = parent.into();
    }
}
pub fn home_git_note(env: &Map<String, Value>, home: &Path) -> Result<Option<String>, String> {
    let Some(home) = paths::home_dir(env, home)? else {
        return Ok(None);
    };
    let dot = home.join(".git");
    if fs::symlink_metadata(&dot).is_err() {
        return Ok(None);
    }
    if paths::is_git_entry(&dot)? {
        Ok(Some(format!(
            "{} is a git repository; it is the repository of sessions started there, not of the unversioned directories below it",
            home.display()
        )))
    } else {
        Ok(Some(format!(
            "{}/.git exists but is not a git repository (git ignores it, and so does TokenEconomizer)",
            home.display()
        )))
    }
}
pub fn listing(
    root: &Path,
    env: &Map<String, Value>,
    home: &Path,
    current: Option<&RepoId>,
) -> Result<(Vec<Value>, usize), String> {
    let idx = read(&store(root)?, INDEX)?;
    let mut checkouts = Map::new();
    for (path, value) in object(&idx["checkouts"]) {
        let key = value["key"].as_str().unwrap_or("").to_string();
        checkouts
            .entry(key)
            .or_insert(json!([]))
            .as_array_mut()
            .unwrap()
            .push(json!(path));
    }
    let home = paths::home_dir(env, home)?;
    let mut keys = list(&repos_dir(root));
    keys.sort();
    let mut rows = Vec::new();
    for key in keys {
        let directory = repo_dir(root, &key);
        if !directory.is_dir() {
            continue;
        }
        let doc = read(&store(&directory)?, "repo.json")?;
        let (mut agents, mut imports, mut cwds, mut last) =
            (Map::new(), Map::new(), BTreeSet::new(), 0.0_f64);
        for host in host_dirs(&directory) {
            let st = store(&directory.join(&host))?;
            let sess = sessions(&st)?;
            let folded = read(&st, "folded.json")?;
            let n = sess.keys().filter(|sid| imported(&folded[*sid])).count();
            agents.insert(host.clone(), json!(sess.len() - n));
            if n > 0 {
                imports.insert(host, json!(n));
            }
            for name in ["ledger.jsonl", "sessions"] {
                if let Some(time) = modified(&st.base.join(name)) {
                    last = last.max(time);
                }
            }
            for rec in sess.values() {
                if let Some(cwd) = rec["cwd"].as_str().filter(|s| !s.is_empty()) {
                    cwds.insert(cwd.to_string());
                }
            }
        }
        let mut notes = Vec::new();
        if idx["checked"].get(&key).is_none() && number(&idx["layout"])? < (LAYOUT as f64) {
            notes.push("recorded by an earlier version; `te repos migrate` sorts it out (it runs by itself at the next session start)".to_string());
        }
        let mut paths: Vec<String> = items(checkouts.get(&key).unwrap_or(&Value::Null))
            .iter()
            .filter_map(Value::as_str)
            .map(String::from)
            .collect();
        paths.sort();
        if home.as_ref().is_some_and(|home| {
            paths
                .iter()
                .any(|p| super::paths::realpath(Path::new(p)).ok().as_ref() == Some(home))
        }) {
            notes.push("your home directory: holds only sessions started in ~ itself".into());
        }
        let label = if truthy(&doc["label"]) {
            scalar_text(&doc["label"])
        } else {
            let label = dir_label(&directory)?;
            if label.is_empty() { key.clone() } else { label }
        };
        rows.push(json!({"key":key,"label":label,"how":doc.get("how").cloned().unwrap_or(json!("")),"remote":doc.get("remote").cloned().unwrap_or(json!("")),"agents":agents,"imported":imports,"last":last,"state":directory,"checkouts":paths,"cwds":cwds,"current":current.is_some_and(|c|c.key==key),"notes":notes}));
    }
    rows.sort_by(|a, b| {
        b["current"]
            .as_bool()
            .cmp(&a["current"].as_bool())
            .then_with(|| {
                b["last"]
                    .as_f64()
                    .unwrap_or(0.0)
                    .total_cmp(&a["last"].as_f64().unwrap_or(0.0))
            })
    });
    Ok((rows, list(&root.join("merged")).len()))
}
