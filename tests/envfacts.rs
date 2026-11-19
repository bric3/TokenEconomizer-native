// Converted from reference/upstream/tests/test_envfacts.py.
pub mod fixtures;
mod support;

use fixtures::{write_claude_transcript, write_codex_transcript};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};
use support::TempDir;
use tokeneconomizer::{
    config::InjectionConfig,
    microskills::{
        distiller::ProposeContext,
        envfacts::{self, EnvFact, EnvFactsDistiller},
        observe::{self, Vocabulary},
        stage::PrefixStage,
    },
    trace::calls::strip_rtk_prefix,
};

const BASE: &str = "/opt/env/base/bin/python";
const PROJ: &str = "/opt/env/proj/bin/python";
const LOCAL: &str = "~/.local/bin/pytest";
const FORM: &str = "form_repair:run_tests:python -m pytest";
const PYTEST: &str = "form_repair:run_tests:pytest";
fn cmd(command: impl AsRef<str>, output: impl AsRef<str>, code: i64) -> Value {
    json!({"type":"command_execution","command":format!("/bin/bash -lc \"{}\"",command.as_ref()),"aggregated_output":output.as_ref(),"exit_code":code})
}
fn edit(path: impl AsRef<str>) -> Value {
    json!({"type":"file_change","changes":[{"path":path.as_ref(),"kind":"update"}]})
}
fn task(root: &Path, i: usize, items: &[Value]) -> PathBuf {
    write_codex_transcript(&root.join(format!("t{i}.jsonl")), items)
}
fn broken(root: &Path, i: usize) -> PathBuf {
    task(
        root,
        i,
        &[
            edit(format!("pkg/mod{i}.py")),
            cmd(
                "python -m pytest tests/test_a.py -q",
                format!("{BASE}: No module named pytest"),
                1,
            ),
            cmd(
                format!("{PROJ} -m pytest tests/test_a.py -q"),
                "1 passed in 0.2s",
                0,
            ),
        ],
    )
}
fn clean(root: &Path, i: usize) -> PathBuf {
    task(
        root,
        i,
        &[
            edit(format!("pkg/c{i}.py")),
            cmd("pytest -q", "bash: pytest: command not found", 127),
            cmd("python -m pytest -q", "4 passed in 0.01s", 0),
        ],
    )
}
fn two_forms(root: &Path, i: usize) -> PathBuf {
    task(
        root,
        i,
        &[
            edit(format!("pkg/m{i}.py")),
            cmd(
                "pytest tests/test_a.py -q",
                "bash: pytest: command not found",
                127,
            ),
            cmd(
                "python -m pytest tests/test_a.py -q",
                format!("{BASE}: No module named pytest"),
                1,
            ),
            cmd(
                format!("{PROJ} -m pytest tests/test_a.py -q"),
                "1 passed",
                0,
            ),
        ],
    )
}
fn distiller(repo: &str) -> EnvFactsDistiller {
    EnvFactsDistiller::new(repo, Vocabulary::default(), None, None)
}
fn stream(root: &Path, n: usize, maker: impl Fn(&Path, usize) -> PathBuf) -> EnvFactsDistiller {
    let mut d = distiller("acme/widget");
    for i in 0..n {
        d.render();
        d.update(&maker(root, i), &format!("w-{i}"), json!(true), None)
            .unwrap();
    }
    d
}

#[test]
fn classify_forms_and_intents() {
    for (command, expected) in [
        (
            "/bin/bash -lc \"source /opt/env/bin/activate proj && python -m pytest tests -q\"",
            ("run_tests", "source activate proj && python -m pytest"),
        ),
        (
            "conda run -n proj python - <<'PY'\nimport x\nPY",
            ("run_interpreter", "conda run -n proj python"),
        ),
        (
            "PYTHONPATH=/testbed ./tests/runtests.py auth_tests -v 1",
            ("run_tests", "python tests/runtests.py"),
        ),
        (
            &format!("{PROJ} bin/test sympy/core/tests/test_a.py"),
            ("run_tests", &format!("{PROJ} bin/test")),
        ),
    ] {
        let observations =
            observe::classify_command(command, 0, &Vocabulary::default(), "", None, None).unwrap();
        assert_eq!(
            observations
                .iter()
                .map(|o| (o.intent.as_str(), o.form.as_str()))
                .collect::<Vec<_>>(),
            [expected],
            "{command}"
        );
    }
    let classify = |command: &str| {
        observe::classify_command(command, 0, &Vocabulary::default(), "", None, None).unwrap()
    };
    let observations =
        classify("git remote -v; git tag --contains HEAD | head; git log --oneline -3");
    assert_eq!(
        observations
            .iter()
            .map(|o| (o.intent.as_str(), o.probe.as_deref().unwrap()))
            .collect::<Vec<_>>(),
        [("probe_vcs_refs", "remote"), ("probe_vcs_refs", "tag")]
    );
    let observations = classify("curl -sS https://api.example.org/x | head -3");
    assert_eq!(observations[0].intent, "fetch_remote");
    assert_eq!(observations[0].hosts, ["api.example.org"]);
    assert!(classify("git tag | tail -5").is_empty());
    assert_eq!(
        classify("python - <<'PY'\npytest_is_a_word = 1\nPY")[0].intent,
        "run_interpreter"
    );
    assert_eq!(
        classify("read -r x <<< x\npytest -q\nx=1")
            .iter()
            .map(|o| o.intent.as_str())
            .collect::<Vec<_>>(),
        ["run_tests"]
    );
}

