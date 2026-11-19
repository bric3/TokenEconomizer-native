// Converted from reference/upstream/tests/test_generic_microskills.py.
pub mod fixtures;
#[path = "support/recording_runner.rs"]
mod recording_runner;
mod support;

use fixtures::write_claude_transcript;
use recording_runner::RecordingRunner;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::Path};
use support::TempDir;
use tokeneconomizer::{
    microskills::{
        distiller::{self, Distiller, ProposeContext},
        envfacts::{self, EnvFactsDistiller},
        observe::{self, Vocabulary},
    },
    plugin::adapter::detect_vocabulary,
    runner::Runner,
};

fn form(command: &str, vocab: &Vocabulary) -> Option<(String, String)> {
    observe::classify_command(command, 0, vocab, "", None, None)
        .unwrap()
        .into_iter()
        .next()
        .map(|o| (o.intent, o.form))
}
fn edit(i: usize, extension: &str) -> Value {
    json!(["Edit",{"file_path":format!("/r/pkg/m{i}.{extension}"),"old_string":"a","new_string":"b"},"ok"])
}
fn sh(command: &str, output: &str) -> Value {
    json!(["Bash",{"command":command},output])
}
fn update(d: &mut EnvFactsDistiller, root: &Path, i: usize, iid: &str, calls: &[Value]) {
    d.update(
        &write_claude_transcript(&root.join(format!("s{i}.jsonl")), calls),
        iid,
        json!(true),
        None,
    )
    .unwrap();
}
fn train(
    root: &Path,
    repo: &str,
    n: usize,
    prefix: &str,
    calls: impl Fn(usize) -> Vec<Value>,
) -> EnvFactsDistiller {
    let mut d = EnvFactsDistiller::new(repo, Vocabulary::generic(), None, None);
    for i in 0..n {
        d.render();
        update(&mut d, root, i, &format!("{prefix}{i}"), &calls(i));
    }
    d
}

#[test]
fn the_generic_reading_knows_builds_and_other_ecosystems() {
    for (cmd, intent, expected) in [
        ("npm run build", "run_build", "npm run build"),
        ("npm run test:unit", "run_tests", "npm run test:unit"),
        ("cargo build --release", "run_build", "cargo build"),
        ("tsc -p .", "run_build", "tsc"),
        ("go test ./...", "run_tests", "go test"),
        ("./gradlew test", "run_tests", "gradlew test"),
        ("bundle install", "package_tool", "bundle"),
    ] {
        assert_eq!(
            form(cmd, &Vocabulary::generic()),
            Some((intent.into(), expected.into())),
            "{cmd}"
        );
    }
    assert_eq!(form("tsc -p .", &Vocabulary::default()), None);
    assert_eq!(
        form("npm run build", &Vocabulary::node_web()),
        Some(("package_tool".into(), "npm".into()))
    );
    assert_eq!(form("./gradlew test", &Vocabulary::default()), None);
}

#[test]
fn a_build_tool_repair_becomes_a_fact() {
    let root = TempDir::new("generic-build-repair");
    let mut d = EnvFactsDistiller::new("acme/web", Vocabulary::generic(), None, None);
    for i in 0..6 {
        let mut changed = edit(i, "ts");
        changed[1]["file_path"] = json!(format!("/r/src/m{i}.ts"));
        update(
            &mut d,
            root.path(),
            i,
            &format!("s{i}"),
            &[
                changed,
                sh("tsc -p .", "Exit code 127\nbash: tsc: command not found"),
                sh("npx tsc -p .", ""),
            ],
        );
    }
    let text = d.render();
    assert!(text.starts_with(envfacts::GENERIC_HEADER));
    assert!(text.contains("`tsc …` fails here with command not found; `npx tsc …` works."));
}

