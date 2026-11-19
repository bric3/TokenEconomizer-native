use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    sync::LazyLock,
};

use indexmap::IndexMap;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha1::{Digest, Sha1};

use crate::{store::Store, value::json_text};

pub const HOME_ENV: &str = "TOKENECONOMIZER_HOME";
pub const REPO_ENV: &str = "TOKENECONOMIZER_REPO";
pub const BY_REMOTE: &str = "remote";
pub const BY_PATH: &str = "path";
pub const BY_DIR: &str = "dir";

fn env<'a>(environment: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    environment
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}
fn absolute(path: &Path) -> Result<PathBuf, String> {
    Store::new(path).map(|s| s.base).map_err(|e| e.to_string())
}

pub fn state_root(
    environment: &Map<String, Value>,
    fallback_home: &Path,
) -> Result<PathBuf, String> {
    absolute(
        &env(environment, HOME_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|| fallback_home.join(".tokeneconomizer")),
    )
}

pub fn secure_root(environment: &Map<String, Value>, fallback_home: &Path) -> Result<(), String> {
    let root = state_root(environment, fallback_home)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if let Ok(metadata) = fs::metadata(&root) {
            // getuid has no preconditions and identifies the caller whose state may be tightened.
            if metadata.mode() & 0o077 != 0 && metadata.uid() == unsafe { libc::getuid() } {
                let _ = fs::set_permissions(root, fs::Permissions::from_mode(0o700));
            }
        }
    }
    #[cfg(not(unix))]
    let _ = root;
    Ok(())
}

pub fn launcher_path(
    environment: &Map<String, Value>,
    fallback_home: &Path,
) -> Result<PathBuf, String> {
    Ok(state_root(environment, fallback_home)?
        .join("bin")
        .join("te"))
}

pub fn executable(path: &Path) -> Result<bool, String> {
    #[cfg(unix)]
    {
        use std::{ffi::CString, os::unix::ffi::OsStrExt};
        let path = CString::new(path.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
        // access reads the NUL-terminated path and does not retain its pointer.
        Ok(unsafe { libc::access(path.as_ptr(), libc::X_OK) } == 0)
    }
    #[cfg(not(unix))]
    {
        Ok(path.exists())
    }
}

pub fn ensure_launcher(
    environment: &Map<String, Value>,
    fallback_home: &Path,
    windows_python: Option<&str>,
) -> Result<Option<PathBuf>, String> {
    let Some(plugin) = env(environment, "TE_PLUGIN_ROOT") else {
        return Ok(None);
    };
    let target = Path::new(plugin).join("scripts").join("te");
    if !executable(&target)? {
        return Ok(None);
    }
    let target = target.to_string_lossy();
    let quoted = if !target.is_empty()
        && target
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c))
    {
        target.to_string()
    } else {
        format!("'{}'", target.replace('\'', "'\"'\"'"))
    };
    let path = launcher_path(environment, fallback_home)?;
    let body = format!(
        "#!/bin/sh\n# Written by the TokenEconomizer plugin at session start: its `te` command line.\nexec {quoted} \"$@\"\n"
    );
    let mut launchers = vec![(path.clone(), body)];
    if let Some(python) = windows_python {
        let sidecar = PathBuf::from(format!("{}.json", path.display()));
        let body = json_text(
            &json!({"python":python,"bootstrap":Path::new(plugin).join("scripts").join("te_bootstrap.py")}),
        ) + "\n";
        launchers.push((sidecar, body));
    }
    for (launcher, body) in launchers {
        if let Ok(bytes) = fs::read(&launcher) {
            let existing = String::from_utf8(bytes)
                .map_err(|e| e.to_string())?
                .replace("\r\n", "\n")
                .replace('\r', "\n");
            if existing == body {
                continue;
            }
        }
        write_launcher(&launcher, body.as_bytes())?;
    }
    Ok(Some(path))
}