#[test]
fn error_classes_are_environmental_only() {
    for (output, expected) in [
        (
            "ModuleNotFoundError: No module named 'x'",
            Some("module_missing"),
        ),
        ("bash: pytest: command not found", Some("command_missing")),
        (
            "curl: (7) Failed to connect to h port 443: Connection refused",
            Some("connection_refused"),
        ),
        ("AssertionError: 1 != 2\nFAILED tests/test_a.py::t", None),
    ] {
        assert_eq!(observe::error_class(output), expected);
    }
}

#[test]
fn interpreter_fact_forms_fires_and_renders() {
    let root = TempDir::new("envfacts-forms");
    let mut d = stream(root.path(), 3, broken);
    assert_eq!(d.render(), "");
    d.update(&broken(root.path(), 3), "w-3", json!(true), None)
        .unwrap();
    let text = d.render();
    assert!(text.starts_with(envfacts::HEADER));
    assert!(text.contains(PROJ) && text.contains("python -m pytest …` fails here"));
    assert_eq!(d.opt_tokens, 0.0);
    let life = &d.facts[FORM].life;
    assert_eq!(life.state, "ACTIVE");
    assert_eq!((life.fire_num, life.fire_den), (4, 4));
    assert!(envfacts::lint_ok(&text));
}

#[test]
fn clean_environment_forms_nothing() {
    let root = TempDir::new("envfacts-clean");
    let mut d = stream(root.path(), 8, |root, i| {
        task(
            root,
            i,
            &[
                edit(format!("pkg/m{i}.py")),
                cmd("python -m pytest tests/test_a.py -q", "1 passed", 0),
            ],
        )
    });
    assert!(d.facts.is_empty());
    assert_eq!(d.render(), "");
}

#[test]
fn prevented_waste_still_counts_as_firing() {
    let root = TempDir::new("envfacts-prevented");
    let mut d = stream(root.path(), 4, broken);
    assert!(!d.render().is_empty());
    d.update(
        &task(
            root.path(),
            9,
            &[
                edit("pkg/z.py"),
                cmd(
                    format!("{PROJ} -m pytest tests/test_z.py -q"),
                    "3 passed",
                    0,
                ),
            ],
        ),
        "w-9",
        json!(true),
        None,
    )
    .unwrap();
    let life = &d.facts[FORM].life;
    assert_eq!((life.fire_num, life.fire_den), (5, 5));
    assert!(!d.render().is_empty());
}

#[test]
fn bare_runner_success_contradicts_and_retires() {
    let root = TempDir::new("envfacts-retire");
    let mut d = stream(root.path(), 4, broken);
    assert!(!d.render().is_empty());
    for i in 20..23 {
        d.render();
        d.update(
            &task(
                root.path(),
                i,
                &[
                    edit(format!("pkg/f{i}.py")),
                    cmd("python -m pytest tests/test_a.py -q", "1 passed", 0),
                ],
            ),
            &format!("w-{i}"),
            json!(true),
            None,
        )
        .unwrap();
    }
    assert_eq!(d.facts[FORM].life.state, "RETIRED");
    assert_eq!(d.render(), "");
}