#[test]
fn an_absent_linter_becomes_a_fact_and_a_working_form_does_not() {
    let root = TempDir::new("generic-absent-linter");
    let mut d = train(root.path(), "acme/py", 4, "a", |i| {
        vec![
            edit(i, "py"),
            sh(
                "ruff check .",
                "Exit code 127\nbash: ruff: command not found",
            ),
            sh("make build", "built"),
        ]
    });
    assert_eq!(d.facts["absent_tool:ruff"].family, "absent_tool");
    assert_eq!(d.facts["absent_tool:ruff"].life.state, "ACTIVE");
    assert!(
        d.render()
            .contains("`ruff …` fails here with command not found")
    );
    assert!(
        !d.facts
            .keys()
            .any(|id| id.starts_with("form_repair") && id.contains("ruff"))
    );
}

#[test]
fn an_absent_tool_retires_when_it_runs() {
    let root = TempDir::new("generic-absent-retire");
    let mut d = train(root.path(), "acme/py", 4, "a", |i| {
        vec![
            edit(i, "py"),
            sh(
                "ruff check .",
                "Exit code 127\nbash: ruff: command not found",
            ),
        ]
    });
    assert_eq!(d.facts["absent_tool:ruff"].life.state, "ACTIVE");
    d.render();
    update(
        &mut d,
        root.path(),
        9,
        "a9",
        &[
            json!(["Edit",{"file_path":"/r/pkg/z.py","old_string":"a","new_string":"b"},"ok"]),
            sh("ruff check .", "All checks passed!"),
        ],
    );
    assert_eq!(d.facts["absent_tool:ruff"].life.state, "RETIRED");
}

#[test]
fn a_program_missing_inside_make_is_not_learned_as_make_missing() {
    let root = TempDir::new("generic-recipe-missing");
    let d = train(root.path(), "acme/c", 4, "m", |i| {
        vec![
            json!(["Edit",{"file_path":format!("/r/src/m{i}.c"),"old_string":"a","new_string":"b"},"ok"]),
            sh(
                "make",
                "Exit code 2\ncc -o app main.c\n/bin/sh: 1: cc: not found\nmake: *** [Makefile:2: app] Error 127",
            ),
            sh("ruff check .", "Exit code 127\n/bin/sh: 1: ruff: not found"),
        ]
    });
    assert!(!d.facts.contains_key("absent_tool:make"));
    assert_eq!(d.facts["absent_tool:ruff"].life.state, "ACTIVE");
}

#[test]
fn an_injected_absent_tool_counts_only_sessions_that_ran_a_check() {
    let root = TempDir::new("generic-absent-evidence");
    let mut d = train(root.path(), "acme/py", 4, "a", |i| {
        vec![
            edit(i, "py"),
            sh(
                "ruff check .",
                "Exit code 127\nbash: ruff: command not found",
            ),
        ]
    });
    assert!(
        d.render()
            .contains("`ruff …` fails here with command not found")
    );
    update(
        &mut d,
        root.path(),
        8,
        "a8",
        &[
            json!(["Edit",{"file_path":"/r/pkg/y.py","old_string":"a","new_string":"b"},"ok"]),
            sh("ls", "pkg"),
        ],
    );
    assert_eq!(
        d.last_digest.as_ref().unwrap()["verdicts"]["absent_tool:ruff"],
        json!([null, false, false])
    );
    d.render();
    update(
        &mut d,
        root.path(),
        9,
        "a9",
        &[
            json!(["Edit",{"file_path":"/r/pkg/z.py","old_string":"a","new_string":"b"},"ok"]),
            sh("mypy .", "Success: no issues found in 3 source files"),
        ],
    );
    assert_eq!(
        d.last_digest.as_ref().unwrap()["verdicts"]["absent_tool:ruff"],
        json!(["confirm", true, true])
    );
}

