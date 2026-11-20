// Converted from reference/upstream/tests/test_dead_code.py.
mod support;

use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    process::Command,
};
use support::TempDir;

const MODULE: &str = r#"import json

LIVE = 1
DEAD_CONST = 2
CALLED_CONST = json.dumps({})


def live():
    return LIVE


# A comment that belongs to the function below.
def dead():
    return 1


def tested():
    return 2


@some_registry.register
def hook():
    return 3


def __getattr__(name):
    raise AttributeError(name)


class Box(json.JSONEncoder):
    def default(self, o):
        return live()

    def unused_method(self):
        return 4

    def kept(self):
        return 5


class Lonely:
    def only_dead(self):
        return 6


print(Box().kept(), Lonely, CALLED_CONST)
"#;

const MOD: &str = "tokeneconomizer/mod.py";
const CASE: [(&str, &str); 2] = [
    (MOD, MODULE),
    (
        "tests/test_mod.py",
        "from tokeneconomizer.mod import tested\n",
    ),
];

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn repo(root: &Path, files: &[(&str, &str)]) {
    for (path, text) in files {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    git(root, &["init", "-q"]);
    git(root, &["add", "-A"]);
}

fn run(root: &Path, args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_te-dead-code"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn report(root: &Path, args: &[&str]) -> Value {
    let mut args = args.to_vec();
    args.push("--json");
    serde_json::from_str(&run(root, &args)).unwrap()
}

fn found(root: &Path) -> BTreeMap<String, String> {
    report(root, &[])["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            let path = item["path"].as_str().unwrap();
            let name = item["name"].as_str().unwrap();
            (
                format!("{path}::{name}"),
                item["status"].as_str().unwrap().into(),
            )
        })
        .collect()
}

fn removed(document: &Value) -> BTreeSet<&str> {
    document["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["removed"] == true)
        .map(|item| item["name"].as_str().unwrap())
        .collect()
}

#[test]
fn unreferenced_and_test_only() {
    let tmp = TempDir::new("dead-code-references");
    repo(tmp.path(), &CASE);
    let actual = found(tmp.path());
    for name in [
        "dead",
        "DEAD_CONST",
        "Box.unused_method",
        "Lonely.only_dead",
    ] {
        assert_eq!(actual[&format!("{MOD}::{name}")], "unreferenced");
    }
    assert_eq!(actual[&format!("{MOD}::tested")], "test-only");
    for name in [
        "live",
        "LIVE",
        "hook",
        "__getattr__",
        "Box.default",
        "Box",
        "Box.kept",
    ] {
        assert!(!actual.contains_key(&format!("{MOD}::{name}")));
    }
}

#[test]
fn a_name_in_any_text_file_counts_as_used() {
    let tmp = TempDir::new("dead-code-docs");
    repo(
        tmp.path(),
        &[
            (MOD, "def from_docs():\n    pass\n"),
            ("docs/NOTES.md", "Call `from_docs` by hand.\n"),
        ],
    );
    assert!(found(tmp.path()).is_empty());
}

#[test]
fn allowlist_and_all() {
    let tmp = TempDir::new("dead-code-allowlist");
    repo(
        tmp.path(),
        &[
            (
                "plugin/host/__init__.py",
                "__all__ = ['exported']\n\ndef exported():\n    pass\n\ndef register(ctx):\n    pass\n\ndef other():\n    pass\n",
            ),
            (
                "scripts/dead_code_allowlist.txt",
                "# The host calls it by name.\nplugin/host/__init__.py::register\n",
            ),
        ],
    );
    assert_eq!(
        found(tmp.path()),
        BTreeMap::from([(
            "plugin/host/__init__.py::other".into(),
            "unreferenced".into()
        )])
    );
}

#[test]
fn experiments_tests_are_not_scanned_but_count_as_tests() {
    let tmp = TempDir::new("dead-code-experiment-tests");
    repo(
        tmp.path(),
        &[
            ("experiments/te_bench/a.py", "def helper():\n    pass\n"),
            (
                "experiments/tests/test_a.py",
                "def unused_in_test():\n    pass\nhelper\n",
            ),
        ],
    );
    assert_eq!(
        found(tmp.path()),
        BTreeMap::from([(
            "experiments/te_bench/a.py::helper".into(),
            "test-only".into()
        )])
    );
}

#[test]
fn fix_deletes_only_unreferenced_and_keeps_files_parseable() {
    let tmp = TempDir::new("dead-code-fix-safe");
    repo(tmp.path(), &CASE);
    let document = report(tmp.path(), &["--fix"]);
    let text = fs::read_to_string(tmp.path().join(MOD)).unwrap();
    let python = std::env::var_os("TE_PYTHON").unwrap_or_else(|| "python3".into());
    let parsed = Command::new(python)
        .current_dir(tmp.path())
        .args(["-m", "py_compile", MOD])
        .output()
        .unwrap();
    assert!(
        parsed.status.success(),
        "{}",
        String::from_utf8_lossy(&parsed.stderr)
    );
    for deleted in [
        "def dead",
        "belongs to the function below",
        "DEAD_CONST",
        "unused_method",
        "\n\n\n\n",
    ] {
        assert!(!text.contains(deleted));
    }
    for kept in [
        "def tested",
        "def live",
        "def hook",
        "CALLED_CONST = json.dumps",
        "def only_dead",
    ] {
        assert!(text.contains(kept));
    }
    assert_eq!(
        removed(&document),
        BTreeSet::from(["dead", "DEAD_CONST", "Box.unused_method"])
    );
    assert!(removed(&report(tmp.path(), &["--fix"])).is_empty());
}

#[test]
fn cli_markdown_and_json() {
    let tmp = TempDir::new("dead-code-cli-formats");
    let orphan = "def orphan():\n    pass\n";
    repo(tmp.path(), &[("scripts/tool.py", orphan)]);
    let flag = ["--root", tmp.path().to_str().unwrap()];
    assert!(run(tmp.path(), &flag).contains("`scripts/tool.py:1` function `orphan`"));
    assert_eq!(
        report(tmp.path(), &flag)["counts"],
        json!({"function/unreferenced":1})
    );
    assert_eq!(
        fs::read_to_string(tmp.path().join("scripts/tool.py")).unwrap(),
        orphan
    );
}

#[test]
fn runs_on_this_repository_without_changing_it() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("reference/upstream");
    let status = ["status", "--porcelain", "--", "."];
    let before = git(&root, &status);
    let document = report(&root, &[]);
    let items = document["items"].as_array().unwrap();
    assert!(
        items
            .iter()
            .all(|item| item["status"] == "unreferenced" || item["status"] == "test-only")
    );
    assert!(items.iter().all(|item| item["name"] != "register"));
    assert_eq!(git(&root, &status), before);
}