pub fn ensure_native_launcher(
    environment: &Map<String, Value>,
    home: &Path,
    target: &Path,
    windows: bool,
) -> Result<Option<PathBuf>, String> {
    if !executable(target)? {
        return Ok(None);
    }
    let path = launcher_path(environment, home)?;
    if realpath(&path)? == realpath(target)? {
        return Ok(Some(path));
    }
    let target_text = target.to_string_lossy();
    let quoted = format!("'{}'", target_text.replace('\'', "'\"'\"'"));
    let body = format!(
        "#!/bin/sh\n# Written by the TokenEconomizer plugin at session start: its `te` command line.\nexec {quoted} \"$@\"\n"
    );
    write_launcher(&path, body.as_bytes())?;
    if windows {
        // Compare bytes so a replacement with unchanged size and timestamps still updates the launcher.
        let binary = fs::read(target).map_err(|e| e.to_string())?;
        write_launcher(&path.with_extension("exe"), &binary)?;
    }
    Ok(Some(path))
}
fn write_launcher(path: &Path, body: &[u8]) -> Result<(), String> {
    if fs::read(path).is_ok_and(|bytes| bytes == body) {
        return Ok(());
    }
    fs::create_dir_all(path.parent().ok_or("missing launcher directory")?)
        .map_err(|e| e.to_string())?;
    let temporary = PathBuf::from(format!("{}.{}.tmp", path.display(), std::process::id()));
    let result = (|| -> Result<(), String> {
        fs::write(&temporary, body).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temporary, fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
        }
        fs::rename(&temporary, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

fn resolve(path: &Path, links: &mut BTreeSet<PathBuf>) -> Result<PathBuf, String> {
    let path = std::path::absolute(path).map_err(|e| e.to_string())?;
    let mut resolved = PathBuf::new();
    for component in path.components() {
        if component == Component::ParentDir {
            resolved.pop();
            continue;
        }
        resolved.push(component);
        if let Ok(target) = fs::read_link(&resolved)
            && links.insert(resolved.clone())
        {
            let link = resolved.clone();
            resolved.pop();
            resolved.push(target);
            resolved = resolve(&resolved, links)?;
            links.remove(&link);
        }
    }
    Ok(resolved)
}

pub fn realpath(path: &Path) -> Result<PathBuf, String> {
    resolve(path, &mut BTreeSet::new())
}

pub fn home_dir(
    environment: &Map<String, Value>,
    fallback_home: &Path,
) -> Result<Option<PathBuf>, String> {
    let home = env(environment, "HOME")
        .map(Path::new)
        .unwrap_or(fallback_home);
    if home.as_os_str().is_empty() || home == Path::new("~") {
        Ok(None)
    } else {
        realpath(home).map(Some)
    }
}

fn prefix(path: &Path, chars: usize) -> Result<Option<String>, String> {
    let Ok(file) = fs::File::open(path) else {
        return Ok(None);
    };
    let mut bytes = Vec::new();
    if file
        .take((chars * 4) as u64)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return Ok(None);
    }
    let text = match std::str::from_utf8(&bytes) {
        Ok(text) => text,
        Err(error) => {
            let valid = std::str::from_utf8(&bytes[..error.valid_up_to()]).unwrap();
            if valid.chars().count() < chars {
                return Err(error.to_string());
            }
            valid
        }
    };
    Ok(Some(text.chars().take(chars).collect()))
}

pub fn gitdir_of_file(dot: &Path) -> Result<Option<PathBuf>, String> {
    let Some(line) = prefix(dot, 4096)? else {
        return Ok(None);
    };
    let Some(gitdir) = line.trim().strip_prefix("gitdir:") else {
        return Ok(None);
    };
    let gitdir = Path::new(gitdir.trim());
    if gitdir.is_absolute() {
        Ok(Some(gitdir.into()))
    } else {
        let joined = dot.parent().unwrap_or(Path::new("")).join(gitdir);
        if joined.is_absolute() {
            absolute(&joined).map(Some)
        } else {
            Ok(Some(normalize(&joined)))
        }
    }
}

pub(crate) fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        if c == Component::CurDir {
            continue;
        }
        if c == Component::ParentDir && out.file_name().is_some_and(|s| s != "..") {
            out.pop();
        } else if c != Component::ParentDir || !out.has_root() {
            out.push(c);
        }
    }
    if out.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        out
    }
}

