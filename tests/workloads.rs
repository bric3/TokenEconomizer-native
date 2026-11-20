// Converted from reference/upstream/experiments/tests/test_workloads.py.
#[path = "support/provider_fixture.rs"]
pub mod provider_fixture;
mod support;

use provider_fixture::{Constructor, Provider};
use serde_json::{Value, json};
use std::{
    collections::{BTreeSet, VecDeque},
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use support::TempDir;
use tokeneconomizer::experiments::{
    datasets,
    swebench::{self, setup},
    workloads,
};

fn set(values: impl IntoIterator<Item = String>) -> BTreeSet<String> {
    values.into_iter().collect()
}
fn constant_set(name: &str) -> BTreeSet<String> {
    serde_json::from_value(workloads::constants()[name].clone()).unwrap()
}
fn added() -> workloads::Groups {
    serde_json::from_value(workloads::constants()["SWEBENCH_ADDED"].clone()).unwrap()
}
fn exported(domain: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("reference/upstream/experiments/workloads")
        .join(format!("{domain}.json"))
}
fn document(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
fn web(name: &str, env: Value) -> Value {
    Provider::new("webbench", env).run(name).unwrap()
}

#[test]
fn swebench_counts_are_the_evaluated_set() {
    assert_eq!(
        json!(workloads::task_counts("swebench").unwrap()),
        json!({"django/django":40,"matplotlib/matplotlib":34,"scikit-learn/scikit-learn":32,"sphinx-doc/sphinx":44,"sympy/sympy":40,"pydata/xarray":39})
    );
}

#[test]
fn webbench_2026_08_export_is_frozen() {
    assert_eq!(
        workloads::constants()["WEBBENCH_TASKS_PUBLISHED_2026_08"],
        json!({"flex":21,"expressjs":20,"react":20,"prisma":20,"calculator":21})
    );
}

#[test]
fn webbench_sweep_set_is_five_distinct_streams() {
    let counts = workloads::task_counts("webbench").unwrap();
    assert_eq!(
        set(counts.keys().cloned()),
        set(["calculator", "expressjs", "react", "zustand", "table"].map(String::from))
    );
    assert_eq!(counts.values().sum::<usize>(), 102);
    assert_eq!(
        set(counts
            .iter()
            .filter(|(_, n)| **n == 21)
            .map(|(p, _)| p.clone())),
        set(["calculator", "table"].map(String::from))
    );
    assert!(counts.values().all(|n| matches!(n, 20 | 21)));
}

#[test]
fn totals() {
    for (domain, total) in [
        ("swebench", 229),
        ("webbench", 102),
        ("creddata", 100),
        ("reposvul", 100),
    ] {
        assert_eq!(workloads::total_tasks(domain).unwrap(), total);
    }
}

#[test]
fn no_duplicate_ids_within_a_group() {
    for domain in ["swebench", "webbench", "creddata", "reposvul"] {
        for group in workloads::groups(domain).unwrap().keys() {
            let ids = workloads::instances(domain, Some(group)).unwrap();
            assert_eq!(ids.len(), set(ids.clone()).len(), "{domain}/{group}");
        }
    }
}

#[test]
fn full_split_only_is_a_subset_and_all_xarray() {
    let full = constant_set("SWEBENCH_FULL_SPLIT_ONLY");
    assert!(full.is_subset(&set(workloads::instances("swebench", None).unwrap())));
    assert_eq!(full.len(), 30);
    assert!(full.iter().all(|id| id.starts_with("pydata__xarray-")));
}

#[test]
fn added_extends_sklearn_and_sympy_to_sixty_runnable() {
    let runnable = workloads::runnable_instances("swebench").unwrap();
    for group in ["scikit-learn/scikit-learn", "sympy/sympy"] {
        assert_eq!(runnable[group].len(), 60);
    }
    for group in [
        "django/django",
        "matplotlib/matplotlib",
        "sphinx-doc/sphinx",
        "pydata/xarray",
    ] {
        assert_eq!(
            runnable[group].len(),
            workloads::instances("swebench", Some(group)).unwrap().len()
        );
    }
}

#[test]
fn added_ids_are_new_and_grouped_under_their_own_repo() {
    let canonical = set(workloads::instances("swebench", None).unwrap());
    let groups = workloads::groups("swebench").unwrap();
    for (group, ids) in added() {
        assert!(groups.contains_key(&group));
        assert!(set(ids.clone()).is_disjoint(&canonical));
        assert_eq!(ids.len(), set(ids).len());
    }
}

#[test]
fn exclusion_still_wins_over_the_extension() {
    let id = "scikit-learn__scikit-learn-14710".to_owned();
    assert!(
        !workloads::runnable_instances("swebench").unwrap()["scikit-learn/scikit-learn"]
            .contains(&id)
    );
    assert!(
        workloads::instances("swebench", Some("scikit-learn/scikit-learn"))
            .unwrap()
            .contains(&id)
    );
}

#[test]
fn added_does_not_move_the_published_record() {
    assert_eq!(workloads::total_tasks("swebench").unwrap(), 229);
    let counts = workloads::task_counts("swebench").unwrap();
    assert_eq!(counts["scikit-learn/scikit-learn"], 32);
    assert_eq!(counts["sympy/sympy"], 40);
}

#[test]
fn added_ids_need_the_full_split() {
    let ids = set(added().into_values().flatten());
    assert!(ids.is_disjoint(&constant_set("SWEBENCH_FULL_SPLIT_ONLY")));
}

#[test]
fn every_added_instance_yields_a_tag() {
    for id in added().into_values().flatten() {
        assert!(workloads::swebench_image_tag(&id).ends_with(":latest"));
    }
}

#[test]
fn copilot_extra_is_not_in_the_canonical_set() {
    assert!(
        constant_set("SWEBENCH_COPILOT_EXTRA")
            .is_disjoint(&set(workloads::instances("swebench", None).unwrap()))
    );
}

#[test]
fn image_tag_uses_the_1776_separator() {
    let tag = workloads::swebench_image_tag("django__django-11532");
    assert_eq!(
        tag,
        "swebench/sweb.eval.x86_64.django_1776_django-11532:latest"
    );
    assert!(!tag.split_once("x86_64.").unwrap().1.contains("__"));
}

#[test]
fn every_swebench_instance_yields_a_tag() {
    for id in workloads::instances("swebench", None).unwrap() {
        assert!(workloads::swebench_image_tag(&id).ends_with(":latest"));
    }
}

#[test]
fn instances_rejects_unknown_domain_and_group() {
    for (domain, group) in [("nope", None), ("swebench", Some("pydata/xarray-typo"))] {
        assert_eq!(
            workloads::instances(domain, group).unwrap_err().kind(),
            "KeyError"
        );
    }
}

#[test]
fn filter_tasks_keeps_order_and_drops_strangers() {
    let ids = workloads::instances("swebench", Some("pydata/xarray")).unwrap();
    let tasks: Vec<_> = std::iter::once(json!({"instance_id":"not-a-real-instance"}))
        .chain(ids.iter().map(|id| json!({"instance_id":id})))
        .collect();
    let indices =
        workloads::filter_task_indices(&tasks, "swebench", Some("pydata/xarray")).unwrap();
    let kept: Vec<_> = indices
        .iter()
        .map(|&i| tasks[i]["instance_id"].as_str().unwrap())
        .collect();
    assert_eq!(kept, ids);
}

#[test]
fn filter_tasks_returns_empty_rather_than_falling_back() {
    assert!(
        workloads::filter_task_indices(
            &[json!({"instance_id":"x"})],
            "swebench",
            Some("sympy/sympy")
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn exported_json_exists_for_every_domain() {
    for domain in workloads::domains() {
        assert!(exported(&domain).is_file());
    }
}

#[test]
fn exported_json_matches_the_module() {
    for domain in workloads::domains() {
        assert_eq!(
            document(&exported(&domain)),
            workloads::manifest(&domain).unwrap(),
            "{domain}"
        );
    }
}

#[test]
fn exported_json_is_small_enough_to_commit() {
    for domain in workloads::domains() {
        assert!(fs::metadata(exported(&domain)).unwrap().len() < 200_000);
    }
}

#[test]
fn exported_json_group_counts_agree_with_task_counts() {
    for domain in workloads::domains() {
        let doc = document(&exported(&domain));
        let counts: serde_json::Map<_, _> = doc["groups"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(g, ids)| (g.clone(), json!(ids.as_array().unwrap().len())))
            .collect();
        assert_eq!(doc["task_counts"], json!(counts));
        assert_eq!(
            doc["total_tasks"].as_u64().unwrap(),
            counts.values().map(|n| n.as_u64().unwrap()).sum::<u64>()
        );
    }
}

#[test]
fn webbench_streams_are_contiguous() {
    for (project, ids) in workloads::groups("webbench").unwrap() {
        let numbered = if ids[0] == "init" {
            &ids[1..]
        } else {
            &ids[..]
        };
        assert_eq!(
            numbered,
            (1..=numbered.len())
                .map(|n| format!("task-{n}"))
                .collect::<Vec<_>>(),
            "{project}"
        );
    }
}

fn projects(root: &Path) -> Vec<Value> {
    let mut expected = Vec::new();
    for (project, ids) in workloads::groups("webbench").unwrap() {
        let directory = root.join(&project);
        fs::create_dir(&directory).unwrap();
        let mut file = fs::File::create(directory.join("tasks.jsonl")).unwrap();
        for id in ids.iter().rev() {
            writeln!(
                file,
                "{}",
                json!({"id":id,"description":format!("{project}:{id}")})
            )
            .unwrap();
        }
        expected.extend(ids.into_iter().map(|id| json!({"instance_id":id,"problem_statement":format!("{project}:{id}"),"repo":project})));
    }
    expected
}
fn external_projects() -> Option<PathBuf> {
    std::env::var_os("TOKENECONOMIZER_WEBBENCH_PROJECTS")
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
}

#[test]
fn every_swept_webbench_project_has_a_tasks_jsonl() {
    if let Some(root) = external_projects() {
        datasets::webbench::build(&root, Path::new(".")).unwrap();
    }
    let root = TempDir::new("workload-projects");
    let expected = projects(root.path());
    assert_eq!(
        datasets::webbench::build(root.path(), root.path()).unwrap(),
        expected
    );
    let path = root.path().join("calculator/tasks.jsonl");
    fs::remove_file(&path).unwrap();
    assert!(
        datasets::webbench::build(root.path(), root.path())
            .unwrap_err()
            .message
            .contains("no tasks.jsonl")
    );
    fs::write(&path, "").unwrap();
    assert!(
        datasets::webbench::build(root.path(), root.path())
            .unwrap_err()
            .message
            .contains("missing")
    );
}

#[test]
fn driver_task_list_is_not_stale() {
    if let Some(root) = external_projects()
        && let Some(out) = std::env::var_os("TOKENECONOMIZER_WEBBENCH_TASKS")
            .map(PathBuf::from)
            .filter(|path| path.is_file())
    {
        assert_eq!(
            document(&out),
            json!(datasets::webbench::build(&root, Path::new(".")).unwrap())
        );
    }
    let root = TempDir::new("workload-driver");
    let expected = projects(root.path());
    let path = root.path().join("driver.json");
    let mut args = vec![
        "--projects-root".into(),
        root.path().to_string_lossy().into_owned(),
        "--out".into(),
        path.to_string_lossy().into_owned(),
    ];
    datasets::webbench::command("export", &args, &Default::default(), root.path()).unwrap();
    assert_eq!(document(&path), json!(expected));
    args.push("--check".into());
    datasets::webbench::command("export", &args, &Default::default(), root.path()).unwrap();
    fs::write(&path, "[]\n").unwrap();
    assert!(
        datasets::webbench::command("export", &args, &Default::default(), root.path())
            .err()
            .expect("stale driver accepted")
            .message
            .contains("STALE")
    );
}

#[test]
fn webbench_task_timeout_is_clamped_under_the_evaluator_abort() {
    let abort = web("webbench_undici_abort", json!({})).as_i64().unwrap();
    let cap = abort
        - setup::constants()["web"]["WEBBENCH_TASK_TIMEOUT_HEADROOM_S"]
            .as_i64()
            .unwrap();
    assert!(cap < abort);
    for shared in ["3600", "1800", "301"] {
        assert_eq!(
            web("webbench_task_timeout", json!({"TE_TASK_TIMEOUT":shared})),
            cap
        );
    }
}

#[test]
fn webbench_task_timeout_defaults_and_overrides() {
    assert_eq!(
        web("webbench_task_timeout", json!({})),
        setup::constants()["web"]["WEBBENCH_TASK_TIMEOUT_DEFAULT"]
    );
    assert_eq!(
        web(
            "webbench_task_timeout",
            json!({"TE_TASK_TIMEOUT":"3600","TE_WEBBENCH_TASK_TIMEOUT":"200"})
        ),
        200
    );
    assert_eq!(
        web("webbench_task_timeout", json!({"TE_TASK_TIMEOUT":"abc"})),
        setup::constants()["web"]["WEBBENCH_TASK_TIMEOUT_DEFAULT"]
    );
}

#[test]
fn swebench_task_timeout_is_untouched_by_the_webbench_clamp() {
    let mut runner =
        Constructor::new(json!({"ANTHROPIC_API_KEY":"sk-ant-x","TE_TASK_TIMEOUT":"3600"}));
    setup::construct("claude", &mut runner).unwrap();
    assert_eq!(runner.fields["task_timeout"], 3600);
    assert_eq!(
        web("webbench_task_timeout", json!({"TE_TASK_TIMEOUT":"3600"})),
        260
    );
}

#[test]
fn webbench_undici_abort_defaults_to_the_stock_ceiling_and_adds_no_env() {
    assert_eq!(
        web("webbench_undici_abort", json!({})),
        setup::constants()["web"]["WEBBENCH_UNDICI_ABORT_DEFAULT_S"]
    );
    assert_eq!(web("webbench_undici_env", json!({})), json!({}));
    assert_eq!(
        web(
            "webbench_undici_env",
            json!({"TE_WEBBENCH_UNDICI_ABORT":"300"})
        ),
        json!({})
    );
}

#[test]
fn raising_the_abort_ceiling_raises_the_cap_and_preloads_the_dispatcher() {
    let env = json!({"TE_WEBBENCH_UNDICI_ABORT":"900","TE_TASK_TIMEOUT":"3600"});
    assert_eq!(web("webbench_undici_abort", env.clone()), 900);
    assert_eq!(
        web("webbench_task_timeout", env.clone()),
        900 - setup::constants()["web"]["WEBBENCH_TASK_TIMEOUT_HEADROOM_S"]
            .as_i64()
            .unwrap()
    );
    let extra = Provider::new("webbench", env)
        .run_with("webbench_undici_env", json!({"repo":"/tmp/wb"}))
        .unwrap();
    assert_eq!(extra["TE_WEBBENCH_UNDICI_ABORT_MS"], "900000");
    let options = extra["NODE_OPTIONS"].as_str().unwrap();
    assert!(options.contains("--import") && options.contains("webbench_undici_preload"));
}

#[test]
fn raising_the_ceiling_preserves_an_existing_node_options() {
    let extra = web(
        "webbench_undici_env",
        json!({"TE_WEBBENCH_UNDICI_ABORT":"600","NODE_OPTIONS":"--max-old-space-size=4096"}),
    );
    let options = extra["NODE_OPTIONS"].as_str().unwrap();
    assert!(options.starts_with("--max-old-space-size=4096 "));
    assert!(options.contains("--import"));
}

#[test]
fn the_swebench_export_writes_the_stream_in_order_and_prefers_verified() {
    struct Rows {
        splits: VecDeque<VecDeque<Value>>,
        current: VecDeque<Value>,
    }
    impl swebench::Runtime for Rows {
        fn call(&mut self, method: &str, _: Value) -> swebench::Result<Value> {
            Ok(match method {
                "rows_begin" => Value::Null,
                "next_split" => {
                    if let Some(rows) = self.splits.pop_front() {
                        self.current = rows;
                        json!(true)
                    } else {
                        json!(false)
                    }
                }
                "next_row" => match self.current.pop_front() {
                    Some(row) => json!({"done":false,"row":row}),
                    None => json!({"done":true}),
                },
                _ => panic!("unexpected dataset callback {method}"),
            })
        }
    }
    let order: Vec<_> = workloads::runnable_instances("swebench")
        .unwrap()
        .into_values()
        .flatten()
        .collect();
    let row = |id: &str, tag: &str| {
        let mut row: serde_json::Map<_, _> = datasets::GRADING_FIELDS
            .iter()
            .map(|key| ((*key).into(), json!(format!("{tag}:{key}"))))
            .collect();
        row.extend(json!({"instance_id":id,"repo":format!("{tag}:repo"),"problem_statement":format!("{tag}:issue")}).as_object().unwrap().clone());
        json!(row)
    };
    let verified = order
        .iter()
        .step_by(2)
        .rev()
        .map(|id| row(id, "verified"))
        .collect();
    let full = order
        .iter()
        .map(|id| row(id, "full"))
        .chain(std::iter::once(row("not__evaluated-1", "full")))
        .collect();
    let (tasks, grading) = datasets::build(
        &mut Rows {
            splits: VecDeque::from([verified, full]),
            current: VecDeque::new(),
        },
        &set(order.clone()),
    )
    .unwrap();
    assert_eq!(
        tasks
            .iter()
            .map(|t| t["instance_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        order
    );
    assert_eq!(grading.keys().cloned().collect::<Vec<_>>(), order);
    assert_eq!(tasks[0]["problem_statement"], "verified:issue");
    assert_eq!(tasks[1]["problem_statement"], "full:issue");
    assert_eq!(
        set(grading[&order[0]].as_object().unwrap().keys().cloned()),
        set(datasets::GRADING_FIELDS.iter().map(|key| (*key).into()))
    );
}
