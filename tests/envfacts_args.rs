// Converted from reference/upstream/tests/test_envfacts_args.py.
pub mod fixtures;
mod support;

use fixtures::write_claude_transcript;
use serde_json::{Value, json};
use std::{fs, path::Path};
use support::TempDir;
use tokeneconomizer::{
    microskills::{
        envfacts::{self, EnvFactsDistiller},
        observe::{self, Observation, Vocabulary},
    },
    trace::calls,
};

const OK: &str = "Ran 4 tests in 0.1s\n\nOK";
const NEED: &str = "Traceback (most recent call last):\n  ...\nRuntimeError: settings are missing. Define the environment variable APP_SETTINGS before importing app.";
const ROOT: &str = "/r";
const CANT_OPEN: &str =
    "Exit code 2\npython: can't open file 'runtests.py': [Errno 2] No such file or directory";
fn edit(i: usize) -> Value {
    json!(["Edit",{"file_path":format!("/r/pkg/mod{i}.py"),"old_string":"a","new_string":"b"},"ok"])
}
fn sh(command: impl AsRef<str>, output: &str) -> Value {
    json!(["Bash",{"command":command.as_ref()},output])
}
fn run(label: impl AsRef<str>, output: &str) -> Value {
    sh(
        format!(
            "python tests/runtests.py {} -v 2 2>&1 | tail -20",
            label.as_ref()
        ),
        output,
    )
}
fn py(code: &str, env: &str, output: &str) -> Value {
    sh(format!("{env}python -c '{code}'"), output)
}
fn at(mut call: Value, cwd: &str) -> Value {
    call.as_array_mut().unwrap().push(json!(cwd));
    call
}
fn stream(root: &Path, tasks: Vec<Vec<Value>>) -> (EnvFactsDistiller, Vec<String>) {
    let mut distiller = EnvFactsDistiller::new("acme/widget", Vocabulary::default(), None, None);
    let mut rendered = Vec::new();
    for (i, task) in tasks.into_iter().enumerate() {
        rendered.push(distiller.render());
        distiller
            .update(
                &write_claude_transcript(&root.join(format!("t{i}.jsonl")), &task),
                &format!("t{i}"),
                json!(true),
                None,
            )
            .unwrap();
    }
    (distiller, rendered)
}
fn classify(command: &str, cwd: &str, root: Option<&str>, top: Option<&str>) -> Vec<Observation> {
    observe::classify_command(command, 0, &Vocabulary::default(), cwd, root, top).unwrap()
}

#[test]
fn only_a_shape_rewrite_counts_as_a_repair() {
    for (a, b, rule) in [
        (
            "tests.model_inheritance",
            "model_inheritance",
            Some(("strip", "tests.")),
        ),
        (
            "tests.admin_views.tests",
            "admin_views.tests",
            Some(("strip", "tests.")),
        ),
        (
            "db.datetime.test_x::DateTests::test_a",
            "db.datetime.test_x.DateTests.test_a",
            Some(("sep", "::")),
        ),
        (
            "db.datetime.test_x::DateTests",
            "db.datetime.test_x",
            Some(("sep", "::")),
        ),
        ("model_fields.DurationFieldTests", "model_fields", None),
        ("admin_views.ChangeListTests", "admin_views", None),
        ("app.tests", "other.tests", None),
    ] {
        assert_eq!(
            observe::label_rule(a, b),
            rule.map(|(kind, text)| (kind, text.into())),
            "{a} -> {b}"
        );
    }
}

#[test]
fn labels_are_the_structured_arguments_after_the_runner() {
    assert_eq!(
        observe::test_labels(
            &classify(
                "python tests/runtests.py tests.app -v 2 --settings=x 2>&1 | tail",
                "",
                None,
                None
            )[0]
        ),
        ["tests.app"]
    );
    assert_eq!(
        observe::test_labels(
            &classify(
                "python -m pytest pkg/test_a.py::T::test_b -q > /tmp/o.log",
                "",
                None,
                None
            )[0]
        ),
        ["pkg/test_a.py::T::test_b"]
    );
}

#[test]
fn a_label_prefix_the_runner_rejects_becomes_a_fact() {
    let root = TempDir::new("args-label-prefix");
    let (distiller, rendered) = stream(
        root.path(),
        (0..6)
            .map(|i| {
                vec![
                    edit(i),
                    run(
                        format!("tests.app{i}"),
                        "ModuleNotFoundError: No module named 'tests'",
                    ),
                    run(format!("app{i}"), OK),
                ]
            })
            .collect(),
    );
    assert!(
        rendered
            .last()
            .unwrap()
            .contains("takes test labels without the `tests.` prefix")
    );
    let claim = distiller
        .facts
        .values()
        .find(|f| f.family == "arg_repair")
        .unwrap()
        .claim();
    assert!(claim.starts_with("`python tests/runtests.py` takes") && claim.contains("(`app"));
    assert!(envfacts::lint_ok(&claim));
}