pub fn expand_user(
    path: &str,
    home: &Path,
    environment: &Map<String, Value>,
) -> Result<PathBuf, String> {
    if !path.starts_with('~') {
        return Ok(path.into());
    }
    let separator = |c| c == '/' || cfg!(windows) && c == '\\';
    let end = path.find(separator).unwrap_or(path.len());
    let user = &path[1..end];
    let mut base = home.to_path_buf();
    if !user.is_empty() {
        #[cfg(unix)]
        {
            use std::{
                ffi::{CStr, CString, OsString},
                mem::MaybeUninit,
                os::unix::ffi::OsStringExt,
                ptr,
            };
            let _ = environment;
            let name = CString::new(user).map_err(|_| "embedded null byte".to_string())?;
            let mut buffer = vec![0u8; 4096];
            loop {
                let mut record = MaybeUninit::<libc::passwd>::uninit();
                let mut found = ptr::null_mut();
                // The reentrant lookup writes the record and strings into this caller's buffer.
                let error = unsafe {
                    libc::getpwnam_r(
                        name.as_ptr(),
                        record.as_mut_ptr(),
                        buffer.as_mut_ptr().cast(),
                        buffer.len(),
                        &mut found,
                    )
                };
                if error == libc::ERANGE {
                    buffer.resize(
                        buffer
                            .len()
                            .checked_mul(2)
                            .ok_or("user record is too large")?,
                        0,
                    );
                    continue;
                }
                if error != 0 {
                    return Err(std::io::Error::from_raw_os_error(error).to_string());
                }
                if found.is_null() {
                    return Ok(path.into());
                }
                // A successful lookup initialises the record; its home string lives in buffer.
                let dir = unsafe { record.assume_init().pw_dir };
                if dir.is_null() {
                    return Ok(path.into());
                }
                base = PathBuf::from(OsString::from_vec(
                    unsafe { CStr::from_ptr(dir) }.to_bytes().to_vec(),
                ));
                break;
            }
        }
        #[cfg(windows)]
        {
            let current = environment.get("USERNAME").and_then(Value::as_str);
            if Some(user) != current {
                let base_name = if base.to_string_lossy().ends_with(['/', '\\']) {
                    ""
                } else {
                    base.file_name().and_then(|s| s.to_str()).unwrap_or("")
                };
                if Some(base_name) != current {
                    return Ok(path.into());
                }
                base = base.parent().unwrap_or(&base).join(user);
            }
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = environment;
            return Ok(path.into());
        }
    }
    #[cfg(windows)]
    {
        Ok(format!("{}{}", base.to_string_lossy(), &path[end..]).into())
    }
    #[cfg(not(windows))]
    {
        let expanded = format!(
            "{}{}",
            base.to_string_lossy().trim_end_matches(separator),
            &path[end..]
        );
        Ok(if expanded.is_empty() {
            std::path::MAIN_SEPARATOR_STR.into()
        } else {
            expanded.into()
        })
    }
}

#[cfg(all(test, windows))]
mod expansion_tests {
    use super::*;

