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
- Prefix stages select, render, update and save the configured fact layers in Rust.
  Composition passes the environment layer's current claims to the paid proposer
  and reports its per-task token delta. Saved-layer discovery, forgetting and
  restoration also execute in Rust.
- Tool pruning now executes in Rust: loaded-but-unused evidence, periodic reaudits,
  protected and ever-used tools, per-task judge classifications and costs, and the
  Codex capability controller. The controller audits traces, freezes masks, protects
  completed uses, applies task hints and sparse probes, and persists its original
  state format and SHA-256 contract fingerprint. Domain adapters and model calls
  still cross callback boundaries until their production implementations are ported.
- Plugin configuration and repository paths now execute in Rust: defaults, aliases,
  source precedence, validation, warnings, configuration writes and removals, repository
  discovery, remote and worktree identity, state directories, owner-only permissions
  and launcher writes. Launchers still target the migration's existing entry points;
  native CLI packaging and Windows execution remain pending.
- Repository-state migration is native: session pins and redirects, legacy keys,
  checkout indexes, migration plans, locked session and whole-directory merges,
  learned-state preservation, ledger deduplication, refold selection, migration
  notices and repository listings. Rust also reads and merges applied-tool ownership
  records. Refolding uses the native context and fold components; host log access
  still crosses the test adapter's callback boundary.
- Deny-list settings now execute in Rust: Claude and Copilot JSON lists, Codex
  marked TOML edits and rollback, applied and restored ownership, trust checks,
  repository-local exclusions and Copilot server proposals. Native subprocesses
  check tracked files and validate Codex configuration without calling a model.
- Background judge orchestration now executes in Rust: request identity and
  persistence, pending-verdict polling, superseded jobs, paused-host runner
  selection, the local confidence floor, redirect selection, immediate cost
  recording and verdict publication. Runner selection and side calls still cross
  callbacks until their production implementations are ported.
- The live domain adapter now executes in Rust: repository vocabulary selection,
  saved and observed catalogs, deferred Claude tools, Copilot checkpoint catalogs,
  usage including Claude subagents, loop snapshots and interactive tool protection.
  Protected tools retain both names and user-defined patterns through the pruning
  stage. Pattern matching follows Python's case-sensitive shell wildcard rules,
  including bracket classes, Unicode and descending ranges.
- Hook queues now execute in Rust: session locks, bounded waits, atomic enqueue,
  ordered drains, removal before replay, session checks, replay error isolation,
  event and time budgets, fallback handling and idle pruning. Taking a lock
  refreshes its timestamps before pruning. The live controller constructs replay
  controllers and invokes their handlers in Rust.
- History import orchestration now executes in Rust: repository matching, ordered
  plans, eligibility checks, fold refusal, budgets, checkpoints, crash attempts,
  heartbeats, restart and stop decisions, notices and status summaries. Imported
  prompt turns seed the envelope through native guards with no judge or envelope
  intervention. The shared Claude shell success-code helper is also native.
  Host log access remains a callback until host integration is ported. Folding and
  process spawning use the native components through the test transport.
- Live context construction now executes in Rust: deferred settlement, session
  pins, configuration reload after migration, headless-mode overrides and mechanism
  directories. Native processes preserve arguments, environment, append-only logs
  and POSIX session detachment. Windows breakaway and WMI fallback decisions,
  command quoting, private spec files and worker cleanup are native too. Host
  detection still crosses a callback. Detached commands and WMI workers retain
  migration entry points until native CLI packaging replaces them.
- Session folding now executes in Rust: fold locks, import ordering, pending
  sessions, transcript workspaces, mechanism learning, cached injection and fact
  forgetting, tool evidence, protected-tool restoration, delayed deny-list writes
  and ledger records. Codex capability evidence and Copilot server summaries are
  native. Imported sessions learn without proposing through a model or changing
  deny lists. Host transcript operations and runner selection remain callbacks;
  test-patched deny-list operations also cross the transport.