#[test]
fn module_and_launcher_runs_name_the_tool_they_run() {
    for (command, intent, expected) in [
        ("python -m mypy .", "run_build", "python -m mypy"),
        ("python3 -m ruff check .", "run_build", "python3 -m ruff"),
        ("uvx ruff check .", "run_build", "uvx ruff"),
        ("uv tool run ruff check .", "run_build", "uvx ruff"),
        ("pipx run black .", "run_build", "pipx run black"),
        ("uvx pytest -q", "run_tests", "uvx pytest"),
        (
            "cd pkg && python -m mypy .",
            "run_build",
            "cd pkg && python -m mypy",
        ),
        ("python -m json.tool f.json", "run_interpreter", "python"),
    ] {
        assert_eq!(
            form(command, &Vocabulary::generic()),
            Some((intent.into(), expected.into())),
            "{command}"
        );
    }
    assert!(!observe::is_bare("uvx ruff"));
    assert!(!observe::is_bare("pipx run black"));
    assert_eq!(
        form("python -m mypy .", &Vocabulary::default()),
        Some(("run_interpreter".into(), "python".into()))
    );
    assert_eq!(form("uvx ruff check .", &Vocabulary::default()), None);
    assert_eq!(form("pipx run pytest", &Vocabulary::default()), None);
}

#[test]
fn a_tool_that_works_another_way_is_not_learned_as_absent() {
    for (bare, working) in [
        ("mypy .", "python -m mypy ."),
        ("mypy .", "python3 -m mypy ."),
        ("ruff check .", "python -m ruff check ."),
        ("ruff check .", "uvx ruff check ."),
        ("ruff check .", "uv tool run ruff check ."),
        ("black .", "pipx run black ."),
        ("ruff check .", "uv run ruff check ."),
        ("ruff check .", ".venv/bin/ruff check ."),
    ] {
        let root = TempDir::new("generic-tool-repair");
        let tool = bare.split_whitespace().next().unwrap();
        let d = train(root.path(), "acme/py", 6, "s", |i| {
            vec![
                edit(i, "py"),
                sh(
                    bare,
                    &format!("Exit code 127\nbash: {tool}: command not found"),
                ),
                sh(working, "All checks passed!"),
            ]
        });
        assert!(
            !d.facts.contains_key(&format!("absent_tool:{tool}")),
            "{bare} -> {working}"
        );
    }
}

#[test]
fn the_working_form_is_named_as_the_repair() {
    for working in ["python -m mypy .", "uvx mypy .", "pipx run mypy ."] {
        let root = TempDir::new("generic-working-form");
        let mut d = train(root.path(), "acme/py", 6, "s", |i| {
            vec![
                edit(i, "py"),
                sh("mypy .", "Exit code 127\nbash: mypy: command not found"),
                sh(working, "Success: no issues found in 3 source files"),
            ]
        });
        let form = working.rsplit_once(' ').unwrap().0;
        assert!(
            d.render().contains(&format!(
                "`mypy …` fails here with command not found; `{form} …` works."
            )),
            "{working}"
        );
    }
}

#[test]
fn an_absent_tool_retires_when_it_runs_another_way() {
    for working in ["python -m mypy .", "uvx mypy .", "pipx run mypy ."] {
        let root = TempDir::new("generic-retire-wrapper");
        let mut d = train(root.path(), "acme/py", 4, "a", |i| {
            vec![
                edit(i, "py"),
                sh("mypy .", "Exit code 127\nbash: mypy: command not found"),
            ]
        });
        assert_eq!(d.facts["absent_tool:mypy"].life.state, "ACTIVE");
        d.render();
        update(
            &mut d,
            root.path(),
            9,
            "a9",
            &[
                json!(["Edit",{"file_path":"/r/pkg/z.py","old_string":"a","new_string":"b"},"ok"]),
                sh(working, "Success: no issues found in 3 source files"),
            ],
        );
        assert_eq!(
            d.facts["absent_tool:mypy"].life.state, "RETIRED",
            "{working}"
        );
    }
}

