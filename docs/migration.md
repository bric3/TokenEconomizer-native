# Migration record

The deliverable is a Rust implementation of TokenEconomizer's production code:
micro-skills, tool pruning, loop guard, trace readers, pricing, accounting,
persistent state, hook processing, CLI, agent integrations, packaging tools and
experiment runners and adapters. Preserve observable behaviour, state formats,
error recovery and integration contracts. Host SDK entry points may require a
small adapter in the host's language; economizer processing belongs in Rust.
Reference experiment code remains a compatibility consumer until its corresponding
production modules are ported.

## Required sequence

1. Pin upstream, initialise Git and run the original offline test suite.
2. Port production modules, using original Python tests as the main driver.
   Test adapters may translate calls and return values; they must not implement
   the production behaviour being checked. Record which calls execute in Rust.
3. Once production parity is verified, make three separate simplification passes.
4. Make three separate performance passes, each with before/after measurements
   on small, typical, large and pathological inputs and correctness checks.
5. Convert the tests to Rust, preserving their behavioural assertions and fixtures.
6. Audit the whole deliverable, run the native test suite, build the release
   binary and verify CLI and hook workflows.

Do not count a green reference test as proof of native coverage. Keep the
reference tests unmodified until the final conversion. Do not mark the port
complete while any runtime component delegates production behaviour to Python.

## Status

- Git initialised on `main`.
- Upstream archived at the revision recorded in `reference/revision.txt`.
- Python test environment: `.venv`, pytest 9.2.0.
- Rust modules implemented: pricing, ledger, live accounting, repository filenames,
  mechanism configuration, guard decisions and the fact lifecycle's promotion,
  retirement, scoring and firing decisions.
- Envelope calculations and Claude trace calls, usage, prompts, turns, safe
  session IDs and deferred tools also execute in Rust.
- Codex and Copilot calls, usage and metadata; Pi active branches and usage;
  opencode exports and run replies; Cursor hook traces; Antigravity incremental
  reads; and Grok updates and results now execute in Rust. The shared trajectory
  loader, working-directory alignment, tool usage and catalog selection are native.
- Droid records, usage, prompt turns and discovery; Claude history discovery and
  bounded metadata; and Hermes parsing, compression lineage and read-only SQLite
  exports are now native. Further timestamp-format compatibility checks remain
  part of the production-parity audit.
- Loop state, streamed call pairing, shell edit and revert detection, verification
  status, runner summaries and prompt-turn tracking now execute in Rust. Turn tracking
  includes sampled/background judge scheduling, backoff, costs, nudges and outcomes.
  Judge prompts, verdict decoding, redirect limits and runner-cost fallback are also
  native. Envelope loading, saving and locked turn recording are native too.
  Runner processes still need conversion.
- The JSON state store, atomic fact writes, JSONL logs and lock timeouts now execute
  in Rust, including bounded Windows retry decisions and temporary-file cleanup.
- Command observation is native: vocabulary-based intent and form classification,
  working-directory tracking, labels, environment-variable evidence, network and
  VCS outcomes, missing-tool attribution and edited-file selection.
- Environment facts learn, select, merge, render, forget and reload in Rust. This
  includes all six upstream families, contradiction streaks, firing gates, rendering
  caps, lifecycle defaults and the shared forgetting helpers.
- Proposed facts now execute in Rust: transcript digests, authored extractors and
  detectors, proposer prompts and payload validation, evidence grounding, admission,
  contradiction-only revisions, lifecycle updates, forgetting and persistence.
  Model calls remain callbacks until runner conversion.
- The test adapter replaces these functions and lifecycle methods in the reference
  process. It records calls reaching Rust in `validation/results/native-calls.json`.
  Other reference code, including Python child processes, still runs in Python.
- Remaining production conversion, six improvement passes and final test conversion
  pending. No performance claim is made from these compatibility runs.

## Remaining production work

- Audit trace-reader edge cases, including the ISO timestamp formats accepted by
  Python's `datetime.fromisoformat`. Trace consumers still run through the reference
  plugin during migration.
- Audit regular-expression compatibility with Python in the production parity
  pass, including word boundaries, whitespace classes and syntax. The current
  library rejects Python's `(?a)` flag; this remains a required compatibility fix.
- Audit JSON numeric edge cases, including non-finite values accepted by Python.
- Port prefix stages, composition and saved-layer forgetting consumers.
- Port tool-pruning evidence, Codex capabilities and pruning stages.
- Port plugin configuration, paths and repository identity,
  deny lists, host settings, history, context, folding, controller and queues.