#[test]
fn pytest_style_separators_the_runner_rejects_become_a_fact() {
    let root = TempDir::new("args-separators");
    let (_, rendered) = stream(
        root.path(),
        (0..6)
            .map(|i| {
                vec![
                    edit(i),
                    run(
                        format!("app{i}.tests::T::test_x"),
                        &format!(
                            "ImportError: Failed to import test module: app{i}.tests::T::test_x"
                        ),
                    ),
                    run(format!("app{i}.tests.T.test_x"), OK),
                ]
            })
            .collect(),
    );
    assert!(
        rendered
            .last()
            .unwrap()
            .contains("takes dotted test labels")
    );
}

#[test]
fn a_plain_mistake_is_not_a_fact() {
    let root = TempDir::new("args-mistake");
    let (distiller, rendered) = stream(
        root.path(),
        (0..6)
            .map(|i| {
                vec![
                    edit(i),
                    run(
                        format!("app{i}.tests.Missing"),
                        &format!(
                            "AttributeError: module 'app{i}.tests' has no attribute 'Missing'"
                        ),
                    ),
                    run(format!("app{i}"), OK),
                ]
            })
            .collect(),
    );
    assert!(!distiller.facts.values().any(|f| f.family == "arg_repair"));
    assert_eq!(rendered.last().unwrap(), "");
}

#[test]
fn a_prefixed_label_that_loads_contradicts_the_fact() {
    let root = TempDir::new("args-prefix-contradiction");
    let mut tasks: Vec<_> = (0..4)
        .map(|i| {
            vec![
                edit(i),
                run(
                    format!("tests.app{i}"),
                    "ModuleNotFoundError: No module named 'tests'",
                ),
                run(format!("app{i}"), OK),
            ]
        })
        .collect();
    tasks.push(vec![edit(9), run("tests.app9", OK)]);
    let (distiller, _) = stream(root.path(), tasks);
    let id = distiller
        .facts
        .keys()
        .find(|id| id.starts_with("arg_repair:"))
        .unwrap();
    assert_eq!(
        distiller.last_digest.as_ref().unwrap()["verdicts"][id][0],
        "contradict"
    );
}

#[test]
fn an_environment_variable_the_code_asks_for_and_its_working_value_become_a_fact() {
    let root = TempDir::new("args-env-value");
    let (distiller,rendered) = stream(root.path(),(0..6).map(|i| vec![edit(i),py("import app; app.run()","",NEED),py("import app; app.run()","APP_SETTINGS=conf.missing ","Traceback (most recent call last):\nModuleNotFoundError: No module named 'conf.missing'"),py("import app; app.run()","APP_SETTINGS=conf.dev ","ran")]).collect());
    assert!(rendered.last().unwrap().contains("Code that imports the project needs `APP_SETTINGS` set here: `APP_SETTINGS=conf.dev` works, `conf.missing` does not."));
    assert!(envfacts::lint_ok(
        &distiller.facts["env_value:APP_SETTINGS"].claim()
    ));
}

#[test]
fn a_value_set_in_the_code_counts_too() {
    let root = TempDir::new("args-env-setdefault");
    let code = "import os; os.environ.setdefault(\"APP_SETTINGS\", \"conf.dev\"); import app";
    let (distiller, _) = stream(
        root.path(),
        (0..6)
            .map(|i| vec![edit(i), py("import app", "", NEED), py(code, "", "ran")])
            .collect(),
    );
    let good = &distiller.facts["env_value:APP_SETTINGS"].params["good"];
    assert_eq!(good.as_object().unwrap().len(), 1);
    assert_eq!(good["conf.dev"].as_f64(), Some(6.0));
}

#[test]
fn no_value_that_worked_means_no_fact() {
    let root = TempDir::new("args-env-no-success");
    let (distiller, _) = stream(
        root.path(),
        (0..6)
            .map(|i| {
                vec![
                    edit(i),
                    py("import app", "", NEED),
                    py(
                        "import app",
                        "APP_SETTINGS=conf.missing ",
                        "Traceback (most recent call last):\nNameError: x",
                    ),
                ]
            })
            .collect(),
    );
    assert!(!distiller.facts.contains_key("env_value:APP_SETTINGS"));
}

