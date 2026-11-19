// Converted from reference/upstream/tests/test_distiller_payload_types.py.
pub mod fixtures;
#[path = "support/recording_runner.rs"]
mod recording_runner;
mod support;

use fixtures::write_claude_transcript;
use recording_runner::RecordingRunner;
use serde_json::{Value, json};
use support::TempDir;
use tokeneconomizer::microskills::distiller::{Distiller, ProposeContext, parse_payload};

fn malformed() -> Vec<String> {
    [json!({"extractors":null,"facts":null}),
        json!({"extractors":[{"name":["a"],"pattern":"x"}],"facts":[]}),
        json!({"extractors":[{"name":{"x":1},"pattern":"x"}],"facts":[]}),
        json!({"extractors":true,"facts":[]}),json!({"extractors":[],"facts":7}),
        json!({"extractors":[{"name":"a","pattern":""}],"facts":[{"id":"f","detector":{}}]}),
        json!({"extractors":[],"facts":[{"id":["a"],"detector":{"scope":"ran_tests"}}]}),
        json!({"extractors":[],"facts":[{"id":{"x":1},"detector":{"scope":"ran_tests"}}]}),
        json!({"extractors":[],"facts":[{"id":"f","detector":{"scope":["bad"]}}]}),
        json!({"extractors":[],"facts":[{"id":"f","detector":{"scope":{"a":1}}}]}),
        json!({"extractors":[],"facts":[{"id":"f","detector":{"scope":"ran_tests","fired":5}}]}),
        json!({"extractors":[],"facts":[{"id":"f","detector":{"scope":"ran_tests","confirm":["x"]}}]}),
        json!({"extractors":[],"facts":[{"id":"f","detector":{"scope":"ran_tests","contradict":{"x":1}}}]}),
        json!({"extractors":[],"facts":[{"id":"f","detector":{"scope":"ran_tests"},"cite":3}]})]
        .into_iter().map(|value|value.to_string()).collect()
}

#[test]
fn parse_payload_drops_wrongly_typed_fields() {
    for raw in malformed() {
        let (extractors, facts) = parse_payload(&raw);
        assert!(extractors.is_empty(), "{raw}");
        assert!(facts.is_empty(), "{raw}");
    }
}

#[test]
fn parse_payload_keeps_valid_entries_next_to_malformed() {
    let raw = json!({"extractors":[{"name":["a"],"pattern":"x"},{"name":"ok","pattern":"x"}],
        "facts":[{"id":{"x":1},"detector":{"scope":"ran_tests"}},
        {"id":"good","detector":{"scope":"ran_tests","confirm":"passed"}}]})
    .to_string();
    let (extractors, facts) = parse_payload(&raw);
    assert_eq!(
        extractors
            .iter()
            .map(|value| &value["name"])
            .collect::<Vec<_>>(),
        [&json!("ok")]
    );
    assert_eq!(
        facts.iter().map(|value| &value["id"]).collect::<Vec<_>>(),
        [&json!("good")]
    );
}

#[test]
fn ground_treats_non_list_cite_as_empty() {
    let root = TempDir::new("distiller-cite-type");
    let mut distiller = Distiller::new("acme/widget", "agent", false);
    let transcript = write_claude_transcript(
        &root.path().join("t.jsonl"),
        &[json!(["bash",{"command":"pytest"},"1 passed"])],
    );
    distiller
        .update(
            &transcript,
            "w-0",
            json!(true),
            &mut ProposeContext::default(),
        )
        .unwrap();
    for cite in [
        json!(3),
        json!("w-0"),
        json!({}),
        json!({"w-0":1}),
        json!([["w-0"],{"x":1},7]),
    ] {
        let spec =
            json!({"id":"f","detector":{"scope":"ran_tests","confirm":"passed"},"cite":cite});
        assert_eq!(distiller.ground(&spec).unwrap(), 0, "{spec}");
    }
}

#[test]
fn update_survives_malformed_proposer_reply() {
    for raw in malformed() {
        let root = TempDir::new("distiller-malformed-reply");
        let mut distiller = Distiller::new("acme/widget", "agent", false);
        distiller.last_propose = -1;
        let mut runner = RecordingRunner::new(&raw, 1.0);
        let mut context = ProposeContext {
            runner: Some(&mut runner),
            propose_every: 1,
            ..ProposeContext::default()
        };
        for i in 0..3 {
            let transcript = write_claude_transcript(
                &root.path().join(format!("t{i}.jsonl")),
                &[json!(["bash",{"command":"pytest"},"1 passed"])],
            );
            distiller
                .update(&transcript, &format!("w-{i}"), json!(true), &mut context)
                .unwrap();
        }
        assert!(distiller.facts.is_empty(), "{raw}");
        assert!(distiller.extractors.is_empty(), "{raw}");
    }
}

fn valid_fact() -> Value {
    json!({"id":"use-pytest","claim":"pytest is canonical","injection":"run pytest",
        "detector":{"scope":"ran_tests","confirm":"passed","fired":"ran_tests"},"cite":["w-0","w-1"]})
}

#[test]
fn valid_fact_admitted_alongside_malformed_ones() {
    let root = TempDir::new("distiller-valid-with-malformed");
    let raw = json!({"extractors":[{"name":["bad"],"pattern":"x"}],
        "facts":[{"id":["bad"],"detector":{"scope":"ran_tests"}},valid_fact()]})
    .to_string();
    let mut distiller = Distiller::new("acme/widget", "agent", false);
    distiller.last_propose = -1;
    let mut runner = RecordingRunner::new(&raw, 1.0);
    let mut context = ProposeContext {
        runner: Some(&mut runner),
        propose_every: 1,
        ..ProposeContext::default()
    };
    for i in 0..2 {
        let transcript = write_claude_transcript(
            &root.path().join(format!("t{i}.jsonl")),
            &[json!(["bash",{"command":"pytest"},"1 passed"])],
        );
        distiller
            .update(&transcript, &format!("w-{i}"), json!(true), &mut context)
            .unwrap();
    }
    assert!(distiller.facts.contains_key("use-pytest"));
    assert_eq!(
        distiller
            .facts
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["use-pytest"]
    );
}

#[test]
fn empty_detectors_do_not_crowd_out_a_valid_fact() {
    let root = TempDir::new("distiller-empty-detectors");
    let mut facts: Vec<_> = (0..4)
        .map(|i| json!({"id":format!("empty-{i}"),"detector":{}}))
        .collect();
    facts.push(valid_fact());
    let raw = json!({"extractors":[],"facts":facts}).to_string();
    let mut distiller = Distiller::new("acme/widget", "agent", false);
    distiller.last_propose = -1;
    let mut runner = RecordingRunner::new(&raw, 1.0);
    let mut context = ProposeContext {
        runner: Some(&mut runner),
        propose_every: 1,
        ..ProposeContext::default()
    };
    for i in 0..2 {
        let transcript = write_claude_transcript(
            &root.path().join(format!("t{i}.jsonl")),
            &[json!(["bash",{"command":"pytest"},"1 passed"])],
        );
        distiller
            .update(&transcript, &format!("w-{i}"), json!(true), &mut context)
            .unwrap();
    }
    assert_eq!(
        distiller
            .facts
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["use-pytest"]
    );
}