    #[test]
    fn windows_profiles_preserve_separators_and_require_a_matching_username() {
        for (home, username, path, expected) in [
            (
                "C:\\Users\\alice",
                Some("alice"),
                "~bob/x",
                "C:\\Users\\bob/x",
            ),
            ("C:/Users/alice/", Some("alice"), "~bob/x", "~bob/x"),
            (
                "C:/Users/alice/",
                Some("alice"),
                "~alice/x",
                "C:/Users/alice//x",
            ),
            (
                "C:/Users/alice/",
                Some("alice"),
                "~\\x",
                "C:/Users/alice/\\x",
            ),
            ("C:/Users/alice/", None, "~bob/x", "~bob/x"),
            ("", Some(""), "~bob", "bob"),
            ("C:\\", Some(""), "~bob", "C:\\bob"),
            ("", None, "~bob", "~bob"),
            ("", None, "~", ""),
        ] {
            let mut environment = Map::new();
            if let Some(username) = username {
                environment.insert("USERNAME".into(), json!(username));
            }
            assert_eq!(
                expand_user(path, Path::new(home), &environment)
                    .unwrap()
                    .as_os_str(),
                expected,
                "home={home:?}, username={username:?}, path={path:?}"
            );
        }
    }
}

pub fn is_git_entry(dot: &Path) -> Result<bool, String> {
    if dot.is_dir() {
        return Ok(dot.join("HEAD").is_file());
    }
    Ok(gitdir_of_file(dot)?.is_some_and(|path| path.is_dir()))
}

pub fn find_repo_root(
    start: &Path,
    environment: &Map<String, Value>,
    fallback_home: &Path,
) -> Result<Option<PathBuf>, String> {
    let mut ceilings = BTreeSet::new();
    for ceiling in std::env::split_paths(env(environment, "GIT_CEILING_DIRECTORIES").unwrap_or(""))
    {
        if !ceiling.as_os_str().is_empty() {
            ceilings.insert(absolute(&ceiling)?);
        }
    }
    let home = home_dir(environment, fallback_home)?;
    let mut current = absolute(start)?;
    loop {
        if is_git_entry(&current.join(".git"))? {
            return Ok(Some(current));
        }
        let Some(parent) = current.parent() else {
            return Ok(None);
        };
        if parent == current
            || ceilings.contains(parent)
            || home
                .as_ref()
                .is_some_and(|home| realpath(parent).ok().as_ref() == Some(home))
        {
            return Ok(None);
        }
        current = parent.into();
    }
}

pub fn git_common_dir(root: &Path) -> Result<Option<PathBuf>, String> {
    let dot = root.join(".git");
    if dot.is_dir() {
        return Ok(Some(dot));
    }
    let Some(gitdir) = gitdir_of_file(&dot)? else {
        return Ok(None);
    };
    match fs::read(gitdir.join("commondir")) {
        Ok(bytes) => {
            let common = String::from_utf8(bytes).map_err(|e| e.to_string())?;
            Ok(Some(normalize(&gitdir.join(common.trim()))))
        }
        Err(_) => Ok(Some(gitdir)),
    }
}

pub fn origin_url(root: &Path) -> Result<Option<String>, String> {
    static REMOTE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"^\s*\[remote\s+"([^"]+)"\]\s*$"#).unwrap());
    static URL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*url\s*=\s*(\S+)\s*$").unwrap());
    if root.as_os_str().is_empty() {
        return Ok(None);
    }
    let Some(gitdir) = git_common_dir(root)? else {
        return Ok(None);
    };
    let Ok(bytes) = fs::read(gitdir.join("config")) else {
        return Ok(None);
    };
    let lines = String::from_utf8_lossy(&bytes);
    let mut remotes = IndexMap::<String, String>::new();
    let mut current = None;
    for line in lines.split([
        '\n', '\r', '\u{b}', '\u{c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2028}',
        '\u{2029}',
    ]) {
        if let Some(m) = REMOTE.captures(line) {
            current = Some(m[1].to_string());
            continue;
        }
        if line.trim_start().starts_with('[') {
            current = None;
            continue;
        }
        if let Some(name) = current
            .as_ref()
            .filter(|n| !n.is_empty() && !remotes.contains_key(*n))
            && let Some(m) = URL.captures(line)
        {
            remotes.insert(name.clone(), m[1].into());
        }
    }
    Ok(remotes
        .get("origin")
        .cloned()
        .or_else(|| remotes.first().map(|(_, v)| v.clone())))
}

pub fn normalize_remote(url: &str) -> String {
    static SCHEME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-z+]+://").unwrap());
    static USER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[^@/]+@").unwrap());
    static SLASHES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"/+").unwrap());
    static DRIVE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-zA-Z]:/").unwrap());
    static PORT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[^/]+:[^/\d]").unwrap());
    static SCP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[^/]+:[^/]").unwrap());
    let url = url.trim();
    let scheme = SCHEME.find(url);
    let url = scheme.as_ref().map_or(url, |m| &url[m.end()..]);
    let url = USER.replace(url, "");
    let url = url.replace('\\', "/");
    let mut url = SLASHES.replace_all(&url, "/").into_owned();
    if !DRIVE.is_match(&url)
        && if scheme.is_some() {
            PORT.is_match(&url)
        } else {
            SCP.is_match(&url)
        }
    {
        url = url.replacen(':', "/", 1);
    }
    let url = url.trim_end_matches('/');
    url.strip_suffix(".git").unwrap_or(url).to_lowercase()
}