- Host hook protocols now execute in Rust: host selection, session and turn IDs,
  tool names and arguments, failure and exit-code parsing, response limits,
  contexts, notices and output merging for all ten agents. Codex shell arguments
  reuse the native command formatter; Grok and Hermes results reuse their trace
  readers. Stateful host preparation, transcript access and runtime detection
  remain pending.
- Host session operations now execute in Rust: transcript paths and conversions,
  usage and prompt counts, history discovery, saved session metadata, headless
  detection, process ancestry, opencode export recovery, deferred Hermes exports,
  Copilot's final-event wait and guarded Cursor spool cleanup. Process environment
  and hook environment remain distinct. Command resolution still calls the
  side-call component until it is ported. Stateful event preparation and spooling
  for Cursor, Antigravity and Grok remain pending.
- Stateful host adapters now execute in Rust. Cursor translates events, filters
  recorded fields and caps stored text. Antigravity persists private replay
  cursors, prunes expired state and emits tool results before a new prompt.
  Grok saves start context, claims it atomically for one hook, preserves the
  tool's own context within its character budget and removes ended-session state.
  Host metadata, notice channels and resume behaviour are also native.
- The live controller now executes in Rust: cached facts and notices, session
  lifecycle, prompt turns, repository-map injection, envelope updates, sampled
  judges, redirects and abort notices. Queued decisions remain uncounted until
  a hook can deliver them. Side-call policy and repository-map generation still
  cross callbacks; the existing native history, folding and launcher components
  are connected through the test transport until CLI integration.
- Shared side-call support now executes in Rust: signed-out pause files and
  resumption, bounded error notes, recursion-safe environment copies, Claude's
  verbose result selection, Copilot model selection, Droid opt-in and usage
  fields, local-role routing and command names. Claude and Hermes installer
  fallbacks and npm shim parsing are native; unsupported batch files are refused
  before a prompt can reach `cmd.exe`. PATH lookup crosses the SDK test transport.
  Other hosts' model calls, availability probes and runner orchestration remain pending.
- Native subprocess capture now accepts UTF-8 prompt input while draining stdout
  and stderr concurrently. The shared timeout covers blocked input and output;
  early stdin closure preserves the child's result.
- Claude side calls now execute in Rust: per-executable help caching, supported
  history and effort flags, empty MCP configuration, prompt input, verbose result
  selection, no-reply pauses and native usage pricing. Each call owns a private
  temporary directory and removes it after subprocess completion or failure.
  Patched subprocesses and temporary-directory constructors remain SDK callbacks
  for the original tests. Native process execution needs no Python callback.
