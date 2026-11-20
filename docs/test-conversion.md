# Native test conversion

The pinned original pytest collection contains 1,591 cases across 67 modules.
The original files remain unchanged under `reference/upstream`; converted tests
call the Rust library directly. Parameterised cases use Rust loops with the
original values. Test function names retain the original names without `test_`.

The first batch converts all assertions and parameter values in these modules:

| Original module                    | Rust tests | Original cases |
| ---------------------------------- | ---------: | -------------: |
| test_cache_write_1h.py             |         17 |             33 |
| test_config.py                     |          5 |              5 |
| test_config_integer_thresholds.py  |          7 |             50 |
| test_contribution_finite_config.py |          1 |              4 |
| test_ledger.py                     |          5 |              5 |
| test_load_calls_malformed.py       |          7 |             19 |
| test_observe_label_failed.py       |          1 |              1 |
| test_pricing_new_models.py         |          4 |             24 |
| test_repo_key_env.py               |          2 |              2 |
| test_store_read_lines.py           |          3 |              3 |
| test_windows_identity.py           |          3 |              3 |
| Total                              |         55 |            149 |

Each module is in `tests/` with the same basename and `.rs`, omitting `test_`.
The source path is recorded at the top of each converted file.
Large modules may be split across Rust files; their annotations retain the same
original source path so the inventory combines their test names.
The first batch's 55 converted tests run alongside 45 existing native regression
tests.

The configuration tests retain integer-type checks and verify that rejected
updates preserve saved bytes. The cache fixtures retain duplicate Claude message
blocks, Pi parent links and usage splits. Pricing comparisons retain pytest's
relative and absolute tolerances. Windows-style label tests create actual files
and exercise all original state writers; they run on macOS and do not establish
native Windows execution. The label-failure test calls Rust directly; Python's
deprecation-warning filter is unnecessary for that implementation.

`tests/support/mod.rs` supplies isolated temporary directories for filesystem
fixtures. It removes successful fixtures and retains failed fixtures with their
path in the test output. No Python runtime or transport is used by these tests.

The second batch adds the trace seam, atomic-store retries and vendor costs:

| Original module                       | Rust tests | Original cases |
| ------------------------------------- | ---------: | -------------: |
| test_trace.py                         |         26 |             26 |
| test_store_windows_replace.py         |          8 |             10 |
| experiments/tests/test_vendor_cost.py |         13 |             13 |
| Batch total                           |         47 |             49 |

The trace fixtures retain request-blob duplicates, all catalog sources, prompt
types, Pi branch links and non-object JSON lines. The six attempt-result tests
now inspect the real aggregator's single-row counts; the original tests checked
a copied calculation helper. Store retry tests inject permission failures into
the same reader and atomic writer that `Store` calls. They check retry counts,
propagated errors, retained state and temporary-file cleanup. The real held-file
test uses Windows reader-sharing flags and a delayed close. That test is
conditional on Windows and has not been compiled or run on macOS.

The third batch adds adapters, index protection and the learned-fact lifecycle:

| Original module                             | Rust tests | Original cases |
| ------------------------------------------- | ---------: | -------------: |
| experiments/tests/test_adapters.py          |          7 |              7 |
| experiments/tests/test_aggregate_index.py   |          5 |              5 |
| experiments/tests/test_envfacts_profiles.py |          7 |              7 |
| experiments/tests/test_facade.py            |          3 |              3 |
| test_injection.py                           |          4 |              4 |
| test_distiller_payload_types.py             |          6 |             32 |
| Batch total                                 |         32 |             58 |

Adapter protocol conformance is checked against Rust's `DomainAdapter` trait;
non-null return contracts use Rust's concrete return types. Index tests write
actual row trees and check both CSVs, including preserving the canonical index
after a refused subset rewrite. The CSV reader accepts only these fixtures'
unquoted fields and asserts that restriction before reading them.