pub fn slug(text: &str) -> String {
    static UNSAFE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-z0-9]+").unwrap());
    let lower = if text.is_empty() { "repo" } else { text }.to_lowercase();
    let slug = UNSAFE
        .replace_all(&lower, "-")
        .trim_matches('-')
        .chars()
        .take(40)
        .collect::<String>();
    if slug.is_empty() { "repo".into() } else { slug }
}
pub fn digest(canonical: &str) -> String {
    format!("{:x}", Sha1::digest(canonical.as_bytes()))[..10].into()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoId {
    pub root: PathBuf,
    pub key: String,
    pub label: String,
    pub remote: String,
    #[serde(default)]
    pub how: String,
}

pub fn main_checkout(root: &Path) -> Result<Option<PathBuf>, String> {
    Ok(git_common_dir(root)?
        .map(|p| normalize(&p))
        .filter(|p| p.file_name().is_some_and(|name| name == ".git"))
        .and_then(|p| p.parent().map(PathBuf::from)))
}

pub fn repo_identity(
    cwd: &Path,
    environment: &Map<String, Value>,
    fallback_home: &Path,
) -> Result<RepoId, String> {
    let found = find_repo_root(cwd, environment, fallback_home)?;
    let root = match &found {
        Some(root) => root.clone(),
        None => absolute(cwd)?,
    };
    let mut remote = if found.is_some() {
        normalize_remote(&origin_url(&root)?.unwrap_or_default())
    } else {
        String::new()
    };
    if remote.is_empty() {
        remote = normalize_remote(env(environment, REPO_ENV).unwrap_or(""));
    }
    let (how, canonical, name, label) = if !remote.is_empty() {
        let name = remote
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("")
            .to_string();
        let label = remote
            .split('/')
            .rev()
            .take(2)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("/");
        (BY_REMOTE, remote.clone(), name, label)
    } else {
        let main = if found.is_some() {
            main_checkout(&root)?
                .or(git_common_dir(&root)?)
                .unwrap_or(root.clone())
        } else {
            root.clone()
        };
        let canonical = realpath(&main)?.to_string_lossy().into_owned();
        let name = main
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        (
            if found.is_some() { BY_PATH } else { BY_DIR },
            canonical,
            name.clone(),
            name,
        )
    };
    Ok(RepoId {
        root,
        key: format!("{}-{}", slug(&name), digest(&canonical)),
        label: if label.is_empty() {
            "repo".into()
        } else {
            label
        },
        remote,
        how: how.into(),
    })
}

pub fn repo_state_dir(
    repo: &RepoId,
    host: &str,
    environment: &Map<String, Value>,
    fallback_home: &Path,
) -> Result<PathBuf, String> {
    Ok(state_root(environment, fallback_home)?
        .join("repos")
        .join(&repo.key)
        .join(host))
}