#[test]
fn output_filtered_away_neither_teaches_nor_contradicts() {
    let root = TempDir::new("args-filtered-label");
    let mut tasks: Vec<_> = (0..4)
        .map(|i| {
            vec![
                edit(i),
                run(
                    format!("app{i}.tests::T::test_x"),
                    &format!("app{i}.tests::T::test_x (unittest.loader._FailedTest) ... ERROR"),
                ),
                run(format!("app{i}.tests.T.test_x"), OK),
            ]
        })
        .collect();
    tasks.push(vec![
        edit(8),
        run("app8.tests::T::test_x", "FAILED (errors=1)"),
        run("app8.tests::T::test_y", ""),
    ]);
    let (distiller, _) = stream(root.path(), tasks);
    let id = "arg_repair:python tests/runtests.py:sep:::";
    assert_ne!(
        distiller.last_digest.as_ref().unwrap()["verdicts"][id][0],
        "contradict"
    );
    assert_ne!(distiller.facts[id].life.state, "RETIRED");
}

#[test]
fn a_repair_needs_the_rewritten_label_to_have_run_tests() {
    let root = TempDir::new("args-label-no-tests");
    let (distiller, _) = stream(
        root.path(),
        (0..6)
            .map(|i| {
                vec![
                    edit(i),
                    run(
                        format!("tests.app{i}"),
                        "ModuleNotFoundError: No module named 'tests'",
                    ),
                    run(format!("app{i}"), ""),
                ]
            })
            .collect(),
    );
    assert!(!distiller.facts.values().any(|f| f.family == "arg_repair"));
}

#[test]
fn being_asked_for_the_variable_again_does_not_confirm_its_value() {
    let root = TempDir::new("args-env-repeat");
    let mut tasks = vec![vec![
        edit(0),
        py("import app", "", NEED),
        py("import app", "APP_SETTINGS=conf.dev ", "ran"),
    ]];
    tasks.extend((1..6).map(|i| vec![edit(i), py("import app", "", NEED)]));
    let (distiller, rendered) = stream(root.path(), tasks);
    assert_eq!(
        distiller.facts["env_value:APP_SETTINGS"].life.state,
        "CANDIDATE"
    );
    assert!(!rendered.last().unwrap().contains("APP_SETTINGS"));
}

#[test]
fn mistyped_labels_do_not_teach_that_the_runner_fails() {
    let root = TempDir::new("args-mistyped-label");
    let bad = "tests (unittest.loader._FailedTest) ... ERROR\nImportError: Failed to import test module: tests\nModuleNotFoundError: No module named 'tests.app'";
    let (distiller, rendered) = stream(
        root.path(),
        (0..6)
            .map(|i| {
                vec![
                    edit(i),
                    run(format!("tests.app{i}.tests.T"), bad),
                    sh(
                        format!("cd tests && python runtests.py app{i}.tests.T -v 2"),
                        OK,
                    ),
                ]
            })
            .collect(),
    );
    assert!(!distiller.facts.values().any(|f| f.family == "form_repair"));
    assert!(!rendered.last().unwrap().contains("fails here"));
}

#[test]
fn a_directory_change_is_part_of_the_form() {
    let observations = classify("cd tests && python runtests.py forms -v 2", "", None, None);
    assert_eq!(observations[0].form, "cd tests && python runtests.py");
    assert_eq!(observe::test_labels(&observations[0]), ["forms"]);
}

#[test]
fn a_missing_runner_is_still_a_form_failure_not_a_label_one() {
    for (label, output, expected) in [
        (
            "tests/test_a.py",
            "/usr/bin/python3: No module named pytest",
            false,
        ),
        (
            "tests.app",
            "ModuleNotFoundError: No module named 'tests'",
            true,
        ),
        (
            "app.tests.Missing",
            "AttributeError: module 'app.tests' has no attribute 'Missing'",
            true,
        ),
        (
            "app.tests",
            "AttributeError: 'NoneType' object has no attribute 'pk'",
            false,
        ),
    ] {
        assert_eq!(
            observe::label_failed(&[label.into()], output),
            expected,
            "{label}"
        );
    }
}