The distiller and facade tests keep proposer payloads and token charges in a
recording runner. No model process is started. The composed profile keeps the
six-task learning sequence and the additional telemetry task. The Codex facade
keeps the full ten-task sequence that reaches its paid proposer cadence. Cadence
is supplied through `ProposeContext`, keeping each test's configuration local.
The injection-profile constants are checked through the native arm registry,
which owns those configurations. Shared Claude and Codex transcript writers
retain the original fixtures' call IDs, optional working directories, item
defaults and event wrappers.

The fourth batch starts the live-core module. `tests/live_core_shell.rs` converts
32 of its 99 functions, preserving 207 of its 274 original cases. It covers
shell edits and reverts, wrapper options, here-strings, heredocs, multi-file
patches, runner detection, hidden exit status, ecosystem summaries, failure
signatures and the judge's verification digest. Commands are data passed to
the native parsers and guard; the tests execute none of those shell commands.

All 18 parameter tables are copied into `tests/fixtures/live_core_shell.json`.
`ExtractLiveCoreCases.java --write` regenerates that fixture; its default mode
checks the file against the pinned source. Java selects the tables and records
the source SHA-256. Python's standard-library AST decoder reads literal data
without executing the test module. Native test runs read the checked JSON and
require no Python interpreter.

The fifth batch adds 32 live-core functions in `live_core_data.rs`,
`live_core_hosts.rs` and `live_core_sessions.rs`. It preserves repository and
worktree metadata, configuration layers, nested locks, duplicate usage entries,
subagents, synthetic notices, Codex rollout wrappers and capability evidence.
Host tests retain the captured payload shapes, context placement, notice
merging, mode detection and streaming replies. Session tests preserve Pi's
active and abandoned branches and opencode's nested, list and flat exports,
including synthetic-text filtering and token/cost accounting. Pi's optional
header fields stay unchanged when its test rewrites a tool command.

The sixth batch adds 14 functions in `live_core_control.rs` and
`live_core_copilot.rs`. They preserve judge scheduling, delayed verdicts, timeout
recovery, the nudge backstop, probe command counts and Pi reply accounting.
Copilot tests cover event logs, working directories, cache accounting, MCP
prefixes, hook progress lines and provider model selection. Claude's persistence
flag is checked through its help lookup. Process results and judge jobs remain
local fixtures; the native parsers, guard and hook entry point process them.

The seventh batch adds nine functions in `live_core_process.rs`. It retains npm's
script, executable and older shim formats, missing targets and unsupported batch
files. It checks Codex rollout metadata and Copilot ancestor arguments, opencode
export retries and preserved copies, and malformed hook records. Inspection
uses the real session-settling path. The plain-terminal test invokes the native
`te` binary and checks that its status names the recent Codex host. Windows
launch decisions use fixtures on macOS; native Windows execution remains open.

The eighth batch completes live-core with 12 functions in
`live_core_sidecalls.rs` and `live_core_probe.rs`. Side-call tests retain model
and effort selection, Pi extensions and older-CLI fallback, MCP isolation,
Copilot log pricing, opencode session deletion, wrapper launch arguments,
cleanup policy and non-object JSON lines. Rust's host-call plan checks replace
Python wrapper introspection. The shared opencode event fixture also drives
the existing reply-parser test.

The probe test runs the generated shell wrappers and Python unittest workload
and checks that its temporary repository is committed cleanly. This Unix-only
test requires Python 3, Git and the configured signing key; it creates a signed
fixture commit. Other converted tests call Rust or its native CLI without a
Python runtime. All 99 live-core functions and 274 original cases are converted.

The ninth batch converts `test_loopguard_productive_turns.py` and
`test_claude_verbose_reply.py`: 16 functions cover 42 original cases. The guard
tests retain all 16 exit-status examples and the passing Playwright/libuv,
source-read, reverted-file, dotenv and successful-edit regressions. Commands
remain parser data. Verbose reply tests cover the final result event, missing
and error results, measurement, proposer failures and recovery, paused hosts and
one-line error logs that exclude the prompt. The nullable parser input goes
through the native bridge, which owns that boundary; other parser inputs call
Rust directly. Runner error strings and typed side-call failures replace the
corresponding Python exceptions.