#[test]
fn prefix_dispatch_and_getattr_dispatch_count_as_overrides() {
    let tmp = TempDir::new("dead-code-dispatch");
    let module = r#"import cmd
from http.server import BaseHTTPRequestHandler


class Handler(BaseHTTPRequestHandler):
    def do_PATCH(self):
        pass

    def stray(self):
        pass


class Shell(cmd.Cmd):
    def do_greet(self, arg):
        pass

    def help_greet(self):
        pass


class Dispatch:
    def handle(self, kind):
        return getattr(self, "on_" + kind)()

    def on_start(self):
        pass


print(Handler, Shell, Dispatch().handle)
"#;
    repo(tmp.path(), &[("tokeneconomizer/serve.py", module)]);
    assert_eq!(
        found(tmp.path()),
        BTreeMap::from([(
            "tokeneconomizer/serve.py::Handler.stray".into(),
            "unreferenced".into()
        )])
    );
}

#[test]
fn dotted_imports_guarded_imports_and_scanned_bases_resolve() {
    let tmp = TempDir::new("dead-code-overrides");
    let logs = r#"import logging.handlers
try:
    from socketserver import BaseRequestHandler
except ImportError:
    BaseRequestHandler = object


class Rotating(logging.handlers.RotatingFileHandler):
    def shouldRollover(self, record):
        return False


class Req(BaseRequestHandler):
    def setup(self):
        pass


print(Rotating, Req)
"#;
    let enc = r#"import json


class Enc(json.JSONEncoder):
    pass


print(Enc)
"#;
    let enc2 = r#"from tokeneconomizer.enc import Enc


class Enc2(Enc):
    def default(self, o):
        return str(o)

    def orphaned(self):
        pass


class Enc3(Enc2):
    def default(self, o):
        return 1


print(Enc2, Enc3)
"#;
    repo(
        tmp.path(),
        &[
            ("tokeneconomizer/logs.py", logs),
            ("tokeneconomizer/enc.py", enc),
            ("tokeneconomizer/enc2.py", enc2),
        ],
    );
    assert_eq!(
        found(tmp.path()),
        BTreeMap::from([(
            "tokeneconomizer/enc2.py::Enc2.orphaned".into(),
            "unreferenced".into()
        )])
    );
}

#[test]
fn fix_leaves_no_gap_under_a_class_header() {
    let tmp = TempDir::new("dead-code-class-spacing");
    let module = "class Box:\n\n    def gone(self):\n        pass\n\n    def kept(self):\n        pass\n\n\nBox().kept()\n";
    repo(tmp.path(), &[(MOD, module)]);
    report(tmp.path(), &["--fix"]);
    assert!(
        fs::read_to_string(tmp.path().join(MOD))
            .unwrap()
            .starts_with("class Box:\n    def kept(self):\n")
    );
}

#[test]
fn cli_fix_repeats_until_nothing_more_goes() {
    let tmp = TempDir::new("dead-code-fixed-point");
    let module = "HELPER_LIMIT = 3\n\n\ndef helper():\n    return HELPER_LIMIT\n\n\ndef orphan():\n    return helper()\n\n\ndef main():\n    pass\n\n\nmain()\n";
    repo(tmp.path(), &[("scripts/tool.py", module)]);
    let document = report(tmp.path(), &["--fix"]);
    assert_eq!(
        removed(&document),
        BTreeSet::from(["orphan", "helper", "HELPER_LIMIT"])
    );
    assert_eq!(
        fs::read_to_string(tmp.path().join("scripts/tool.py")).unwrap(),
        "def main():\n    pass\n\n\nmain()\n"
    );
}