#[test]
fn a_shared_launcher_does_not_make_one_tool_repair_another() {
    for (failing, other) in [
        ("python -m mypy .", "python -m ruff check ."),
        ("uvx mypy .", "uvx ruff check ."),
        ("pipx run mypy .", "pipx run ruff check ."),
    ] {
        let root = TempDir::new("generic-launcher-identity");
        let d = train(root.path(), "acme/py", 6, "s", |i| {
            vec![
                edit(i, "py"),
                sh(
                    failing,
                    "Exit code 1\n/usr/bin/python: No module named mypy",
                ),
                sh(other, "All checks passed!"),
            ]
        });
        assert!(
            !d.facts.keys().any(|id| id.starts_with("form_repair")),
            "{failing} -> {other}"
        );
    }
}

#[test]
fn every_edited_file_counts_in_the_generic_reading() {
    let root = TempDir::new("generic-edited");
    let path = write_claude_transcript(
        &root.path().join("s0.jsonl"),
        &[
            json!(["Edit",{"file_path":"/r/src/main.go","old_string":"a","new_string":"b"},"ok"]),
            json!(["Write",{"file_path":"/r/web/app.ts","content":"x"},"ok"]),
            json!(["Edit",{"file_path":"/r/pkg/m.py","old_string":"a","new_string":"b"},"ok"]),
        ],
    );
    assert_eq!(
        observe::observe_task(&path, &Vocabulary::generic())
            .unwrap()
            .edited,
        ["app.ts", "m.py", "main.go"]
    );
    assert_eq!(
        observe::observe_task(&path, &Vocabulary::default())
            .unwrap()
            .edited,
        ["m.py"]
    );
}

#[test]
fn the_generic_digest_is_language_neutral_and_relative() {
    let root = TempDir::new("generic-digest");
    let repo = root.path().join("repo");
    fs::create_dir_all(repo.join(".git")).unwrap();
    let path = write_claude_transcript(
        &root.path().join("t.jsonl"),
        &[
            json!(["Edit",{"file_path":repo.join("src/main.ts"),"old_string":"a","new_string":"b"},"ok",repo]),
            json!(["Bash",{"command":format!("cd {} && npm run build",repo.display())},format!("Exit code 2\n{}/src/main.ts(3,5): error TS2304: Cannot find name 'foo'.",repo.display()),repo]),
            json!(["Bash",{"command":"node scripts/gen.js"},"Error: Cannot find module 'left-pad'\nExit code 1",repo]),
            json!(["Bash",{"command":"cargo build"},"Finished dev",repo]),
        ],
    );
    let digest = distiller::build_generic_digest(&path, "s0", json!(true), "").unwrap();
    assert_eq!(digest["edited_files"], json!(["main.ts"]));
    assert_eq!(
        digest["check_commands"],
        json!(["npm run build", "cargo build"])
    );
    assert_eq!(
        digest["failed_commands"],
        json!([{"cmd":"npm run build","error":"src/main.ts(N,N): error TS2304: Cannot find name 'foo'."},{"cmd":"node scripts/gen.js","error":"Error: Cannot find module 'left-pad'"}])
    );
    assert_eq!(digest["missing_dependency"], true);
    assert_eq!(digest["missing_command"], false);
    let shown = distiller::proposer_view(&digest).to_string();
    assert!(!shown.contains(repo.to_str().unwrap()) && !shown.contains("_cmd_blob"));
}

#[test]
fn the_plugin_reads_every_repository_generically() {
    let root = TempDir::new("generic-vocab");
    fs::write(root.path().join("package.json"), "{}").unwrap();
    let vocabulary = detect_vocabulary(root.path(), "");
    assert!(vocabulary.generic);
    assert_eq!(vocabulary.canonical_interpreter, "node");
    assert_eq!(
        serde_json::to_value(detect_vocabulary(&root.path().join("nowhere"), "")).unwrap(),
        serde_json::to_value(Vocabulary::generic()).unwrap()
    );
    assert_eq!(
        serde_json::to_value(detect_vocabulary(root.path(), "python-git")).unwrap(),
        serde_json::to_value(Vocabulary::default()).unwrap()
    );
}