- Codex, Pi, Cursor, Grok and opencode side calls now execute in Rust. This
  includes read-only command arguments, model and effort selection, usage
  normalisation, Pi's older-CLI retry, opencode's cached flags and session deletion,
  and Grok's tool restrictions. Pi resolves extension paths before entering its
  temporary directory, including named-user homes through the platform account
  database. Expansion follows the maintained
  [Python contract](https://docs.python.org/3.15/library/os.path.html#os.path.expanduser).
- Copilot and Droid side calls now execute in Rust. Copilot excludes built-in and
  configured MCP tools, waits for its session's final usage event and prices that
  log. Droid allows only `Read`, accepts structured and older plain-text replies,
  and reads usage from the response or its session settings. The shared native
  host runtime resolves commands and process ancestry without a test callback.
  Runner orchestration remains pending.
- The on-device `fm` command now executes in Rust: platform and executable checks,
  per-executable availability caching, private JSON schema files, prompt stdin,
  greedy non-streaming arguments, ANSI removal, empty and error replies, timeout
  notes and zero-cost results. Runner routing and fallback still cross callbacks.
- Hermes side calls now execute in Rust: private query files, gateway-safe
  environments, usage and title polling, per-model pricing, deletion after
  charging, deferred records under a host lock, abandoned-session handling and
  retrying a failed deletion without charging it again. Timed-out sessions are
  recovered by their source, prompt and start time. The existing read-only
  Hermes database reader supplies the session and usage rows.
- Side-call failure notes now execute in Rust. Timeout and exit-status notes omit
  argv, other exceptions use one bounded line, and failures remain available to
  the caller after logging. Timeout durations follow Python's default
  [general-format rules](https://docs.python.org/3.15/library/string.html#format-specification-mini-language).
  The test transport retains the original callable metadata and exception object.
- Side-call runner selection and local-model fallback now execute in Rust:
  per-host models and effort, configured CLI paths, Droid opt-in, role schemas,
  availability gates and the state recording which runner answered. Local calls
  fall back to the selected host or an explicit caller-provided runner. The native
  runner implements the shared runner contract; patched model functions and
  callable metadata remain SDK transport for the original tests.
- Tool measurement and verification policies now execute in Rust: Claude prompt
  usage and deny-list settings files, Codex prompt rendering and feature checks,
  Copilot request sizes and remaining server tools, and CLI version parsing.
  Token sums preserve integer precision; character counts follow Python's Unicode
  and JSON contracts. Claude and Codex reject failed measurement probes.
- Tool discovery and catalog refresh now execute in Rust too. Claude's probe
  stops at its init event or deadline and terminates its process group. Copilot's
  probe captures requests on localhost, answers JSON or streaming completions,
  replaces provider credentials with a local key and keeps the first request.
  Both hosts save sorted catalogs and version metadata. Patched process fixtures
  retain their Python pipe handles; parsing, deadlines and stop decisions are native.
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
  Configuration numeric coercion still needs Unicode decimal digits and integers
  outside the current i64 range. Audit path-normalisation edge cases too.
- Complete named-user and Windows tilde expansion for extra history directories
  in the path parity audit.
- Include history, folding, controller and host replay counters beyond i64 in the numeric
  parity audit.
- Include side-call usage beyond i128 in the numeric parity audit.
- Include tool-measurement totals and savings outside JSON's i64/u64 range in the
  numeric parity audit.
- Audit temporary-directory cleanup against Python's permission-repair retries,
  including native Windows behaviour. Current removal uses `remove_dir_all`.
- Integrate the native controller and host operations into CLI and hooks.
- Port end-to-end model probes, repository maps, CLI commands and hooks.
  Verify each host integration and its launcher and packaging contracts.
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
- Prefix-stage, composition and forgetting-focused run: 45 passed. Full suite
  with native prefix stages: 1,558 passed, 33 skipped (104.51 seconds).
  `validation/prefix-calls.json` records 386 stage constructions, 404 task folds
  and 398 renders reaching Rust, including callback replay. The original composed
  profile test checks that the proposer receives newly injectable environment
  claims and that its effective tokens reach the per-task ledger.
- The bridge probe checks rendering two non-empty layers in order, selecting
  environment IDs and using the paid layer's token count and task index. Saved-layer
  checks cover duplicate IDs, environment ownership of shared IDs, unknown-ID
  validation before any write, restoration without a record and saving only the
  changed layer. Clippy and the 256-file upstream integrity check also pass.
- Tooling, routing, trace and controller-focused run: 372 passed, 2 skipped.
  Full suite with native tool pruning: 1,558 passed, 33 skipped (93.73 seconds).
  `validation/tooling-calls.json` records 357 tooling-stage constructions, 689
  selections and 1,025 task folds reaching Rust, including callback replay. Native
  constants and the controller fingerprint match the pinned original at test startup.
- The bridge probe checks protected-tool enforcement, malformed judge responses,
  runner failures, cost approximation, Unicode issue limits, stage-level candidate
  filtering and ledger fields, periodic reaudits and adapter failures. Controller
  checks cover sparse recovery, permanent protection, probe backoff, task hints,
  saved-state normalization, incompatible version/fingerprint/repository resets,
  and fallback transcript IDs across blank lines. Clippy, prompt verification and
  the 256-file upstream integrity check also pass.
- Plugin-configuration-focused run: 101 passed. Repository, path and safety-focused
  run: 330 passed. Full suite with native plugin foundations: 1,558 passed,
  33 skipped (95.82 seconds). `validation/plugin-foundations-calls.json` records
  4,929 configuration loads, 865 repository identities, 4,811 root-permission checks
  and 701 launcher checks reaching Rust. Internal merges and filesystem reads stay
  inside the native process.
- The bridge probe checks numeric separators, validation before mixed configuration
  writes, removal through a legacy threshold name, private state and launcher modes,
  unchanged-launcher timestamps, literal shell arguments through a path containing
  an apostrophe, and repository identity through a missing symlink target. Clippy
  and the 256-file upstream integrity check also pass. The test adapter transports
  non-finite floating inputs explicitly so native validation rejects them normally.
- Repository and history-focused run: 75 passed. Recovery run including the original
  injected-write-failure test: 74 passed. Full suite with native repository migration:
  1,558 passed, 33 skipped (91.34 seconds). `validation/repos-calls.json` records
  native bridge calls; nested migration, lock, filesystem and index operations stay
  in Rust. The original tests cover renamed remotes, session fragments, imported
  history, pin redirects and keeping the appropriate learned state.
- The bridge probe checks merge lock exclusion and release, ledger deduplication,
  timestamp ordering, newest session records, bounded envelope merges, applied and
  restored tool ownership, and discovery with an empty environment. Copy-fallback
  checks verify that a failed copy retains every source file and that a successful
  copy preserves data, symlinks and modification times. Clippy and the 256-file
  upstream integrity check pass. Native Windows execution remains unverified.
- The adapter forwards a test-patched `Store.write` to preserve the original
  apply/restore failure injection; record construction and normal I/O remain native.
- Deny-list focused run: 148 passed, 2 skipped. Full suite with native deny-list
  settings: 1,558 passed, 33 skipped (92.44 seconds). Native bridge entries are
  recorded in `validation/denylist-calls.json`. The original tests check ownership
  isolation, symlink refusal, committed-file protection, trust and Codex rollback.
- The bridge probe checks unbilled Codex validation through a local CLI fixture:
  both output streams beyond pipe capacity, Unicode truncation, line endings,
  exit and signal codes, timeout, success and missing executables. Clippy and the
  256-file upstream integrity check pass. Native Windows execution is unverified.
- Background judge focused run: 135 passed, 2 skipped. Local-model follow-up:
  29 passed. Full suite with native background jobs: 1,558 passed, 33 skipped
  (90.31 seconds). `validation/live-judge-calls.json` records native calls; nested
  storage, verdict decoding and redirect generation remain inside Rust.
- The bridge probe checks superseded jobs, paused-host selection, stale verdicts,
  request persistence before spawning and detached command construction. It also
  injects a publication failure after model replies and verifies that both calls'
  costs remain recorded. Clippy, formatting and upstream integrity checks pass.
  Actual detached process launch still uses the reference context callback.
- Live adapter focused run: 160 passed, 2 skipped. Full suite with the native live
  adapter: 1,558 passed, 33 skipped (92.48 seconds).
  `validation/live-adapter-calls.json` records native entries. The original tests
  cover vocabulary singleton identity and restoring a tool after a user protects it.
- The bridge probe checks Unicode wildcards, bracket and descending ranges,
  malformed brackets, literal metacharacters, newlines and path separators. It
  also verifies pattern protection inside the pruning stage, saved descriptions,
  deferred tools, Copilot checkpoints with bare CR line endings and malformed
  saved-catalog recovery. Clippy, formatting and upstream integrity checks pass.
- Hook queue focused run: 17 passed. Full suite with native queues: 1,558 passed,
  33 skipped (52.86 seconds). `validation/hookqueue-calls.json` records native
  entries, including 4,801 queue runs. Callback resumes are transport operations.
- `QueueProbe.java` verifies nine native processes sharing a session lock without
  losing updates, plus ordering, event/time limits, foreign-session checks,
  removal before replay, replay failure isolation, contended prompt fallback,
  draining after a failed handler and lock release. It also verifies that a waiting
  hook leaves the bridge available to its lock holder. The established bridge
  probe, Clippy, formatting and upstream integrity checks pass. Native Windows
  execution and the complete native hook executable remain unverified.
- History import focused run: 31 passed. Full suite with native import
  orchestration: 1,558 passed, 33 skipped (52.44 seconds).
  `validation/history-import-calls.json` records native entries; nested planning,
  persistence, guard replay and envelope updates remain inside Rust.
- The original tests verify oldest-first selection, stale heartbeats, refusal
  after live folds, budget resumes, fold-lock contention, bounded crash retries,
  recovery after a fold committed before process death, no model calls and green
  turn seeding. Clippy, formatting, both native probe tools and the 256-file
  upstream integrity check pass. Host and folding callbacks remain pending.
- Context, Windows spawning, repository, history and status focused run: 113 passed.
  Full suite with native context: 1,558 passed, 33 skipped (52.05 seconds).
  `validation/context-calls.json` records 4,897 context constructions, 4,802
  settlements and 737 mechanism-directory requests reaching Rust. Callback
  resumes are transport operations; nested repository and store calls stay native.
- `ContextProbe.java` verifies deferred writes, session pinning, repeated settlement,
  literal arguments and Unicode, environment propagation and combined log output.
  A child verifies its POSIX session ID and finishes after the native parent exits.
  WMI checks cover spaces and apostrophes, failed-launch cleanup, owner-only spec
  permissions and removing the spec before running its command. The established
  bridge and queue probes, Clippy, formatting and all 256 reference files pass.
  Actual Windows execution and native CLI packaging remain pending.
- Folding, hooks, history, Hermes, forgetting, repository and status focused run:
  283 passed, 8 skipped. Full suite with native folding: 1,558 passed, 33 skipped
  (44.70 seconds).
  `validation/fold-calls.json` records native entries. Mechanism stages, trace
  analysis, accounting and normal persistence execute inside Rust.
- `FoldProbe.java` checks two native processes contending for one fold lock,
  learning each session once, deferred active sessions, releasing locks and deleting
  workspaces after a failure, imported-session boundaries, saved numeric strings,
  malformed-counter rejection and chronological pending sessions. The established
  bridge, context and queue probes, Clippy, formatting and the 256-file upstream
  integrity check pass. Complete native host and runner integration remains pending.
- Host-protocol focused run: 513 passed, 14 skipped. Full suite with native host
  protocols: 1,558 passed, 33 skipped (44.75 seconds).
  `validation/host-protocol-calls.json` records
  15,290 session IDs, 3,484 tool calls, 4,788 host selections and 499 output merges
  reaching Rust. Nested response, trace and shell conversions stay native.
- The original tests verify all ten agents' payloads, interrupted and failed tools,
  patch paths, notice channels, host precedence and a temporarily registered adapter.
  The bridge probe checks negative Unicode response limits, suppressing structured
  patches, failure-code precedence, metadata booleans and merged contexts and progress.
  Clippy, formatting and the 256-file upstream integrity check pass. Stateful host
  behaviour, transcript orchestration and headless detection remain pending.
- Host-session focused run: 544 passed, 14 skipped. Full suite with native host
  session operations: 1,558 passed, 33 skipped (52.26 seconds).
  `validation/host-session-calls.json` records native entries; conversions, usage,
  history selection and normal subprocess capture remain inside Rust.
- `HostSessionProbe.java` runs a local export fixture through the native process
  launcher, checks its environment and working directory, preserves a saved copy
  after malformed output and removes temporary files. It verifies relative input
  and output paths, command-ancestry decisions, bridge responsiveness during a
  final-event wait, and protecting an external transcript during Cursor cleanup.
  The context, folding and established bridge probes, Clippy, formatting and all
  256 pinned reference files pass. Real agent commands and native Windows
  execution remain unverified.
- Stateful-host focused run: 459 passed, 8 skipped. Adapter-only run after the
  metadata checks: 88 passed, 6 skipped. Full suite with native stateful adapters:
  1,558 passed, 33 skipped (52.64 seconds). `validation/host-state-calls.json`
  records 196 event preparations, 47 Cursor recordings, 63 expansions and 160
  renderings reaching Rust. Startup compares every built-in host's metadata with
  its original class, including the shared base class.
- `HostStateProbe.java` checks private spool permissions, dropping unrecorded
  fields, Unicode text limits, eight races between two native processes claiming
  the same saved context, malformed replay-state rejection, exact advancement of
  a large integer string and pruning only expired files. Grok's filename character
  classes follow the [Python word-character contract](https://docs.python.org/3.15/library/re.html#regular-expression-syntax);
  the probe includes a combining mark and a Unicode numeric character. The host
  session probe, Clippy, formatting and all 256 pinned reference files pass.
  Full native entry points and real-agent integration remain pending.
- Controller-focused run: 181 passed, 3 skipped. Full suite with the native
  controller: 1,558 passed, 33 skipped (50.93 seconds).
  `validation/controller-calls.json` records entries reaching Rust; nested guard,
  queue and envelope operations stay inside the native controller.
- `ControllerProbe.java` runs nine concurrent native controller processes against
  one session, checks envelope learning from the completed turn, holds a redirect
  reached during replay and delivers it once on the next live hook. The response
  retains `PostToolUseFailure` and the queued event is removed. No reference hook
  executable or agent CLI runs in this probe. Clippy, formatting and the integrity
  check of all 256 archived files pass.
- Side-call support focused run: 534 passed, 8 skipped. Full suite with native
  pause state, command resolution and policy helpers: 1,558 passed, 33 skipped
  (51.43 seconds). `validation/sidecall-support-calls.json` records native entries,
  including 2,277 pause reads, 445 local-role checks and six npm shim parses.
  The native controller process probe, Clippy, formatting and all 256 archived
  files pass. Actual agent commands and native Windows execution remain unverified.
- `ProcessInputProbe.java` sends a large UTF-8 prompt with line breaks to a local
  Java fixture while it fills both output pipes. It verifies the input digest,
  complete output, early stdin closure and child removal after a blocked-input
  timeout. The existing bridge and host export probes pass. Full reference suite
  after the shared process change: 1,558 passed, 33 skipped (50.53 seconds).
  Clippy and formatting pass; no real model command runs in these probes.
- Claude side-call focused run: 323 passed. Full suite after native Claude calls:
  1,558 passed, 33 skipped (49.56 seconds). `validation/claude-sidecall-calls.json`
  records native bridge entries, including the original side-call fixtures.
- `ClaudeCallProbe.java` uses a local CLI fixture to verify prompt stdin, private
  workspaces, empty MCP configuration, model and effort flags, one help invocation
  across calls, verbose usage pricing, signed-out pause state and cleanup after
  success and a non-zero exit. Clippy, formatting and the 256-file reference
  integrity check pass. Real Claude calls and native Windows execution remain
  unverified; runner selection and the other hosts' side calls remain pending.
- Agent side-call focused run: 346 passed, 3 skipped. Full suite with native
  Codex, Pi, Cursor, Grok and opencode calls: 1,558 passed, 33 skipped (49.50 seconds).
  `validation/agent-sidecalls-calls.json` records native entries.
- `AgentCallsProbe.java` executes each of those five commands through local CLI
  fixtures. It checks prompt boundaries, safe flags, usage pricing, subtraction of
  large Codex integer counts before conversion, Pi's retry and extension paths,
  named-user home expansion, opencode's help cache and deletion attempts, and
  private workspace cleanup. No model or credential is used. Clippy and formatting
  pass; real-agent and native Windows execution remain unverified.
- Copilot/Droid focused run: 410 passed, 2 skipped. Full suite with both native
  calls: 1,558 passed, 33 skipped (49.58 seconds).
  `validation/copilot-droid-sidecalls-calls.json` records native entries.
  The expanded `AgentCallsProbe.java` checks all seven hosts, including a Copilot
  shutdown event appended by a process after the CLI exits, Droid settings usage
  and its plain-text fallback. The native host probe, Clippy, formatting and all
  256 reference files pass. No real model command runs in these checks.
- Local-model focused run: 323 passed. Full suite with native `fm` calls and
  availability checks: 1,558 passed, 33 skipped (49.64 seconds).
  `validation/fm-sidecall-calls.json` records native entries.
- `LocalModelProbe.java` uses a local CLI fixture to check the platform gate,
  availability caching across an executable-permission change, schema files,
  UTF-8 prompt stdin, ANSI stripping, zero-cost replies, error rejection and
  timeout cleanup without recording the prompt in the error log. The seven-host
  probe, Clippy, formatting and all 256 reference files pass. The system's actual
  on-device model and native Windows execution remain unverified.
- Hermes side-call focused run: 370 passed, 6 skipped. Full suite with native
  Hermes calls and deferred recovery: 1,558 passed, 33 skipped (51.17 seconds).
  `validation/hermes-sidecall-calls.json` records native entries, including 13
  Hermes calls. Patched accounting callbacks retain the original per-model spy;
  normal pricing runs inside Rust.
- `HermesCallProbe.java` runs a local CLI fixture against SQLite. It checks
  private UTF-8 query files, gateway environment filtering, distinct model prices,
  title usage appended after the CLI exits, charge-before-delete ordering,
  timed-out-session recovery, failed-delete retries and eight concurrent ledger
  updates. The seven-host probe, Clippy and formatting pass. All 256 archived
  files pass the integrity check. No real gateway or model runs in these checks;
  native Windows execution remains unverified.
- Failure-note focused run: 390 passed, 6 skipped. Full suite with native notes:
  1,558 passed, 33 skipped (50.69 seconds).
  `validation/sidecall-notes-calls.json` records calls entering Rust.
  `SidecallFailureProbe.java` checks success without a log entry, timeout and exit
  diagnostics, general-format boundaries, Python's extra control separators,
  Unicode character limits and one line per failure. The original tests verify
  that the exception reaches the caller and the wrapped callable stays visible.
  Clippy, formatting and the 256-file reference integrity check pass.

- Runner-focused run: 437 passed, 6 skipped. Full suite with native runner routing
  and local fallback: 1,558 passed, 33 skipped (49.96 seconds).
  `validation/runner-routing-calls.json` records native entries.
  `RunnerProbe.java` checks host selection, model and effort precedence, Droid
  opt-in, the judge schema, repeated local calls, empty local answers, host and
  external fallback, native Pi execution, workspace cleanup and private failure
  notes. Clippy and the 256-file reference integrity check pass. Real model calls
  and native Windows execution remain unverified.
- Tool-measurement focused run: 31 passed. Full suite with native measurement
  and verification policies: 1,558 passed, 33 skipped (51.78 seconds).
  `validation/toolcheck-measure-calls.json` records native entries.
  `ToolcheckProbe.java` uses local CLI fixtures and captured request bodies to
  check token sums above 2^53, private settings files, workspace cleanup,
  Unicode character counts, negative savings, feature state despite a non-zero
  exit, relative repository paths, Unicode version lines and tool-name precedence.
  Clippy, formatting and all 256 archived reference files pass. The focused suite's
  streaming-discovery tests still exercise the reference implementation.
- Tool-discovery focused run: 31 passed. Full suite with native discovery,
  HTTP capture and catalog refresh: 1,558 passed, 33 skipped (49.95 seconds).
  `validation/toolcheck-discovery-calls.json` records native entries.
  `DiscoveryProbe.java` checks silent-process deadlines, init-event parsing,
  descendant termination, private workspaces, provider filtering, local JSON and
  streaming replies, header limits, first-request retention, timeout recovery,
  listener shutdown and both hosts' saved catalogs. Delaying request bytes
  reproduced the accepted-socket race on macOS; the native server now explicitly
  selects blocking reads after [accepting a connection](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/accept.2.html).
  The delayed-request fixture passes twice after the fix. The existing measurement
  probe, Clippy, formatting and all 256 reference files pass. Real host CLIs and
  native Windows execution remain unverified.

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
`sha2` 0.10.9 preserves the Codex pruning controller's SHA-256 contract fingerprint,
which decides whether saved evidence can be reused. Its API is verified against the
pinned crate's source documentation and its result against the original controller.
`sha1` 0.10.7 preserves the existing repository-key digest. `libc` 0.2.191 was already
transitive; the direct dependency supplies POSIX ownership and executable-access
checks for private state and launchers, and POSIX process session detachment.
`indexmap` 2.14.2 was already present through `serde_json`; it is now declared
directly with serialization support for typed fact maps. Discovery order determines
ties in fact selection and rendering.
[TOML](https://docs.rs/toml/1.2.1+spec-1.1.0/toml/) 1.2.1 validates the complete
Codex configuration before writing. The native line editor preserves comments and
existing keys; parsing checks that the edited document remains valid.
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

Deny-list write and validation callbacks suspend and resume the same native
operation. This preserves the original failure-injection tests' write ordering
and avoids replaying an operation after its settings file has changed.
Background judge jobs use the same transport to retain their request and model
replies across callbacks. The native job appends its costs before publishing its
verdict. Fake stores and patched judge functions are transported for the original
tests; normal storage and verdict parsing execute in Rust.
The live adapter transport preserves vocabulary singleton identity and the
protected set's pattern metadata. Membership checks execute in Rust, including
checks made by the remaining reference folding code. The wildcard contract is
checked against the maintained [Python documentation](https://docs.python.org/3.15/library/fnmatch.html)
and installed Python 3.15.2 implementation; the native matcher uses bounded
dynamic programming without regular-expression backtracking.

Small Java validation tools check the migration infrastructure:

```sh
jbang validation/VerifyReference.java
jbang validation/BridgeProbe.java
jbang validation/QueueProbe.java
jbang validation/ContextProbe.java
jbang validation/FoldProbe.java
jbang validation/HostSessionProbe.java
jbang validation/HostStateProbe.java
jbang validation/ControllerProbe.java
jbang validation/ProcessInputProbe.java
jbang validation/ClaudeCallProbe.java
jbang validation/AgentCallsProbe.java
jbang validation/LocalModelProbe.java
jbang validation/HermesCallProbe.java
jbang validation/SidecallFailureProbe.java
jbang validation/RunnerProbe.java
jbang validation/ToolcheckProbe.java
jbang validation/DiscoveryProbe.java
jbang validation/ExtractProposerPrompts.java
```

The bridge probe covers malformed-request recovery, pricing validation, Unicode
character counts, JSON object filtering, all three text line endings and a
redirect regression where a file is literally named `one file`. It also checks
structured Cursor result text, partial-record offsets and decimal cost rounding. These
probes also cover metadata limits and read-only database access. The upstream
test suite is still the main driver.

The queue probe starts nine native bridge processes against one session lock
and checks that their handlers preserve every shared-state update. It also
checks queue ordering, event and time limits, removal before replay, foreign
session refusal, failure recovery, contended fallback and a waiting job sharing
the bridge with its lock holder. It does not use the reference hook executable;
the user-facing native hook and CLI workflows remain pending.

IntelliJ has no open project for this checkout. Use Cargo's compiler, formatter
and Clippy until IDE access becomes available.