- Port side calls, model probes, tool checks, repository maps, CLI commands and
  hooks. Verify each host integration and its launcher and packaging contracts.
- Port experiment production consumers, adapters, runner orchestration and
  supporting tools, preserving their original test contracts.
- Simplification passes: 0 of 3. Performance passes: 0 of 3.
- Final test conversion: pending until the preceding gates pass.

## Verified milestones

- Reference baseline: 1,558 passed, 33 skipped (28.06 seconds).
- First native substitution run: 1,558 passed, 33 skipped (26.52 seconds).
  For example, 523 lifecycle updates and 462 live pricing calls executed in Rust.
  This establishes parity only for the functions substituted by the adapter.
- `cargo build --offline` and `cargo clippy --offline --all-targets -- -D warnings`
  passed on rustc 1.99.1. Cargo's lockfile pins the dependencies used.
- Configuration-focused run: 45 passed. Guard-focused run: 331 passed.
- Claude-focused run: 364 passed. Expanded native substitutions run after the
  reader/validation corrections: 1,558 passed, 33 skipped (28.68 seconds).
- `VerifyReference.java` verified all 256 upstream files against the pinned Git
  tree. It accounts for upstream `.gitattributes` exporting `.cmd` with CRLF.
- Codex/Copilot and trajectory-focused run: 345 passed, 3 skipped.
- Catalog-focused run: 66 passed. Pi/opencode and consumer run: 430 passed,
  2 skipped. Cursor/Antigravity run: 60 passed, 6 skipped. Grok run: 28 passed.
- The full suite with the expanded trace substitutions passes: 1,558 passed,
  33 skipped. `validation/traces-calls.json` records this phase's native calls.
  Counters record bridge entries; nested Rust calls stay inside the Rust process.
- Java probes cover UTF-8 byte offsets across partial Antigravity records and
  vendor-cost rounding against `BigDecimal`'s exact value of the stored double.
- Droid/history-focused run: 76 passed. Hermes-focused run: 67 passed, 6 skipped.
- Full suite with native history and Hermes database access: 1,558 passed,
  33 skipped. `validation/history-calls.json` records bridge calls for this phase.
- The bridge probe checks oversized metadata records, character limits on emoji,
  invalid UTF-8 replacement, the 200-line metadata cap and read-only access to a
  missing Hermes database. The original Hermes tests check export redaction,
  compression lineage, duplicate calls and auxiliary/subagent cost accounting.
- Loop state and shell-focused run: 339 passed. Turn-focused run: 313 passed.
- Full suite with native turn tracking: 1,558 passed, 33 skipped (31.78 seconds).
  `validation/loopguard-calls.json` records bridge calls, including 4,863 turn folds.
  Callback requests and replays count as separate bridge entries; nested Rust state
  updates, shell parsing and rule calls stay inside the process.
- Java probes also check failure fingerprints, distinct null/string call IDs,
  malformed shell-line recovery, sampled judge costs and backoff, live verification
  text and the redirect writer's failure fallback. Background judge tests cover
  job reuse, stale jobs, productive verdicts and one-use redirects.
- Judge-focused run: 417 passed, 2 skipped. Full suite with native judge functions:
  1,558 passed, 33 skipped (33.27 seconds). `validation/judge-calls.json` records
  this phase's coverage. Java probes check clamped and malformed confidence,
  runner errors, cost estimation fallback and Unicode redirect truncation.
- Store-focused run: 317 passed, 1 skipped. Concurrent history, configuration and
  queue run after the bridge lock fix: 66 passed. The original tests inject read
  and replace failures; Rust owns retry decisions and atomic-write cleanup.
- Full suite with native persistence: 1,558 passed, 33 skipped (93.88 seconds).
  `validation/persistence-calls.json` records 20,224 store reads, 8,167 store writes,
  475 atomic fact writes and 325 envelope recordings reaching Rust. These include
  replay entries and exclude nested Rust calls. Worker polling adds transport
  overhead, so this duration is not a measurement of the finished application.
- The Java probe checks sorted ASCII JSON output, atomic cleanup after a failed
  rename, cross-process lock exclusion and concurrent envelope updates. Two native
  bridge processes each record 32 turns without losing updates; history stays
  bounded at 500 turns.