fn build_test() -> Vec<Value> {
    vec![
        sh("make build", "built"),
        sh("make test", "ok  ./... 0.01s"),
    ]
}
fn build_lint() -> Vec<Value> {
    vec![sh("make build", "built"), sh("make lint", "0 issues")]
}
fn fold_with(
    root: &Path,
    runner: Option<&mut dyn Runner>,
    sessions: &[Vec<Value>],
    start: usize,
) -> Distiller {
    let state = root.join("state");
    let mut context = ProposeContext {
        runner,
        propose_every: 3,
        ..Default::default()
    };
    for (offset, calls) in sessions.iter().enumerate() {
        let i = start + offset;
        let mut d = Distiller::load("acme/tool", &state, "agent", true).unwrap();
        d.update(
            &write_claude_transcript(&root.join(format!("s{i}.jsonl")), calls),
            &format!("s{i}"),
            json!(true),
            &mut context,
        )
        .unwrap();
        d.save(&state).unwrap();
    }
    Distiller::load("acme/tool", &state, "agent", true).unwrap()
}
fn proposer() -> RecordingRunner {
    RecordingRunner::new(&json!({"extractors":[],"facts":[{"id":"build-before-test","claim":"make test needs make build first","injection":"`make test` runs the built binary: run `make build` first.","detector":{"scope":"make test","confirm":"make build"},"cite":["s0","s1"]}]}).to_string(),100.0)
}