The tenth batch converts `test_fleet_budget.py` and `test_envfacts_args.py`:
33 functions cover 39 original cases. Fleet tests use an isolated cgroup tree
and real file permissions. The Codex launcher test runs an empty cell list and
checks the shared CPU quota and per-container limit in its output, replacing
the original source-text checks. No Docker process or task workload runs.
These fleet tests are Unix-only, matching their cgroup and UID fixtures.

Argument facts retain all label-rewrite examples, six-task learning streams,
contradictions, filtered output, requested environment values and working
directories. Repository fixtures cover a `.git` directory and a worktree file.
The tests call the native observer and distiller; their command strings remain
data. They preserve the preference for a working form that keeps the shell's
directory unchanged even when another form has more observations.

The eleventh batch converts all 22 functions in `test_envfacts.py`. It preserves
promotion and retirement, prevented waste, empty history probes, refused hosts,
fact and token caps, merged claims, assignment-dependent forms, per-session
evidence counts and loss of a working form. Persistence and Claude/Codex schema
tests retain their fixtures. Prefix-stage wiring checks zero optimiser cost;
the classifier checks every original RTK prefix and bare command. Transcript
commands and URLs remain data and trigger no model or network calls.

The twelfth batch converts all 24 functions and 37 cases in
`test_generic_microskills.py`. It retains build and checker repairs, absent-tool
retirement, launcher identity, edited files across languages, relative digests,
vocabulary selection, saved proposer windows and existing claims. Proposer tests
use the recording runner and the original cadence. Forgotten-fact tests retain
restatement checks, evidence windows, candidate limits and legacy saved entries.
The output extractor reads every saved output line. Python vocabulary identity
checks compare the complete native vocabulary values.

The thirteenth batch converts `test_winspawn.py` and `test_live_safety.py`:
13 functions cover 15 original cases. Spawn tests retain hidden-console flags,
WMI fallback, suffixed interpreters and the windowless runner command. Runtime
fixtures record launch decisions without starting Windows processes.
Safety tests retain symlink refusal, unchanged targets and malformed settings,
external Git metadata and private state. The permission test also covers the
native hook entry point; each child starts with umask 022. Both native entry
points now set umask 077, matching the original bootstrap. Before this fix,
the native CLI created `home/repos` with group and other read/execute bits.
Permission-bit checks remain Unix-only. Native Windows execution stays open.

The fourteenth batch converts `test_local_model.py`: 22 functions cover 29
original cases. It preserves settings, platform and availability routing,
schema-based local verdicts, host fallback, zero charge, greedy arguments,
stdin prompts, ANSI stripping and failure logs. Judge jobs use the real store
and routed runner with supplied model replies. Tests retain the local
confidence floor, local nudge and host-spend cases. The paused-host hook checks
its judge mode without probing availability or starting a process. No on-device
model is invoked.

The fifteenth batch converts `test_status_explain.py`: 22 functions cover 34
original cases. It retains opt-in text, unchanged JSON fields, lifecycle and
proposal states, warmup, paused calls, shadow mode and private-error filtering.
The status fixture supplies the selected context and captures output; its
runtime delegates only the saved-state operations that status uses. Byte
snapshots check that explanations change no saved files. The interactive Claude
fixture drives real native hooks through four learning sessions. A native CLI
test selects a repository containing spaces and quotes from another directory
and preserves its pending proposal.

The sixteenth batch converts `test_toolcheck_measure.py`: seven functions cover
11 original cases. It retains the 400-tool settings file, short argv, rejected
empty and failed measurements, and Codex's empty-render checks. Native pipe
output has a concrete `String`; the original missing-stdout case uses its empty
boundary value. A small Rust child fixture flushes optional JSON lines and then
waits five seconds, replacing the original Python child scripts. The real probe
reads its pipes and terminates it at init or deadline; the original four-second
limits remain. The validation child is excluded from packaged executables.