#[test]
fn bare_one_liner_success_is_not_a_contradiction() {
    let root = TempDir::new("envfacts-one-liner");
    let mut d = stream(root.path(), 4, broken);
    d.update(
        &task(
            root.path(),
            9,
            &[
                edit("pkg/q.py"),
                cmd("python -c 'import pkg; print(pkg.__version__)'", "1.0", 0),
                cmd(format!("{PROJ} -m pytest tests -q"), "2 passed", 0),
            ],
        ),
        "w-9",
        json!(true),
        None,
    )
    .unwrap();
    assert_eq!(d.facts[FORM].life.state, "ACTIVE");
}

#[test]
fn history_fact_forms_only_when_probes_return_nothing() {
    let root = TempDir::new("envfacts-history");
    let mut d = stream(root.path(), 4, |root, i| {
        task(
            root,
            i,
            &[
                edit(format!("pkg/e{i}.py")),
                cmd("git remote -v; git tag --contains HEAD | head", "", 0),
                cmd(format!("{PROJ} -m pytest tests -q"), "1 passed", 0),
            ],
        )
    });
    assert!(
        d.render()
            .contains("history ends at the checked-out commit")
    );
    let mut d = distiller("acme/leaky");
    for i in 0..4 {
        d.render();
        d.update(
            &task(
                root.path(),
                i,
                &[
                    edit(format!("pkg/l{i}.py")),
                    cmd("git tag --contains HEAD | head", "0.22\n0.22.1\n0.23.0", 0),
                ],
            ),
            &format!("w-{i}"),
            json!(true),
            None,
        )
        .unwrap();
    }
    assert_eq!(d.render(), "");
    assert_eq!(d.facts["empty_probe:vcs_refs"].life.state, "RETIRED");
}

#[test]
fn refused_hosts_form_a_boundary_fact_and_a_reachable_host_does_not() {
    let root = TempDir::new("envfacts-boundary");
    let mut d = stream(root.path(), 4, |root, i| {
        task(
            root,
            i,
            &[
                edit(format!("pkg/n{i}.py")),
                cmd(
                    "curl -sS https://raw.example.org/a/b.py",
                    "curl: (7) Failed to connect to raw.example.org port 443: Connection refused",
                    7,
                ),
                cmd(
                    "curl -sS https://api.example.org/search?q=x",
                    "curl: (7) Failed to connect to api.example.org port 443: Connection refused",
                    7,
                ),
            ],
        )
    });
    let text = d.render();
    assert!(
        text.contains("raw.example.org")
            && text.contains("api.example.org")
            && text.contains("refused")
    );
    d.update(
        &task(
            root.path(),
            9,
            &[cmd(
                "curl -sS https://api.example.org/ok",
                r#"{"ok": true}"#,
                0,
            )],
        ),
        "w-9",
        json!(true),
        None,
    )
    .unwrap();
    assert_eq!(d.facts["boundary:fetch_remote"].life.state, "RETIRED");
}

#[test]
fn lint_refuses_completion_advice() {
    assert!(!envfacts::lint_ok(
        "You are done once the tests pass; skip the rest."
    ));
    assert!(envfacts::lint_ok(
        "`python …` fails here with command not found; `/x/bin/python …` works."
    ));
}

#[test]
fn block_is_capped_in_facts_and_tokens() {
    let mut d = distiller("acme/widget");
    for i in 0..7 {
        let id = format!("form_repair:run_tests:tool{i}");
        let mut fact = EnvFact::new(&id,"form_repair",false,json!({"intent":"run_tests","a":format!("tool{i}"),"b_counts":{format!("/very/long/path/number/{i}/that/keeps/going/bin/tool{i}"):3},"error_counts":{"command_missing":3}}).as_object().unwrap().clone());
        fact.life.state = "ACTIVE".into();
        fact.life.fire_num = 10;
        fact.life.fire_den = 10;
        d.facts.insert(id, fact);
    }
    let text = d.render();
    assert!(!text.is_empty());
    assert!(text.chars().count() <= envfacts::MAX_TOKENS as usize * 4);
    assert!(text.matches("\n- ").count() <= envfacts::MAX_FACTS as usize);
}

#[test]
fn form_repair_facts_sharing_the_working_form_merge_into_one_line() {
    let root = TempDir::new("envfacts-merge");
    let text = stream(root.path(), 4, two_forms).render();
    assert_eq!(text.matches("\n- ").count(), 1);
    assert!(text.contains("pytest …`") && text.contains("python -m pytest …`"));
}

#[test]
fn a_merged_line_with_one_working_form_says_works() {
    let root = TempDir::new("envfacts-merged-works");
    assert!(
        stream(root.path(), 4, two_forms)
            .render()
            .trim()
            .lines()
            .next_back()
            .unwrap()
            .ends_with(&format!("`{PROJ} -m pytest …` works."))
    );
}