#[test]
fn a_bare_runner_in_a_directory_an_earlier_cd_left_is_learned_with_that_cd() {
    let root = TempDir::new("args-carried-cwd");
    let (distiller,rendered) = stream(root.path(),(0..6).map(|i| vec![at(edit(i),ROOT),at(sh("python -m pytest tests/app/tests.py -q","/venv/bin/python: No module named pytest"),ROOT),at(sh(format!("cd tests && python runtests.py app{i} -v 2"),OK),ROOT),at(sh(format!("python runtests.py app{i}.tests -v 2"),OK),"/r/tests"),at(sh("python runtests.py app.tests 2>&1 | tail -3","python: can't open file 'runtests.py': [Errno 2] No such file or directory"),ROOT)]).collect());
    assert!(
        rendered
            .last()
            .unwrap()
            .contains("`(cd tests && python runtests.py …)` works")
    );
    assert!(
        !rendered
            .last()
            .unwrap()
            .contains("`python runtests.py …` works")
    );
    assert!(
        !distiller
            .facts
            .keys()
            .any(|k| k.ends_with(":python runtests.py"))
    );
}

#[test]
fn a_cd_into_the_start_directory_by_absolute_path_is_no_cd() {
    for (cmd, cwd, expected) in [
        (
            "cd /r && python tests/runtests.py app",
            "",
            "python tests/runtests.py",
        ),
        (
            "cd /r/tests && python runtests.py app",
            "",
            "cd tests && python runtests.py",
        ),
        (
            "cd .. && python runtests.py app",
            "tests",
            "python runtests.py",
        ),
        (
            "python runtests.py app",
            "tests",
            "cd tests && python runtests.py",
        ),
        (
            "cd /elsewhere && python runtests.py app",
            "",
            "cd /elsewhere && python runtests.py",
        ),
    ] {
        assert_eq!(
            classify(cmd, cwd, Some(ROOT), None)[0].form,
            expected,
            "{cmd}"
        );
    }
}

#[test]
fn a_cd_up_to_the_repository_from_a_subdirectory_is_no_cd() {
    for (cmd, expected) in [
        ("cd /r && pytest -q", "pytest"),
        ("cd .. && pytest -q", "pytest"),
        (
            "cd /r && cd tests && python runtests.py app",
            "cd tests && python runtests.py",
        ),
        (
            "cd /r/tests && python runtests.py app",
            "cd /r/tests && python runtests.py",
        ),
        ("cd /elsewhere && pytest -q", "cd /elsewhere && pytest"),
    ] {
        assert_eq!(
            classify(cmd, "", Some("/r/pkg"), Some(ROOT))[0].form,
            expected,
            "{cmd}"
        );
    }
    assert_eq!(
        classify("cd /r && pytest -q", "", Some("/r/pkg"), None)[0].form,
        "cd /r && pytest"
    );
}

#[test]
fn the_repository_is_the_nearest_directory_holding_git() {
    let root = TempDir::new("args-repo-top");
    fs::create_dir_all(root.path().join("repo/.git")).unwrap();
    fs::create_dir_all(root.path().join("repo/pkg/sub")).unwrap();
    fs::create_dir_all(root.path().join("wt/pkg")).unwrap();
    fs::write(
        root.path().join("wt/.git"),
        "gitdir: /elsewhere/.git/worktrees/wt\n",
    )
    .unwrap();
    for (start, expected) in [("repo/pkg/sub", "repo"), ("wt/pkg", "wt")] {
        assert_eq!(
            observe::repository_top(root.path().join(start).to_str().unwrap()).as_deref(),
            root.path().join(expected).to_str()
        );
    }
    assert_eq!(observe::repository_top("relative/path"), None);
    assert_eq!(
        observe::repository_top(root.path().join("repo/gone").to_str().unwrap()),
        None
    );
}

#[test]
fn sessions_started_in_a_subdirectory_learn_the_repositorys_plain_form() {
    let root = TempDir::new("args-repo-subdir");
    let repo = root.path().join("repo");
    fs::create_dir_all(repo.join(".git")).unwrap();
    fs::create_dir(repo.join("pkg")).unwrap();
    let start = repo.join("pkg").to_string_lossy().into_owned();
    let (distiller,rendered) = stream(root.path(),(0..6).map(|i| vec![at(json!(["Edit",{"file_path":repo.join(format!("pkg/m{i}.py")),"old_string":"a","new_string":"b"},"ok"]),&start),at(sh(format!("cd {} && pytest -q",repo.display()),"bash: line 1: pytest: command not found"),&start),at(sh(format!("cd {} && python -m pytest -q",repo.display()),"1 passed in 0.1s"),&start)]).collect());
    assert!(
        rendered
            .last()
            .unwrap()
            .contains("`pytest …` fails here with command not found")
    );
    assert!(!rendered.last().unwrap().contains(repo.to_str().unwrap()));
    assert!(
        !distiller
            .facts
            .keys()
            .any(|k| k.contains(repo.to_str().unwrap()))
    );
}