The seventeenth batch converts all 19 functions in `test_droid.py`. It retains
session headers and timestamps, hidden prompts, tool mapping, adjacent settings
usage, fallback pricing, host detection and explicit side-call opt-in. The side
call fixture checks its read-tool restriction and private working directory.
Real native history imports skip subagents, recent and other-repository logs,
then learn facts and price five sessions. The native session-start hook returns
the learned fact. Packaging checks the native Droid bundle; the original Python
library-presence assertion checks the native hook executable instead.

The eighteenth batch converts `test_cursor.py` and `test_cursor_recording.py`:
22 functions cover 32 original cases. It retains captured payloads, hook
declarations, trace reconstruction, stop statuses, headless turn closure,
transcript fallback and newline-containing tool IDs. The hook/controller fixture
records fold requests and leaves background maintenance idle so the tests can
inspect the captured spool; recording, parsing, turn state and envelope updates
use native code. Separate recording tests run the actual native hook process
and check enablement layers, side-call suppression, email removal and input and
output caps. The settle test removes only the owned spool.

The nineteenth batch converts 16 functions and 32 cases in
`experiments/tests/test_answer_key_guard.py`. It retains every abusive and
legitimate command, hosts entries, native hook exit codes, the lift switch,
trace counts and idempotent hook merging. The Codex installer uses recorded
container calls and a validated ELF fixture; it checks native hook registration
and fresh and resumed trust flags without running Docker. A real native hook
installer checks the matcher. Container removal injects a timeout and verifies
that the live set is cleared. The Python SDK's shared-export identity assertion
remains pending; the other original guard cases also pass through the adapter.

The twentieth batch completes `experiments/tests/test_grading_integrity.py`:
17 functions cover all 17 cases. It retains container naming, ungraded
denominators, every declared overhead key and project-environment selection.
The source-inspection stall test now injects both timeout and runtime failures
into patch extraction and grading, verifies cleanup and retained usage, and runs
the next task. Both implementations overwrite `is_error` and `api_error_status`
when merging segment fields after the grading failure. The test records that
existing behaviour: `grading_error` survives and excludes the row from the
graded denominator. No production behaviour changes in this conversion.

The twenty-first batch converts 14 accounting functions in
`experiments/tests/test_usage_recovery.py`. It retains cumulative-message
deduplication, missing-output flags, session-log duplicates, exact recovery,
in-flight turns and degradation across both segment orders. The two Python
guard-signature and Web-Bench construction cases remain pending. All 16 original
cases still pass through the native adapter. The full native suite through the
twentieth batch passes with GPG-agent access for its signed Git fixture.

The twenty-second batch completes `experiments/tests/test_spiral.py`: all 17
functions and cases. It retains shipped and defensive policies, shell-patch
events, trajectory edits, repeated-green checks, runaway thresholds, distinct
nudge reasons, arm configuration and envelope units. Native field access checks
the guard counters; the Codex SDK's `segment` property reads `nudges_done`.
The envelope test now checks the native controller's telemetry row instead of
inspecting its source. A shared argv fixture supplies SDK object operations to
the production invocation builder. The installed Codex CLI's resume help accepts
every generated flag; without that CLI the test checks argv and reports the
missing external check.

Run the native suite and the conversion inventory from the checkout root:

```sh
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
jbang validation/TestConversionInventory.java
jbang validation/ExtractLiveCoreCases.java
```

The inventory collects the pinned original test IDs and matches their function
names to the Rust files' source annotations. It reports remaining modules and
functions. Name matching does not verify assertions, parameter values, fixture
behaviour or execution; those require source review and native test runs.
The inventory's Python dependency is limited to collecting the archived suite.

Conversion remains in progress: 37 pytest modules are complete, with 890 of
1,591 original cases mapped, including the conditional Windows case. The other 701
cases and standalone executable smoke tests
still need native equivalents. The broader parity and platform audits in
[the migration record](migration.md) remain open.