#[test]
fn an_environment_fact_forms_in_a_repository_where_every_session_edits_one_file() {
    let root = TempDir::new("envfacts-same-file");
    let mut d = stream(root.path(), 4, |root, i| {
        task(
            root,
            i,
            &[
                edit("pkg/stock.py"),
                cmd("pytest -q", "bash: pytest: command not found", 127),
                cmd("python -m pytest -q", "4 passed in 0.01s", 0),
            ],
        )
    });
    assert_eq!(d.facts[PYTEST].life.state, "ACTIVE");
    assert!(
        d.render()
            .contains("`pytest …` fails here with command not found")
    );
}

#[test]
fn a_form_that_only_worked_with_an_assignment_before_it_is_not_named() {
    let root = TempDir::new("envfacts-assignment-form");
    let mut d = stream(root.path(), 3, clean);
    for i in 3..6 {
        d.render();
        d.update(&task(root.path(),i,&[edit(format!("pkg/m{i}.py")),cmd("pytest -q","bash: pytest: command not found",127),cmd("pytest -q","bash: pytest: command not found",127),cmd(format!("{LOCAL} -q"),"E   ModuleNotFoundError: No module named 'stock'\nInterrupted: 1 error during collection\n1 error in 0.08s",2),cmd(format!("PYTHONPATH=. {LOCAL} -q"),"4 passed in 0.01s",0),cmd(format!("PYTHONPATH=. {LOCAL} -q"),"4 passed in 0.01s",0)]),&format!("m-{i}"),json!(true),None).unwrap();
    }
    assert!(d.facts[PYTEST].params["b_counts"].get(LOCAL).is_none());
    assert!(
        d.render()
            .trim()
            .lines()
            .next_back()
            .unwrap()
            .ends_with("`python -m pytest …` works.")
    );
}

#[test]
fn one_session_is_one_piece_of_evidence_per_form() {
    let root = TempDir::new("envfacts-evidence");
    let mut d = stream(root.path(), 2, clean);
    let mut items = vec![edit("pkg/r2.py")];
    items.extend(vec![
        cmd("pytest -q", "bash: pytest: command not found", 127);
        3
    ]);
    items.extend(vec![cmd(format!("{PROJ} -m pytest -q"), "1 passed", 0); 2]);
    d.render();
    d.update(&task(root.path(), 2, &items), "r", json!(true), None)
        .unwrap();
    let fact = &d.facts[PYTEST];
    let counts = &fact.params["b_counts"];
    assert_eq!(counts.as_object().unwrap().len(), 2);
    assert_eq!(counts["python -m pytest"].as_f64(), Some(2.0));
    assert_eq!(counts[format!("{PROJ} -m pytest")].as_f64(), Some(1.0));
    assert_eq!(fact.working_form().as_deref(), Some("python -m pytest"));
}

#[test]
fn a_named_form_that_stops_working_is_named_no_more() {
    let root = TempDir::new("envfacts-no-working-form");
    let mut d = stream(root.path(), 4, clean);
    assert!(d.render().contains("`python -m pytest …` works"));
    for i in 4..9 {
        d.render();
        d.update(
            &task(
                root.path(),
                i,
                &[
                    edit(format!("pkg/g{i}.py")),
                    cmd("pytest -q", "bash: pytest: command not found", 127),
                    cmd(
                        "python -m pytest -q",
                        format!("{BASE}: No module named pytest"),
                        1,
                    ),
                ],
            ),
            &format!("g-{i}"),
            json!(true),
            None,
        )
        .unwrap();
    }
    let fact = &d.facts[PYTEST];
    assert_eq!(
        fact.params["b_fails"]["python -m pytest"].as_f64(),
        Some(5.0)
    );
    assert!(fact.working_form().is_none());
    assert_eq!(fact.claim(), "");
    assert!(!d.render().contains("python -m pytest"));
}

#[test]
fn env_assignments_after_env_are_part_of_how_a_command_was_typed() {
    let classify = |command: &str| {
        observe::classify_command(command, 0, &Vocabulary::default(), "", None, None).unwrap()
    };
    let observations = classify("env PYTHONPATH=. pytest -q");
    assert_eq!(observations.len(), 1);
    let o = &observations[0];
    assert_eq!(
        (o.intent.as_str(), o.form.as_str(), o.env),
        ("run_tests", "pytest", true)
    );
    let observations = classify("PYTHONPATH=. python -m pytest -q");
    assert_eq!(observations.len(), 1);
    assert_eq!(
        (observations[0].form.as_str(), observations[0].env),
        ("python -m pytest", true)
    );
    let observations = classify("python -m pytest -q");
    assert_eq!(observations.len(), 1);
    assert!(!observations[0].env);
}

