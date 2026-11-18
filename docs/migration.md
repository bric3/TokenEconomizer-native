# Migration record

The deliverable is a Rust implementation of TokenEconomizer's production code:
micro-skills, tool pruning, loop guard, trace readers, pricing, accounting,
persistent state, hook processing, CLI, agent integrations, packaging tools and
experiment runners and adapters. Preserve observable behaviour, state formats,
error recovery and integration contracts. Host SDK entry points may require a
small adapter in the host's language; economizer processing belongs in Rust.
The optional aider library remains an external dependency. Its binding translates
values and invokes the SDK; repository-map policy and file selection belong in Rust.
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
- Repository-map state and orchestration now execute in Rust: shared cache paths,
  atomic state writes, commit and token-budget freshness, interpreter discovery,
  first-prompt identifiers, bounded generator calls, cached fallback and refresh
  scheduling. A failed refresh preserves the prior map and its commit.
- The repository-map generator now runs through `te-repomap-gen`. Rust parses its
  options, normalises Unicode decimal budgets and selects tracked or walked files.
  Git mode filters generated and vendored assets and caps file sizes; walk mode
  preserves the upstream Python-file rules. Rust passes the selected files,
  identifiers and cache directory to aider. The embedded Python
  binding only calls that optional SDK and separates console output from map text.
  The live generator call's deadline terminates its process group and removes its
  private workspace.
- Hook lifecycle and input/output policy now execute in Rust: host preparation,
  deferred context settlement, expanded event ordering, output merging and rendering,
  UTF-8 input, disable flags, event arguments, progress lines and error logging.
  The standalone `te-hook` connects the controller to native session, folding,
  history, judge, repository-map and migration services. It reads stdin directly,
  writes hook JSON to stdout and directs service diagnostics to stderr. Keep `te`
  beside `te-hook` for background work and the generated `bin/te` launcher.
  On Windows the launcher includes a native `te.exe` copy; its update policy has
  local filesystem checks, while execution on Windows remains unverified.
- CLI status and configuration processing now execute in Rust: saved-state totals,
  explanations, configuration grouping, updates and rejections, write checks,
  session-location notices, proposal filtering and quoted argument-line expansion.
  Ledger counts remain integers when aggregated, including values above 2^53;
  checked addition reports counts outside the current i64 range.
- CLI learning and repository commands now execute in Rust too: fact display,
  forgetting and restoration, history import and status, map display and refresh,
  session folding and detached judges, repository listing, migration and merges.
  Session commands use a pin only while its repository holds the session record.
  Unknown fact IDs preserve saved facts; merge keys must name one state directory.
- Tool-pruning CLI processing now executes in Rust: proposal and evidence display,
  apply/restore ordering, kept-item updates, trust and verification notices, catalog
  refresh and measurement output. The pending-apply waiter checks its heartbeat,
  bounds each delay, reloads settings and removes its heartbeat after completion
  or a reported error. CLI error handling preserves backend exception categories.
- Installation diagnostics, recent-host selection and RTK hook discovery now
  execute in Rust. Diagnostics reuse native trust, model-selection and repository
  helpers, preserve error-log line boundaries and accept runtime-version metadata
  from the caller. Provider URL display validates bracketed addresses and Unicode
  authority delimiters. The standalone CLI reports its native package version.
  Packaged installation workflows remain pending.
- Copilot probe orchestration now executes in Rust: the isolated repository and
  missing-pytest fixture, session environment, CLI reply parsing, log inspection,
  fold deadlines, deny-list measurement and restoration, reporting and cleanup.
  Fixture wrappers quote paths for the shell. Fixture commits use the configured
  signing key and a conventional message to follow the user's commit policy.
  The optional pytest trap still invokes an external Python interpreter. The
  standalone CLI connects host and process I/O through a native runtime; no paid
  sessions have been run to verify packaged plugin behaviour.
- CLI argument declarations, parsing and entry orchestration now execute in Rust.
  This includes aliases, common options before and after the command, defaults,
  repeated sessions, variable argument lists, Unicode integer parsing, required
  arguments, help wrapping, errors and choice suggestions matching Python 3.15.
  The entry path expands slash-command argument lines, selects the default host,
  dispatches the command and secures the state directory after success or failure.
  The standalone `te` executable connects command execution to Rust context,
  configuration, state, repository, deny-list, host-session, folding, history,
  judging, model-runner, map and probe services. Background commands launch `te`
  directly. The compatibility adapter translates namespaces and exceptions for
  the original in-process tests.
- Windows background execution retains the WMI fallback and uses the native CLI
  to consume its launch specification. The fallback requests a hidden process
  through [Win32_ProcessStartup](https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-processstartup).
  Native worker execution has local checks; native Windows execution remains
  unverified.