#[test]
fn the_proposer_keeps_its_window_across_folds() {
    let root = TempDir::new("generic-proposer-window");
    let mut runner = proposer();
    let d = fold_with(
        root.path(),
        Some(&mut runner),
        &[build_test(), build_test()],
        0,
    );
    let prompts = runner.prompts.borrow();
    assert_eq!(prompts.len(), 1);
    assert!(prompts[0].contains("whatever its language"));
    assert!(d.facts.contains_key("build-before-test"));
    let again = Distiller::load("acme/tool", &root.path().join("state"), "agent", true).unwrap();
    assert_eq!(
        again
            .window
            .iter()
            .map(|d| d["iid"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["s0", "s1"]
    );
}

#[test]
fn the_proposer_is_shown_the_facts_it_already_has() {
    let root = TempDir::new("generic-proposer-existing");
    let mut runner = proposer();
    fold_with(root.path(), Some(&mut runner), &vec![build_test(); 5], 0);
    let prompts = runner.prompts.borrow();
    assert_eq!(prompts.len(), 2);
    assert!(prompts[0].contains("Existing facts, as `id: claim`") && prompts[0].contains("(none)"));
    assert!(prompts[1].contains("  - build-before-test: make test needs make build first"));
    let prompt = distiller::build_proposer_prompt(
        "acme/tool",
        &[],
        &json!({"build-before-test":"x y z"}),
        &BTreeSet::new(),
        &[],
        &[],
        false,
    );
    assert!(
        prompt.contains("Existing fact ids (re-emit")
            && prompt.contains("\nbuild-before-test\n")
            && !prompt.contains("x y z")
    );
}

#[test]
fn the_experiments_proposer_prompt_is_the_papers() {
    let hash = format!(
        "{:x}",
        Sha256::digest(distiller::PROPOSER_SYSTEM.as_bytes())
    );
    assert_eq!(&hash[..16], "fad9adfb8fda5875");
    let generic = distiller::PROPOSER_SYSTEM_GENERIC;
    assert!(!generic.contains("repro_written") && !generic.contains("importerror"));
    let predicates = [
        "ran_checks",
        "ran_tests",
        "command_failed",
        "missing_command",
        "missing_dependency",
        "bad_usage",
        "edited",
    ];
    for name in predicates {
        assert!(distiller::predicate(name, &json!({})).is_some(), "{name}");
    }
    assert_eq!(generic.matches(&predicates.join(", ")).count(), 2);
}

#[test]
fn a_label_rewrite_the_runner_loads_becomes_a_fact_under_generic() {
    let root = TempDir::new("generic-label-rewrite");
    let mut d = EnvFactsDistiller::new("acme/api", Vocabulary::generic(), None, None);
    for i in 0..6 {
        update(
            &mut d,
            root.path(),
            i,
            &format!("s{i}"),
            &[
                json!(["Edit",{"file_path":format!("/r/app/m{i}.rb"),"old_string":"a","new_string":"b"},"ok"]),
                sh(
                    "rspec spec/models/user_spec.rb",
                    "ERROR: file or directory not found: spec/models/user_spec.rb",
                ),
                sh("rspec models/user_spec.rb", "3 examples, 0 failures"),
            ],
        );
    }
    let fact = d.facts.values().find(|f| f.family == "arg_repair").unwrap();
    assert_eq!(fact.params["form"], "rspec");
    let examples = &fact.params["examples"];
    assert_eq!(examples.as_object().unwrap().len(), 1);
    assert_eq!(
        examples["spec/models/user_spec.rb models/user_spec.rb"].as_f64(),
        Some(6.0)
    );
}

#[test]
fn the_generic_digest_keeps_line_breaks_of_list_tool_results() {
    let root = TempDir::new("generic-list-output");
    let path = write_claude_transcript(
        &root.path().join("t.jsonl"),
        &[
            json!(["Bash",{"command":"npm run lint"},[{"type":"text","text":"Exit code 1\nerror: missing script: lint"}]]),
            json!(["Bash",{"command":"npm test"},[{"type":"text","text":"Tests run: 3"},{"type":"text","text":"ok"}]]),
        ],
    );
    let calls = distiller::load_calls(&path);
    assert_eq!(calls[0].2, "Exit code 1\nerror: missing script: lint");
    assert_eq!(calls[1].2, "Tests run: 3\nok");
    let digest = distiller::build_generic_digest(&path, "s0", json!(true), "").unwrap();
    assert_eq!(
        digest["failed_commands"],
        json!([{"cmd":"npm run lint","error":"error: missing script: lint"}])
    );
}

fn fact(id: &str) -> Value {
    json!({"id":id,"claim":format!("{id} claim"),"injection":format!("`{id}`: run make build first."),"detector":{"scope":"make test","confirm":"make build"},"cite":["s0","s1"]})
}
fn fact_runner(value: Value) -> RecordingRunner {
    RecordingRunner::new(&json!({"extractors":[],"facts":[value]}).to_string(), 1.0)
}
fn learn_and_forget(root: &Path, value: Value) -> Distiller {
    let id = value["id"].as_str().unwrap().to_owned();
    let mut d = fold_with(
        root,
        Some(&mut fact_runner(value)),
        &[build_test(), build_test()],
        0,
    );
    assert!(d.facts.contains_key(&id));
    d.forget(&id).unwrap();
    d.save(&root.join("state")).unwrap();
    d
}

#[test]
fn a_forgotten_fact_is_not_admitted_again_under_a_new_id() {
    let root = TempDir::new("generic-forgotten-new-id");
    learn_and_forget(root.path(), fact("build-before-test"));
    let mut d = fold_with(root.path(), None, &[build_lint(), build_lint()], 2);
    assert_eq!(
        d.admit(&mut fact("make-build-first")).unwrap(),
        (false, "restates-forgotten".into())
    );
    let mut narrow = fact("make-build-first");
    narrow["cite"] = json!(["s0"]);
    assert!(!d.admit(&mut narrow).unwrap().0);
    let mut wider = fact("build-first-everywhere");
    wider["detector"]["scope"] = json!("make (test|lint)");
    assert_eq!(d.admit(&mut wider).unwrap(), (true, "admitted".into()));
    let mut lint = fact("build-before-lint");
    lint["detector"]["scope"] = json!("make lint");
    lint["cite"] = json!(["s2", "s3"]);
    assert_eq!(d.admit(&mut lint).unwrap(), (true, "admitted".into()));
}

#[test]
fn a_forgotten_id_takes_no_candidate_slot() {
    let root = TempDir::new("generic-forgotten-slot");
    learn_and_forget(root.path(), fact("build-before-test"));
    let mut facts = vec![fact("build-before-test"); distiller::MAX_CANDS];
    let mut lint = fact("build-before-lint");
    lint["detector"]["scope"] = json!("make lint");
    lint["cite"] = json!(["s2", "s3"]);
    facts.push(lint);
    let mut runner = RecordingRunner::new(&json!({"extractors":[],"facts":facts}).to_string(), 1.0);
    let d = fold_with(
        root.path(),
        Some(&mut runner),
        &[build_lint(), build_lint(), build_lint()],
        2,
    );
    assert!(d.facts.contains_key("build-before-lint"));
    assert!(!d.facts.contains_key("build-before-test"));
}

#[test]
fn a_forget_refuses_only_restatements_of_its_own_evidence() {
    let root = TempDir::new("generic-forgotten-evidence");
    let mut broad = fact("make-does-it-all");
    broad["detector"]["confirm"] = json!("make");
    let mut d = learn_and_forget(root.path(), broad);
    assert_eq!(
        d.forgotten["make-does-it-all"]["evidence"],
        json!(["s0", "s1"])
    );
    let mut corrected = fact("make-test-builds-itself");
    corrected["detector"]["confirm"] = json!("make test");
    assert_eq!(
        d.admit(&mut corrected.clone()).unwrap(),
        (false, "restates-forgotten".into())
    );
    let mut d = fold_with(root.path(), None, &[build_test()], 2);
    corrected["cite"] = json!(["s1", "s2"]);
    assert_eq!(d.admit(&mut corrected).unwrap(), (true, "admitted".into()));
    let mut d = fold_with(root.path(), None, &vec![build_test(); 9], 3);
    assert!(
        d.window
            .iter()
            .all(|digest| digest["iid"] != "s0" && digest["iid"] != "s1")
    );
    let mut other = fact("go-test-passes-quietly");
    other["detector"]["confirm"] = json!(r"ok\s+\./\.\.\. 0\.0");
    other["cite"] = json!(["s10", "s11"]);
    assert_eq!(d.admit(&mut other).unwrap(), (true, "admitted".into()));
}

#[test]
fn a_forgotten_entry_without_evidence_falls_back_to_its_cites() {
    let root = TempDir::new("generic-forgotten-legacy");
    let mut d = learn_and_forget(root.path(), fact("build-before-test"));
    d.forgotten
        .get_mut("build-before-test")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("evidence");
    d.save(&root.path().join("state")).unwrap();
    let mut d = Distiller::load("acme/tool", &root.path().join("state"), "agent", true).unwrap();
    assert_eq!(
        d.admit(&mut fact("make-build-first")).unwrap(),
        (false, "restates-forgotten".into())
    );
}

#[test]
fn an_output_extractor_authored_on_a_tick_reads_every_output_line() {
    let root = TempDir::new("generic-output-extractor");
    let mut runner = RecordingRunner::new(&json!({"extractors":[{"name":"missing_module","kind":"out_regex","pattern":"Cannot find module"}],"facts":[{"id":"install-first","claim":"npm test needs npm install first","injection":"Run `npm install` before `npm test` in a fresh checkout.","detector":{"scope":"missing_module","confirm":"Cannot find module"},"cite":["s0","s1"]}]}).to_string(),100.0);
    let mut d = Distiller::new("acme/web", "agent", true);
    let mut context = ProposeContext {
        runner: Some(&mut runner),
        propose_every: 3,
        ..Default::default()
    };
    for i in 0..2 {
        let path = write_claude_transcript(
            &root.path().join(format!("s{i}.jsonl")),
            &[sh(
                "npm test",
                "> web@1.0.0 test\n> jest\n\nFAIL src/app.test.js\nCannot find module 'left-pad' from 'src/app.js'",
            )],
        );
        d.update(&path, &format!("s{i}"), json!(true), &mut context)
            .unwrap();
    }
    assert_eq!(
        d.window
            .iter()
            .map(|d| d["features"]["missing_module"].clone())
            .collect::<Vec<_>>(),
        [json!(true), json!(true)]
    );
    assert!(d.facts.contains_key("install-first"));
}