#[test]
fn a_command_stopped_by_its_own_cd_teaches_nothing() {
    let root = TempDir::new("args-cd-failure");
    let path = write_claude_transcript(
        &root.path().join("t.jsonl"),
        &[at(
            sh(
                "cd tests && python runtests.py app",
                "Exit code 1\n/bin/bash: line 1: cd: tests: No such file or directory",
            ),
            "/r/tests",
        )],
    );
    assert!(
        observe::observe_task(&path, &Vocabulary::default())
            .unwrap()
            .obs
            .is_empty()
    );
}

#[test]
fn call_directories_line_up_with_the_calls() {
    let root = TempDir::new("args-call-cwds");
    let path = write_claude_transcript(
        &root.path().join("t.jsonl"),
        &[
            at(sh("ls", "a"), ROOT),
            json!(["Read",{"file_path":"/r/a"},"x"]),
            at(sh("ls", "b"), "/r/tests"),
        ],
    );
    assert_eq!(
        calls::call_cwds(&path),
        [Some(ROOT.into()), None, Some("/r/tests".into())]
    );
    assert_eq!(calls::load_trajectory(&path).len(), 3);
}

#[test]
fn a_runner_script_run_where_it_is_not_becomes_a_fact() {
    let root = TempDir::new("args-script-cwd");
    let (_, rendered) = stream(
        root.path(),
        (0..6)
            .map(|i| {
                vec![
                    at(edit(i), ROOT),
                    at(sh(format!("python runtests.py app{i}"), CANT_OPEN), ROOT),
                    at(
                        sh(format!("cd tests && python runtests.py app{i}"), OK),
                        ROOT,
                    ),
                ]
            })
            .collect(),
    );
    assert!(rendered.last().unwrap().contains("`python runtests.py …` fails here with a file-not-found error; `(cd tests && python runtests.py …)` works."));
}

#[test]
fn an_option_the_runner_rejects_is_not_a_failing_form() {
    let root = TempDir::new("args-bad-option");
    let bad = "Exit code 2\nusage: runtests.py [-h] [module [module ...]]\nruntests.py: error: unrecognized arguments: test_x";
    let (distiller, _) = stream(
        root.path(),
        (0..6)
            .map(|i| {
                vec![
                    at(edit(i), ROOT),
                    at(
                        sh(format!("python runtests.py app{i} -k test_x"), bad),
                        "/r/tests",
                    ),
                    at(sh(format!("python tests/runtests.py app{i}"), OK), ROOT),
                ]
            })
            .collect(),
    );
    assert!(!distiller.facts.values().any(|f| f.family == "form_repair"));
}

#[test]
fn a_run_whose_output_was_filtered_away_does_not_show_a_form_works() {
    let root = TempDir::new("args-filtered-form");
    let (distiller, _) = stream(
        root.path(),
        (0..6)
            .map(|i| {
                vec![
                    at(edit(i), ROOT),
                    at(
                        sh(
                            "python -m pytest tests/app",
                            "Exit code 1\n/venv/bin/python: No module named pytest",
                        ),
                        ROOT,
                    ),
                    at(
                        sh(
                            format!("python runtests.py app{i} 2>&1 | grep -E '^(OK|FAILED|Ran )'"),
                            "(Bash completed with no output)",
                        ),
                        ROOT,
                    ),
                ]
            })
            .collect(),
    );
    assert!(!distiller.facts.values().any(|f| f.family == "form_repair"));
}

#[test]
fn a_working_form_that_leaves_the_shell_where_it_is_is_the_one_named() {
    let root = TempDir::new("args-preferred-form");
    let mut tasks: Vec<_> = (0..6)
        .map(|i| {
            vec![
                at(edit(i), ROOT),
                at(sh(format!("python runtests.py app{i}"), CANT_OPEN), ROOT),
                at(
                    sh(format!("cd tests && python runtests.py app{i}"), OK),
                    ROOT,
                ),
                at(
                    sh(format!("cd tests && python runtests.py app{i}.tests"), OK),
                    ROOT,
                ),
            ]
        })
        .collect();
    tasks[3].push(at(sh("python tests/runtests.py app3", OK), ROOT));
    let (distiller, rendered) = stream(root.path(), tasks);
    let counts = &distiller.facts["form_repair:run_tests:python runtests.py"].params["b_counts"];
    assert!(
        counts["cd tests && python runtests.py"].as_f64().unwrap()
            > counts["python tests/runtests.py"].as_f64().unwrap()
    );
    assert!(
        rendered
            .last()
            .unwrap()
            .contains("`python tests/runtests.py …` works")
    );
}