- Native bundle assembly now runs through `te-package`: marketplace metadata,
  pinned host assets, Droid's filtered layout and command rewriting, native
  executables and optional ZIP archives. `--source` selects the asset checkout;
  `--binaries` selects the three runtime executables, defaulting to the packager's
  directory. Bundles carry their platform's runtime, Apache-2.0 licence, locked
  dependencies' legal texts and native usage instructions. They retain no Python
  core package or bootstrap. Pi, opencode and Hermes adapters call the bundled
  executables. Codex and Copilot
  Windows hook commands select native executables and preserve UTF-8 input.
  Codex's command uses [PowerShell's UTF-16LE EncodedCommand](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_powershell_exe?view=powershell-5.1#-encodedcommand-base64encodedcommand)
  to pass the same script through cmd.exe and PowerShell quoting.
- Hermes gateway policy now executes in Rust: read-only session lookup, compression
  aliases, recorded working directories, bounded ignored/ended caches, per-session
  serialisation, resumed starts, held redirects, notice delivery and finalisation.
  The SDK binding exchanges JSON with `te --hermes-bridge`; its Python code handles
  the Hermes API, process I/O and logging. Profile lookups run in the calling SDK
  thread so a gateway's concurrent profiles keep their own homes. The gateway
  launches native hook processes in each session's directory with a 20-second
  deadline and a five-second limit on contended session locks. Native Windows
  requests hidden child processes; execution on Windows remains unverified.
  Cancelled profile lookups release their native waiters. A malformed backend
  response stops that backend, releases SDK callers and allows a fresh backend
  on the next callback. The SDK can start it after the gateway's directory has
  been removed.
- Pi session policy now executes in Rust: carried sections and compaction resets,
  session sources, prompt counters, manual accumulation, tool-result text,
  steering messages, notices and command arguments. The TypeScript binding
  translates SDK values and actions through `te --pi-bridge`. It projects branch
  metadata and text blocks without copying conversation or image bodies into IPC.
  Native hooks and commands retain the 60-second deadline and request hidden
  child processes on Windows. Idle IPC handles allow Node to exit. Hook failures
  retain the session manual. Rust sends recovery snapshots with the manual,
  prompt counter, turn ID and fallback actions for accepted requests. After a
  backend failure the binding releases those callers and restores a replacement
  backend on the next call, without replaying hooks. A request that fails before
  preparation returns no SDK action. Snapshots remain in memory and contain no
  prompt, tool-result or environment payloads.
- opencode v1 and v2 session policy now executes in Rust: open-session tracking,
  manuals, prompt counters, redirects, notices, turn settlement and exports.
  The TypeScript bindings register SDK hooks, project JSON values and apply native
  actions. Shared JavaScript whitespace, truthiness and text coercion also serve
  Pi. Each request captures its caller's environment and working directory.
  v1 exports through the host CLI; v2 records prompts, assistant steps, tools,
  tokens and costs. Shared message references retain later step updates, and an
  empty plugin copy preserves another copy's existing export. Duplicate v2 end
  signals settle a turn once. Redirects mutate SDK results before notices.
  A synchronous exit helper uses an authenticated loopback connection to the
  backend that owns the open sessions. Exit freezes pending continuations,
  writes exports and dispatches the remaining session ends before acknowledging.
  Accepted native I/O drains after EOF while stopped SDK callbacks are released.
  The Unix backend uses a separate process group and releases idle handles, as
  described by [Node's child-process lifecycle](https://nodejs.org/api/child_process.html#optionsdetached).
  Windows keeps the attached launch and propagates the SDK's hidden-process
  option. Native Windows execution remains unverified. Incremental checkpoints
  preserve manuals, counters, session order, recorded messages and shared step
  references across backend failures. The binding stores opaque slots in memory;
  Rust restores them on the next callback without replaying interrupted hooks.
  A pending prompt retains its counter and export record. Checkpoints transfer
  only changed messages and metadata, and contain no environment snapshots.
  v2 checkpoints contain the conversation records needed for exports; completed
  v1 callbacks retain no prompt or tool-result bodies. If the backend has already
  stopped at process exit, the synchronous helper restores the checkpoint and
  finishes the open sessions locally.
- The test adapter replaces these functions and lifecycle methods in the reference
  process. It records calls reaching Rust in `validation/results/native-calls.json`.
  Direct module CLI subprocess launches are redirected to the native executable.
  Other reference code, including packaging launchers, still runs in Python.
- Remaining production conversion, six improvement passes and final test conversion
  pending. No performance claim is made from these compatibility runs.
- The SWE-bench, Web-Bench, CredData and ReposVul dataset adapters now execute in
  Rust. They read task specifications, redact solution URLs, discover dataset
  entries, select domain prompts and vocabularies, and reuse the native trace,
  catalog, loop-state and tool-policy implementations. They implement the native
  tooling interface. The original Python classes serve as test bindings during
  migration; benchmark task execution remains in Python.
- The experiment spiral stage and Codex controller now execute in Rust. The stage
  freezes each task's guard parameters, records telemetry, advances generic and
  Codex envelopes, and routes optional judge/writer calls through the native model
  contract. The Codex controller parses shell patches before command results,
  tracks failed verification scopes, selects deterministic nudges, and continues
  only on a captured thread. Its iterator retains the source's lazy first-thread
  behaviour. The original classes serve as data and callback bindings in tests.
- The generic experiment nudge controller now executes in Rust. It folds streamed
  tool calls, samples the judge with productive backoff, records token costs and
  decisions, and builds continuation prompts. Continuation resets the segment
  state while retaining cumulative turns, costs and history. Its cumulative
  backstop aborts immediately, including when nudge budget remains.
- The experiment facade now constructs and owns the native prefix, tooling and
  spiral stages. It resolves each stage's optimizer runner only when required,
  retains the selected runners, merges per-task parameters and processes results
  in tooling, spiral, injection order. It supports the native dataset adapters
  and custom adapters through their shared domain interface. The original facade
  class serves as a data and callback binding during migration.
- The task-runner usage parsers and segment merges now execute in Rust for Claude,
  Codex and Copilot. Claude recovery deduplicates message snapshots and clears
  degradation only when the session log accounts for every observed message.
  Codex preserves unknown segment totals; Copilot retains known counts and marks
  mixed sources. The original harness functions serve as data bindings in tests.
- The shared SWE-bench task lifecycle is native, including task prompts, container
  names, resumed segments, artifacts, error rows and cleanup ordering. Host
  callbacks still supply container, scaffold, grader and stream operations.
- Grading orchestration is native too, including eval-script execution order,
  partial timeout logs, failed-execution reports and recovery from grader errors.
  Container commands remain host callbacks.
- The official-grader wrapper selects table and SDK paths, loads and normalises
  grading rows, builds predictions and selects instance reports in Rust. Its host
  binding imports and calls the external SWE-bench SDK and retains opaque test
  specifications across callbacks.
- SWE-bench runner provenance is native, including the applied tool mask, manual
  fingerprint, CPU quota and thread caps, spiral parameters and scaffold paths.
  Project environment selection and activation variables also run in Rust.
  SWE-bench and Web-Bench share the native network-tool policy.
- Per-task container policy is native for all three SWE-bench agents: command
  construction, start order, exited-container reaping, live tracking, cleanup and
  measured environment activation. Copilot seed and mount arguments and Codex
  mount arguments are native too. The host binding executes processes, retains
  process objects and expands host home directories through the standard library.
- Codex request configuration is native: protected-capability rejection, network
  toggles, ordered TOML tables, system-skill entries and prompt-section trimming.
  SWE-bench installs the configuration and optional shortened prompt through a
  native base64 command. Scaffold result handling and Codex MCP wiring are native;
  the host binding retains the scaffold notes and server specification objects.
- Copilot and Codex SWE-bench task wrappers are native. They persist Copilot debug
  logs, reset per-task Codex state, record the capability mask and shortened-prompt
  count, and measure RTK compliance and MCP calls from saved transcripts. The host
  binding retains the shared task's row and output-directory objects.
- The answer-key guard policy and standalone hook are native. Payload key priority,
  bounded command search, command rendering, Unicode matching, host-list generation,
  lift settings, refusal logs and transcript attempt counts run in Rust. The
  standalone executable is `te-net-guard`; original guard subprocess assertions
  execute it through the validation adapter. Container hook installation remains
  part of the runner conversion.
- SWE-bench DNS guard installation, resolver checks, refusal probes, refusal counts
  and graded-diff commands are native. The hook configuration merger is available
  through `te-net-guard --install-hook` for all three agents. It preserves existing
  hooks and recognises an identical native entry when the executable name uses
  hyphens. Container binary deployment and runner registration still need conversion.
- SWE-bench controller integration is native for all three agents: controller
  selection, constructor parameters, optional-parameter compatibility, continuation
  dispatch and telemetry fields. The SDK binding retains policies, callbacks,
  controller results and telemetry objects by reference.
- Codex host installation policy is native: binary and vendor discovery, version
  reporting, captured-prompt lookup, offline bundled-skills discovery, strict
  configuration validation, process-group cleanup and temporary-directory retries.
  The SDK binding supplies filesystem operations, process handles and clocks.
- SWE-bench constructor coordination is native for Claude, Copilot and Codex:
  model and effort selection, optimizer-override rejection, scaffold setup,
  configuration probes, initial state and validation order. The SDK binding
  retains argument, scaffold, profile, routing and configuration objects.
  Web-Bench task calls and scaffold backends remain
  callbacks until their implementations are ported.
- Web-Bench constructor coordination is native for Claude, Copilot and Codex,
  including model and effort selection, scaffold validation, dependency calls,
  timeout fields, preflights and initial stream state. The shared stream-state
  method also runs in Rust and preserves subclass overrides when called by a
  constructor. SDK bindings retain arguments, helper results, profiles and
  configuration objects, and supply object protocols and numeric conversions.
  Codex keeps its early scaffold check, separate vendor and binary discovery,
  namespace-probe order, duplicate start-timeout and workspace-flag assignments,
  and the final stream reset.
- Web-Bench repository and Node discovery, evaluator abort and task-timeout
  policy, preload environment assembly and namespace-probe decisions are native.
  SDK bindings supply filesystem and process I/O, URI encoding and Python object
  protocols. Native decimal arithmetic preserves arbitrary-size integer results
  without introducing an extra Python integer-to-string digit-limit check.
- Web-Bench stream start, session ownership checks and session close coordination
  are native for all three agents. Project selection, directory checks, evaluator
  arguments and partial state after errors follow the source order. Task lists,
  sessions, helper references and unchanged project strings retain their references.
  Evaluator process cleanup also uses native policy.
- Web-Bench evaluator event and request waits use native deadlines, five-second
  polling bounds, empty-queue handling, completion checks and event selection.
  SDK bindings supply clocks, queue I/O and numeric, mapping and formatting
  protocols, retaining raw queue results by reference.
- Web-Bench evaluator stdout processing uses native log-write order, attempt
  parsing, event publication, exit-state updates and final logging. SDK bindings
  supply stream iteration, file and queue I/O, object protocols and integer
  conversion. Custom matchers retain their bound methods and group results.
  The standard pattern recognises Python whitespace and Unicode 17 decimal digits,
  including Tolong Siki digits absent from the installed regex tables.
- Web-Bench evaluator task-file loading uses native path assembly, line filtering,
  ID collection, context-exit decisions and `OSError` handling. SDK bindings retain
  file handles and JSON decoder results by reference, and supply file iteration,
  JSON library decoding and object protocols. Suppressed read errors return
  `None`; unsuppressed file errors return a fresh empty list.
  Context exits observe the handled exception and its traceback. Unsupported
  context managers report Python's missing-special-method diagnostics. Optimizer
  logging uses the same corrected context-manager binding.
- Web-Bench evaluator startup uses native field assignment, HTTP/thread setup,
  configuration fields, subset selection and warnings, file-write decisions,
  entry validation, environment assembly, process launch and session registration.
  SDK bindings retain task IDs, handler bases, thread targets and helper references,
  and supply queues, HTTP servers, threads, JSON encoding and process I/O. The
  runtime handler factory uses the source module context so the generated class
  keeps its original module and session binding. Constructor failures preserve
  the source's partial state and resource-creation order.
- Web-Bench HTTP handling uses native request parsing order, parse-error responses,
  request/reply rendezvous, timeout fallback, response construction and broken-pipe
  handling. SDK bindings supply JSON library codecs, bytes and object protocols,
  queues and HTTP I/O. Parsed requests, reply queues and payloads retain their
  references. Getters, formatters and callbacks in exception handlers observe
  the handled exception and preserve exception chains. HTTP access logging remains
  quiet through a native no-op method.
- Web-Bench evaluator cleanup uses native registry removal, HTTP shutdown,
  process-group termination and escalation, bounded waits, log-file close and
  completion updates. Live-session sweeping snapshots the registry and preserves
  per-session exception boundaries. Signal registration and handlers also run in
  Rust, including resetting and repeating the received signal. SDK bindings supply
  process, signal and object APIs through a separate bridge connection so cleanup
  can interrupt a call holding the main RPC lock.
- SWE-bench live-container sweeping uses native snapshot iteration, forced Docker
  removal and registry updates. Names retain their references, command results are
  ignored and subprocess or registry errors propagate in source order. Both domains
  share native signal registration and signal exit-status handling. SDK bindings
  supply subprocess and signal I/O through independent cleanup connections.
- Web-Bench workspace materialisation and answer collection use native path
  validation, directory pruning, supplied-file exceptions, scaffold and evaluator
  exclusions, size limits, read-error handling, change detection and deletion
  reporting. Ignore-file loading and ancestor construction also use native policy.
  The shared wildcard matcher accepts Python code points, including surrogates.
  SDK bindings supply filesystem I/O, path normalisation and object protocols;
  replaced wildcard functions retain their external calls. Scaffold exclusion
  metadata still comes from the runner's source helper.
- The shared Web-Bench task loop uses native context setup, project validation,
  retry and continuation decisions, evaluator event checks, workspace coordination,
  replies, transcript and request-artifact order, resolve metrics and telemetry
  selection. Unreached rows preserve their absent verdicts; exhausted `init` tasks
  preserve the source's stream rule. SDK bindings retain raw session, queue, file,
  guard, prompt and result objects, and supply object and numeric protocols.
  Guard attributes remain lazy while standard `any` and `sum` consume them, retaining
  short-circuit reads and floating-point summation semantics. Supporting scaffold
  helpers remain pending.
- Copilot and Codex Web-Bench task wrappers use native superclass coordination,
  debug log persistence, capability snapshots, row metadata, compliance calls and
  task home pruning. SDK bindings resolve `super` from the original method's class
  and retain parent result references. Copilot error formatting and logging observe
  the handled file error. Codex pruning preserves provenance files, the workspace
  retention flag and error boundaries for each entry. Context-manager bindings evaluate
  an exit result's truthiness while the handled exception remains active.
- Web-Bench run configuration uses native tool-mask selection, field assembly,
  scaffold and injection metadata, spiral provenance and file-write coordination.
  Guard construction uses native selection, budget conversion, optional parameter
  checks and constructor arguments for the shared and Codex controllers. Prompt
  construction uses native description precedence, transport-instruction filtering,
  error truncation, pinned-template expansion and manual injection. Runtime template
  replacements retain their formatting calls. SDK bindings supply object protocols,
  hashing, JSON encoding and filesystem I/O while retaining raw argument references.
- Web-Bench agent segments use native command coordination, environment assembly,
  prompt delivery, transcript streaming, guard monitoring, deadlines, process-group
  termination, wait handling, configuration-refusal checks and usage recovery for
  Claude, Copilot and Codex. Claude session logs use native discovery, sorted reads,
  concatenation and error boundaries. SDK bindings supply object protocols, files,
  clocks, processes and pipes. Exception matching retains runtime class lookups and
  active error contexts. Supporting scaffold backends remain pending.
- Web-Bench per-task home paths, scaffold execution environments and Copilot cache
  preparation use native policy. Copilot configuration seeding preserves an existing
  task home and copies only the three named configuration files. The Web-Bench Codex
  configuration writer uses native lean-token metadata, argument assembly and file
  coordination with the existing native Codex configuration functions. Accounting
  hooks use native merge coordination, evaluator usage selection and row metadata
  for all three agents. SDK bindings retain raw values, comparisons, helper results
  and context managers. Copilot cache and configuration-directory discovery also use
  native environment selection with path-library bindings.
- Web-Bench network guard setup uses native settings paths, hook entries,
  installation, probe construction, refusal checks and probe-log removal. Existing
  hook lists and settings retain their references. Codex arming uses lexical
  superclass dispatch and reads the assigned state before returning. Refusal
  counting and hook-presence checks use native generator predicates, with SDK
  iteration and standard `sum` and `any` calls. Hosts-file generation and namespace
  path quoting also use native policy. SDK bindings supply file contexts, JSON
  encoding, process I/O and object protocols.
- Copilot and Codex argument builders use native flag order, exclusion selection,
  optional arguments and hook-trust flags. Codex resume arguments preserve the
  session, raw model reference and JSON effort setting. Codex environment additions
  use native defaults and update coordination. Copilot configuration-refusal
  detection uses native line selection, lazy pattern matching and truncation.
  SDK bindings retain raw arguments and object protocols, including generator
  captures that follow loop updates when callers retain a generator.
- Web-Bench scaffold integration uses native profiles, context construction,
  execution and host callbacks, hardlink placement, copy fallbacks, destination
  redirection, setup coordination and answer exclusions. SDK bindings construct
  callbacks with the source signatures and retain their captured objects after
  the creating method returns. Fallback I/O preserves the handled error for
  callbacks and exception chaining. Setup error metadata uses fresh scaffold and
  exception names and preserves the detail limit of 300 code points. The RTK and
  repo-map backends and SWE-bench context assembly remain pending.
- Shared scaffold profile and context constructors, read/log callbacks, instruction
  writes, MCP configuration and merge-helper staging use native policy. SDK bindings
  retain raw objects, file contexts, JSON encoding and process I/O. The staged merge
  program still uses Python; native container-helper deployment remains a production
  gate.
- Serena installation policy, version reporting, MCP server configuration, Codex
  server handoff and navigation instructions use native policy. SDK bindings
  retain callback results, server references and file I/O. The install command
  still installs Serena's external Python toolchain as required by the baseline.
- Provider routing and credential policy are native for Claude, Copilot and Codex.
  This includes in-place credential filtering, forwarding auth and provider
  variables, Copilot model normalisation and effort applicability, and the pricing
  validation call. Codex catalogue parsing, caching and model verification also
  use native policy. The binding retains mappings, opaque credentials, frozen
  catalogue objects and HTTP response handles.
  The guard and catalogue now share the Python-compatible JSON reader; guard
  command search and refusal-log formatting remain with the guard.
- Both shared endpoint preflight functions use native control flow: the
  `TE_OPTIMIZER_PREFLIGHT` gate, sixteen-token probe arguments, response checks,
  rejection messages and Codex success logging. The binding retains credentials,
  routing and model objects, and captures the Codex logger before invoking the
  optimizer.
- Messages and Responses optimizer calls use native request construction,
  credential selection, retry control flow, text extraction, warnings and usage
  accounting. Responses text extraction also runs in Rust. The binding supplies
  HTTP contexts, clocks, observers and pricing calls, and retains model, prompt,
  credential and pricing-result references. Native counters preserve decimal
  integers beyond machine range. Observer mutations follow each source function's
  billing order, including the second output-token read on the Responses path.
- Copilot CLI optimizer coordination is native: tool denial and exclusion flags,
  model selection, process arguments, cleanup, assistant-event parsing, usage
  observation and billing order. The binding retains opaque arguments and usage
  objects, and supplies process and filesystem I/O.
  The source's log-directory retention after timeout and launch failure is
  preserved. The shared JSON reader accepts Unicode code points directly for
  process output, including lone surrogates.
- Copilot host binary discovery, version selection, installation validation and
  environment assembly also use native control flow. The binding supplies path,
  filesystem, process and mapping operations, preserving opaque credentials and
  helper results. Version reporting prefers stdout, falls back to stderr, and
  keeps the first 120 characters of the first stripped line. Installation checks
  include cache and config paths when container seeding is requested.
- Shared optimizer metadata is native: log-path selection, cell-key derivation,
  context replacement with only `None` values removed, and classification against
  the nine prompt signatures within the first 600 Unicode code points. The
  binding retains the shared context and opaque values. Web-Bench aliases use
  the same native helpers.
- Shared optimizer log records and verbosity control use native policy: the log
  path gate, field order, context overrides, truncation, character counts, error
  handling, `TE_VERBOSE` selection, message prefix and explicit stdout flush.
  The binding retains record objects until encoding, preserving callback
  mutations and JSON dictionary-key behaviour. It supplies file contexts, clocks,
  Python object protocols, JSON encoding and stream I/O. Logger capture precedes
  exception formatting, and normal resource return values are ignored.
  Shared exact-string bindings use code point arrays so adjacent surrogates stay
  distinct from astral characters. Classification, Copilot version reporting,
  log excerpts, character counts and printed messages preserve that distinction.
- Optimizer and preflight methods use native routing for both domains. All six
  optimizer methods and five owned preflight methods preserve getter order,
  captured helpers, argument identity and ignored preflight results. Codex
  SWE-bench accepts explicit model overrides; Codex Web-Bench uses its runner
  model. Codex SWE-bench retains its inherited Claude preflight method when that
  method is called directly.
  Both runner factories use native agent defaults, registry checks, scaffold
  defaults and Web-Bench scaffold validation. Signal cleanup is invoked in the
  source order and remains a helper callback. Constructor results are retained
  by reference. The SDK also binds methods when Web-Bench loads after SWE-bench.
- The shared JSON reader supplies syntax messages, Unicode positions and line
  and column information. Catalogue HTTP contexts receive reconstructed
  `JSONDecodeError` objects with the source fields. The grader loader uses those
  diagnostics for invalid JSON; its three recorded syntax differences are fixed.

## Remaining production work

- Audit trace-reader edge cases, including the ISO timestamp formats accepted by
  Python's `datetime.fromisoformat`. Trace consumers still run through the reference
  plugin during migration.
- Audit regular-expression compatibility with Python in the production parity
  pass, including word boundaries, whitespace classes and syntax. The current
  library rejects Python's `(?a)` flag; this remains a required compatibility fix.
  Include Unicode-version differences outside the evaluator stdout pattern,
  whose explicit Tolong Siki range matches Python 3.15 and the
  [Unicode 17 numeric data](https://www.unicode.org/Public/17.0.0/ucd/extracted/DerivedNumericType.txt).
- Audit JSON numeric edge cases and parser nesting limits, including non-finite
  values accepted by Python.
  Audit remaining JSON object and exception-message codecs for adjacent surrogate
  code points; the shared exact-string and logging-format bindings preserve them.
  Include the Hermes redirect helper's raw JSON validation in that audit.
  Include the guard's numeric spelling, malformed Unicode and non-default stdin
  encodings. Its own input reader preserves Python-style non-finite numbers,
  surrogate code points, surrogate-escaped UTF-8 bytes and integer digit limits.
  Include dataset IDs outside the documented string schemas and exact JSON error
  diagnostics in that audit.
  Include grading-table string keys and JSON error wording and positions.
  Extend JSON syntax diagnostic coverage beyond the current source corpus.
  Grading-table values accepted by Python but unsupported by serde still need
  numeric, Unicode and nesting checks.
  Include non-string protected tool names and scaffold paths in the schema audit.
  Include malformed judge/writer token totals and partial threshold dictionaries
  in the exception-diagnostics audit.
  Configuration numeric coercion still needs Unicode decimal digits and integers
  outside the current i64 range. Audit path-normalisation edge cases and typed I/O
  errors from repository identity too.
- Complete named-user and Windows tilde expansion for extra history directories
  in the path parity audit.
  Include the project environment helpers, which currently use POSIX path rules.
- Include native OS arguments, environment values and paths outside UTF-8 in
  that audit. The CLI currently rejects them with a diagnostic.
- Include history, folding, controller, host replay and experiment counters beyond
  i64 in the numeric parity audit.
- Include harness accounting totals outside JSON's i64/u64 range in that audit.
- Include task turn caps outside i64 and task filesystem error diagnostics in
  the numeric and I/O parity audits.
  Include container timeouts outside JSON's integer range.
  Include Codex probe timeouts and version objects outside the tested JSON schemas,
  including non-finite numbers and custom numeric or string conversion methods.
- Include CLI and ledger counts beyond i64 in the numeric parity audit.
- Include `probe --sessions` integers outside i64 in that audit.
- Include displayed tool counters in that audit, and audit CLI errors for malformed
  saved state and invalid encodings.
- Include displayed map sizes and extreme CLI dates in the numeric/timestamp audits.
- Include side-call usage beyond i128 in the numeric parity audit.
- Include tool-measurement totals and savings outside JSON's i64/u64 range in the
  numeric parity audit.
- Audit temporary-directory cleanup against Python's permission-repair retries,
  including native Windows behaviour. Current removal uses `remove_dir_all`.
- Verify native CLI interruption and heartbeat cleanup when packaging the executables.
- Verify each host integration in its own runtime, including Windows launchers.
- Include SDK JSON nesting, environment snapshots and exit-control packet bounds
  in the production-parity audit.
- Audit configuration reference sharing and mutations made by callbacks in the
  original API alongside the remaining production parity checks.
  Include nested grading-row values passed to SDK callbacks.
  Include side effects when configuration objects are converted to strings.
  Include Codex capability collections with non-string elements and custom mappings.
  Include guard refusal counts outside JSON's native integer range in complete
  task rows; the direct method binding transports normalised decimal strings.
  Include custom parameter objects in `guard_accepts` and controller budget
  conversion outside the tested numeric and string cases.
  Include constructor subclass attributes with side effects and helper results
  outside their ordinary schemas. Captured configuration callables and arguments
  already retain their references when the prompt callback replaces them.
- Port the task runners and supporting tools, preserving their original test
  contracts.
  Their usage accounting and shared SWE-bench task lifecycle are native.
  Container guard binary deployment and registration, the RTK and repo-map backends, native
  merge-helper deployment, SWE-bench context assembly and agent streaming
  still need conversion.
  Stream start and session close coordination are native. Per-task container
  policy, grading orchestration, the official-grader wrapper, Codex config writing,
  scaffold result handling and Copilot/Codex SWE-bench task wrappers are native.
  SWE-bench controller construction parameters, continuation and telemetry are
  native too, along with constructor coordination for all three agents.
  Provider routing, credential filtering, model normalisation, catalogue reads,
  model verification and shared endpoint preflight control flow are native.
  Messages and Responses optimizer policy is native; HTTP I/O retains SDK bindings.
  Codex installation discovery, captured-prompt loading and offline probes are
  native. Experiment asset packaging and host bindings still need to replace the
  validation binding's reference-data paths.
  Process and SDK calls retain Python I/O bindings.
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
- Repository-map focused run: 24 passed, 1 skipped. Full suite with native map
  orchestration: 1,558 passed, 33 skipped (52.58 seconds).
  `validation/repomap-calls.json` records native entries, including seven freshness
  checks, five refresh requests and five first-prompt lookups. The original hook
  tests exercise prompt-ranked injection, cached fallback, session-start mode,
  shadow mode and an explicitly named interpreter without aider.
  `RepoMapProbe.java` checks state recovery, interpreter precedence, shebangs,
  Unicode word boundaries and error bounds, the 60-identifier cap, timeout cleanup,
  preserved caches and refresh scheduling. Clippy, formatting and all 256 reference
  files pass. The real aider generator is skipped because aider is not installed;
  its production helper still needs conversion.
- Generator-focused run: 43 passed, 2 skipped. Full suite with the native generator:
  1,558 passed, 33 skipped (51.19 seconds).
  `validation/repomap-generator-calls.json` records native entries. Both plugin and
  experiment test transports select the native generator.
  `GeneratorProbe.java` compares requests against the unchanged original generator
  using an isolated SDK fixture. It covers tracked and walked files, minified and
  vendored assets, size boundaries, Unicode and newline filenames, symlinks,
  decimal budgets beyond i64, console routing, SDK failures and signal status.
  The timeout case checks SDK descendant termination and workspace removal.
  The orchestration and subprocess-input probes, Clippy, formatting and all 256
  reference files pass. Real aider execution and native Windows remain unverified.
- Full suite with native hook lifecycle and input/output policy: 1,558 passed,
  33 skipped (51.68 seconds). `validation/hook-calls.json` records 4,720 hook-main
  entries and 44 direct hook-run entries. Callbacks and nested facade calls are
  recorded separately; reference child processes retain their Python entry points.
  `HookProbe.java` checks malformed UTF-8 before disable checks, unreadable input,
  scalar payloads, event precedence, Unicode progress, error boundaries, log-write
  failure and state permissions. Clippy, formatting and all 256 reference files
  pass. The native CLI and standalone hook workflow remain unverified.
- CLI-focused run: 106 passed. Full suite with native status, configuration and
  argument helpers: 1,558 passed, 33 skipped (50.57 seconds).
  `validation/cli-calls.json` records 47 status commands, two configuration commands,
  48 proposal filters and 104 argument-line expansions. Reference child processes
  retain their Python entry points.
  `CliProbe.java` compares human status and configuration output with the unchanged
  upstream CLI. It also checks rejected settings, I/O error types, relative write
  checks, quoted arguments, exact ledger counts above 2^53 and bounded overflow.
  Clippy, formatting and all 256 reference files pass. Standalone native CLI and
  hook workflows remain pending.
- Learning/repository CLI focused run: 92 passed, 1 skipped. Full suite with the
  expanded CLI substitutions: 1,558 passed, 33 skipped (52.94 seconds).
  `validation/learning-cli-calls.json` records 17 fact commands, seven history
  commands, ten repository commands and one explicit fold command. Reference child
  processes retain their Python entry points.
  `LearningCliProbe.java` compares fact, history, map, migration, abbreviation and
  date output with the unchanged upstream implementation. It also checks import
  outcomes, detached dispatch, explicit sessions, pin-record fallback, quiet
  migration, repository-list output and invalid merge keys.
  The status/configuration probe, Clippy, formatting and all 256 reference files
  pass. Real model calls and native Windows execution remain unverified.
- Tool-pruning CLI focused run: 140 passed, 13 skipped. Full suite with native
  tool-pruning command processing: 1,558 passed, 33 skipped (52.10 seconds).
  `validation/toolprune-cli-calls.json` records 14 tool-pruning commands. Reference
  child processes retain their Python entry points.
  `ToolpruneCliProbe.java` compares proposal output for Claude, Codex and Copilot
  with the unchanged upstream CLI, including stable streak ordering and its row
  limit. It checks apply/restore ordering, kept items, exception subclasses,
  measurement fallback, trust and verification notices, catalog reporting, waiter
  suppression, bounded sleeps, settings reload and cleanup after reported errors.
  The status/configuration probe, Clippy, formatting and all 256 reference files
  pass. Native executable interruption and native Windows remain unverified.
- Diagnostic/default-host focused run: 306 passed, 11 skipped. Full suite with the
  expanded CLI substitutions: 1,558 passed, 33 skipped (52.18 seconds).
  `validation/diagnostic-cli-calls.json` records three recent-host lookups; doctor
  subprocesses launched by the original tests retain their Python entry points.
  `DoctorProbe.java` compares complete native diagnostic output with unchanged
  upstream output for installed and absent agents. It checks trust and pause
  reporting, relative installation paths, malformed package records, read-only
  state, runtime-version gates, host-recency ties and filesystem errors, and the
  error-log tail. URL cases match the installed Python parser; the Unicode check
  normalises every scalar value and verifies all 19 authority-delimiter guards.
  The status/configuration probe, Clippy, formatting and all 256 reference files
  pass. Packaged native CLI/hook workflows and native Windows remain unverified.
- Probe-focused run: 5 passed. Full suite with probe substitutions: 1,558 passed,
  33 skipped (50.52 seconds). `validation/probe-calls.json` records the fixture
  setup, three command classifications, one session reply and two log inspections
  reaching Rust. The original fixture checks its pytest traps, passing unittest
  and clean Git status. Git operations retain the configured signing home while
  the original tests isolate agent history; the run used GPG agent access.
  `ProbeProbe.java` compares offline reports with upstream and checks the test
  adapter's report path, Unicode command boundaries, environment filtering, reply
  parsing, timeout propagation, log inspection, ties-to-even rounding, plugin
  discovery, fold deadlines, shell quoting, signed-commit arguments, no-session
  reporting, deny-list restoration and temporary-directory cleanup on failure.
  The CLI regression probe, Clippy, formatting and all 256 reference files pass.
  Paid probe sessions, native runtime integration and packaged workflows remain
  unverified. The original tests remain unchanged.
- CLI entry focused run: 78 passed, 8 skipped. Full suite with the native parser
  and entry path: 1,558 passed, 33 skipped (51.16 seconds).
  `validation/cli-entry-calls.json` records 99 entry calls and one parser-metadata
  request reaching Rust. `ParserProbe.java` compares 179 argument namespaces,
  help outputs and errors with the pinned CLI on Python 3.15, including aliases,
  option placement and abbreviations, repeated sessions, list arguments,
  terminators, invalid choices and suggestions, and widths from 10 to 200.
  It also checks Unicode, underscored, invalid and oversized `COLUMNS` values.
  `CliEntryProbe.java` checks default and explicit hosts, dispatch, return values,
  parse exits and state permissions after failures, including reported interrupts
  and relative state paths. The compatibility parser retains its creation-time
  working directory and returns argparse namespaces with native command wrappers.
  The repository-map generator probe passes after sharing Unicode integer
  normalisation. Clippy, formatting and all 256 reference files pass.
  The native executables, concrete service runtime and native interruption remain
  unverified; this does not complete the production-integration gate.
- Native runtime focused run: 107 passed. Original spawn/probe assertions:
  10 passed. Full suite: 1,558 passed, 33 skipped (54.82 seconds).
  `validation/native-runtime-calls.json` records three subprocess calls through
  the standalone Rust `te`; the original CLI repository-selection test checks
  quoted paths, JSON/explanation output and preservation of pending proposals.
  In-process calls retain the compatibility transport for patched dependencies.
  `NativeCliProbe.java` compares standalone command output with upstream and
  checks native configuration, deny-list application/restoration, kept tools,
  map-refresh failure, history, folding and repository services. JSON numbers are
  compared by value, accepting equivalent forms such as `0` and `0.0`.
  The native workflows run with only Git on PATH; reference comparisons invoke
  Python by its absolute path. A native fold persists once, and merging previously
  folded state launches a detached native refold and records its result. Runtime
  diagnostics report the native package version. The native worker consumes its
  launch specification, writes the child log and removes the specification.
  Missing-agent and missing-Python probe failures leave no temporary directory.
  Context/WMI and offline probe regression checks, Clippy, formatting and all
  256 reference files pass. Model services were not contacted. Native Windows,
  interruption handling and installed plugin/hook workflows remain unverified.
- Standalone native hook learning/resume checks: 2 passed, with 24 hook-process
  invocations recorded in `validation/native-hook-process-calls.json`. These
  original tests exercise learning, inline folding, next-session injection,
  once-only notices, resume and shadow behaviour with no agents on PATH.
  Safety and reference packaging regressions: 41 passed, 8 skipped. Full suite:
  1,558 passed, 33 skipped (56.36 seconds); `validation/native-hook-calls.json`
  records the default native substitutions, preserving dependency patches.
  `NativeHookProbe.java` compares seeded-fact output with upstream for all ten
  hosts, including Cursor and Grok payload translation and Antigravity expansion.
  It checks fail-open UTF-8/JSON input, state permissions, quoted native launchers,
  idempotent launcher writes, same-size executable updates and self-overwrite
  protection. Windows copy semantics are simulated on macOS. Native history
  diagnostics stay on stderr; failed map refreshes preserve cached context, prompt
  map lookup uses that cache, queued prompt/tool events replay in order, and a
  hook's detached native fold persists its ledger record. Native CLI regression
  checks, Clippy, formatting and all 256 pinned reference files pass. The original
  tests remain unchanged. Native Windows, paid model sessions, interruption and
  installed plugin workflows remain unverified; packaging is the next gate.
- Native bundle assembly focused run: 51 passed, 8 skipped. Full suite after
  timestamp preservation and legal-text integration: 1,558 passed, 33 skipped
  (236.66 seconds). An earlier packaging run completed in 54.55 seconds; these
  compatibility runs do not establish application performance.
  `validation/package-calls.json` records one builder subprocess and one direct
  Droid builder call reaching Rust. The transport retains the reference Python
  library as fixture data for the original bundle assertions; production
  `te-package` supplies native executables.
  `PackageProbe.java` checks marketplace and Droid layouts, command rewriting,
  unchanged hook events/deadlines, CLI path forms and abbreviations, invalid
  arguments, output overlap guards, rebuild boundaries and missing binary inputs.
  ZIP entries preserve file contents, executable permissions and modification
  times. Copied assets retain their modification times. The relocated bundle
  answers status commands for all ten hosts, handles a session-start payload and
  creates a working native CLI launcher with an empty PATH. The packaged map
  generator reports a missing interpreter through its native entry point.
  Windows command generation is checked structurally, including the decoded
  UTF-16LE command, native `.cmd` files and Git Bash wrappers; Windows binaries
  are simulated in that check. The bundle includes the upstream licence and
  legal texts verified against all 81 locked registry crates.
  Debug/release builds, Clippy, formatting and all 256 pinned reference files
  pass. Original tests remain unchanged. Native Windows and live SDK-host
  installation workflows remain unverified; SDK policy and experiment production
  conversion still precede the six improvement passes and final test conversion.
- Hermes gateway focused run: 67 passed, 6 skipped. Full suite with native
  gateway policy: 1,558 passed, 33 skipped (54.93 seconds).
  `validation/hermes-gateway-calls.json` records 176 gateway operations over
  28 bridge instances, six direct redirect checks and a held-lock check.
  The original assertions cover session selection, unreadable-row retries,
  compression aliases, bounded ended-session retention, prompt contexts,
  once-only notices, held redirects, per-repository concurrency, serial tool
  callbacks and lock deadlines. Original tests remain unchanged.
  `HermesGatewayProbe.java` compares the packaged binding with the upstream
  binding through the same original SQLite schema. Thread-bound profiles retain
  their homes and repositories, concurrent callbacks persist nine and eight tool
  calls in the respective turn summaries, compressed IDs reuse their root, and
  each repository folds once. Gateway snapshots retain no prompt text. The
  binding works after its gateway directory is removed and after its backend is
  restarted. Invalid requests, cancelled profile lookups, EOF and malformed
  backend output release callers; a subsequent SDK request remains usable.
  The Python fixture only supplies SDK, SQLite and JSON I/O. Rust owns the
  gateway decisions and invokes native hooks with an empty PATH.
  Marketplace/Droid regression checks, debug/release builds, Clippy, formatting,
  all 256 reference files and dependency notices pass. Native Windows, paid
  sessions and the real Hermes gateway's event scheduler remain unverified.
- Pi-focused original tests: 8 passed, 8 skipped. Full suite with native Pi
  carried-section calls: 1,558 passed, 33 skipped (54.21 seconds).
  `validation/pi-sdk-calls.json` records the original carried-section test
  reaching the native helper, alongside the existing native substitutions.
  `PiSdkProbe.java` compares the packaged TypeScript binding with the original
  extension in 62 callback cases against identical local hook I/O. It checks session sources,
  compaction and removal, repeated sections, prompt counters, overlapping prompts,
  steering, UI/stderr notices, scalar and malformed hook output, disable flags,
  numeric text coercion, command arguments, hook failures and a removed working
  directory. Protocol errors,
  EOF and malformed backend output release SDK callers. The SDK fixture only
  handles event, process and JSON I/O; all production policy remains in Rust.
  Native marketplace/Droid relocation checks, debug/release builds, Clippy,
  formatting, all 256 reference files and dependency notices pass. The original
  tests remain unchanged. Native Windows and a live Pi installation remain
  unverified.
- Full suite with Pi recovery snapshots: 1,558 passed, 33 skipped (53.64 seconds).
  `validation/pi-recovery-calls.json` records the original native substitutions.
  `PiSdkProbe.java` compares 76 callback cases with the original extension. Six
  forced backend kills cover prompts, tool results, resumed and cleared sessions,
  turn end and two simultaneously pending prompts. Callers receive the native
  fallback actions; subsequent callbacks retain the same manual and prompt IDs
  as the original binding. The hook call logs confirm recovery does not replay
  hook processes. A seventh kill followed by two malformed backend responses
  retains the last snapshot and resumes after the executable is restored.
  Restore validation rejects invalid counters and mid-session replacement;
  snapshots exclude prompt and environment values and remove completed requests.
  Callback barriers give the concurrency fixture a defined completion order.
  Native marketplace/Droid bundles, debug/release builds, Clippy, formatting,
  all 256 reference files and legal texts for 81 registry crates pass. No
  performance claim is made. Live Pi and Windows execution remain unverified.
- The three original opencode runtime/service tests pass against the native
  binding, hooks and CLI on opencode 2.3.0 and macOS. They use the original local
  response fixture; the fold retains the session's prompt. The test transport
  supplies native executables and a Rust console I/O probe while preserving the
  original assertions, service configuration and asynchronous folding.
  Full suite with these tests enabled: 1,561 passed, 30 skipped (74.55 seconds).
  `validation/opencode-sdk-calls.json` records one original runtime harness,
  two original service workflows and the native CLI bootstrap subprocess.
  `OpencodeSdkProbe.java` compares 108 callback cases across v1/v2 and Node/Bun.
  It covers session starts, repeated sections, turn IDs, tool-result coercion,
  redirects before notices, image preservation, failed tools, disabled hooks,
  toast fallback, message references, tokens, cost, repeated end signals,
  missing SDK registrations, subscription cleanup and synchronous exports.
  It also runs the original runtime harness under Bun and opencode's embedded
  Bun runtime. `OpencodeProtocolProbe.java` rejects unauthenticated control,
  invalid configuration and duplicate request IDs, then completes a synchronous
  exit while an SDK callback is waiting. Two Rust checks cover late session-start
  replies after exit and empty-copy export preservation, including file modes.
  Marketplace/Droid bundles, Pi's 76-case regression, debug/release builds,
  Clippy, formatting, all 256 reference files and the 81 registry crates' legal
  texts pass. Original tests remain unchanged. Native Windows and backend-crash
  recovery remain unverified or incomplete; no improvement pass is counted.
- opencode recovery now passes 184 callback comparisons across v1/v2 and Node/Bun,
  including 24 backend kills during hooks and idle periods. Manuals, counters,
  overlapping prompts, tool names, shared messages and synchronous exit exports
  survive replacement without replaying interrupted hooks. The original runtime
  harness also passes. Five Rust checks cover recovery validation, shared message
  references, incremental checkpoint contents, interrupted prompts, exit ordering
  and export preservation. The authenticated exit protocol passes with more than
  1 MB of recovery data; the live backend receives only its control request.
  The full original suite, including the three installed-opencode workflows,
  passes with 1,561 passed and 30 skipped (69.51 seconds).
  `validation/opencode-recovery-calls.json` records native coverage. Marketplace
  and Droid bundles, debug/release builds, Clippy, formatting and the 256-file
  reference check pass. Original tests remain unchanged.
  Native Windows execution remains unverified. No improvement pass is counted.
- Dataset-adapter comparisons pass in 127 cases across all four domains, covering
  environment defaults, explicit paths, constructor snapshots, selected IDs,
  duplicate selections, hidden entries, directories, globbed roots, task overrides,
  Unicode redaction, catalog sources, trace reading, tool exclusions and malformed
  input error types. Four prompt resources match the pinned source exactly.
  Relative dataset and transcript paths follow the calling process's directory;
  public path methods preserve their lexical paths.
  The original adapter, tooling, facade, vocabulary and registry tests pass:
  57 passed. A Rust integration check passes task prompts and protected tools
  through the native tooling stage without Python callbacks or model calls.
  Full original suite with installed-opencode tests enabled: 1,561 passed,
  30 skipped (64.80 seconds). `validation/experiment-adapters-calls.json` records
  native adapter entries from the original tests. All six Rust library checks,
  debug/release builds, Clippy, formatting and the 256-file reference check pass.
  Original tests remain unchanged; no improvement pass is counted.
  Dataset IDs currently follow their documented string schemas; other JSON key
  types and exact parser diagnostics remain in the parity audit.
- Codex spiral and shared-stage comparisons pass in 485 cases. They cover all
  deterministic nudge rules, scope clearing, missing thread IDs, per-reason budgets,
  continuation requests, lazy first-thread iteration, shell patch ordering, Unicode
  filtered-test boundaries and case folding, failure signatures, envelope learning,
  model-cost fallbacks, snapshot errors and combined telemetry. Eight nudge resources
  match the pinned source exactly. The original spiral, facade and optimizer-contract
  tests pass: 40 passed. The full original suite with installed-opencode tests enabled
  passes with 1,561 passed and 30 skipped (70.69 seconds).
  `validation/experiment-spiral-calls.json` records the native entries, including
  182 iterator calls, 94 live monitor calls and 17 stage post-task calls. All nine
  Rust library checks, debug/release builds, Clippy, formatting, the adapter's
  127-case regression and the 256-file reference check pass. Original tests remain
  unchanged. Integer-range and project-wide regex audits remain open; no improvement
  pass is counted. The experiment facade, generic nudge controller and task runners
  remain in Python.
- Generic nudge-controller comparisons pass in 88 cases. They cover judge cadence
  and productive backoff, callback failures and malformed replies, token costs,
  targeted and fallback nudges, empty/custom resume headers, continuation resets,
  batched stream results, cumulative hard backstops and verbose telemetry. The
  resume header matches the pinned source exactly. Batched callback replay retains
  the first unanswered request. The focused original tests pass: 52 passed.
  The full original suite with installed-opencode tests enabled passes with
  1,561 passed and 30 skipped (68.33 seconds). The original tests exercise the
  constructor; the differential probe and two Rust checks exercise the controller.
  `validation/experiment-nudge-calls.json` records the original suite's native
  entries. All 11 Rust library checks and the callback replay check pass, alongside
  debug/release builds, Clippy, formatting, the 485-case spiral regression, the
  127-case adapter regression and the 256-file reference check. Original tests
  remain unchanged. No improvement pass is counted. The facade and task runners
  remain in Python; numeric limits and malformed-input diagnostics remain in the
  production-parity audit.
- Experiment-facade comparisons pass in 87 cases. They cover bare and protocol
  runners, lazy factories, separate stage runners, every spiral callback flag
  combination, proposer cadence during construction and errors, default/custom
  adapters, all three injection profiles, persisted-state reloads, configuration
  changes, stage failure order, model costs and combined summaries. Codex
  `newly_pruned` telemetry now sorts its selected capabilities as the source does.
  A Rust check drives all three concrete stages and verifies their post-task order
  and saved state without Python. The focused original tests pass: 90 passed.
  The full original suite with installed-opencode tests enabled passes with
  1,561 passed and 30 skipped (68.58 seconds).
  `validation/experiment-facade-calls.json` records the original suite's native
  entries, including 27 facade post-task transport entries. All 12 Rust library
  checks, the callback replay check, debug/release builds, Clippy, formatting,
  the 485-case spiral regression, 127-case adapter regression, 88-case nudge
  regression and 256-file reference check pass. Original tests remain unchanged.
  No improvement pass is counted. Task runners and production-parity audits remain
  open.
- Harness usage accounting matches the source in 474 comparisons across all three
  schemas. They cover interrupted recovery, repeated snapshots, partial session
  logs, message-ID equality, large numeric IDs, counters above 2^53, cache writes,
  reasoning tokens, missing usage, distinct segment-merge rules, malformed-event
  diagnostics and log-directory reading with UTF-8 replacement and universal
  newlines, including relative paths after a working-directory change. Four Rust
  checks verify exact counters, unknown segment totals,
  recovery completeness and distinct large numeric message IDs. The focused
  original tests pass: 75 passed, 2 skipped.
  The full original suite with installed-opencode tests enabled passes with
  1,561 passed and 30 skipped (65.97 seconds).
  `validation/experiment-usage-calls.json` records the native entries, including
  20 recovery observations, 17 synthesized results and 14 Codex usage rows.
  All 16 Rust library checks, the callback replay check, debug/release builds,
  Clippy, formatting, the 87-case facade regression, 88-case nudge regression and
  256-file reference check pass. Original tests remain unchanged. Task execution
  and the production-parity audits remain open; no improvement pass is counted.
  Log filenames outside UTF-8 remain unverified on this Mac: the local filesystem
  rejects the fixture with `Illegal byte sequence`. The reader matches raw filename
  bytes without discarding such entries.

- The shared SWE-bench task lifecycle matches the source in 166 comparisons.
  They cover prompt text, container names, repeated and falsey guards, resumes,
  transcript lifetime, task artifacts, grading failures, all three row schemas,
  exception inheritance and cleanup ordering. The host retains one transcript
  handle across segments and closes it before grading or cleanup. Runtime
  callbacks use the existing worker channel without replaying operations.
  A Rust check also exercises resumed task execution and container-start failure.
  The full original suite with installed-opencode tests enabled passes with
  1,561 passed and 30 skipped (67.37 seconds).
  `validation/experiment-task-calls.json` records seven native container-name
  calls. The original grading tests inspect the source through wrapper metadata;
  they do not execute the native task lifecycle. The differential probe and Rust
  check provide that runtime evidence.
  All 17 Rust library checks, the callback replay check, debug/release builds,
  Clippy, formatting, the 474-case usage regression and 256-file reference check
  pass. Original tests remain unchanged. Container, scaffold, grader and stream
  callbacks still use the reference implementation outside the probe's local
  stand-ins. No Docker, paid model or live grading calls were run. Production
  conversion and parity audits remain open; no improvement pass is counted.

- Grading orchestration extends the task probe to 262 source comparisons. They
  cover empty patches, zero and nonzero command exits, empty and partial output,
  timeout subclasses, malformed UTF-8 bytes, stderr truncation by character,
  failed diagnostic writes, grader exceptions, `KeyboardInterrupt` and `SystemExit`.
  Combined task cases exercise native grading inside resumed execution, including
  nested callbacks and cleanup. Relative paths are checked after changing the working
  directory. A Rust check verifies retained timeout output and score recovery.
  All 18 Rust library checks, the callback replay check, debug/release builds,
  Clippy, formatting, the 474-case usage regression and 256-file reference check
  pass. The full original suite with installed-opencode tests enabled passes with
  1,561 passed and 30 skipped (66.73 seconds). Its native task entries still cover
  container naming; grading runtime coverage comes from the task probe and Rust
  check. The call record remains `validation/experiment-task-calls.json`.
  Original tests remain unchanged. Container commands and the official-grader
  integration still use reference callbacks outside the probe's local stand-ins.
  No live Docker or official grading run was performed. The production-parity
  audits and remaining runner conversion stay open; no improvement pass is counted.

- The official-grader wrapper matches the source in 92 exact comparisons. They
  cover grading paths, table loading with universal newlines, JSON-encoded test
  lists, malformed row structures, prediction arguments, report selection,
  override dispatch and nested task execution. Missing-file and directory errors
  preserve errno and filename on this Mac; UTF-8 errors preserve their bytes,
  offsets and reasons. Paths containing NUL raise `ValueError` before file access.
  Artifact comparisons use raw bytes. A Rust check verifies row normalisation
  without changing the loaded table.
  Three additional checks preserve the JSON exception type and document but
  expose different wording or positions. The probe records both outputs in
  `validation/results/grader-json-diagnostics.json`; those checks do not establish
  diagnostic parity. JSON diagnostics and callback reference sharing remain open.
  The full original suite with installed-opencode tests enabled passes with
  1,561 passed and 30 skipped (66.12 seconds). Its task coverage exercises container naming;
  grader-wrapper execution is checked by the new probe. All 19 Rust library
  checks, the callback replay check, debug/release builds, Clippy, formatting,
  the 262-case task regression and 256-file reference check pass. Original tests
  remain unchanged. The external SDK uses local stand-ins; no live Docker or
  official grading run was performed. No improvement pass is counted.

- SWE-bench settings and provenance match the source in 196 comparisons. They cover
  CPU quotas and matching thread caps, Unicode decimal digits, large finite
  limits, infinity errors, environment selection, activation variables, network
  masks, manual fingerprints and scaffold exclusions. Configuration files match
  byte for byte, including Unicode and `default=str` values. Metadata reads,
  default attributes, hook failures, file errors and configuration recording
  inside task execution preserve the observed call order.
  The focused original tests pass with 46 passed. Their call record confirms four
  native environment selections, two activation environments, nine network-block
  checks and seven container names. The validation import hook installs harness
  bindings when modules finish importing, so these tests exercise native helpers
  when they import the harness inside test functions.
  The full original suite with installed-opencode tests enabled passes with
  1,561 passed and 30 skipped (67.30 seconds). Its call record,
  `validation/experiment-task-calls.json`, includes eleven native network-block
  checks, four environment selections and two activation environments. The
  configuration writer is exercised by the differential probe; the original
  configuration test inspects the reference AST.
  All 20 Rust library checks, the callback replay check, debug/release builds,
  Clippy, formatting, the 262-case task regression, 92-case grader regression and
  256-file reference check pass. The grader probe still records three JSON
  diagnostic differences separately. Original tests remain unchanged. Windows
  path semantics, non-string tool/path schemas and object-conversion side effects
  remain in the production-parity audit. No Docker, paid model or live grading
  calls were run. No improvement pass is counted.

- Per-task container orchestration matches the source in 292 comparisons across
  Claude, Copilot and Codex. They check command arguments and ordering, mounts,
  seed setup, environment discovery, verification, recorded activation, live
  tracking and cleanup. The probe starts with empty live tracking when checking
  startup failures. It verifies ordinary errors, timeout errors,
  `KeyboardInterrupt` and `SystemExit`, including failures in mounts and Codex
  configuration callbacks. Returned process objects, command arguments and stored
  activation dictionaries retain their host identity. Files match byte for byte.
  Two Rust checks verify cleanup after ordinary errors, fatal exception
  propagation and rejection of unknown agent labels before any runtime call.
  The focused original tests pass with 79 passed and a recorded native stop call.
  The full original suite with installed-opencode tests enabled passes with
  1,561 passed and 30 skipped (65.93 seconds). Its call record remains
  `validation/experiment-task-calls.json`. Container execution coverage comes
  from the new process-stand-in probe; original tests mainly inspect runner source.
  All 22 Rust library checks, the callback replay check, debug/release builds,
  Clippy, formatting, the 196-case settings regression, 262-case task regression,
  92-case grader regression and 256-file reference check pass. The three recorded
  grader JSON diagnostic differences remain open. Original tests remain unchanged.
  No live Docker, model, official grading or OS-signal run was performed. Native
  Windows and the remaining production-parity audits are open. No improvement
  pass is counted.

- Codex configuration matches the source in 466 comparisons. They cover all 64
  supported capability masks with three network settings, protected-capability
  rejection, exact TOML bytes, MCP argument and environment conversion, skill
  ordering, synthetic prompts and the captured 0.153.1 prompt. Operational
  paragraphs survive trimming. Configuration write commands and failure text
  match; scaffold errors, fatal exceptions, falsey scaffolds, stale MCP reset and
  notes/server identity also match through local SDK and process stand-ins.
  The focused original tests pass with 40 passed and 2 skipped. The full original
  suite with installed-opencode tests enabled passes with 1,561 passed and 30
  skipped (68.12 seconds). Its call record in `validation/experiment-task-calls.json`
  includes 19 native TOML calls, five toggle calls, one skill-entry call, 14 prompt
  trimming calls and two Codex scaffold calls. Write-command and base-scaffold
  runtime coverage comes from the new probe. All 23 Rust checks, debug/release
  builds, Clippy, formatting, the four existing runner probes and the 256-file
  reference check pass. The three recorded grader JSON diagnostic differences
  remain open. Original tests remain unchanged. No live Docker, model, installed
  Codex strict-config or OS-signal run was performed. Production-parity audits and
  runner conversion remain open; no improvement pass is counted.

- Copilot and Codex task wrappers and scaffold measurements match the source in
  178 comparisons. They cover task argument and return-object identity, per-task
  state resets, capability conversion, partial row updates when token conversion
  fails, debug-file bytes and write errors, shell wrappers, RTK prefixes, MCP
  completion and invocation shapes, unknown-item samples, missing transcripts,
  decoding replacement and universal line endings. The task lifecycle probe now
  passes 282 comparisons, including 20 new cases that run both wrappers through
  native task execution, resumed segments and cleanup failures. No model or Docker
  process is used by these comparisons.
  The focused original tests pass with 51 passed and 2 skipped. The full original
  suite with installed-opencode tests enabled passes with 1,561 passed and 30
  skipped (68.45 seconds). Native call counts remain in
  `validation/experiment-task-calls.json`; wrapper execution coverage comes from
  the new probes. All 23 Rust checks, debug/release builds, Clippy, formatting,
  the existing configuration/container/settings/grader probes and the 256-file
  reference check pass. The three recorded grader JSON diagnostic differences
  remain open. Original tests remain unchanged. No live Docker, model or OS-signal
  run was performed. Production conversion and parity audits remain open;
  simplification and performance passes remain at zero.

- The native answer-key guard matches the source in 417 comparisons. They cover
  payload priority, bounded recursive lookup, scalar and container command values,
  dotted/dotless I, hosts entries, lift settings, exact refusal and telemetry bytes,
  append failures, truncation and 24 concurrent refusals sharing one log. Both the
  standalone executable and imported entry point are exercised. Transcript attempt
  counts distinguish missing files from clean traces and preserve argv conversion.
  The input reader handles non-finite numbers, float and large-integer spelling,
  escaped surrogates, surrogate-escaped stdin bytes, malformed JSON and 5,000-level
  unused metadata. Parsing, formatting and destruction use heap stacks. Integer
  digit limits match the default and tested `PYTHONINTMAXSTRDIGITS` settings.
  A Rust check verifies that lifting does not read stdin and read failures allow
  the call. The focused original tests pass with 102 passed and 2 skipped. The full
  original suite with installed-opencode tests enabled passes with 1,561 passed and
  30 skipped (70.59 seconds). Native call counts remain in
  `validation/experiment-task-calls.json`. All 24 Rust checks, debug/release builds,
  Clippy, formatting, the 178-case wrapper, 282-case task, 466-case configuration
  regressions and 256-file reference check pass. The three recorded grader JSON
  diagnostic differences remain open. Original tests remain unchanged. No live
  Docker, agent hook invocation, DNS or OS-signal verification was performed.
  Native Windows, non-default input encodings and the remaining production-parity
  audits remain open; no improvement pass is counted.

- Guard integration passes 291 source comparisons and two native checks. They
  cover DNS command order, resolver failure, ordinary and fatal errors, refusal
  payloads, exact decimal counts and configured integer limits, lazy patch
  exclusions, hook entries for three agents, idempotency, malformed configuration,
  Unicode, non-finite values, metadata preservation and unchanged files on schema
  errors. The native checks reject unknown agents before creating directories and
  prevent duplicate entries for the native executable name.
  Two source comparisons run the DNS installation in an isolated Docker container
  using the local `eclipse-temurin:25-jdk-noble` image (`95e350ad1eb7`, Linux arm64).
  Network access is disabled. `/etc/hosts` bytes match the source, `github.com`
  resolves to `127.0.0.1`, and repeated installation preserves the file. The probe
  removes its container. This does not verify an agent invoking the native hook or
  deployment into an official SWE-bench image.
  The focused original tests pass with 79 passed. The full original suite with
  installed-opencode tests enabled passes with 1,561 passed and 30 skipped
  (71.19 seconds). Its call record remains `validation/experiment-task-calls.json`;
  runtime coverage of the new runner methods and merger comes from the new probe.
  All 24 Rust checks, debug/release builds, Clippy, formatting, the 417-case guard,
  282-case task and 196-case settings regressions and the 256-file reference check
  pass. The three grader JSON diagnostic differences remain open. Original tests
  remain unchanged. Linux guard binary packaging, actual agent hook invocation,
  native Windows and the remaining production-parity audits are still pending.
  No simplification or performance pass is counted.

- Runner controller integration passes 196 source comparisons, including 18 flows
  through the real controllers. They cover all three SWE-bench agents, constructor
  keyword order, eager default evaluation, budget conversion and configured integer
  limits, optional parameters, exception classes, continuation arguments and
  telemetry access order. Identity assertions cover policies, callbacks,
  constructor results, continuation results and telemetry events. The real-controller
  comparisons normalise integral floating-point JSON spelling; they do not prove
  numeric type parity.
  Focused original tests pass with 73 passed and two skipped. The full original
  suite, with installed-opencode tests enabled, passes with 1,561 passed and 30
  skipped (69.36 seconds). `validation/experiment-task-calls.json` records four
  native `guard_accepts` calls; the new probe supplies runtime coverage of runner
  construction, continuation and telemetry.
  The 282-case task, 178-case wrapper and 88-case nudge regressions, all 24 Rust
  checks, debug/release builds, Clippy, formatting and the 256-file reference check
  pass. IDE inspections are unavailable because this checkout is not open in the
  connected IDE. Original tests remain unchanged. No paid model, live agent or
  container run was performed for this change. Production conversion and parity
  audits remain open; no simplification or performance pass is counted.

- Codex installation policy passes 240 source comparisons. They cover binary
  overrides and PATH lookup, sorted vendor selection, missing files, symlinks,
  version output, captured prompts, error classes, cleanup retries, probe deadlines
  and configuration diagnostics. Seven comparisons use the real filesystem:
  three captured-prompt lookups, vendor discovery through a symlink, and three
  offline process probes. A Java CLI fixture installs two skills, emits a strict
  configuration error or waits past the timeout. Both source and native probes
  terminate its child process and remove their temporary home on macOS.
  Focused original tests pass with 69 passed and two Linux-only tests skipped.
  The full original suite, with installed-opencode tests enabled, passes with
  1,561 passed and 30 skipped (67.07 seconds). Its record is
  `validation/experiment-task-calls.json`, including one native bundled-skills probe,
  two configuration validations, three prompt lookups and their cleanup calls.
  The 466-case Codex configuration regression, all 24 Rust checks, debug/release
  builds, Clippy, formatting and the 256-file reference check pass. IDE inspections
  remain unavailable because this checkout is not open in the connected IDE.
  Original tests remain unchanged. Native Windows and Linux process-group behaviour,
  experiment asset packaging, runner conversion and the remaining parity audits
  are still pending. No simplification or performance pass is counted.

- SWE-bench constructor coordination passes 483 source comparisons, including 27
  flows through the real routing, pricing and model-normalisation helpers. The
  comparisons check all three agents, partial state after failures, environment
  precedence, effort levels, exact integers and digit limits, scaffold profiles,
  object identity, skill defaults, configuration errors and model verification.
  A prompt callback replaces runner fields and the configuration function; the
  constructor retains the callable and arguments evaluated before that callback.
  Three additional flows complete constructor setup through the actual Codex
  prompt and configuration implementations, exercising nested native callbacks.
  The integer digit-limit check is shared with controller budget conversion.
  Focused original tests pass with 83 passed and two skipped. The full original
  suite, with installed-opencode tests enabled, passes with 1,561 passed and 30
  skipped (67.88 seconds). `validation/experiment-task-calls.json` records three
  native Codex constructor entries. Those original calls stop at credential
  validation; the new probe supplies runtime coverage of the remaining setup.
  The 196-case controller and 466-case Codex configuration regressions, all 24 Rust
  checks, debug/release builds, Clippy, formatting and the 256-file reference check
  pass. Original tests remain unchanged. IDE inspections remain unavailable because
  this checkout is not open in the connected IDE. Constructor preflights use SDK
  stand-ins; no live provider or agent run was performed for this change. Provider
  helpers, factories, global cleanup, streaming and the remaining parity audits
  stay open. No simplification or performance pass is counted.

- Provider policy passes 266 source comparisons across fifteen helper functions.
  They cover explicit and process environments, credential identity and in-place
  mutations, mapping access order, auth precedence, provider secrets by name,
  model normalisation, effort applicability, ordinary and fatal errors, HTTP
  context cleanup, catalogue cache identity, sorting and the twelve-model sample.
  The shared JSON reader handles non-finite metadata, integer digit limits,
  5,000-level metadata and escaped surrogate IDs. Model-verification messages
  preserve those IDs through the binding's JSON codec.
  A Java HTTP server verifies two real loopback requests with the expected
  authorization header; subsequent source and native cache hits make no request
  and retain the frozen catalogue object.
  Focused original tests pass with 56 passed and two skipped. The full original
  suite, with installed-opencode tests enabled, passes with 1,561 passed and 30
  skipped (69.39 seconds). `validation/experiment-task-calls.json` records the
  provider calls, including three catalogue reads and three model verifications.
  The 483-case constructor and 417-case guard regressions, 289 guard-integration
  comparisons and two native checks, all 24 Rust checks, debug/release builds,
  Clippy, formatting and the 256-file reference check pass. Original tests remain
  unchanged. No external provider, paid model or Docker run was performed for this
  change. IDE inspections remain unavailable because this checkout is not open in
  the connected IDE. Exact syntax-error diagnostics for custom HTTP contexts,
  remaining provider calls, runner conversion and the parity audits stay open.
  No simplification or performance pass is counted.

- Shared endpoint preflights pass 241 source comparisons. They cover exact gate
  values, opaque argument identity, keyword order, sixteen-token calls, response
  truthiness and stripping, malformed results, ordinary and fatal errors, deferred
  logger capture, callback replacement, ignored logger results and cost formatting.
  Optimizer requests use stand-ins; no provider or paid model is called.
  JSON HTTP-context checks pass 392 source comparisons on CPython 3.15.2, covering
  valid values, incomplete numbers, delimiters, trailing commas, escapes, control
  characters, BOMs and Unicode positions. The grader passes 127 comparisons,
  including 35 strict JSON diagnostic comparisons. The three earlier differences
  are now failures if they recur; `validation/results/grader-json-diagnostics.json`
  contains an empty list after the passing run.
  Focused original tests pass with 72 passed and two skipped. The full original
  suite, with installed-opencode tests enabled, passes with 1,561 passed and 30
  skipped (63.89 seconds). Its record remains
  `validation/experiment-task-calls.json`. The original suite does not enter the
  new native preflight branches; the new probe supplies their runtime coverage.
  The 267-case provider and 417-case guard regressions, 289 guard-integration
  comparisons and two native checks, all 24 Rust checks, debug/release builds,
  Clippy, formatting and the
  256-file reference check pass. Original tests remain unchanged. IDE inspections
  remain unavailable because this checkout is not open in the connected IDE.
  Live provider checks, optimizer transports, runner conversion and the remaining
  parity audits stay open. No simplification or performance pass is counted.

- HTTP optimizer calls pass 235 source comparisons, including four real loopback
  HTTP requests. They cover exact request bytes and headers, credential precedence,
  request reuse, retry codes and delays, HTTP-body truncation, response extraction,
  malformed responses, suppressed context errors, ordinary and fatal exceptions,
  logger replacement, observer mutations, Unicode and arbitrary-size counters.
  The 267-case provider and 241-case preflight regressions also pass.
  Focused original tests pass with 72 passed and two skipped. The full original
  suite, with installed-opencode tests enabled, passes with 1,561 passed and 30
  skipped (68.06 seconds). Its record is
  `validation/experiment-task-calls.json`; it includes three Messages calls, one
  Responses call and one Responses text-extraction call. All 24 Rust checks,
  debug/release builds, Clippy, formatting and the 256-file reference check pass.
  Original tests remain unchanged. IDE inspections remain unavailable because
  this checkout is not open in the connected IDE. No external provider or paid
  model was called. Copilot CLI optimizer calls, shared optimizer helpers, runner
  conversion and the remaining parity audits stay open. No simplification or
  performance pass is counted.

- Copilot CLI optimizer coordination passes 164 source comparisons. They cover
  exact arguments and environment merging, denial and exclusion flags, model
  conversions, callable replacement, cleanup failures, JSON events, Unicode line
  boundaries, lone surrogates, opaque results, observer mutations and ordinary
  and fatal exceptions. Offline Java subprocess fixtures check successful output,
  timeout and launch failure, including the source's directory cleanup outcomes.
  The 235-case HTTP optimizer and 267-case provider regressions also pass.
  The full original suite, with installed-opencode tests enabled, passes with
  1,561 passed and 30 skipped (68.74 seconds). Its record remains
  `validation/experiment-task-calls.json`. The original suite does not execute
  the Copilot optimizer call; the new probe supplies its runtime coverage.
  All 24 Rust checks, debug/release builds, Clippy, formatting and the 256-file
  reference check pass. Original tests remain unchanged. IDE inspections remain
  unavailable because this checkout is not open in the connected IDE. No external
  provider or paid model was called. Copilot host helpers, shared optimizer
  helpers, runner conversion and the remaining parity audits stay open.
  No simplification or performance pass is counted.

- Copilot host helpers pass 238 source comparisons, including real symlink
  resolution, home-location discovery, PATH lookup and offline Java version
  execution. They cover discovery precedence, missing install paths, optional
  container seeds, opaque credentials, environment override order, stdout/stderr
  selection, Unicode truncation and line boundaries, foreign string protocols,
  ordinary and fatal exceptions, and exact exception matching by inheritance.
  The 164-case Copilot optimizer and 483-case constructor regressions also pass.
  The full original suite, with installed-opencode tests enabled, passes with
  1,561 passed and 30 skipped (65.93 seconds). Its record remains
  `validation/experiment-task-calls.json`. All 24 Rust checks, debug/release
  builds, Clippy, formatting and the 256-file reference check pass. Original tests
  remain unchanged. IDE inspections remain unavailable because this checkout is
  not open in the connected IDE. No external provider or paid model was called.
  Shared optimizer helpers, runner conversion and the remaining parity audits
  stay open. No simplification or performance pass is counted.

- Optimizer metadata passes 211 source comparisons. They cover explicit and
  derived log paths, cell keys, Web-Bench aliases, context identity and replacement,
  removal of `None` values while retaining falsey values, all nine prompt
  signatures, Unicode slicing, malformed collections, foreign protocols and
  exception propagation. Path checks also verify captured callables when nested
  path operations replace their neighbouring functions.
  The 235-case HTTP optimizer regression passes. The full original suite, with
  installed-opencode tests enabled, passes with 1,561 passed and 30 skipped
  (69.23 seconds). Its record remains `validation/experiment-task-calls.json`;
  it includes two context updates, five log-path selections and four prompt
  classifications. The new probe supplies cell-key coverage.
  All 24 Rust checks, debug/release builds, Clippy, formatting and the 256-file
  reference check pass. Original tests remain unchanged. IDE inspections remain
  unavailable because this checkout is not open in the connected IDE. No external
  provider or paid model was called. Shared log writing, runner conversion and
  the remaining parity audits stay open. No simplification or performance pass
  is counted.

- Optimizer logging passes 290 source comparisons. They cover exact JSON rows,
  context overrides and mutations, numeric dictionary keys, cyclic usage values,
  Unicode truncation at 400 and 4,000 characters, character counts, file-context
  descriptors and lookup order, ignored return values, suppressed exceptions,
  ordinary and fatal failures, deferred exception formatting, verbosity gates,
  Web-Bench aliases, and real file appends and stdout printing.
  A failing source comparison reproduced adjacent surrogates collapsing into one
  character through JSON string encoding. The corrected shared binding uses
  code point arrays. The metadata corpus now passes 217 comparisons and Copilot
  host helpers pass 243, including surrogate-pair truncation and classification
  boundaries. HTTP optimizer and Copilot optimizer regressions pass 235 and 164
  comparisons respectively.
  The full original suite, with installed-opencode tests enabled, passes with
  1,561 passed and 30 skipped (69.65 seconds). Its record remains
  `validation/experiment-task-calls.json`; it includes five native log-record
  calls and twenty native verbosity calls. All 24 Rust checks, debug/release
  builds, Clippy, formatting and the 256-file reference check pass. Original tests
  remain unchanged. IDE inspections remain unavailable because this checkout is
  not open in the connected IDE. No external provider or paid model was called.
  Runner conversion, host bindings and the remaining parity audits stay open.
  No simplification or performance pass is counted.

- Runner call wrappers and factories pass 914 source comparisons. They cover all
  six optimizer methods, five owned preflight methods and inherited preflight
  behaviour, API/CLI selection, model labels and overrides, malformed endpoints,
  repeated endpoint getters, callable replacement, opaque identities, keyword
  order, ignored results, factory defaults, scaffold support, Unicode registry
  keys and ordinary and fatal failures. Twelve comparisons run the actual
  optimizer and preflight helpers with HTTP I/O and pricing fixtures. Web-Bench
  bindings are installed after SWE-bench and a repeated install preserves method
  identity.
  The full original suite, with installed-opencode tests enabled, passes with
  1,561 passed and 30 skipped (67.41 seconds). Its record remains
  `validation/experiment-task-calls.json`; it includes four SWE-bench factory
  calls and six Web-Bench factory calls. The new probe supplies the optimizer
  and preflight method coverage. The 483-case constructor and 241-case shared
  preflight regressions, all 24 Rust checks, debug/release builds, Clippy,
  formatting and the 256-file reference check pass. Original tests remain
  unchanged. IDE inspections remain unavailable because this checkout is not open
  in the connected IDE. No external provider or paid model was called.
  Web-Bench constructors and task calls, signal cleanup, host bindings and the
  remaining parity audits stay open. No simplification or performance pass is
  counted.

- Web-Bench setup passes 852 source comparisons across three constructors and
  the shared stream-state method. They cover partial state, getter and setter
  order, dependency failures, early scaffold rejection, profile/preflight
  callable capture, effort differences, API/CLI endpoint selection, timeout
  defaults, retry clamping, arbitrary-size integers, digit limits, non-finite
  and malformed values, foreign numeric protocols, subclass stream overrides,
  fresh stream lists and ignored helper results. Twenty-seven comparisons use
  the actual credential, provider, normalisation and pricing helpers with fixture
  environments. No provider or paid model is called.
  The full original suite, with installed-opencode tests enabled, passes with
  1,561 passed and 30 skipped (68.83 seconds). Its record remains
  `validation/experiment-task-calls.json`; it includes four Codex Web-Bench
  constructor calls. The new probe supplies the other constructor paths and
  stream-state coverage. The 483-case SWE-bench constructor and 914-case runner
  call regressions, all 24 Rust checks, debug/release builds, Clippy, formatting
  and the 256-file reference check pass. Original tests remain unchanged. IDE
  inspections remain unavailable because this checkout is not open in the
  connected IDE. Web-Bench host/configuration helpers, task lifecycle, signal
  cleanup, host bindings and the remaining parity audits stay open.
  No simplification or performance pass is counted.

- Web-Bench host helpers pass 680 source comparisons covering repository and Node
  discovery, environment fallback and lookup order, abort and timeout bounds,
  preload options and URI encoding, namespace return codes and exception handling.
  Cases include arbitrary-size integers, digit limits, foreign object protocols,
  captured path callables, real symlinks and PATH lookup. An offline Java process
  fixture checks the namespace probe's exact arguments and successful and failing
  exits; it does not execute Linux namespace isolation.
  The constructor corpus passes 861 comparisons, including nine cases through
  these helpers. The 914-case runner and 235-case HTTP regressions also pass.
  The full original suite, with installed-opencode tests enabled, passes with
  1,561 passed and 30 skipped (71.64 seconds). Its updated call record is
  `validation/experiment-task-calls.json`: task timeout runs seven times, abort
  policy thirteen times and evaluator environment assembly four times. The focused
  probe supplies discovery and namespace coverage. All 24 Rust checks, debug and
  release builds, Clippy, formatting and the 256-file reference check pass.
  Original tests remain unchanged. IDE inspections remain unavailable because
  this checkout is not open in the connected IDE. Live providers, Linux namespace
  isolation, Web-Bench task lifecycle, signal cleanup, host bindings and the
  remaining parity audits stay open. Simplification and performance passes
  remain at 0/3 each; final test conversion is pending.

- Web-Bench stream coordination passes 519 source comparisons across the three
  runner classes. They cover task iteration, project fallback, unchanged project
  references, real project-directory checks, evaluator arguments, result-root
  fallback, session ownership, close overrides, exception timing and partial state.
  Cases also cover callback replacement, arbitrary-size reply-timeout arithmetic,
  foreign object protocols and adjacent surrogate code points.
  Evaluator creation uses a recording fixture; no evaluator or paid model runs.
  The full original suite, with installed-opencode tests enabled, passes with
  1,561 passed and 30 skipped (70.96 seconds). Its updated call record remains
  `validation/experiment-task-calls.json`. Original tests do not call the new
  stream methods; the focused corpus supplies their runtime coverage.
  The 680-case host-helper and 861-case constructor regressions pass. All 24 Rust
  checks, debug/release builds, Clippy, formatting and the 256-file reference check
  pass. IDE inspections remain unavailable because this checkout is not open in
  the connected IDE. The evaluator lifecycle, task calls, agent streaming, host
  bindings and remaining parity audits stay open. Original tests remain unchanged;
  simplification and performance passes remain at 0/3 each.

- Web-Bench evaluator waits pass 272 source comparisons for event and request
  queues: deadlines, clock changes, polling bounds, empty-queue retries, completion
  checks, exit events, skipped-event logs, repeated mapping reads and raw result
  identities. Cases cover foreign numeric and mapping protocols, non-finite
  values, callback replacement, adjacent surrogate code points and exception
  boundaries. Real local queues cover successful reads and timeout handling.
  The shared SDK binding also preserves Python's invalid-argument messages and
  exception context; invalid calls fail before the original function body runs.
  The full original suite, with installed-opencode tests enabled, passes with
  1,561 passed and 30 skipped (68.27 seconds). Its updated call record remains
  `validation/experiment-task-calls.json`. Original tests do not call these waits;
  the focused corpus supplies their runtime coverage. The 519-case stream,
  914-case runner and 235-case HTTP regressions pass, as do all 24 Rust checks,
  debug/release builds, Clippy, formatting and the 256-file reference check.
  Original tests remain unchanged. IDE inspections remain unavailable because
  this checkout is not open in the connected IDE. Evaluator startup, stdout
  parsing, process cleanup, task calls, host bindings and remaining parity audits
  stay open. No live evaluator or provider is called. Simplification and
  performance passes remain at 0/3 each; final test conversion is pending.

- Web-Bench stdout processing passes 535 source comparisons covering attempt
  prefixes, project and task names, whitespace, Unicode decimal digits, ordinal
  conversion and digit limits, retained line references, foreign matchers and
  group results, callback replacement, log-write order, event order and partial
  state after exceptions. The source corpus exposed the Unicode 17 Tolong Siki
  digit gap in the installed regex tables; the evaluator pattern now handles it.
  An offline Java process checks real stdout reads, log writes and both successful
  and failing exits. It does not start Web-Bench or call a provider.
  The full original suite, with installed-opencode tests enabled, passes with
  1,561 passed and 30 skipped (73.66 seconds). Its updated call record remains
  `validation/experiment-task-calls.json`. Original tests do not call the stdout
  method; the focused corpus supplies its runtime coverage. The 272-case evaluator
  wait and 519-case stream regressions pass, as do all 24 Rust checks, debug/release
  builds, Clippy, formatting and the 256-file reference check.
  Original tests remain unchanged. IDE inspections remain unavailable because
  this checkout is not open in the connected IDE. Evaluator startup and task-file
  loading, process and signal cleanup, task calls, host bindings and remaining
  parity audits stay open. Simplification and performance passes remain at 0/3
  each; final test conversion is pending.

- Web-Bench task-file loading passes 217 source comparisons covering path capture,
  file and iteration errors, JSON values and diagnostics, integer digit limits,
  Unicode and adjacent surrogate code points, blank-line filtering, opaque IDs,
  context lookup order, unsupported context managers, suppression, handled
  exception state and exception chains. Real files cover mixed line endings,
  UTF-8 decoding, invalid bytes and missing paths. SDK bindings provide JSON
  library decoding and preserve the decoded ID objects.
  The shared context-exit correction also passes 307 optimizer-logging
  comparisons, including new exception-state and missing-special-method cases.
  The full original suite, with installed-opencode tests enabled, passes with
  1,561 passed and 30 skipped (88.61 seconds). Its updated call record remains
  `validation/experiment-task-calls.json`. Original tests do not call the task-file
  loader; the focused corpus supplies its runtime coverage. The 535-case stdout
  and 272-case evaluator-wait regressions, all 24 Rust checks, debug/release
  builds, Clippy, formatting and the 256-file reference check pass.
  Original tests remain unchanged. IDE inspections remain unavailable because
  this checkout is not open in the connected IDE. Evaluator startup, process
  and signal cleanup, task calls, host bindings and remaining parity audits stay
  open. Simplification and performance passes remain at 0/3 each; final test
  conversion is pending.

- Web-Bench evaluator startup passes 413 source comparisons covering initial
  fields, queue and thread setup, handler class ownership, configuration fields
  and order, full/subset task ranges, warning text, retained IDs, helper capture,
  environment updates, missing entry points, launch arguments, registration and
  partial state after failures. Cases include foreign protocols, callback
  replacement, arbitrary-size integers, digit limits and context suppression.
  Two offline Java evaluator runs use real files, threads and a loopback HTTP
  server. They load the native task list, receive an agent request, send a fixture
  reply and read Started/verdict lines through the native reader. The queue waits
  consume both events and observe the evaluator exit. The second run uses a task
  subset and larger undici abort. Both verify
  a separate process group; a third case checks a real executable launch failure.
  The source HTTP request handler supplies that part of the integration. No
  Web-Bench/Playwright evaluator or paid model is called.
  The full original suite, with installed-opencode tests enabled, passes with
  1,561 passed and 30 skipped (77.66 seconds). Its updated call record remains
  `validation/experiment-task-calls.json`. Original tests do not construct evaluator
  sessions; the focused corpus supplies that runtime coverage. The 519-case stream,
  217-case task-loader and 535-case stdout regressions pass, as do all 24 Rust
  checks, debug/release builds, Clippy, formatting and the 256-file reference check.
  Original tests remain unchanged. IDE inspections remain unavailable because
  this checkout is not open in the connected IDE. HTTP request policy, process
  and signal cleanup, task calls, host bindings and remaining parity audits stay
  open. Simplification and performance passes remain at 0/3 each; final test
  conversion is pending.

- Web-Bench HTTP policy passes 269 source comparisons covering content lengths,
  integer digit limits, UTF-8 and JSON errors, raw request/reply identities, reply
  timeouts, response bytes and headers, quiet logging and exception boundaries.
  Cases distinguish timeout handling from enqueue and encoding failures, and
  final-write broken pipes from failures while sending headers. Getters,
  formatters and loggers observe the active exception; formatter/logger failures
  retain their exception chains. Error text preserves adjacent surrogate code
  points. Seven real local HTTP requests cover successful responses, timeout
  fallback, malformed JSON, invalid UTF-8, invalid lengths and Unicode replies.
  The 413-case startup corpus now runs its combined offline pipeline through the
  native HTTP handler, task loader, queue waits and stdout reader. Its two real
  HTTP/process runs and executable launch-failure case pass. No Web-Bench/Playwright
  evaluator or paid model is called.
  The full original suite, with installed-opencode tests enabled, passes with
  1,561 passed and 30 skipped (70.68 seconds). Its updated call record remains
  `validation/experiment-task-calls.json`. Original tests do not call the evaluator
  HTTP methods; the focused corpora supply their runtime coverage. The 217-case
  task-loader and 307-case logging regressions, all 24 Rust checks, debug/release
  builds, Clippy, formatting and the 256-file reference check pass.
  Original tests remain unchanged. IDE inspections remain unavailable because
  this checkout is not open in the connected IDE. Process and signal cleanup,
  task calls, host bindings and remaining parity audits stay open. Simplification
  and performance passes remain at 0/3 each; final test conversion is pending.

- Web-Bench evaluator close policy passes 235 source comparisons covering registry
  removal, shutdown order, process polling, TERM/KILL escalation, wait failures,
  file close, completion state, callable capture and exception boundaries. Registry
  and signal coordination passes another 214 comparisons covering snapshots,
  mutation during cleanup, handler identity, eager signal capture, registration
  errors and handler failures. Eight real signal cases compare source and native
  TERM, INT, HUP and process-group TERM while the main RPC lock is held. They verify
  child reaping, listener shutdown, separate cleanup process groups and preservation
  of the signal exit status. No Web-Bench evaluator or paid model is called.
  Startup's 413 comparisons, including its real HTTP/process pipeline, and the
  914-case shared runner corpus pass with the new provider bindings.
  The full original suite, with installed-opencode tests enabled, passes with
  1,561 passed and 30 skipped (70.34 seconds). Its call record remains
  `validation/experiment-task-calls.json` and includes two native signal-registration
  calls. Original tests do not exercise evaluator close, session sweeping or signal
  handlers. All 24 Rust checks, debug/release builds, Clippy,
  formatting and the 256-file reference check pass. Original tests remain unchanged.
  IDE inspections remain unavailable because this checkout is not open in the IDE.
  SWE-bench global cleanup, Web-Bench task calls, host bindings and remaining parity
  audits stay open. Simplification and performance passes remain at 0/3 each;
  final test conversion is pending.

- SWE-bench global cleanup and signal policy passes 272 source comparisons covering
  snapshot order, opaque container names, ignored command results, registry removal,
  mutation during cleanup, getter failures, error propagation and handler captures.
  Eight real signal cases compare source and native TERM, INT, HUP and process-group
  TERM while the shared RPC lock is held. An owned Java subprocess replaces Docker
  I/O and returns status 39; cleanup waits for it, discards the tracked name and
  preserves the parent's signal exit status. No Docker daemon or paid model is used.
  Shared signal and snapshot policy now lives in `swebench/cleanup.rs`; evaluator
  resource cleanup remains in `swebench/web_close.rs`.
  The 214-case Web-Bench registry corpus, its eight real signal cases and the
  914-case runner corpus pass. The full original suite, with installed-opencode
  tests enabled, passes with 1,561 passed and 30 skipped (70.05 seconds). Its record
  remains `validation/experiment-task-calls.json`; it does not exercise the new
  SWE-bench cleanup operations. All 24 Rust checks, debug/release builds, Clippy,
  formatting and the 256-file reference check pass. Original tests remain unchanged.
  IDE inspections remain unavailable because this checkout is not open in the IDE.
  Web-Bench task calls, scaffold backends, streaming, host bindings and remaining
  parity audits stay open. Simplification and performance passes remain at 0/3
  each; final test conversion is pending.

- Web-Bench workspace policy passes 333 source comparisons covering workspace
  replacement, invalid relative paths, non-string contents, directory pruning,
  supplied hidden files, scaffold exclusions, file-size limits, unreadable files,
  suppressed context errors, partial results and exception-aware path formatting.
  Real filesystem checks verify workspace replacement and collected answers,
  including Unicode files, binary files, ignore files and tracked dot-directories.
  Ignore helpers pass another 725 comparisons covering eager path assembly,
  ignore-file order, line filtering, ancestor prefixes, wildcard classes, Unicode
  and surrogate code points, foreign matchers and callable replacement during path
  normalisation. Their normal string matching executes in Rust. The shared matcher
  retains its string API and now also accepts code-point slices.
  Existing bridge/protected-tool checks, the 87-case facade corpus and the 914-case
  shared runner corpus pass. The full original suite, with installed-opencode tests
  enabled, passes with 1,561 passed and 30 skipped (71.15 seconds). Its record remains
  `validation/experiment-task-calls.json`; original tests do not exercise the new
  workspace operations. All 24 Rust checks, debug/release builds, Clippy, formatting
  and the 256-file reference check pass. Original tests remain unchanged.
  IDE inspections remain unavailable because this checkout is not open in the IDE.
  The task loop, agent execution, scaffold helpers, host bindings and remaining
  parity audits stay open. Simplification and performance passes remain at 0/3
  each; final test conversion is pending.

- The shared Web-Bench task loop passes 476 source comparisons covering retries,
  unreached and broken streams, event desynchronisation, missing requests and
  verdicts, workspace cleanup, one guard per attempt, continuation, falsey guards,
  empty scaffold notes, reply accounting, nested file contexts, exception chains,
  provenance, lazy telemetry and floating-point totals. Raw task, directory,
  kwargs, request-file, collected-file, queue and transcript references retain
  their identities. Real-file checks execute materialisation, agent edits and
  collection through the task loop and verify request and transcript artifacts.
  Two additional source/native offline pipelines use the real HTTP server, queues,
  evaluator subprocess, stdout reader, workspace helpers and cleanup. A Java
  evaluator stand-in verifies the edited files, input/output token totals and
  transcript path before emitting its verdict. Native-operation counts confirm
  that the combined pipeline executes the converted components. No model or
  Playwright evaluator is called.
  The 413-case startup regression, all 24 Rust checks, debug/release builds,
  Clippy, formatting and the 256-file reference check pass. The full original suite,
  with installed-opencode tests enabled, passes with 1,561 passed and 30 skipped
  (72.22 seconds). Its record remains `validation/experiment-task-calls.json`;
  original tests do not exercise the shared Web-Bench task loop. Original tests
  remain unchanged. IDE inspections remain unavailable because this checkout is
  not open in the IDE. Agent wrappers and execution, configuration, prompt, guard
  and scaffold helpers, host bindings and remaining parity audits stay open.
  Simplification and performance passes remain at 0/3 each; final test conversion
  is pending.

- Web-Bench agent wrappers and task home pruning pass 360 source comparisons
  covering multiple inheritance, parent result and argument identities, capability
  snapshots, large counters, partial row updates, debug log writes, file errors,
  exception chains, retained provenance, cleanup order and symlinks. Real files
  verify UTF-8 debug output, preserved instructions, removed CLI state and retained
  workspaces. A comparison exposed a shared context binding gap: `__exit__` return
  values must run `__bool__` in the handled exception's scope. The binding now keeps
  the exit call and truth test in that scope, including errors from either call.
  The 476-case task loop, 217-case task loader, 307-case optimizer logging,
  333-case workspace and 413-case startup regressions pass with the binding fix.
  Six source/native offline HTTP/process pipelines cover Claude, Copilot and Codex,
  including native wrappers calling the native shared loop, actual file edits,
  evaluator replies, Copilot debug output and Codex metadata. Native-operation
  counts confirm the converted entry points ran. No model or Playwright is called.
  The focused original tests pass with 19 passed. The full original suite, with
  installed-opencode tests enabled, passes with 1,561 passed and 30 skipped
  (74.07 seconds). Its record remains `validation/experiment-task-calls.json` and
  includes two native Codex wrapper calls and three native home-pruning calls.
  All 24 Rust checks, debug/release builds, Clippy, formatting and the 256-file
  reference check pass. Original tests remain unchanged. IDE inspections remain
  unavailable because this checkout is not open in the IDE. Agent execution,
  configuration, prompt, guard and scaffold helpers, host bindings and remaining
  parity audits stay open. Simplification and performance passes remain at 0/3
  each; final test conversion is pending.

- Web-Bench run contracts pass 368 source comparisons covering configuration field
  order, protected tools, network-tool duplicates, scaffold provenance, injection
  hashes, eager budget defaults, optional guard arguments, falsey Codex policies,
  callback identities, prompt precedence, transport-line filtering, formatting calls,
  custom templates, surrogate code points and the 8,000-character error boundary.
  Prompt assets match the pinned source exactly. Real-file checks compare complete
  `run_config.json` output and preserve duplicate network exclusions from the source.
  Six source/native HTTP/process pipelines now use the real configuration writer,
  prompt helpers and guard selection across all three agents. Their operation
  counts confirm the new native helpers ran. No model or Playwright is called.
  The 476-case task-loop and 360-case wrapper regressions pass. The focused original
  tests pass with 19 passed. The full original suite, with installed-opencode tests
  enabled, passes with 1,561 passed and 30 skipped (75.49 seconds). Its record remains
  `validation/experiment-task-calls.json` and includes three native Codex guard helper
  calls and two shared guard helper calls. The original suite does not exercise the
  new configuration and prompt methods. All 24 Rust checks, debug/release builds,
  Clippy, formatting and the 256-file reference check pass. Original tests remain
  unchanged. IDE inspections remain unavailable because this checkout is not open
  in the IDE. Agent execution, accounting, network/scaffold integration, host
  bindings and remaining parity audits stay open. Simplification and performance
  passes remain at 0/3 each; final test conversion is pending.

- Web-Bench agent segments and Claude session logs pass 1,160 source comparisons
  covering command and getter order, environment assembly, repeated network
  exclusions, resume requests, namespace arguments, raw object references, streaming,
  falsey guards, timeouts, stderr limits, refusal errors, process cleanup, exception
  class replacement and nested failure contexts. Real session files verify sorted
  concatenation, replacement decoding and unreadable-entry handling. Eighteen owned
  process checks exercise all three agents in source and native modes, including
  normal completion, guard stops, deadlines and the two-second SIGTERM-to-SIGKILL
  sequence. Claude recovers exact usage from real session files after a stop.
  Six complete offline HTTP/process task pipelines now run the real segment methods
  and usage parsers with owned Java agent stand-ins. No model or Playwright is called.
  The 476-case task-loop, 360-case wrapper and 368-case run-contract regressions pass.
  The full original suite, with installed-opencode tests enabled, passes with
  1,561 passed and 30 skipped (71.81 seconds). Its record remains
  `validation/experiment-task-calls.json`, including one Claude segment, seven Codex
  segments, three Copilot segments and one Claude session-log call. All 24 Rust
  checks, debug/release builds, Clippy, formatting and the 256-file reference check
  pass. Original tests remain unchanged. IDE inspections remain unavailable because
  this checkout is not open in the IDE. Live agents, Linux namespace execution and
  Windows remain unverified. Supporting agent helpers, scaffold and host bindings,
  and remaining production-parity audits stay open. Simplification and performance
  passes remain at 0/3 each; final test conversion is pending.

- Web-Bench agent support passes 1,681 source comparisons covering home paths,
  static method binding, execution environments, Copilot configuration seeding and
  cache preparation, Codex configuration writes, lean-token metadata and accounting
  hooks. The corpus retains raw usage values, helper return identities, custom row
  mappings, comparison objects, repeated truth checks and nested error contexts.
  Real-file checks verify configuration and instructions output, the three-file
  Copilot copy rule, preservation of existing task settings and operator files,
  and cache symlink reuse. Copilot host helpers pass 279 comparisons including the
  new cache and configuration-directory discovery paths. Six offline HTTP/process
  task pipelines now use the native homes, configuration writer and accounting hooks.
  Codex's trusted workspace, instructions path and lean-token count are checked
  against the generated files; Copilot's source settings remain unchanged.
  The 1,160-case segment, 476-case task-loop, 360-case wrapper and 368-case
  run-contract regressions pass, as do all 18 owned segment-process checks.
  The full original suite, with installed-opencode tests enabled, passes with
  1,561 passed and 30 skipped (69.77 seconds). Its record remains
  `validation/experiment-task-calls.json` and includes the new home, configuration
  and accounting operations. All 24 Rust checks, debug/release builds, Clippy,
  formatting and the 256-file reference check pass. Original tests remain unchanged.
  IDE inspections remain unavailable because this checkout is not open in the IDE.
  Live agents, Linux namespace execution and Windows remain unverified. Argument
  builders, network guard and scaffold integration, host bindings and remaining
  parity audits stay open. Simplification and performance passes remain at 0/3
  each; final test conversion is pending.

- Web-Bench network guard setup passes 1,823 source comparisons covering settings
  paths and entry shapes, existing hooks, idempotence, malformed settings, probe
  construction, refusal results, probe-log removal, refusal counts, hosts files,
  path quoting, lexical superclass dispatch, armed-state reads and failure contexts.
  Lazy generator checks retain iterator timing, short-circuit reads, aggregate
  overrides and `StopIteration` handling. Real-file checks preserve existing settings
  and host entries, remove the probe's refusal record, count non-empty log lines
  and preserve decoding failures. Source checks execute the Python guard; native
  checks execute the Rust guard through SDK process bindings.
  Six offline HTTP/process task pipelines now use real guard installation,
  verification and refusal counting across all three agents. Their native operation
  counts confirm one Rust guard probe per pipeline. The 1,681-case agent-support,
  1,160-case segment, 476-case task-loop, 360-case wrapper and 368-case run-contract
  regressions pass. The full original suite, with installed-opencode tests enabled,
  passes with 1,561 passed and 30 skipped (70.41 seconds). Its record remains
  `validation/experiment-task-calls.json`, including two Codex installations and
  verifications, six hosts-file calls and five namespace path-quoting calls.
  All 24 Rust checks, debug/release builds, Clippy, formatting and the 256-file
  reference check pass. Original tests remain unchanged. IDE inspections remain
  unavailable because this checkout is not open in the IDE. Live agent hook
  invocation, Linux namespace execution and Windows remain unverified. Argument
  builders, scaffold integration, host bindings and remaining parity audits stay
  open. Simplification and performance passes remain at 0/3 each; final test
  conversion is pending.

- Agent invocation helpers pass 961 source comparisons covering Copilot and Codex
  flags, repeated exclusions, protected tools, optional model and effort settings,
  MCP arguments, hook trust, resume sessions, JSON effort settings, environment
  updates, refusal patterns, Unicode truncation, raw references and failure order.
  Retained-generator checks verify live loop captures, including consumption after
  the refusal helper returns. All 24 owned-process checks pass across source and
  native modes. The Java agent stand-ins validate the consumed flags, resumed Codex
  stdin and late Copilot/Codex configuration refusals, alongside normal streaming,
  guard stops, deadlines and process cleanup. Six complete offline HTTP/process
  pipelines confirm the native invocation helpers ran through the shared task loop.
  The 1,823-case network-guard, 1,160-case segment and 279-case Copilot-host
  regressions pass. The focused original tests pass with 69 passed. The full
  original suite, with installed-opencode tests enabled, passes with 1,561 passed
  and 30 skipped (70.42 seconds). Its record remains
  `validation/experiment-task-calls.json` and includes the new invocation operations.
  All 24 Rust checks, debug/release builds, Clippy, formatting and the 256-file
  reference check pass. Original tests remain unchanged. IDE inspections remain
  unavailable because this checkout is not open in the IDE. Live agent hook
  invocation, Linux namespace execution and Windows remain unverified. Scaffold
  integration, host bindings and remaining parity audits stay open. Simplification
  and performance passes remain at 0/3 each; final test conversion is pending.

- Web-Bench scaffold integration passes 1,350 source comparisons covering profile
  arguments, tuple values, callback signatures and modules, retained captures,
  execution environments, host commands, file placement, setup results, exclusions
  and nested failure contexts. Real filesystem checks verify hardlinks, symlink
  replacement, copy and destination fallbacks, executable modes and preserved file
  contents. Real process checks verify task HOME, environment overrides, working
  directory and host execution. Checks cover raw setup notes, repeated error
  formatting, renamed exception types and Unicode truncation.
  All six offline HTTP/process pipelines now install a fixture scaffold through
  the real setup method and callbacks for all three agents. They hardlink an
  executable, run an installation command, invoke the host callback and keep
  scaffold artifacts out of the evaluator answer. Native operation counts confirm
  profile construction, MCP callbacks, context creation, setup, execution, placement
  and exclusion predicates ran. The 476-case task-loop, 368-case run-contract,
  333-case workspace and 1,681-case agent-support regressions pass. The focused
  original tests pass with 30 passed. The full original suite, with
  installed-opencode tests enabled, passes with 1,561 passed and 30 skipped
  (68.37 seconds). Its record remains `validation/experiment-task-calls.json`;
  original coverage of the new operations is one Codex profile call. All 24 Rust
  checks, debug/release builds, Clippy, formatting and the 256-file reference check
  pass. Original tests remain unchanged. IDE inspections remain unavailable because
  this checkout is not open in the IDE. Live agents, Linux namespaces and Windows
  remain unverified. Scaffold backends, supporting context methods, SWE-bench
  integration, host bindings and remaining parity audits stay open. Simplification
  and performance passes remain at 0/3 each; final test conversion is pending.

- Shared scaffold classes pass 611 source comparisons covering constructor state,
  raw callbacks, environment copies, MCP arguments, instruction and configuration
  writes, merge-program staging and failure order. The baseline-only SDK binding
  installs without importing either harness and supports repeated installation.
  Real file checks verify successful cleanup and source-compatible temporary-file
  retention when copying or removal fails. All six offline Web-Bench pipelines
  write MCP configuration and execute the generated merge program, preserving
  existing JSON keys. That program still executes Python; this milestone ports its
  construction and orchestration, and native container-helper deployment stays open.
  The 1,350-case scaffold, 368-case run-contract and 476-case task-loop regressions
  pass. Focused original tests pass with 36 passed and one skipped. The full
  original suite, with installed-opencode tests enabled, passes with 1,561 passed
  and 30 skipped (67.21 seconds). Its native call record remains
  `validation/experiment-task-calls.json`, including eight profile constructors,
  21 context constructors, ten instruction writes, six MCP writes and two merge
  operations. All 24 Rust checks, debug/release builds, Clippy, formatting and the
  256-file reference check pass. Original tests remain unchanged. IDE inspections
  remain unavailable because this checkout is not open in the IDE. Live agents,
  Linux namespaces and Windows remain unverified. Scaffold backends, native
  container-helper deployment, SWE-bench integration, host bindings and remaining
  parity audits stay open. Simplification and performance passes remain at 0/3
  each; final test conversion is pending.

- Serena passes 426 source comparisons covering installation success and failure,
  version and error truncation, agent-specific MCP context, Codex server reference
  identity, raw configuration and instruction results, generator evaluation and
  failure order. Real file checks verify MCP configuration and project instructions
  for Claude, Copilot and Codex, with no temporary files after success. These checks
  do not download or install Serena. The 611-case shared scaffold regression and
  all six offline Web-Bench HTTP/process pipelines pass. Focused original tests
  pass with 36 passed and one skipped. The full original suite, with
  installed-opencode tests enabled, passes with 1,561 passed and 30 skipped
  (69.51 seconds). The native call record in
  `validation/experiment-task-calls.json` includes five Serena setups, five server
  configurations, four navigation instructions and 20 tool-name evaluations.
  All 24 Rust checks, debug/release builds, Clippy, formatting and the 256-file
  reference check pass. Original tests remain unchanged. IDE inspections remain
  unavailable because this checkout is not open in the IDE. Serena's actual
  installation and MCP startup, live agents, Linux namespaces and Windows remain
  unverified. RTK, repo-map, native container-helper deployment, SWE-bench integration,
  host bindings and remaining parity audits stay open. Simplification and performance
  passes remain at 0/3 each; final test conversion is pending.

## Dependencies

Build with Rust 1.89 or newer and a C compiler for bundled SQLite. Validation uses
Rust 1.99.1. The SQLite wrapper requires the minimum Rust version change; its
unused cache and WebAssembly default features are disabled. Clippy's conditional
syntax fixes after that version change are part of the port, not one of the
three requested simplification passes.

[Chrono](https://docs.rs/chrono/0.4.45/chrono/) supplies calendar parsing and local
timezone handling. [Rusqlite](https://docs.rs/rusqlite/0.43.0/rusqlite/) supplies
SQLite connections opened explicitly with `SQLITE_OPEN_READ_ONLY`. Trace readers
use a five-second busy timeout; gateway lookups retain the SDK's two-second limit.
The lockfile pins both libraries and their transitive dependencies.
`fancy-regex` 0.20.2 preserves the upstream patterns' lookarounds and backreferences.
The existing `serde_json` dependency enables `raw_value` so Hermes can validate
tool JSON before copying its existing field text after a redirect. Python's
non-finite extensions remain in the JSON parity audit.
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
[ZIP](https://docs.rs/zip/8.6.0/zip/) 8.6.0 writes plugin archives and executable
permissions. Only its `deflate-flate2-zlib-rs` feature is enabled; encryption and
other compression formats remain disabled. The crate supports Rust 1.88, below
this project's minimum. The lockfile pins the five additional archive dependencies.
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
JSON. `te-bridge` is the development test transport; `te` and `te-hook` are the
standalone native entry points. Python is needed for the original test driver
and the optional external aider installation. Reference-payload packaging tests
retain the Python library and launchers as fixture data; production `te-package`
bundles the native executables.

Enable the original opencode runtime and service tests when opencode is installed:

```sh
TE_OPENCODE_E2E=1 TE_OPENCODE_BIN="$(command -v opencode)" PYTHONPATH=validation \
  .venv/bin/python -m pytest -o addopts= -p native_plugin \
  reference/upstream/tests/test_opencode_plugin.py
```

The adapter uses `te-console-probe` only for the original console assertions.
Production bundles contain the native entry points without this validation shim.

The optional `TE_NATIVE_HOOK_TEST_EXEC` route runs selected original hook tests
through `te-hook`. Use tests whose dependencies require no Python patches; this
route creates a separate process and cannot retain those patches. The learning
and resume checks run with an empty PATH to prevent model-agent calls:

```sh
PATH= TE_NATIVE_HOOK_TEST_EXEC="$PWD/target/debug/te-hook" PYTHONPATH=validation \
  .venv/bin/python -m pytest -o addopts= -p native_plugin \
  reference/upstream/tests/test_live_hooks.py \
  -k 'a_learned_fact_reaches_the_next_session_and_is_announced_once or resume_does_not_inject_again_and_shadow_never_injects'
```

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
jbang validation/AdapterProbe.java
jbang validation/SpiralProbe.java
jbang validation/NudgeProbe.java
jbang validation/FacadeProbe.java
jbang validation/UsageProbe.java
jbang validation/TaskProbe.java
jbang validation/WebSegmentProbe.java
jbang validation/WebAgentProbe.java
jbang validation/WebGuardProbe.java
jbang validation/WebScaffoldProbe.java
jbang validation/ScaffoldBaseProbe.java
jbang validation/SerenaScaffoldProbe.java
jbang validation/InvocationProbe.java
jbang validation/WebSegmentProcessProbe.java
jbang validation/WebTaskPipelineProbe.java
jbang validation/GraderProbe.java
jbang validation/SettingsProbe.java
jbang validation/ContainerProbe.java
jbang validation/CodexConfigProbe.java
jbang validation/RunnerWrapperProbe.java
jbang validation/NetGuardProbe.java
jbang validation/GuardInstallProbe.java --docker
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
jbang validation/RepoMapProbe.java
jbang validation/GeneratorProbe.java
jbang validation/HookProbe.java
jbang validation/NativeHookProbe.java
jbang validation/PackageProbe.java
jbang validation/HermesGatewayProbe.java
jbang validation/PiSdkProbe.java
jbang validation/OpencodeSdkProbe.java
jbang validation/OpencodeProtocolProbe.java
jbang validation/CliProbe.java
jbang validation/LearningCliProbe.java
jbang validation/ToolpruneCliProbe.java
jbang validation/DoctorProbe.java
jbang validation/ExtractProposerPrompts.java
```

After changing Rust dependencies, run `cargo fetch --locked`, then
`jbang scripts/ThirdPartyNotices.java`. The generator copies the licence, notice
and copyright files from all 81 locked registry crates, including platform and
build dependencies. `jbang scripts/ThirdPartyNotices.java --check` verifies the
committed file against cached crate sources. The native packager checks its
embedded notices against the compiled lockfile hash before replacing a bundle.

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
standalone hook checks cover queued replay and the native services separately.

IntelliJ has no open project for this checkout. Use Cargo's compiler, formatter
and Clippy until IDE access becomes available.
