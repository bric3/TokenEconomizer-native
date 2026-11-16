use std::sync::LazyLock;

use regex::Regex;

pub fn repo_filename(repo: &str) -> String {
    static UNSAFE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"[<>:"/\\|?*]+"#).unwrap());
    let replaced = UNSAFE.replace_all(repo, "__");
    let name = replaced.trim_end_matches(['.', ' ']);
    if name.is_empty() {
        "repo".into()
    } else {
        name.into()
    }
}