- Observation-focused run: 366 passed. Full suite with native observation:
  1,558 passed, 33 skipped (95.20 seconds). `validation/observe-calls.json` records
  native bridge coverage; observation's nested trace and lifecycle calls stay in Rust.
- Environment-fact-focused run: 103 passed. Full suite with native environment
  facts: 1,558 passed, 33 skipped (109.54 seconds).
  `validation/envfacts-calls.json` records 702 updates, 716 renders, 393 loads and
  400 saves reaching Rust. Updates call the native observer and lifecycle internally.
- The Java probe also checks lifecycle defaults from a minimal record, unknown
  forgotten IDs, an unforget with no saved record, and insertion order when fact
  selection ties at the rendering cap.
- Proposer-focused run: 105 passed. Full suite with native proposed facts:
  1,558 passed, 33 skipped (107.80 seconds). `validation/proposer-calls.json`
  records native calls, including 214 updates, 74 saves and 63 loads. Counts
  include callback replay; nested digest, detector and lifecycle calls stay in Rust.
- `ExtractProposerPrompts.java` verifies both native prompt assets against the
  pinned source constants, including the paper prompt's SHA-256. Clippy, the
  bridge probe and the 256-file upstream integrity check also pass.

## Dependencies

Build with Rust 1.89 or newer and a C compiler for bundled SQLite. Validation uses
Rust 1.99.1. The SQLite wrapper requires the minimum Rust version change; its
unused cache and WebAssembly default features are disabled. Clippy's conditional
syntax fixes after that version change are part of the port, not one of the
three requested simplification passes.

[Chrono](https://docs.rs/chrono/0.4.45/chrono/) supplies calendar parsing and local
timezone handling. [Rusqlite](https://docs.rs/rusqlite/0.43.0/rusqlite/) supplies
SQLite connections opened explicitly with `SQLITE_OPEN_READ_ONLY` and a five-second
busy timeout. The lockfile pins both libraries and their transitive dependencies.
`fancy-regex` 0.20.2 preserves the upstream patterns' lookarounds and backreferences.
`md5` 0.8.1 preserves existing short failure fingerprints.
`indexmap` 2.14.2 was already present through `serde_json`; it is now declared
directly with serialization support for typed fact maps. Discovery order determines
ties in fact selection and rendering.
The state store uses the standard library's
[file locks](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock),
available since Rust 1.89. Native Windows filesystem behaviour remains untested
on this macOS checkout; the original injected-error tests exercise its retries.

## Validation

Run the unmodified reference suite from its own root:

```sh
cd reference/upstream
../../.venv/bin/python -m pytest tests experiments/tests
```

The suite replaces agent side calls and redirects home directories to test
fixtures. Tests requiring native Windows tools may skip on macOS. No paid model
calls, Docker benchmarks or changes to the user's agent settings are required.

From the native checkout root, run the original suite with native substitutions:

```sh
cargo build --locked
PYTHONPATH=validation .venv/bin/python -m pytest -p native_plugin \
  reference/upstream/tests reference/upstream/experiments/tests
```

`validation/native_plugin.py` only transports calls and adapts Python values to
JSON. `te-bridge` is the development test transport; the user-facing CLI and
hooks are still pending. Python is needed for the original test driver during
this phase, and will not be a runtime dependency of the finished port.

The adapter also transports judge callbacks requested by Rust. It records both
assessment requests when a callback is required; those counts are transport
calls rather than distinct turns. Reference child processes do not yet load
the substitutions. The call report therefore documents native coverage without
claiming that the whole suite already exercises a native application.

For injected filesystem operations, the bridge transports the original tests'
read/replace callbacks and their errors. Lock acquisition runs in a bridge worker
so a waiting Python thread leaves the request channel available to the holder.
The timeout and lock decisions stay in Rust.

Small Java validation tools check the migration infrastructure:

```sh
jbang validation/VerifyReference.java
jbang validation/BridgeProbe.java
jbang validation/ExtractProposerPrompts.java
```

The bridge probe covers malformed-request recovery, pricing validation, Unicode
character counts, JSON object filtering, all three text line endings and a
redirect regression where a file is literally named `one file`. It also checks
structured Cursor result text, partial-record offsets and decimal cost rounding. These
probes also cover metadata limits and read-only database access. The upstream
test suite is still the main driver.

IntelliJ has no open project for this checkout. Use Cargo's compiler, formatter
and Clippy until IDE access becomes available.