#[test]
fn save_load_roundtrip() {
    let root = TempDir::new("envfacts-roundtrip");
    let mut d = stream(root.path(), 4, broken);
    d.save(root.path()).unwrap();
    let mut loaded = EnvFactsDistiller::load(
        "acme/widget",
        root.path(),
        Vocabulary::default(),
        None,
        None,
    )
    .unwrap();
    assert_eq!(loaded.now, 4);
    assert_eq!(loaded.render(), d.render());
    let saved: Value = serde_json::from_slice(
        &fs::read(
            root.path()
                .join(envfacts::STATE_SUBDIR)
                .join("acme__widget.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(saved["opt_tokens"].as_f64(), Some(0.0));
}

#[test]
fn claude_schema_yields_the_same_fact() {
    let root = TempDir::new("envfacts-claude");
    let mut d = distiller("acme/widget");
    for i in 0..4 {
        let path = write_claude_transcript(
            &root.path().join(format!("c{i}.jsonl")),
            &[
                json!(["Edit",{"file_path":format!("/testbed/pkg/c{i}.py")},"ok"]),
                json!(["Bash",{"command":"python -m pytest tests/test_a.py -q"},format!("Exit code 1\n{BASE}: No module named pytest")]),
                json!(["Bash",{"command":format!("{PROJ} -m pytest tests/test_a.py -q")},"1 passed"]),
            ],
        );
        d.render();
        d.update(&path, &format!("c-{i}"), json!(true), None)
            .unwrap();
    }
    assert!(d.render().contains(PROJ));
}

#[test]
fn prefix_stage_envfacts_profile_needs_no_runner_and_reports_zero() {
    let root = TempDir::new("envfacts-prefix");
    let config = InjectionConfig {
        enabled: true,
        profile: "envfacts".into(),
        ..Default::default()
    };
    let mut stage = PrefixStage::new(
        &config,
        "acme/widget",
        root.path(),
        true,
        "codex",
        Vocabulary::default(),
        &mut ProposeContext::default(),
    )
    .unwrap();
    assert_eq!(stage.render(), "");
    let run = root.path().join("run");
    fs::create_dir(&run).unwrap();
    write_codex_transcript(
        &run.join("transcript.jsonl"),
        &[
            cmd(
                "python -m pytest -q",
                format!("{BASE}: No module named pytest"),
                1,
            ),
            cmd(format!("{PROJ} -m pytest -q"), "1 passed", 0),
        ],
    );
    let mut row = json!({"resolved":true});
    let (tokens, reason) = stage
        .after_task(
            &json!({"instance_id":"w-1"}),
            &run,
            &mut row,
            &mut ProposeContext::default(),
        )
        .unwrap();
    assert_eq!(tokens, 0.0);
    assert_eq!(reason, "envfacts");
    assert_eq!(row["manual_facts"]["_profile"], "envfacts");
    assert!(row["manual_facts"].get(FORM).is_some());
}

#[test]
fn the_classifier_sees_through_rtks_command_prefix() {
    let classify = |command: &str| {
        observe::classify_command(command, 0, &Vocabulary::default(), "", None, None)
            .unwrap()
            .into_iter()
            .map(|o| (o.intent, o.form))
            .collect::<Vec<_>>()
    };
    for bare in [
        "python -m pytest tests/ -q",
        "pytest -q",
        "bin/test sympy/x.py",
        "K=1 pytest -q",
        "pip install foo",
        "curl https://x",
        "git status",
        "rg -n foo src",
    ] {
        for prefix in ["rtk ", "rtk proxy ", "rtk K=2 "] {
            let wrapped = format!("{prefix}{bare}");
            assert_eq!(classify(&wrapped), classify(bare), "{wrapped}");
        }
    }
    for (input, expected) in [
        ("rtk pytest -q", "pytest -q"),
        ("rtk proxy make", "make"),
        ("rtk", "rtk"),
        ("rtkfoo bar", "rtkfoo bar"),
        ("pytest", "pytest"),
    ] {
        assert_eq!(strip_rtk_prefix(input), expected);
    }
}
