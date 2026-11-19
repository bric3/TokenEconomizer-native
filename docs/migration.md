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
  Runner process orchestration is native, with SDK callbacks supplying process I/O.
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
  The source tests retain their Python fixtures and data classes. Operational
  scripts listed under remaining production work still need native entry points.
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
  exception names and preserves the detail limit of 300 code points.
- SWE-bench scaffold context assembly, retained Docker execution and copy
  callbacks, and binary reads use native policy. SDK bindings retain callback
  captures, signatures, metadata and raw result objects. Execution preserves
  environment argument order; binary reads preserve stdout and format truncated
  stderr when the process fails.
- SWE-bench agent segments use native Docker arguments, credential forwarding,
  tool masks, MCP arguments, stream handling, guard and deadline stops, container-side
  termination, usage recovery, configuration refusal detection and Claude session-log
  reads. Both benchmark harnesses share transcript processing, guard monitoring,
  usage recovery and wait-timeout handling. Launch and termination policy remain
  with each harness. SDK bindings provide process, pipe, clock and object APIs.
- Shared scaffold profile and context constructors, read/log callbacks, instruction
  writes, MCP configuration and merge-helper staging use native policy. SDK bindings
  retain raw objects, file contexts, JSON encoding and process I/O. The staged merge
  invocation calls the native helper with a quoted JSON payload. SWE-bench deploys
  the architecture-matched helper lazily; WebBench uses the host helper beside the
  executable. Experiment helper packaging remains a production gate.
- Serena installation policy, version reporting, MCP server configuration, Codex
  server handoff and navigation instructions use native policy. SDK bindings
  retain callback results, server references and file I/O. The install command
  still installs Serena's external Python toolchain as required by the baseline.
- RTK cache selection, pinned host staging, fallback installation, Claude and
  Copilot hook registration, execution and rewrite probes, and Codex instruction
  inlining use native policy. SDK bindings retain object and environment protocols,
  JSON encoding and process I/O. Stage and hook results retain their raw references
  and source evaluation order.
- Scaffold registry lookup and arm selection use native policy, including
  constructor failures and handled exception scopes. Repo-map token budgets,
  identifier selection, aider preflight diagnostics, archive-member filtering,
  generator orchestration, instruction writes and cleanup use native policy too.
  SDK bindings supply archive extraction, Unicode character properties, object
  protocols and process I/O. Repo-map generation selects the native executable
  beside the engine and passes the current interpreter to its aider SDK binding.
  Aider remains an external Python dependency.
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

- Trace-reader ISO timestamp parsing and its local-time mapping now follow
  CPython 3.15.2. The shared helper covers Claude/Droid heads and Droid discovery.
  Other platform time-zone environments and surrogate-containing timestamp
  strings remain part of the platform and JSON-codec audits below.
- Audit regular-expression compatibility with Python in the production parity
  pass, including word boundaries, whitespace classes and syntax. Detector and
  extractor patterns now support Python's global and scoped ASCII flags through
  the native compiler. Character classes, boundaries, literal case folding,
  named-character escapes and group identifiers use Unicode 17 data.
  Backreferences apply scoped ASCII or Unicode 17 lowercase rules. Continue the
  remaining syntax and engine runtime-error audit, including nesting and resource
  limits and patterns containing surrogate code points.
  Include Unicode-version differences outside the evaluator stdout pattern,
  whose explicit Tolong Siki range matches Python 3.15 and the
  [Unicode 17 numeric data](https://www.unicode.org/Public/17.0.0/ucd/extracted/DerivedNumericType.txt).
- Audit JSON numeric edge cases and parser nesting limits, including non-finite
  values accepted by Python.
  Audit remaining JSON object and exception-message codecs for adjacent surrogate
  code points; the shared exact-string and logging-format bindings preserve them.
  Include the Hermes redirect helper's raw JSON validation in that audit.
  Include the native scaffold merge command's invalid-UTF-8 diagnostics, deeply
  nested JSON and filesystem error details outside its verified file corpus.
  Include scaffold merge callback failures and path objects outside string paths,
  including filenames with non-UTF-8 bytes or unpaired surrogate code points.
  Include guard merge-launcher stderr and I/O error formats beyond the compared
  final diagnostic line.
  Include WebBench launcher creation, executable permissions and paths outside
  the normal writable task homes in the I/O parity audit.
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
- Verify tilde expansion on a native Windows host, including home-directory
  fallback through `USERPROFILE`, `HOMEDRIVE` and `HOMEPATH`. Claude history now
  uses the shared expansion helper. Environment-file selection calls the supplied
  platform path functions and has source comparisons with POSIX and Windows rules.
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
- Finish supporting experiment tools, preserving their original test contracts.
  `scripts/dead_code.py` has a native replacement, `te-dead-code`. Its 11 original
  tests invoke Rust scanner, deletion, sweep and report policy through the API
  binding and native CLI. Python supplies standard-library AST and reflection
  operations through an embedded transport. The static documentation figures,
  `docs/assets/results_overview.py` and `framework_overview.py`, now have Rust
  replacements through `te-figures`. Matplotlib retains its rendering SDK role;
  layout, text, paper data, geometry and collision checks execute in Rust.
  The animated `docs/assets/teaser.py` also has a Rust replacement through
  `te-figures teaser`, including font selection, logo loading, timeline, layout,
  poster and ffmpeg export policy. Pillow supplies drawing and image APIs;
  ffmpeg supplies the three codecs. Its paper rows are shared with the static
  results figure, and its logos are embedded from verified pinned assets.
  The workload registry, manifest exporter, sweep planner, both task exporters,
  YAML mirror, paired comparison reports, shared launch helpers, fleet CPU
  controls, main sweep and Codex A/B launcher have native implementations.
  The production-entry-point inventory also found the standalone catalogue
  capture tool; it now runs through `te-bench capture-catalog`, with native capture
  and recorder policy. `te_harness.webbench_smoke` and `launch/smoke_test.sh` are
  executable tests and remain archived until final test conversion.
  Original Codex launcher tests now invoke the
  native command with the archive as their fixture working directory. Parent
  bridge counters do not cover child processes; dedicated main/Codex oracles
  check native launch execution, driver arguments and results with no upstream
  import path.
  Aggregation has a native entry point, `te-bench aggregate`, and its source tests
  now call Rust through the validation binding. Weighted costs and per-cell totals
  retain f64 accounting. Include unusually large totals, non-finite JSON, non-UTF-8
  paths, filesystem error wording and CLI argument edge cases in the parity audit.
  The task runners' usage accounting and shared SWE-bench task lifecycle are native.
  SWE-bench guard deployment and registration use native helpers. Native
  scaffold merge helpers are deployed through harness callbacks. Both harnesses
  select native guard executables. The runtime bundle ships executables and
  assets. The standalone benchmark command and its SDK are packaged. Bindings
  assemble both built-in harnesses. SWE-bench's namespace supplies all three agent
  classes, profiles, grader and helpers; WebBench's supplies its three runners,
  evaluator session and HTTP handler.
  Generic and Codex controller SDK bindings are packaged and used by the original
  controller tests. The driver uses the Codex policy binding too; state transitions
  remain native.
  Stream start and session close coordination are native. Per-task container
  policy, grading orchestration, the official-grader wrapper, Codex config writing,
  scaffold result handling and Copilot/Codex SWE-bench task wrappers are native.
  SWE-bench controller construction parameters, continuation and telemetry are
  native too, along with constructor coordination for all three agents.
  Provider routing, credential filtering, model normalisation, catalogue reads,
  model verification and shared endpoint preflight control flow are native.
  Messages and Responses optimizer policy is native; HTTP I/O retains SDK bindings.
  Codex installation discovery, captured-prompt loading and offline probes are
  native. Codex captures and the WebBench preload use native runtime asset paths.
  Codex installation and configuration probe SDK bindings are packaged too.
  Agent accounting and interrupted-stream recovery also have packaged SDK bindings.
  The shared object/I/O binding and provider routing namespace are packaged too.
  Optimizer calls, preflights, logging, Copilot discovery and sealed Copilot/Codex
  invocation builders also have packaged bindings.
  Shared scaffold classes, RTK, Serena, repo-map and their registry are packaged too.
  Task-lifecycle and constructor object/I/O bindings are packaged and used by the
  original API adapters. Guard properties, attempt forwarding, merge-program
  construction, hook-entry writing and per-agent registration also execute in
  Rust through the SDK. SWE-bench segments, retained scaffold callbacks, optimizer
  wrappers, factories and cleanup also have packaged bindings. Both class
  assemblies are packaged and selected with `te_runtime.swebench:make_runner` or
  `te_runtime.webbench:make_runner`.
  Process and SDK calls retain Python I/O bindings.
- Benchmark driver runner and dataset selection, workload and task filtering,
  group matching and overhead-field copying are native. The arm registry and
  environment-file parsing, selection and application are native too. Main stream
  coordination and the offline stub runner are native. The standalone benchmark
  command, core SDK and runtime packaging are implemented; built-in harness
  parity audits remain open.
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
  `validation/experiment-task-calls.json`, including eight setups, eight staging
  checks, 15 cache reads, five rewrite probes and three Codex instruction setups.
  All 24 Rust checks, debug/release builds,
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

- RTK passes 780 source comparisons covering cache precedence, cached and downloaded
  staging paths, installation fallback, agent-specific hook configuration, Copilot
  output parsing, rewriting probes, Codex instructions, raw results and failure
  order. Original static and class method descriptors retain their calling contracts.
  The SDK binding supports repeated installation without importing either harness.
  All six offline Web-Bench pipelines now execute the RTK scaffold with an owned
  Java RTK executable. They verify Claude settings, Copilot hook files, actual
  rewrite-probe output, Codex instruction inlining and preserved operator settings.
  The fixture supplies the staged executable; host downloads remain fixture-only.
  A login shell selected the host's RTK during fixture development; the successful
  pipelines use an owned BASH_ENV file to pin PATH to the fixture executable.
  Deployment still needs to verify the RTK version selected by login-shell PATH.
  The 611-case shared scaffold and 426-case Serena regressions pass. Focused
  original tests pass with 36 passed and one skipped. The full original suite,
  with installed-opencode tests enabled, passes with 1,561 passed and 30 skipped
  (70.12 seconds). The native call record remains
  `validation/experiment-task-calls.json`. All 24 Rust checks, debug/release builds,
  Clippy, formatting and the 256-file reference check pass. Original tests remain
  unchanged. IDE inspections remain unavailable because this checkout is not open
  in the IDE. Actual RTK downloads, live agent hook invocation, Linux namespaces
  and Windows remain unverified. Repo-map, scaffold registry, native container-helper
  deployment, SWE-bench integration, host bindings and remaining parity audits stay
  open. Simplification and performance passes remain at 0/3 each; final test
  conversion is pending.

- The scaffold registry passes 134 source comparisons covering falsey names,
  custom mappings, constructor results and failures, changing registries, arm
  selection, error formatting and suppressed exception context. Repo-map passes
  704 comparisons covering token coercion, identifier patterns, Unicode word
  boundaries, identifier limits, archive filtering, subprocess arguments, raw
  callbacks, preflight diagnostics, cleanup and failure scopes. Real archive,
  process and file checks preserve filenames with spaces, reject unsafe members,
  generate instructions and verify cleanup. Failed fetches retain their partial
  directory; failed generators remove the assigned source directory. Checks also
  cover cleanup failures and failures while resolving exception classes.
  The generator process in these checks is an owned Java fixture; aider execution
  remains unverified here. Experiment packaging must still select the native
  generator command, and SDK bindings retain archive extraction and Unicode
  character properties. The six offline Web-Bench HTTP/process pipelines pass.
  Focused original tests pass with 36 passed and one skipped. The full original
  suite, with installed-opencode tests enabled, passes with 1,561 passed and
  30 skipped (70.51 seconds). Its native call record remains
  `validation/experiment-task-calls.json`, including 38 registry lookups, ten arm
  selections, five identifier extractions, four token-budget calls, two repo-map
  setups and two preflights. All 24 Rust checks, debug/release builds, Clippy,
  formatting and the 256-file reference check pass. Original tests remain
  unchanged. IDE inspections remain unavailable because this checkout is not open
  in the IDE. Native container-helper deployment, SWE-bench integration, host
  bindings and remaining parity audits stay open. Live agents, Linux namespaces
  and Windows remain unverified. Simplification and performance passes remain at
  0/3 each; final test conversion is pending.

- SWE-bench scaffold context assembly passes 820 source comparisons covering
  constructor argument identity, retained callbacks, signatures and metadata,
  independent contexts, global runner rebinding, environment order, raw values,
  binary results and failure boundaries. Owned Java Docker fixtures execute real
  subprocess checks for binary stdout, environment arguments and failed reads.
  A repo-map pipeline streams a real tar through the context, filters archive
  members, copies nested project instructions and removes the source directory.
  It asserts that the shared context, repo-map policy and all four new SWE-bench
  context operations execute natively. These checks use no Docker daemon or aider.
  The 704-case repo-map regression and all six offline Web-Bench HTTP/process
  pipelines pass. Focused original tests pass with 36 passed and one skipped.
  The full original suite, with installed-opencode tests enabled, passes with
  1,561 passed and 30 skipped (72.43 seconds). Its record remains
  `validation/experiment-task-calls.json`; the original suite records no calls
  to the new SWE-bench context operations, whose coverage comes from the dedicated
  comparisons and pipeline. All 24 Rust checks, debug/release builds, Clippy,
  formatting and the 256-file reference check pass. Original tests remain
  unchanged. IDE inspections remain unavailable because this checkout is not open
  in the IDE. Actual Docker execution, live agents, Linux namespaces and Windows
  remain unverified. Native container-helper deployment, SWE-bench agent streaming,
  experiment packaging, host bindings and remaining parity audits stay open.
  Simplification and performance passes remain at 0/3 each; final test conversion
  is pending.

- SWE-bench streaming passes 1,067 source comparisons covering all three agents'
  Docker arguments, credentials forwarded by name, environment order, protected
  and network tool masks, MCP arguments, per-segment Copilot logs, Codex resumes,
  transcripts, guard stops, deadlines, usage recovery, session logs and failure
  scopes. Shared stream processing moves into a module used by both benchmark
  harnesses; this extraction is part of the port, not a formal simplification pass.
  Web-Bench's 1,160-case streaming regression passes.
  All 24 source/native SWE-bench process checks pass with separate owned Java
  Docker-client and agent processes. They verify streams, accounting, configuration
  refusals, Codex resumes, secret forwarding without key values in argv, preserved
  workspace edits and termination of the internal agent. Codex uses exact process-name
  matching; Claude and Copilot use command-line matching. The process fixture skips
  the two-second grace delay; the comparison corpus verifies the sleep argument.
  Web-Bench's 24 process checks and six offline HTTP/process pipelines pass too.
  These fixtures invoke no model and require no Docker daemon.
  Focused original tests pass with 52 passed and one skipped. The full original
  suite, with installed-opencode tests using a loopback model fixture, passes with
  1,561 passed and 30 skipped (71.08 seconds). Its record remains
  `validation/experiment-task-calls.json`; it includes one native Claude segment.
  Dedicated comparisons and process checks cover the other streaming paths.
  All 24 Rust checks, debug/release builds, Clippy, formatting and the 256-file
  reference check pass. Original tests remain unchanged. IDE inspections remain
  unavailable because this checkout is not open in the IDE. Actual Docker execution,
  live model-backed agents, Linux namespaces and Windows remain unverified. Native
  container-helper deployment, experiment packaging, host bindings and remaining
  parity audits stay open. Simplification and performance passes remain at 0/3
  each; final test conversion is pending.

- The native guard executable now accepts `--merge-json <path> <patch-file>`.
  Its merge policy matches the source-generated program in 185 file/process
  comparisons, including one-level dictionary updates, preserved keys, malformed
  and invalid-UTF-8 destination files, non-object data, Unicode and surrogate
  escapes, non-finite numbers, integer limits, JSON diagnostics and failure effects.
  This command is independent of the generated Python program; harness staging
  still selects that program and remains a deployment gate.
  `scripts/BuildContainerHelpers.java` builds static arm64 and amd64 Linux helpers
  from a minimal Docker context. The builder uses a digest-pinned Rust 1.100 Alpine
  image and locked Cargo dependencies. It checks the ELF architecture and absence
  of a dynamic interpreter, records binary and lockfile hashes, and supplies
  compiler details and matching third-party notices. Local validation still uses
  Homebrew Rust 1.99.1. Build artifacts remain under `target/container-helpers/`.
  Both architectures pass nine scratch-container checks each: native merging,
  hook preservation and idempotence for all three agents, actual refusal and
  telemetry. The scratch images contain only the helper executable and run with
  no Python, shell or network. These checks establish helper execution on Linux;
  amd64 runs through Docker's cross-architecture execution on the arm64 host.
  Complete task-container installation and live agent hook invocation remain open.
  The full original suite, with installed-opencode tests using the loopback model
  fixture, passes with 1,561 passed and 30 skipped (78.13 seconds). Its call record
  remains `validation/experiment-task-calls.json`; the new command has dedicated
  file/process and container coverage. All 24 Rust checks, host debug/release
  builds, Clippy, formatting and the 256-file reference check pass. Original tests
  remain unchanged. IDE inspections remain unavailable because this checkout is
  not open in the IDE. Native helper staging, experiment packaging, host bindings
  and remaining parity audits stay open. Live model-backed agents and Windows
  remain unverified. Simplification and performance passes remain at 0/3 each;
  final test conversion is pending.

- Normal SWE-bench guard installation now selects a static helper using the task
  container's architecture. It validates the manifest hash, ELF machine type and
  absence of a dynamic interpreter before copying the helper. It creates the agent
  home, installs the agent's hook after scaffold setup, enables Codex hook trust and
  verifies a refusal before returning success. Missing or invalid artifacts and
  process failures retain the failure log and unguarded-row result. Process-control
  exceptions propagate, and the network escape hatch performs no container I/O.
  The explicit legacy registration methods retain their Python-program contract;
  the normal installer no longer calls them.
  Dedicated validation passes 44 local checks covering both architectures and
  their aliases, malformed or mismatched artifacts, command failures and trust.
  The same probe passes 15 further checks in an actual arm64 task container with
  no Python or network: all three agents preserve existing hooks, install
  idempotently, quote shell-sensitive paths and refuse the probe. A control command
  passes and each guard records one refusal. This invokes the registered command
  directly; live agent hook dispatch remains unverified. Existing guard integration
  still passes 289 source comparisons and two native checks.
  The full unchanged original suite passes with 1,561 passed and 30 skipped
  (67.94 seconds), including installed-opencode tests using the loopback model
  fixture. Its call record remains `validation/experiment-task-calls.json`;
  the original suite exercises the installer's network escape hatch, while the
  dedicated probe covers deployment. All 24 Rust checks, host debug/release
  builds, Clippy, formatting and the 256-file reference check pass. IDE
  inspections remain unavailable because this checkout is not open in the IDE.
  The validation binding supplies `target/container-helpers`; the standalone native
  operation defaults to `container-helpers` beside its executable. Experiment
  packaging, standalone host bindings, native scaffold merge staging, WebBench
  guard command selection and remaining parity audits stay open. Simplification
  and performance passes remain at 0/3 each; final test conversion is pending.

- Shared scaffold merging now stages a shell invocation of the native helper with
  a quoted JSON payload. The generated Python merge program is removed. The
  existing unique temporary names and cleanup remain, and shell-sensitive paths,
  substitutions, backticks, newlines and delimiter text stay data.
  SWE-bench contexts retain a lazy helper callback. It applies the guard installer's
  architecture, manifest and ELF checks, copies the helper into the retained agent
  profile's home and makes it executable. Guard registration still
  follows scaffold setup. WebBench and standalone contexts use the host helper
  beside the native executable. Deployment exceptions log a failure and return
  `false`; cancellation exceptions propagate.
  Shared scaffold APIs pass 496 source comparisons. Merge staging passes 185
  file/process comparisons against the source, checking exact bytes and failure
  effects. These replace the supplemental comparisons of generated Python text;
  original upstream tests remain unchanged. SWE-bench's retained callbacks pass
  their 820 source comparisons; the new private helper callback is excluded from
  that state comparison and exercised by the deployment pipeline.
  The final pipeline passes 21 checks across three WebBench agents and three
  SWE-bench agents in a Python-free, network-disabled arm64 container. It covers
  existing keys, one-level updates, quoted paths and payloads, temporary-file
  cleanup, guard registration after merging, retained profile state, helper
  failures and cancellation. Both static Linux helpers rebuild from the final
  source and pass all 18 scratch-container checks. Guard regressions pass all 59
  staging checks, 289 source comparisons and two native checks.
  The unchanged original suite passes with 1,561 passed and 30 skipped
  (81.16 seconds), including installed-opencode tests using the loopback model
  fixture. Its call record remains `validation/experiment-task-calls.json`.
  The private container-helper callback is covered by the dedicated pipeline.
  All 24 Rust checks, host debug/release builds, Clippy, formatting and the
  256-file reference check pass. IDE inspections remain unavailable because this
  checkout is not open in the IDE. Live agent hook dispatch, Windows and normal
  amd64 task-container staging remain unverified; scratch tests exercise amd64
  through Docker's cross-architecture execution on the arm64 host.
  Native helper deployment is shared by scaffold merges and guard installation.
  This extraction is part of production migration; formal simplification and
  performance passes remain at 0/3 each. WebBench guard command selection,
  experiment packaging, standalone host bindings and remaining parity audits
  stay open. Final test conversion is pending.

- WebBench hook commands now execute a per-task native-guard launcher. Its
  `net_guard.py` filename preserves the original Codex hook contract. The file is
  an executable POSIX shell script that forwards stdin and arguments to the native
  helper beside the experiment executable. Command quoting retains Unicode code
  points and handles spaces and apostrophes. Verification writes the launcher,
  invokes it directly, requires refusal status 2 and removes the probe's log.
  Existing settings, hook lists, guard-presence rules and Codex arming policy
  retain their source behaviour.
  The unchanged guard APIs pass 997 source comparisons covering hook shapes,
  settings paths, refusal counts, hosts files, raw objects, exception handling
  and lexical superclass dispatch. Native wiring passes 229 dedicated checks,
  including three agents, malformed settings, exact settings-file equality after
  replacing only the guard command, idempotence, executable permissions,
  Unicode and shell-sensitive paths, actual installed-hook dispatch with an empty PATH,
  refusal and telemetry. Injected launcher writes, permission changes, process
  calls, result reads, log cleanup and logger failures cover failure boundaries
  and cancellation. These checks replace supplemental traces of the old Python
  command construction; original upstream tests remain unchanged.
  Focused original tests pass with 48 passed. The full unchanged suite passes with
  1,561 passed and 30 skipped (73.02 seconds), including installed-opencode tests
  using the loopback model fixture. Its call record remains
  `validation/experiment-task-calls.json`; direct native subprocesses no longer
  need the validation binding's Python-command substitution. All 24 Rust checks,
  debug/release builds, Clippy, formatting and the 256-file reference check pass.
  IDE inspections remain unavailable because this checkout is not open in the IDE.
  Live agent hook dispatch and Windows remain unverified. Launchers require a
  writable task home and a POSIX shell. Experiment packaging, standalone host
  bindings and remaining parity audits stay open. Formal simplification and
  performance passes remain at 0/3 each; final test conversion is pending.

- `te-package --experiments --container-helpers PATH` builds a native experiment
  runtime payload from five host executables, static Linux arm64 and amd64 helpers,
  the captured Codex base prompt and the WebBench Node preload. Prompt and preload
  bytes are embedded in the packager. The mode reads its explicit binary/helper
  inputs and requires no upstream checkout at runtime. It snapshots all payload
  inputs before creating a fresh output directory, validates Linux architecture,
  ELF linkage, manifest and dependency hashes, and matching third-party notices.
  The manifest records every payload file's SHA-256. Optional ZIP output reuses
  native archive generation and preserves executable permissions.
  Dedicated validation passes 103 checks with an empty PATH: asset byte equality,
  manifest hashes, ZIP contents, relocation, native JSON-lines services, default
  helper discovery inside the relocated bundle, CLI and merge execution, invalid
  helper rejection, existing-output preservation and input/output overlap guards.
  The default output is `dist/experiments` under the caller's working directory.
  ZIP names append `.zip`, including when the output directory has a suffix.
  Both Linux helpers rebuild with the embedded licence and assets in their minimal
  Docker context and pass all 18 scratch-container checks. Existing marketplace
  and Droid packaging, ZIP permissions, host launchers, relocation and output
  guards pass their regression probe.
  The full unchanged upstream suite passes with 1,561 passed and 30 skipped
  (74.84 seconds), including installed-opencode tests using the loopback model
  fixture. Its call record remains `validation/experiment-task-calls.json`.
  All 24 Rust checks, debug/release builds, Clippy, formatting and the 256-file
  reference check pass. IDE inspections remain unavailable because this checkout
  is not open in the IDE. Windows bundle execution and live agents remain
  unverified.
  The bundle README describes its current scope. Standalone experiment host
  bindings, runtime asset selection, the benchmark entry point and remaining
  parity audits stay open. Formal simplification and performance passes remain
  at 0/3 each; final test conversion is pending.

- Codex prompt lookup now uses `assets/codex_base_prompt/<version>.md` in an
  installed runtime bundle. Unbundled development engines use the embedded pinned
  capture. Missing bundled captures return `None`; additional captures and
  Unicode version numbers use the same version matcher and file-read binding.
  The source-module path callback is removed, and the missing-capture diagnostic
  names the bundle directory. Runtime packaging and prompt lookup share one asset
  catalogue.
  Repo-map scaffolds select `te-repomap-gen` beside the engine and pass the current
  interpreter through `--python`. The generator continues to use the external
  aider SDK while repository walking, filtering and process control remain native.
  The scaffold no longer selects a reference Python generator file.
  Dedicated validation passes 54 checks for default and relocated prompt lookup,
  missing and additional captures, Unicode versions, file-read failures, actual
  native generation, budget errors, SDK failures and missing companion binaries.
  Actual generation runs through the debug engine and a relocated engine with an
  isolated aider SDK and no model calls. The repo-map corpus retains all 704 source
  comparisons: its fixture validates the native launch prefix, compares the shared
  request arguments and keeps its existing Java process stand-in. The dedicated
  probe supplies real native-generator execution. Codex installation retains 217
  comparisons, including actual capture output and offline CLI/process checks.
  SWE-bench constructor comparisons pass all 483 cases with the new capture hint.
  Focused unchanged original tests pass with 76 passed and three skipped.
  The full unchanged upstream suite passes with 1,561 passed and 30 skipped
  (109.05 seconds), including installed-opencode tests using the loopback model
  fixture. All 24 Rust checks, debug/release builds, Clippy, formatting and the
  256-file reference check pass. The runtime bundle regression passes 103 checks.
  Its debug-executable ZIP exceeded the original 30-second probe limit; the bundle
  probe now allows 120 seconds and retains the same debug-executable coverage.
  IDE inspections remain unavailable because this checkout is not open in the
  IDE. Windows execution and live agents remain unverified.
  WebBench preload selection, standalone host bindings, the benchmark entry point
  and remaining parity audits stay open. This asset catalogue extraction is part
  of production migration; formal simplification and performance passes remain at
  0/3 each. Original tests remain unchanged; final test conversion is pending.

- WebBench preload selection now uses the installed runtime's
  `assets/webbench_undici_preload.mjs`. Unbundled development engines use the
  native source asset in the build checkout. The Python binding only converts
  the selected path to a file URI. The stock abort ceiling returns before asset
  lookup; raised ceilings preserve the existing `NODE_OPTIONS` and timeout values.
  The dedicated preload probe passes 22 checks, including actual Node imports
  from development and relocated paths containing spaces, Unicode, apostrophes,
  `#` and `%`. An isolated undici fixture verifies both dispatcher timeouts without
  network or model calls. Missing bundled assets reach Node's module-load error;
  they do not select a reference file. Invalid runtime manifests propagate errors.
  The 680-case WebBench host corpus retains its object and failure checks. Its
  path fixture validates the native asset path before mapping it to the source
  fixture's path for comparison. The dedicated probe checks actual URI conversion
  and file loading. The 54 prompt/repo-map checks also pass.
  The full unchanged upstream suite passes with 1,561 passed and 30 skipped
  (70.70 seconds), including installed-opencode tests against the loopback model
  fixture. All 24 Rust checks, release build, Clippy, formatting and the 256-file
  reference check pass. IDE inspections remain unavailable because this checkout
  is not open in the IDE. Windows execution and live agents remain unverified.
  Standalone host bindings, the benchmark entry point and remaining parity audits
  stay open. Formal simplification and performance passes remain at 0/3 each;
  final test conversion is pending.

- Benchmark driver selection now executes in Rust: `load_runner`, `load_adapter`,
  `workload_ids`, `_same_group`, `load_tasks` and `lift_overhead`. It reuses the
  existing object callback protocol. Imports, signature inspection, object calls,
  mappings and file handles remain with the host binding; Rust owns the selection,
  filtering, environment precedence, fallback and copying decisions.
  Driver selection and overhead copying pass 427 source comparisons covering factory
  signatures, agent/model/scaffold forwarding, inherited environment values,
  missing factories, stub diagnostics, exception identity and active signature
  errors. Overhead copying preserves the original row and nested value references,
  honours existing fields and uses the ledger's declared keys.
  Task selection passes 1,099 source comparisons covering workload groups,
  duplicate IDs across projects, legacy prefix matching, positive/negative/large
  limits, task labels, actual JSON/UTF-8/filesystem errors and context-manager
  suppression and cleanup. No agent or external dataset is used by these probes.
  The 48 focused original tests pass. The full unchanged upstream suite passes
  with 1,561 passed and 30 skipped (76.09 seconds), including installed-opencode
  tests against the loopback model fixture. Its native call record demonstrates
  `lift_overhead`; the dedicated probes exercise the other five driver operations.
  All 24 Rust checks, debug/release builds, Clippy, formatting and the 256-file
  reference check pass. IDE inspections remain unavailable because this checkout
  is not open in the IDE. Windows execution and live agents remain unverified.
  The driver command, main stream loop, arm registry, environment-file loader,
  offline stub runner, standalone host bindings and remaining parity audits stay
  open. Formal simplification and performance passes remain at 0/3 each; final
  test conversion is pending.

- The native arm registry supplies all 27 pinned configurations in canonical
  order, including mode labels, effort overlays and the eight scaffold mappings.
  Every lookup owns its configuration. The Python binding constructs data objects
  from the native result; arm factories select their configuration in Rust.
  The registry passes 30 dedicated checks covering the full source table,
  configuration independence and lookup from a relocated engine with an empty
  environment. All 75 relevant original arm/configuration tests also pass.
  The benchmark environment loader now parses assignments, selects files,
  applies variable precedence and builds missing-credential diagnostics in Rust.
  Literal shell syntax stays literal, empty values are skipped and existing
  non-empty exports win unless override is requested. File handles, OS environment
  operations and the configuration root remain caller-owned binding data.
  The native parser is also available directly as `driver::parse_env`.
  Its 230 source comparisons exercise both parser entry points, Unicode whitespace
  and line boundaries, quotes, duplicate keys, path precedence, missing/invalid
  files, credential errors, context suppression and cleanup. Real child-process
  environment checks cover Unicode values and OS rejection of NUL in names or
  values. These probes use fixture variables and owned files.
  The full unchanged upstream suite passes with 1,561 passed and 30 skipped
  (73.30 seconds), including installed-opencode tests against the loopback model
  fixture. Its call record demonstrates one registry read and 160 native config
  lookups. Environment-loader coverage comes from the dedicated source probe.
  The 427 driver-selection and 1,099 task-selection regressions also pass.
  All 24 Rust checks, debug/release builds, Clippy, formatting and the 256-file
  reference check pass. IDE inspections remain unavailable because this checkout
  is not open in the IDE. Windows execution and live agents remain unverified.
  The driver command, main stream loop, offline stub runner, standalone host
  bindings and remaining parity audits stay open. Formal simplification and
  performance passes remain at 0/3 each; final test conversion is pending.

- The benchmark main loop now executes in Rust, including argument definitions,
  optimizer-label validation, arm overlays, the preflight signal before runner
  construction, stream setup, task coordination, UTC run-directory names and
  row/stream output. Argparse and SDK object operations remain host bindings.
  The loop preserves existing row fields, lifts declared mechanism overhead,
  counts only `resolved is True` and closes stream resources on failures within
  the task loop. Context suppression and active exceptions reach cleanup hooks.
  The offline stub's construction, transcript selection, default model, synthetic
  token/grade row and fixed optimizer answer also execute in Rust. Transcript
  bytes, tuple results, float optimizer cost and arbitrary-size task indices match
  the source. Unknown stub operations fail before requesting I/O.
  The dedicated probe passes 198 checks: 197 source comparisons and one protocol
  rejection check. It covers CLI output/errors, setup order, actual stub bytes,
  accounting, preserved fields, strict resolve counts and failures around stream
  and row files, task execution and cleanup. Five comparisons run the real
  driver/facade/adapter/registry/environment/stub path: baseline on all three
  agents, free Codex environment facts and a ten-task injection stream that
  records the stub optimizer's synthetic cost. These compare row/stream JSON
  values and transcript bytes. They make no agent or external service calls.
  The unchanged upstream suite passes with 1,561 passed and 30 skipped
  (71.50 seconds), including installed-opencode tests against the loopback model
  fixture. Main-loop and stub execution evidence comes from the dedicated probe.
  The 427 driver-selection, 1,099 task-selection and 230 environment regressions
  also pass. All 24 Rust checks, debug/release builds, Clippy, formatting and the
  256-file reference check pass. IDE inspections remain unavailable because this
  checkout is not open in the IDE. Windows and live agents remain unverified.
  The standalone benchmark command, host bindings and remaining parity audits
  stay open. Formal simplification and performance passes remain at 0/3 each;
  final test conversion is pending.

- The standalone `te-bench` command now runs the native driver through a packaged
  standard-library SDK binding. Rust owns the registry, environment policy,
  adapters, facade state, stream loop and stub behaviour. The SDK holds Python
  objects, invokes external runner callbacks and transports requests without
  importing the reference checkout. Its native facade path uses all four typed
  dataset adapters. External runners retain the `module:factory` interface.
  The experiment bundle now carries six native executables and four SDK files,
  with 23 payload hashes and an additive `benchmark_entrypoint` manifest field.
  Missing bundled SDK files fail without selecting development files.
  The relocated bundle passes 128 checks, including payload hashes, archive bytes,
  empty-PATH execution and missing-SDK rejection.
  The standalone probe passes 89 checks, including all 27 arms on three agents
  over ten-task streams, CLI errors, external callbacks, Codex policy hooks and
  interrupted stream cleanup. Row and stream comparisons normalise equivalent
  JSON numbers (`0` and `0.0`); transcript comparisons retain exact bytes.
  Another 87 transport checks exercise concurrent requests, malformed responses,
  EOF, signal delivery during and between frames, interruptible callbacks,
  restored signal handlers and owned-child cleanup without replaying requests.
  The existing 198 driver and 87 facade checks pass. The unchanged upstream suite
  passes with 1,561 passed and 30 skipped (110.61 seconds), including installed
  opencode tests against the loopback model fixture. All 24 Rust checks,
  debug/release builds, Clippy and the 256-file reference check pass.
  Built-in SWE-bench and WebBench harness SDK bindings and the remaining parity
  audits stay open. IDE inspections are unavailable because this checkout is
  not open in the IDE. Windows and live agents remain unverified. Formal
  simplification and performance passes remain at 0/3 each; final test conversion
  is pending.

- Codex policy, event, state, resume-guard and envelope-tracker objects now have
  production SDK bindings. Policy and tracker defaults come from Rust. Frozen
  policy/event dataclasses, counters, bounded command deques, failure-scope sets
  and lazy event iteration retain their Python object contracts; parsing,
  decisions, continuation requests and telemetry execute in Rust.
  The standalone driver uses these policy objects. The original test adapter now
  delegates to the production binding, removing its duplicate controller wrappers.
  The experiment bundle includes the binding among five SDK files and records
  24 payload hashes. All 132 bundle checks pass, including relocation and an
  actual benchmark command with an empty PATH.
  The controller probe passes 486 source comparisons, including shared stage
  regressions. Its standalone mode passes 444 comparisons with reference and
  `native_plugin` imports forbidden. These cover every decision rule, lazy iteration,
  resume requests, telemetry, frozen-policy replacement and mutable state
  collections. The 89 standalone benchmark and 87 transport checks also pass.
  The full unchanged upstream suite passes with 1,561 passed and 30 skipped
  (67.46 seconds), including installed-opencode tests against the loopback model
  fixture. All 24 Rust checks, debug/release builds, Clippy and the 256-file
  reference check pass. IDE inspections remain unavailable because this checkout
  is not open in the IDE. Windows, other Python versions and live agents remain
  unverified. Built-in harness SDK bindings and remaining parity audits stay open.
  Formal simplification and performance passes remain at 0/3 each; final test
  conversion is pending.

- The generic streamed-rollout controller now has a production SDK binding.
  Rust owns state folding, decisions, judge sampling, callback costs, continuation
  resets and telemetry. The binding retains judge/writer callables and exposes
  the controller's attributes and slotted state object. State fields, default
  thresholds and the resume header come from Rust. Its state methods also use
  the native fold, snapshot, digest and derived-signal operations.
  The original test adapter delegates to this binding and retains the original
  class references used by harness callers. Both its ordinary probe and its
  standalone mode pass 92 source comparisons. The standalone mode forbids
  reference-package and `native_plugin` imports. The cases cover callback
  failures, sampling backoff, costs, continuation resets, batched events,
  cumulative backstops and direct state methods. Runner controls pass 196
  comparisons, including constructor signatures, continuation and telemetry.
  The runtime bundle now contains six SDK files and 25 payload hashes. Its
  137 checks include a relocated controller with a real writer callback,
  an empty PATH and reference-package imports forbidden.
  The unchanged upstream suite passes with 1,561 passed and 30 skipped
  (66.07 seconds), including installed-opencode tests against the loopback model
  fixture. All 24 Rust checks, debug/release builds, Clippy and the 256-file
  reference check pass. IDE inspections remain unavailable because this checkout
  is not open in the IDE. Windows, other Python versions and live agents remain
  unverified. Built-in harness SDK bindings and remaining parity audits stay open.
  Formal simplification and performance passes remain at 0/3 each; final test
  conversion is pending.

- Codex installation discovery and configuration probes now have a production
  SDK binding. Rust owns binary/vendor selection, captured prompt selection,
  bundled-skill discovery, configuration diagnostics, polling, timeouts and
  cleanup ordering. The binding supplies filesystem, environment, process and
  clock operations and retains process objects across native callbacks.
  It defers signals between protocol frames, keeps SDK callbacks interruptible
  and retains callback exception objects when native cleanup propagates them.
  The original test adapter now delegates to the production binding while
  preserving its callable metadata and caller-owned SDK namespace.
  The ordinary and standalone probes each pass 220 source comparisons. The
  standalone mode forbids reference-package and `native_plugin` imports. Real
  offline fixtures verify skill discovery, configuration errors, timeouts,
  SIGINT/SIGTERM propagation, temporary-home removal and process-group cleanup.
  Path objects and callback exception identity are covered too. Runner
  constructors pass 483 comparisons; the transport passes 87 checks.
  The runtime bundle contains seven SDK files and 26 payload hashes. Its 141
  checks include relocated captured prompt loading through the new binding,
  controller callbacks, archive bytes and execution with an empty PATH.
  The unchanged upstream suite passes with 1,561 passed and 30 skipped
  (68.55 seconds), including installed-opencode tests against the loopback model
  fixture. All 24 Rust checks, debug/release builds, Clippy and the 256-file
  reference check pass. IDE inspections remain unavailable because this checkout
  is not open in the IDE. Windows, other Python versions and live agents remain
  unverified. Built-in harness SDK bindings and remaining parity audits stay open.
  Formal simplification and performance passes remain at 0/3 each; final test
  conversion is pending.

- Agent usage accounting and Claude's stream recovery now have production SDK
  bindings. Rust parses all three usage schemas, combines segment counters,
  preserves unknown costs, deduplicates assistant messages and decides whether
  recovered rows remain incomplete. The binding restores `_UsageRecovery._by_msg`
  and sends warnings to the supplied log function. `session_output_tokens`
  returns the total and a set of message IDs. Copilot log reads resolve relative
  paths against `os.getcwd()` supplied by the binding.
  The original test adapter delegates to these bindings while retaining its
  original function and class references. The ordinary and standalone probes
  each pass 474 source comparisons. The standalone mode forbids reference-package
  and `native_plugin` imports. Cases include numeric coercion, unknown costs,
  message-ID aliases, partial recovery, session-log completeness, segment merges
  and actual log-directory reads. SWE-bench and WebBench each pass 24 owned
  process checks across all three agents, including guard stops, timeouts,
  configuration refusals and resume requests. These use offline agent fixtures.
  The runtime bundle now contains eight SDK files and 27 payload hashes. Its
  145 checks include relocated accounting calls for all three agents and Claude
  recovery warnings delivered through the supplied log function.
  The unchanged upstream suite passes with 1,561 passed and 30 skipped
  (65.55 seconds), including installed-opencode tests against the loopback model
  fixture. All 24 Rust checks, debug/release builds, Clippy and the 256-file
  reference check pass. IDE inspections remain unavailable because this checkout
  is not open in the IDE. Windows, other Python versions and live agents remain
  unverified. Built-in harness SDK bindings and remaining parity audits stay open.
  Formal simplification and performance passes remain at 0/3 each; final test
  conversion is pending.

- The shared provider, runner, scaffold and driver object/I/O binding now ships
  in the SDK. The original test adapter registers methods and supplies counted
  test channels; its duplicate callback dispatcher has been removed. The SDK
  retains opaque objects, context-manager methods, active exceptions, captured
  callbacks and lazy generators while Rust drives the operations. Cleanup uses
  isolated owned engine connections.
  The source-independent provider namespace binds 15 credential, routing,
  catalogue and pricing operations. Rust supplies credential names and endpoint
  defaults. Both provider probes pass 279 source comparisons, including real
  loopback HTTP, cache identity, mapping mutations and actual pricing checks.
  Those checks exposed Rust debug formatting in unknown-model diagnostics;
  pricing now formats the model and agent with Python-compatible representations.
  The SDK transport passes 97 checks, including isolated process lifetime and
  continued use of the outer channel.
  Six complete offline WebBench task pipelines exposed a missing task home on
  the Copilot path: its settings live elsewhere, and writing the native guard
  launcher failed before the scaffold created that home. Native guard verification
  now creates the home first. All six pipelines and 236 guard checks pass,
  including fresh homes, actual refusals and telemetry. The evaluator-start probe
  checks pinned preload bytes and the native asset URI. The task probe verifies
  the installed native guard instead of expecting an in-process bridge call.
  Targeted optimizer, logging, preflight, runner, scaffold, evaluator, workspace
  and driver probes pass too, including eight real signal-cleanup cases.
  The runtime bundle contains nine SDK files and 28 payload hashes. Its 149
  checks include relocated provider routing and credential-map mutation with
  reference imports forbidden. The standalone benchmark passes 89 checks.
  The unchanged upstream suite passes with 1,561 passed and 30 skipped
  (68.18 seconds), including installed-opencode tests against the loopback model
  fixture. All 24 Rust checks, debug/release builds, Clippy and the 256-file
  reference check pass. IDE inspections remain unavailable because this checkout
  is not open in the IDE. Windows, other Python versions and live agents remain
  unverified. Built-in harness SDK bindings and remaining parity audits stay open.
  Formal simplification and performance passes remain at 0/3 each; final test
  conversion is pending.

- The SDK now assembles the shared Claude, Copilot and Codex optimizer namespaces
  without importing reference code. Its 23 new bindings cover optimizer calls,
  endpoint preflights, log paths and context, prompt classification, logging,
  Copilot installation discovery and sealed Copilot/Codex invocation builders.
  Rust supplies the default endpoint, prompt signatures, tool lists and container
  paths. The existing shared binding preserves opaque arguments and callback
  ordering; the SDK connects the Copilot call to native usage accounting.
  Seven standalone probe groups pass 2,407 source comparisons: 237 HTTP, 165 CLI,
  217 context, 307 logging, 241 preflight, 279 Copilot discovery and 961 invocation
  cases. The same groups pass through the original API bindings. Real loopback
  HTTP responses for Claude and Codex, plus an offline Copilot executable, also
  reach native pricing and real optimizer log files. Copilot's integrated case
  verifies cache-read/cache-write accounting and owned-directory cleanup.
  The bundle passes 153 checks with ten SDK files and 29 payload hashes. Its relocated SDK check
  includes optimizer logging, prompt classification, Codex environment and Copilot
  invocation with reference imports forbidden. Adjacent provider and standalone
  benchmark probes pass 279 comparisons and 89 checks. All 24 Rust checks,
  debug/release builds, Clippy and the 256-file reference check pass. The unchanged
  upstream suite passes with 1,561 passed and 30 skipped (72.29 seconds), including
  installed-opencode tests against the loopback fixture.
  IDE inspections remain unavailable because this checkout is not open in the IDE.
  Windows, other Python versions and live agents remain unverified. Built-in
  harness/scaffold class bindings and the remaining parity audits stay open.
  Formal simplification and performance passes remain at 0/3 each; final test
  conversion is pending.

- The SDK now ships shared scaffold classes, RTK, Serena, repo-map and their registry.
  Its 33 bindings preserve caller-owned profiles, contexts, callbacks, subclass
  dispatch, static/class methods and retained generators. Rust supplies pinned
  metadata and the companion repo-map generator path; arm selection reuses the
  native arm registry. Setup, archive filtering and cleanup use the existing Rust
  policies through the shared object/I/O binding.
  Five standalone probe groups pass 2,725 source comparisons: 496 shared-class,
  185 merge-staging, 426 Serena, 780 RTK, 704 repo-map and 134 registry cases.
  The same probes pass through the original API bindings. Coverage includes actual
  merge processes, archives, files, callbacks and offline generator processes;
  no tools are downloaded. The bundle passes 161 checks with 12 SDK files and 31
  payload hashes. Its relocated SDK check selects real scaffold classes and writes
  instruction and MCP files while rejecting reference imports.
  The unchanged upstream suite passes with 1,561 passed and 30 skipped (73.47
  seconds), including the installed-opencode loopback fixture. All 24 Rust checks,
  debug/release builds, Clippy and the 256-file reference check pass. IDE inspections
  remain unavailable because this checkout is not open in the IDE. Windows, other
  Python versions, live tools and paid agents remain unverified. Built-in harness
  class bindings and the remaining parity audits stay open. Formal simplification
  and performance passes remain at 0/3 each; final test conversion is pending.

- The task-lifecycle and constructor object/I/O dispatchers now ship in the SDK.
  The original API adapters register the packaged bindings and supply counted
  channels. The SDK accepts caller-provided class declarations; signature-only
  fixture interfaces raise if an unbound body executes, and all reference imports
  are forbidden in standalone probes. Rust supplies effort levels, protected
  capabilities, default system skills and Web-Bench timeout/scaffold metadata.
  Six standalone probe groups pass 2,592 source comparisons: 288 task/grading,
  292 container, 196 controller, 466 Codex configuration, 489 SWE-bench constructor
  and 861 Web-Bench constructor cases. Both task and constructor probes exercise
  real SIGINT/SIGTERM delivery, cleanup or partial state, and handler restoration
  (12 signal cases in total). Original API probes pass too. The guard probe passes
  289 source comparisons and four native checks, including a changed helper
  directory; helper paths resolve at each installation.
  The bundle passes 169 checks with 14 SDK files and 33 payload hashes. Its
  relocated SDK exercises native constructor rejection and network-tool policy
  without executing supplied interface bodies or importing reference code.
  The unchanged upstream suite passes with 1,561 passed and 30 skipped (66.49
  seconds), including the installed-opencode loopback fixture. All 24 Rust checks,
  debug/release builds, Clippy and the 256-file reference check pass. IDE inspections
  remain unavailable because this checkout is not open in the IDE. Windows, other
  Python versions and live agents remain unverified. Full built-in class assembly,
  guard convenience APIs and the remaining parity audits stay open. Formal
  simplification and performance passes remain at 0/3 each; final test conversion
  is pending.

- The eight SWE-bench guard descriptors and convenience methods now execute in
  Rust through packaged SDK bindings, replacing the original class bodies in
  upstream tests. Native policy builds paths, forwards attempt counts, constructs
  entries, preserves custom merge programs, checks process results and records
  Codex hook trust. The legacy merge-program API returns a Python I/O launcher;
  JSON hook merging and idempotence execute in the native helper.
  The helper accepts entries over stdin, preserving arbitrary JSON values and
  the source's marker-based idempotence. Native registration writes commands that
  launch the guard executable directly. Generic merge callers retain their entry
  data. Guard probes pass 554 source comparisons and five native checks through
  both the original API and a reference-free SDK namespace, including malformed
  files, repeated registration, callback failures, Unicode, arbitrary entries and
  an entry exceeding 200,000 characters. All 33 original answer-key guard tests pass too;
  recorded calls confirm that the convenience APIs reach Rust.
  Unknown guard operations return a validation error and leave the owned channel
  usable.
  Both static Linux architectures were rebuilt and pass 26 scratch-container
  checks, including large stdin entries and both idempotence rules, without Python,
  a shell or network. The runtime bundle passes 173 checks with 15 SDK files and
  34 payload hashes. Its relocated SDK executes both merge modes and verifies
  native hook commands while rejecting reference imports. Standalone task
  integration also passes 288 source comparisons, including real signal cases.
  The unchanged upstream suite passes with 1,561 passed and 30 skipped (71.24
  seconds), including the installed-opencode loopback fixture. All 24 Rust checks,
  debug/release builds, Clippy and the 256-file reference check pass. IDE inspections
  remain unavailable because this checkout is not open in the IDE. Windows, other
  Python versions and live agents remain unverified. Full built-in class assembly
  and the remaining parity audits stay open. Formal simplification and performance
  passes remain at 0/3 each; final test conversion is pending.

- Sixteen SWE-bench runner method and factory bindings now ship in the
  SDK. They cover agent segments, segment merging, optimizer and preflight calls,
  retained scaffold callbacks, session logs, runner construction, container
  sweeping and signal registration. Original API adapters use the same bindings.
  Cleanup retains a separate engine connection. Callback documentation now lives
  with its SDK signature instead of being extracted from the original function's
  bytecode. Native harness metadata also supplies Copilot's container log path.
  A reference-free namespace passes 1,067 segment, 457 runner/factory, 272 cleanup
  and 820 scaffold-context source comparisons. Its 24 owned process checks cover
  three agents, guard stops, deadlines, configuration refusals, resumes and usage
  recovery without a model or Docker daemon. The original API also passes 914
  runner/factory, 1,067 segment and 820 scaffold-context comparisons.
  The relocated bundle passes 177 checks with 16 SDK files and 35 payload hashes,
  including retained binary-read callbacks, session logs and runner factory calls.
  Standalone guard integration retains 554 source comparisons and five native
  checks, and task integration retains 288 comparisons. The unchanged upstream
  suite passes with 1,561 passed and 30 skipped (72.07 seconds), including the
  installed-opencode loopback fixture. All 24 Rust checks, debug/release builds,
  Clippy and the 256-file reference check pass.
  Full built-in class assembly and WebBench's remaining namespace bindings are
  still open, along with the parity audits. Formal simplification and performance
  passes remain at 0/3 each; final test conversion is pending. IDE inspections,
  Windows, other Python versions and live agents remain unverified.

- Built-in SWE-bench class assembly is now packaged. The SDK supplies all three
  runner classes, their descriptors and signatures, the grader, profiles and
  helper namespaces. Every declared method is bound before the namespace is
  returned. The benchmark driver retains this namespace for
  `te_runtime.swebench:make_runner`; external factories continue through the
  standard import binding. Native metadata supplies the three profiles and guard
  probe command. Claude binary discovery, CLI version decisions and official
  grader import order now execute in Rust too, preserving opaque return values
  and exception boundaries through SDK callbacks.
  The standalone validation host now uses these production classes, removing its
  SWE-bench signature-only fixtures. It passes 489 constructor, 288 task, 1,067
  segment, 820 scaffold-context, 457 runner/factory and 272 cleanup comparisons.
  Guard integration passes 554 source comparisons and five native checks.
  The new host-helper probe passes 263 comparisons through both the original API
  and standalone SDK, including real version processes, local official-grader
  package imports, repeated-path handling and the three agent profiles. All 24
  owned offline agent process checks pass through the assembled classes.
  The relocated bundle passes 182 checks with 17 SDK files and 36 payload hashes.
  It verifies bound descriptors, profile assembly, retained factory namespaces,
  native selection and startup credential validation without reference imports.
  Driver main/stub validation retains 198 checks, and standalone WebBench
  constructors retain 861 comparisons. The unchanged upstream suite passes with
  1,561 passed and 30 skipped (69.45 seconds). All 24 Rust checks, debug/release
  builds, Clippy and the 256-file reference check pass. WebBench assembly and
  remaining parity audits stay open. Formal simplification and performance passes
  remain at 0/3 each; final test conversion is pending. IDE inspections, Windows,
  other Python versions and live agents remain unverified.

- Built-in WebBench class assembly and its method mappings now ship in the SDK.
  The namespace supplies 16 module functions, five classes and 71 method
  signatures, including all three runners, the evaluator session and HTTP handler.
  Every declaration is bound before the namespace is returned. Native callbacks
  retain the lexical class for `super()` dispatch in task wrappers and Codex guard
  installation. Rust supplies prompts, filters, attempt-pattern metadata, timeout
  defaults and class constants. Both factories retain their namespaces and share
  the same agent and scaffold services in a driver. The source-independent
  validation host now assembles both production harnesses; its final signature-only
  fixture module has been removed. Original API adapters use the shipped WebBench
  mappings too.
  The assembled namespace passes 861 constructor comparisons and 214 cleanup
  comparisons. Runner/factory validation passes 914 comparisons across both
  domains. Six owned offline HTTP/process pipelines pass across three agents,
  exercising guard registration, workspace edits, agent segments, usage replies,
  scaffold callbacks, exclusions and evaluator cleanup without a model or
  Playwright. The relocated bundle passes 191 checks with 19 SDK files and 38
  payload hashes, including both native factory selections, bound descriptors,
  shared services and startup credential validation without reference imports.
  Driver main/stub validation retains 198 checks. The unchanged upstream suite
  passes with 1,561 passed and 30 skipped (71.71 seconds). All 24 Rust checks,
  debug/release builds, Clippy and the 256-file reference check pass. Production
  parity audits remain open. Formal simplification and performance passes remain
  at 0/3 each; final test conversion is pending. IDE inspections, Windows, other
  Python versions and live agents remain unverified.

- Detector and extractor patterns now compile Python's global and scoped ASCII
  flags in Rust, including Unicode overrides, character classes, word boundaries,
  case folding, backreferences and conditionals. Original pattern text remains in
  snapshots. Invalid syntax remains a missing matcher; incompatible global ASCII
  and Unicode flags propagate the source error through detector, extractor, fact
  and composed-stage construction. PCRE2 replaces the detector's matching engine
  because the previous engine rejected valid mixed-character ASCII backreferences
  and compared some Unicode backreferences using the wrong byte length. Complete
  UTF-8 atoms preserve character boundaries during ASCII caseless byte matching.
  The compiler passes 199 source comparisons, including 40 detector verdict cases,
  12 extractor cases, invalid references, Unicode group names and quantifiers.
  All 14 focused injection, profile and facade tests pass. The unchanged upstream
  suite passes with 1,561 passed and 30 skipped (68.33 seconds), including installed
  opencode tests against their local stand-in model. All 24 Rust checks,
  debug/release builds, Clippy, formatting and the 256-file reference check pass.
  Both rebuilt static Linux helpers pass 26 scratch-container checks without
  Python, a shell or network. The relocated bundle passes 196 checks, including
  ASCII detector matches, mixed-character backreferences and a rejected match.
  The macOS bridge has no dynamic PCRE2 dependency. Third-party notices cover all
  86 locked registry crates and include bundled PCRE2 and JIT licence headers.
  Unicode character-name escapes, combined ASCII/Unicode caseless backreferences,
  remaining Unicode-version differences and error parity remain open. Formal
  simplification and performance passes remain at 0/3 each; final test conversion
  is pending. IDE inspections remain unavailable because this checkout is not
  open in the IDE. Windows and other Python versions remain unverified.

- Detector and extractor patterns now resolve `\N{name}` in Rust, including
  character classes, ranges, quantifiers and backreferences. Unicode 17's
  compressed name database supplies canonical characters; an exact copy of the
  official 481-entry alias table preserves Python's spellings. Canonical-name
  validation retains spaces and hyphens while accepting ASCII case changes.
  The compiler derives names for the two Tangut ranges that the crate omits.
  Unknown names, loose spellings, malformed escapes and named sequences retain
  the source's missing-matcher result. Lookup runs inside the executable without
  reading files or starting Python.
  The expanded corpus passes 2,256 source comparisons, including all aliases in
  original and lowercase spellings, class and ASCII-caseless contexts, algorithmic
  range boundaries, escaped punctuation and detector/extractor integration.
  The unchanged upstream suite passes with 1,561 passed and 30 skipped (77.59
  seconds), including installed opencode tests against their local stand-in model.
  All 24 Rust checks, debug/release builds, Clippy, formatting and the 256-file
  reference check pass. Both rebuilt static Linux helpers pass 26 scratch-container
  checks without Python, a shell or network. The relocated bundle passes 198
  checks, including named escapes and their ASCII caseless backreferences.
  Dependency notices cover all 101 locked registry crates and the Unicode data
  licence. All 86 previous crate sections remain unchanged; fifteen are added.
  Mixed ASCII/Unicode caseless backreferences, remaining Unicode-version and error
  parity audits remain open. Formal simplification and performance passes remain
  at 0/3 each; final test conversion is pending. IDE inspections remain unavailable
  because this checkout is not open in the IDE. Windows and other Python versions
  remain unverified.

- Detector and extractor Unicode character classes now include all 4,657 word
  characters and ten decimal digits added in Unicode 17. Literal and range
  matching also includes its 28 changed simple-fold groups. Group-name validation
  uses the already-locked Unicode 17 identifier library directly. The pinned
  regex-syntax tables remain the Unicode 16 base; generated corrections supply
  the additions before classes and boundaries are compiled into PCRE2 patterns.
  ASCII mode retains its ASCII classes and folding rules.
  The Unicode probe passes 4,560 source comparisons. Its anchored positive and
  negative class checks cover 3,336,192 memberships: every Unicode scalar for
  each of word, digit and whitespace. It also checks every simple-fold group,
  all valid identifier code points with positive source assertions, invalid
  identifier boundaries and case-insensitive ranges. The existing regex corpus
  passes 2,264 comparisons, including the reproduced Tolong Siki and Beria Erfe
  failures. The unchanged upstream suite passes with 1,561 passed and 30 skipped
  (72.49 seconds), including installed opencode tests against their local stand-in
  model. All 24 Rust checks, debug/release builds, Clippy, formatting and the
  256-file reference check pass.
  Both rebuilt static Linux helpers pass 26 scratch-container checks without
  Python, a shell or network. The relocated bundle passes 203 checks, including
  Unicode 17 word/digit classes, literal folding and group identifiers.
  The generator verifies five SHA-256-pinned official Unicode inputs and
  reproduces the committed tables byte for byte. The distribution notices now
  include the official Unicode data licence alongside all 101 locked crates.
  Unicode backreference folding, mixed ASCII/Unicode caseless backreferences,
  other production patterns and the remaining syntax/error audits stay open.
  Formal simplification and performance passes remain at 0/3 each; final test
  conversion is pending. IDE inspections remain unavailable because this checkout
  is not open in the IDE. Windows and other Python versions remain unverified.

- Caseless detector and extractor backreferences now apply Python's per-reference
  ASCII or Unicode 17 lowercase rules. Mixed ASCII/Unicode scopes compile and
  match natively. This fixes the previous engine's `Sſ`, `Σς`, `İi` and Beria Erfe
  differences. Literal case folding retains its separate equivalence rules.
  PCRE2 calls Rust to validate the captured substring and determine its matching
  end position. An atomic wrapper retains that position while PCRE2 consumes the
  reference. Native backreference consumption handles ordinary matches; UTF-8
  character matching handles the remaining valid lowercase pairs. The compiler
  tracks fixed character widths for lookbehind. Original pattern text and capture
  numbering remain intact. ASCII-only subjects use the existing native matcher.
  The whole-pattern byte encoder and its mixed-mode restriction have been removed.
  The dedicated probe passes 92 source comparisons covering 61,458 raw searches
  and detector/extractor integrations. They include every Unicode 17 simple
  lowercase and case-fold pair, mixed scopes, conditionals, optional and repeated
  captures, lookarounds, fixed-width quantifiers and 32,769-character captures.
  All 27 Rust checks pass, including concurrent matches, embedded NUL/newline
  captures and long successful/unsuccessful searches. Each Linux architecture's
  build also runs the three callback tests before exporting its static helper.
  The existing regex corpus retains 2,264 comparisons. The Unicode corpus retains
  4,560 comparisons and 3,336,192 scalar class memberships. The unchanged upstream
  suite passes with 1,561 passed and 30 skipped (78.66 seconds), including installed
  opencode tests against their local stand-in model. Debug/release builds, Clippy,
  formatting, exact table regeneration and the 256-file reference check pass.
  Both static Linux helpers pass 26 scratch-container checks without Python, a
  shell or network. The relocated runtime bundle passes 212 checks, including
  Unicode lowercase, mixed-mode, lookbehind and empty backreferences.
  Remaining syntax, runtime-error, resource-limit and other production parity
  audits stay open. Formal simplification and performance passes remain at 0/3
  each; final test conversion is pending. IDE inspections remain unavailable
  because this checkout is not open in the IDE. Windows and other Python versions
  remain unverified.

- The results aggregator now runs entirely in Rust as `te-bench aggregate`.
  It discovers result trees, normalises cells, keeps each task's newest run and
  writes the task and cell CSVs using the native pricing and overhead ledger.
  Legacy integer task costs and integer overhead sums retain their exact digits
  within JSON's i64/u64 range; larger integer sums return a diagnostic.
  It preserves vendor-cost, incomplete-usage and ungraded-task flags, including
  WebBench attempt denominators. The subset guard runs before directory creation
  or CSV writes, preserving an existing canonical index. CSV records retain
  CRLF, quoting, integer/float spellings and Python-style boolean text.
  All 35 original aggregation, vendor-cost and grading-integrity tests pass
  through the native binding. Their function trace executes only the source
  aggregator's module initialiser. The dedicated probe passes 155 source comparisons,
  including exact CSV bytes, console messages, invalid JSON, help exits and
  subset-index preservation. The native command runs with an empty PATH and an
  unavailable Python executable. The relocated runtime bundle passes 214 checks,
  including aggregation of the actual standalone benchmark's output.
  The full original suite passes with 1,561 passed and 30 skipped (69.46 seconds),
  including the installed opencode integration against its local model fixture.
  All 27 Rust checks, debug/release builds, Clippy, formatting and the 256-file
  reference check pass.
  The production coverage audit uses Python's built-in function tracer and native
  call counts. It exposed unconverted workload, sweep and export functions, now
  listed explicitly above. Original tests remain unmodified. Formal simplification
  and performance passes remain at 0/3 each; final test conversion is pending.
  Extreme counters, non-finite JSON, unusual paths and remaining CLI/I/O diagnostic
  differences still need parity checks. Windows remains unverified. The existing
  Linux guard helpers were reused; this change adds no helper behaviour or dependencies.

- The published workload registry and runnable-manifest exporter now execute in
  Rust. Published SWE-bench counts stay at 229 tasks. New runs apply the one
  exclusion and 49 additions to produce 277 tasks; the original stream order is
  preserved. WebBench, CredData and ReposVul retain their own groups and seeds.
  Task filtering preserves record identity and input order. The native registry
  supplies image tags, task counts, published instances and runnable instances.
  `te-bench export-workloads` writes the four manifests into `./workloads`;
  `--check` reports missing or stale files without rewriting them. Generated JSON
  bytes, provenance and original console output match the source. Check mode
  accepts CRLF files using the source's universal-newline rule. The compiled
  executable carries all task data; it needs no Python, SDK or source checkout.
  Workload constants and descriptions are regenerated and checked by
  `jbang validation/WorkloadProbe.java --write` and the same command without
  `--write`. The probe exports data through the original module and compares
  every field with the embedded asset. It adds 288 source comparisons for the
  APIs, manifest bytes, errors, record identity, help and stale-file preservation.
  All 78 original workload, sweep-arm and aggregation-index tests pass through
  the bindings. A function trace of the 73 workload and sweep-arm tests executes
  only module initialisers in the two ported source files. The full original
  suite passes with 1,561 passed and 30 skipped
  (69.77 seconds), including installed opencode against its local model fixture.
  The 155 aggregation comparisons still pass after sharing relative-path and
  console result handling. The relocated bundle passes 220 checks, including
  byte comparisons of all four manifests with the committed source versions and
  check mode with Python unavailable. All 27 Rust checks, debug/release builds,
  Clippy, formatting and the unchanged 256-file source check pass.
  The original tests remain unmodified. Sweep planning, dataset exports and the
  supporting-script audit remain pending before the six improvement passes and
  final test conversion. Windows, unusual paths and remaining CLI/I/O diagnostic
  details remain unverified. Existing Linux guard helpers were reused because
  their behaviour and dependencies did not change.

- Sweep planning now executes in Rust as `te-bench sweep-plan`, including the
  complete `arms`, `cells`, `counts` and `estimate` modes. It reads a local workload
  manifest when present and otherwise uses the embedded runnable registry.
  Explicit relative manifests use the caller's working directory. Seed-major
  enumeration, empty seed lists, per-cell limits, duplicate-repository counting,
  unknown-repository warnings and WebBench scaffold exclusions preserve the source
  behaviour. Derived time and dollar assumptions remain behind `--show-cost`.
  The planner reuses the native manifest reader and argument-choice diagnostics.
  The dedicated probe passes 404 source comparisons covering all modes, exact
  console text, Unicode group names, manifest precedence, malformed JSON, UTF-8
  errors, integer rate types, rounding, signed and padded seeds and refusal paths.
  All 73 original sweep-arm and workload tests pass with native function bindings.
  The full original suite passes with 1,561 passed and 30 skipped (68.30 seconds),
  including installed opencode against its local model fixture. The relocated
  runtime bundle passes 224 checks, including native planning from the exported
  manifests, local overrides and scaffold filtering with an empty PATH.
  All 27 Rust checks, debug/release builds, Clippy, formatting and the unchanged
  256-file reference check pass. No dependencies or guard-helper behaviour changed;
  the existing Linux helpers were reused. The archived shell launchers still run
  their Python child scripts and remain pending production work. Source-test
  function traces cover the pytest process; the native CLI is checked separately.
  Original tests remain unmodified. Dataset exports and the supporting-script
  audit remain pending before the three simplification passes, three performance
  passes and final test conversion. The planner currently requires finite rates
  and uses Rust decimal float spellings, JSON integers for seeds and i64 for
  limits. Wider counters and additional Python numeric spellings still need
  conversion. Native Windows execution and remaining CLI/I/O diagnostic details
  remain unverified.

- SWE-bench dataset export now uses Rust for task selection, Verified-row
  precedence, canonical stream order, grading-field projection, missing-task
  checks and all three output files. `te-bench export-swebench --out-dir DIR`
  keeps only the external Hugging Face `load_dataset(path, split="test")` call
  and iterator operations in the Python SDK binding. Rust requests one record
  at a time, so a malformed row stops iteration before another row is read.
  SDK errors retain their original Python exception object. Missing tasks stop
  the export before it creates directories or writes files. Filesystem failures
  preserve the source's write order and partial outputs. JSON escaping, field
  order, spaces and image-tag newlines match the original files. Help and
  argument errors run in the native frontend without Python or the dataset SDK.
  All 32 original workload tests pass, including the export-order assertion.
  Their parent-process function trace executes only the exporter module's
  initialiser; the build and iterator decisions reach Rust through counted calls.
  The dedicated probe passes 57 source comparisons for helper and CLI behaviour,
  complete file bytes, first-row precedence, malformed records, missing tasks,
  SDK import failures, iterator failures and stop points, exception identity,
  empty output paths and file-write errors. The actual standalone command also
  writes the three reference files byte-for-byte against the offline SDK fixture.
  The full original suite passes with 1,561 passed and 30 skipped (66.91 seconds),
  including installed opencode against its local model fixture. All 27 Rust
  checks, debug/release builds, Clippy, formatting and the unchanged 256-file
  reference check pass. The relocated runtime bundle passes 233 checks. It
  carries 20 SDK files, 39 manifest hashes and 40 physical files; all archive
  bytes and hashes pass. Its exporter runs with an empty PATH and rejects
  reference-package imports while reproducing all three reference exports.
  The dataset API call follows the
  [official loading documentation](https://huggingface.co/docs/datasets/loading).
  Live dataset downloads remain unverified. No dependency or guard-helper
  behaviour changed; existing Linux helpers were reused. Original tests remain
  unmodified. The WebBench exporter, YAML mirror, launch workflows and supporting
  tools remain pending before the three simplification passes, three performance
  passes and final test conversion. Windows, additional CLI spellings and values
  outside the dataset's JSON row schema remain outside the verified corpus.

- WebBench driver task export now runs entirely in Rust as `te-bench
  export-webbench`. It reads the five local project files, selects all 102 tasks
  in registry order, uses the last record for duplicate IDs and preserves
  description values. Flags override the two environment defaults; empty flags
  fall back to those defaults. The exporter validates every project before
  writing, compares existing output with universal newline handling and leaves
  matching files untouched. `--check` refuses missing or stale output without
  writing. Missing parent directories and file errors retain the source's
  behaviour. Native JSON output preserves UTF-8, key order, Python numeric
  spellings, two-space indentation and the final newline.
  The dedicated probe passes 71 source comparisons and five standalone checks
  with an empty PATH and an unavailable Python interpreter. It also passes all
  32 original workload tests with populated project and task paths, including
  the exporter assertions that otherwise return early. The existing SWE-bench
  exporter retains 57 comparisons and the official grader retains 127, including
  35 exact JSON diagnostics. The relocated runtime bundle passes 238 checks;
  its WebBench command reproduces the reference bytes and checks stale output
  without Python. Payload counts remain 20 SDK files, 39 manifest hashes and
  40 physical files. The full original suite passes with 1,561 passed and 30
  skipped (70.27 seconds), including installed opencode against its local model
  fixture. All 27 Rust checks, debug/release builds, Clippy, formatting and the
  unchanged 256-file reference check pass. The full-suite log is
  `validation/results/web-export-full.log`; counted native calls are in
  `validation/results/web-export-full-calls.json`. The suite needs the configured
  Git signing agent and localhost socket access; its sandboxed attempt failed
  on those restrictions, and the rerun with that access passed.
  Original tests remain unmodified. The YAML mirror, launch
  workflows and supporting tools remain pending before the three simplification
  passes, three performance passes and final test conversion. Windows and the
  remaining CLI/JSON edge audits remain unverified. JSONL currently decodes each
  complete file before parsing its lines, so compound malformed-JSON/late-UTF-8
  failures can occur in a different order from the source's buffered reader.

- The WebBench YAML mirror now uses Rust for project selection, field projection,
  conversion decisions, JSONL record assembly, stale-file checks and writes.
  `te-bench webbench-from-yaml` accepts explicit project names or `--all`, which
  selects swept projects with a `tasks.yml` in registry order. Its iterator API
  checks each project when consumed, preserving changes made between yields.
  The existing dataset SDK binding calls PyYAML `safe_load`, retains its objects
  and performs requested value operations and JSON encoding. Dates, aliases,
  non-finite numbers, large integers and dependency exceptions therefore retain
  their Python behaviour. Rust completes all four field conversions before
  requesting JSON encoding, preserving failure order. Missing fields become
  empty strings; dates become strings and descriptions are stripped.
  The native command preserves matching files, checks all selected stale files
  without writing, and retains earlier writes and console lines when a later
  project fails. YAML files remain authoritative and unchanged. Help and
  argument errors run before Python startup.
  The dedicated probe passes 103 source comparisons and seven standalone checks,
  covering actual PyYAML 6.0.3 objects, malformed documents,
  block scalars, merge aliases, cyclic values, conversion/encoding error order,
  lazy project selection, environment precedence, partial writes, missing SDK
  dependencies and callback exception identity. Standalone and relocated-bundle
  checks reject reference/test-package imports and compare reference file bytes.
  The relocated bundle passes 245 checks and retains 20 SDK files, 39 manifest
  hashes and 40 physical files. The SWE-bench exporter retains 57 source
  comparisons. The full unchanged original suite passes with 1,561 passed and
  30 skipped (72.04 seconds), including installed opencode against its local
  model fixture. All 27 Rust checks, debug/release builds, Clippy, formatting and
  the unchanged 256-file reference check pass. Full-suite evidence is in
  `validation/results/yaml-mirror-full.log` and
  `validation/results/yaml-mirror-full-calls.json`. The original workload suite
  contains no direct YAML-mirror test; the dedicated probe supplies that coverage.
  PyYAML was added only to the ignored validation virtual environment; the
  runtime interpreter needs that existing upstream dependency, documented by
  [PyYAML's API reference](https://pyyaml.org/wiki/PyYAMLDocumentation).
  No Rust dependency or Linux guard-helper behaviour changed. Original tests
  remain unmodified. The experiment inventory still includes `compare_arms.py`
  and five unported launch shell scripts. Those and the supporting-tool audit remain
  pending before the three simplification passes, three performance passes and
  final test conversion. Windows and remaining CLI/Unicode edge audits remain
  unverified. YAML parsing, Python object operations and JSON value encoding
  remain SDK calls; their process and transport costs need performance coverage.

- Paired cell comparison now runs entirely in Rust as `te-bench compare-arms`.
  The loader sorts run paths, keeps the latest row per instance, and retains an
  older valid run configuration when a newer row has no valid configuration.
  The report excludes unpaired and usage-incomplete tasks, charges the shared
  model-weighted execution and mechanism costs, and preserves the original
  console report and JSON metrics. It includes prefix groups, resolve flips,
  saved fact lifecycles, the next rendered fact block, and the first stream
  line with an injectable fact. Errors retain earlier console output and leave
  an existing JSON report untouched until the final write.
  Source comparisons exposed two numeric discrepancies: default JSON float
  parsing changed one finite input by one floating-point step, and a rounded
  sum followed by division differed from Python's exact mean. JSON parsing now
  enables `float_roundtrip`; means use `num-rational` with exact integer and
  binary-float inputs before one final conversion. The lockfile adds
  `num-rational`, `num-bigint` and `num-integer`; `num-traits` is now also a direct
  dependency. The 104-crate third-party notice set has been regenerated.
  A separate source case found lost low-order terms in the shared ledger's
  floating-point overhead sum. The ledger and report now use the same
  compensated summation helper, preserving both the cancellation example
  (101 effective tokens) and positive terms of different magnitudes.
  The dedicated probe passes 89 source comparisons and three standalone checks
  with Python unavailable, covering loader replacement/configuration retention,
  wildcard paths, numeric instance keys, fractional and large token counts,
  strict resolve booleans, prefix groups, saved facts, stream line numbering,
  malformed metadata, exceptions and exact console/JSON output. Aggregation
  retains 155 source comparisons after the shared numeric fixes.
  The full unchanged original suite passes with 1,561 passed and 30 skipped
  (83.07 seconds), including installed opencode against its local model fixture.
  All 27 Rust checks, debug/release builds, Clippy, formatting and the unchanged
  256-file reference check pass. The full-suite log and counted calls are
  `validation/results/compare-full.log` and
  `validation/results/compare-full-calls.json`. Both static Linux helpers were
  rebuilt against the updated lockfile and parser; their backreference tests
  pass and both architectures pass 26 scratch-container checks without Python,
  a shell or network. The relocated runtime bundle passes 248 checks with
  20 SDK files, 39 manifest hashes and 40 physical files.
  Original tests remain unmodified; their suite has no direct comparison-tool
  test, so a dedicated source oracle supplies that coverage. The remaining
  experiment launch scripts and supporting-tool audit precede the three
  simplification passes, three performance passes and final test conversion.
  Windows, non-finite or wider-than-native JSON numeric values, mixed wide
  integer/float sums through the shared f64 ledger API, remaining CLI
  spellings and malformed saved-state edge cases remain outside this corpus.
  Whole-file stream decoding can prioritise a later UTF-8 error over an earlier
  malformed JSON line. Exact-rational arithmetic still needs performance coverage.

- Shared launch configuration, preflights and fleet CPU controls now execute in
  Rust. `te-bench fleet-budget [cores]` discovers the parent of a rootless Docker
  scope, falls back to the canonical user-manager cgroup, and checks or writes
  `cpu.max`. It reports throttling and optional Docker usage without Python or
  shell utilities. The original seven fleet tests drive this native command
  against fake cgroup trees; no real host CPU budget was changed.
  The shared library ports all `_env.sh` functions: environment-file selection
  and loading, exported-variable precedence, model spelling and tree tags,
  credential routing, endpoint removal/validation, Copilot authentication warm-up,
  quota refusal, model fallback, effort caching, billed-probe dry-run skipping
  and detached process startup. Test compatibility code transports Bash scalar
  values and export flags; it applies only the changes returned by Rust. The
  archived source files remain unchanged. Ordinary returned statuses and fatal
  launcher exits stay distinct. Child processes inherit exported variables;
  shell-local values remain available to helper policy without being exported.
  Detached commands start a new Unix session, ignore SIGHUP with `sigaction`,
  and write the session leader's PID before exec. The post-fork callback uses
  [async-signal-safe OS operations](https://man7.org/linux/man-pages/man7/signal-safety.7.html)
  and stack storage for PID formatting. Log or PID-file failures refuse startup
  rather than announce a background run without a usable PID file. The private
  legacy-script binding supplies Bash; the native library accepts a command
  argument vector directly and needs no `setsid` or `nohup` utility.
  Dedicated checks use fake Copilot/Docker programs, fake cgroups and real kernel
  session/group identities. The source oracle's missing macOS `setsid` utility
  is supplied by a Java fixture calling `setsid` and `execvp` through the
  [JDK 25 foreign linker](https://docs.oracle.com/en/java/javase/25/docs/api/java.base/java/lang/foreign/Linker.html).
  No live model probe or benchmark lane was launched. Native kernel tests also
  start their own test executable directly, covering PID-file availability,
  session/group identity, ignored SIGHUP and the detached/unbuffered exports.
  The dedicated launch probe passes 164 helper and detachment checks plus three
  standalone fleet checks. The focused unchanged tests pass with 72 passed and
  two skipped. The full unchanged suite passes with 1,561 passed and 30 skipped
  (293.89 seconds), including the installed-opencode loopback fixture. Its call
  record, `validation/results/launch-full-calls.json`, records 33 binding loads
  and 41 rewritten shell processes; helper calls made in separate child
  transports are not included in that parent counter. All 29 Rust tests,
  Clippy, formatting and the release build pass. Both Linux architectures pass
  all 28 library tests during the helper build and 26 scratch-container checks.
  The relocated runtime bundle passes 252 checks with 20 SDK files, 39 manifest
  hashes and 40 physical files. All 256 archived upstream files match the pinned
  Git tree. Logs are in `validation/results/launch-full.log`,
  `launch-rust-tests.log`, `launch-container-build.log`,
  `launch-container-probe.log` and `launch-bundle.log`.
  Original Python tests remain unconverted. The main sweep, Codex A/B and smoke
  launch entry points still execute archived scripts and are the next production
  work, followed by the supporting-tool audit, three simplification passes,
  three performance passes and final test conversion. Windows detachment,
  locale-dependent shell text, non-UTF-8 variables, general Bash arithmetic
  expressions and interleaved probe stdout/stderr remain outside this corpus.

- The main sweep now runs through `te-bench main-sweep`. Rust loads launcher
  settings, normalises the model ID, rejects unpinned router IDs, selects the
  workload and builds the plan before credential routing. The existing native
  planner supplies cells, counts and the limit-aware estimate. Dry runs print
  the plan, skip billed probes and create no results directory. The working
  directory supplies `workloads/`, `.env` and the default results root.
  Execution uses the shared native detachment helper and re-executes `te-bench`
  for each cell. Child arguments preserve the arm, agent, group, seed, limit,
  adapter, task file, workload, results tree, runner, model, optimizer agent and
  fresh-state flag. The two archived built-in runner names resolve to the shipped
  SDK factories. `PYTHON` is captured before loading `.env`, matching the source
  launcher's interpreter selection, and supplies the fallback when
  `TE_BENCH_PYTHON` is empty or unset; launcher and aggregation policy require
  neither Python nor shell tools. Progress is flushed before credential probes and
  before each cell starts.
  Stored `row.json` counts and the workload's expected count control resume.
  Partial cells rerun; completed cells skip. `ON_FAIL=continue` records every
  failure, while `ON_FAIL=abort` stops the cell loop after its first failure.
  Both paths aggregate every arm present in the results tree and return failure
  when a cell failed. Existing arms remain in the comparison indexes.
  The dedicated source oracle copies the archived scripts into owned fixtures;
  it leaves the 256 archived files unchanged. 81 comparisons and checks
  cover source/native dry runs, defaults, environment-file precedence, endpoint
  and credential refusal, invalid domains/arms, missing manifests, web scaffold
  filtering, real offline Claude/Codex SWE-bench and Copilot WebBench stub cells,
  row counts and CSV/transcript outcomes, full and partial resume, whole-tree aggregation, failure policies,
  literal limits including double-zero and octal resume counts, forced reruns,
  native detachment, interpreter fallback/override and standalone startup.
  Only run-name timestamps are normalised in execution output and CSV comparisons.
  Native runs have an empty
  `PATH` and no upstream Python import path. Stub execution still uses the
  shipped task SDK for I/O; no model or real benchmark runner was called.
  The full unchanged upstream suite passes with 1,561 passed and 30 skipped
  (264.76 seconds), including the installed-opencode loopback fixture. Its native
  call record is `validation/results/main-sweep-full-calls.json`. All 29 Rust
  tests, Clippy, formatting and the release build pass. The shared launch-helper
  regression probe still passes 164 checks plus three standalone fleet checks.
  Both rebuilt Linux helpers pass 26 scratch-container checks across arm64 and
  amd64; their builds also run all 28 library tests. Main-sweep execution
  comparisons run on macOS. The relocated runtime bundle passes 256 checks,
  including dry-run startup without Python, actual SDK execution through the
  interpreter fallback and override, and resume. It still contains 20 SDK files,
  39 manifest hashes and 40 physical files. All 256 archived upstream files
  match the pinned Git tree.
  Logs are `validation/results/main-sweep-full.log`,
  `main-sweep-probe.log`, `main-sweep-rust-tests.log`,
  `main-sweep-clippy.log`, `main-sweep-debug.log`, `main-sweep-release.log`,
  `main-sweep-launch-regression.log`, `main-sweep-container-build.log`,
  `main-sweep-container-probe.log` and `main-sweep-bundle.log`.
  Original tests remain unconverted; none directly executes the main sweep.
  The Codex A/B launcher and supporting-tool audit remain production work before
  the three simplification and three performance passes. The archived smoke test
  remains unchanged until final test conversion. General Bash expression/glob
  expansion, custom shell utilities, malformed planner exception formatting,
  non-UTF-8 settings, Windows detachment and filesystem errors beyond the tested
  corpus remain outside these comparisons.

- The Codex A/B launcher now runs through `te-bench codex-ab`. Rust preserves
  explicit arm lists, the `ARM` and `BASELINE` modes, model defaults, the source
  arm-list match, credential and endpoint preflight order, domain/runner checks,
  task-file and workload selection, and seed/group/arm execution order. Default
  groups come from the whole workload. Dry runs need a task-file setting and
  create neither results nor CPU-budget changes. The launcher exports the
  per-container CPU cap, detaches through the native session helper, then applies
  and reports the shared fleet budget before running cells.
  Main and Codex sweeps share row counting, limit-aware completion checks, child
  process I/O, resume, failure tracking and whole-tree aggregation. Workflow
  selection retains their distinct optimizer labels, Codex's explicit 80-turn
  limit, failure messages and post-run instructions. The Codex comparison hint
  uses `te-bench compare-arms`. Python supplies task SDK I/O; launch planning,
  fleet policy, cell coordination and aggregation execute in Rust.
  Child cells receive the launcher's selected `TOKENECONOMIZER_ENV_FILE`.
  A relocated-bundle regression first reproduced a child loading unrelated
  `TE_EFFORT_LEVEL` settings from the installed runtime's `.env`. Forwarding the
  selected path keeps both main and Codex cells on the working directory's
  configuration; explicit file selections retain their precedence. The failing
  reproduction is `validation/results/codex-sweep-environment-before.log`.
  The two original whole-workload launcher checks now execute the native command.
  Their compatibility binding anchors fixture paths at the archived experiment
  root without changing the original tests. A dedicated source oracle copies the
  scripts into an owned fixture, records actual driver arguments and exported
  CPU/agent/model settings, and compares stdout, stderr, return status, CSVs and
  transcripts. It covers every documented arm, baseline modes and overrides,
  duplicate and first-line arm lists, default groups, task-file fallback,
  preflight refusal, model tags, real offline stub cells for both domains, fleet
  writes to fake cgroups, full and partial resume, whole-tree aggregation,
  continue/abort failure handling and native detached execution.
  Source comparisons normalise run-name timestamps and the comparison-command
  hint. Native runs use an empty `PATH` with no upstream import path. No paid
  model or real dataset runner was called, and no host cgroup was changed.
  The Codex oracle passes 174 checks. The main-sweep regression retains all 81
  checks, and the shared launch probe retains 164 checks plus three standalone
  fleet checks. The focused original tests pass with 88 passed and two skipped.
  The full unchanged suite passes with 1,561 passed and 30 skipped (77.67 seconds),
  including installed opencode against its loopback fixture. Its call record,
  `validation/results/codex-sweep-full-calls.json`, records 33 binding loads and
  43 rewritten shell processes. The separately scoped original launcher test
  records two native CLI processes, one per domain, in
  `codex-sweep-original-cli-calls.json`.
  All 29 Rust tests, Clippy, formatting and debug/release builds pass. Both Linux
  helper builds pass all 28 library tests and 26 scratch-container checks across
  both architectures. The relocated bundle passes 261 checks and retains 20 SDK
  files, 39 manifest hashes and 40 physical files. All 256 upstream files match
  the pinned Git tree. Main/Codex execution comparisons run on macOS; Linux
  library and helper checks cover arm64 and amd64.
  Logs are under `validation/results/`: `codex-sweep-full.log`,
  `codex-sweep-probe.log`, `codex-sweep-focused.log`,
  `codex-sweep-main-regression.log`, `codex-sweep-launch-regression.log`,
  `codex-sweep-rust-tests.log`, `codex-sweep-clippy.log`,
  `codex-sweep-debug.log`, `codex-sweep-release.log`,
  `codex-sweep-container-build.log`, `codex-sweep-container-probe.log` and
  `codex-sweep-bundle.log`.
  The original smoke test remains unchanged for final test conversion. The
  supporting-tool and parity audits still precede the three simplification
  passes, three performance passes and original-test conversion. Windows,
  general Bash arithmetic and glob expansion, shell-special variables, malformed
  schema diagnostics and filesystem failures outside the corpus remain open.

- The developer dead-code scanner now runs through `te-dead-code`. Rust owns
  tracked-file selection, word counts, allowlists, exports, decorator and override
  policy, deletion safeguards, repeated sweeps, JSON and Markdown reports, and
  argument handling. The binary embeds a standard-library Python binding for
  [AST parsing](https://docs.python.org/3/library/ast.html#ast.parse),
  [standard-library module names](https://docs.python.org/3/library/sys.html#sys.stdlib_module_names)
  and [module imports](https://docs.python.org/3/library/importlib.html#importlib.import_module).
  Rust selects the imports and performs inheritance and dynamic-dispatch checks;
  the binding retains Python objects and invokes reflection APIs. `TE_PYTHON`
  selects the interpreter. The standalone default root is the working directory.
  The original test binding preserves the archived script's default root.
  Python 3.10+ and Git are runtime requirements; no third-party Python package or
  companion source file is needed. Parser and standard-library behaviour follow
  the selected interpreter; these comparisons use Python 3.15.2 on macOS.
  An additional deletion guard keeps statements that share a line. The oracle
  reproduces the source fixer deleting referenced `B` when it removes unused
  `A` from `A = 1; B = 2`. The native fixer keeps that line and reports why.
  Original safeguards for chained assignments, decorated classes, methods,
  comments, newlines and parse failures remain covered. Markdown tables use
  padded columns, with only table padding normalised in report comparisons.
  The dedicated probe passes 328 checks covering source results, file edits,
  Unicode paths and literals, binary files, symlinks, inheritance, dynamic
  dispatch, exports, sweep limits, large literals, deep ASTs, report output and
  argument errors. Relocation checks run the copied binary outside the checkout,
  verify help without Python or Git, and check deletion and executable-mode
  preservation in owned fixtures. The 11 unchanged original scanner tests pass;
  the full unchanged suite passes with 1,561 passed and 30 skipped (80.31 seconds),
  including installed opencode against its loopback fixture. Its call record,
  `validation/results/dead-code-full-calls.json`, records nine native scans,
  three native fixes and four native CLI processes for the scanner tests.
  All 29 Rust tests, formatting, Clippy and debug/release builds pass. All 256
  reference files still match the pinned tree. Each Linux helper build passes
  all 28 library tests; the helpers pass 26 scratch-container checks across arm64
  and amd64. The relocated experiment bundle retains all 261 checks, 20 SDK
  files, 39 manifest hashes and 40 physical files. That runtime payload excludes
  this developer tool; its separate executable is built by Cargo.
  Logs are under `validation/results/`: `dead-code-probe.log`,
  `dead-code-focused.log`, `dead-code-full.log`, `dead-code-rust-tests.log`,
  `dead-code-clippy.log`, `dead-code-debug.log`, `dead-code-release.log`,
  `dead-code-container-build.log`, `dead-code-container-probe.log` and
  `dead-code-bundle.log`.
  Windows scanner execution, non-UTF-8 paths, custom Python import state and
  filesystem error details outside the corpus remain unverified. The remaining
  supporting-tool and parity audits precede the three simplification passes,
  three performance passes and original-test conversion, all still pending.

- The static documentation figures now regenerate through `te-figures results`
  and `te-figures framework`. Rust owns paper rows, labels, colours, axis and
  card geometry, rounded-bar transforms, marker placement, text-width checks,
  tool chips, rollout spans and export options. A small embedded object binding
  invokes [Matplotlib's figure and artist APIs](https://matplotlib.org/stable/api/figure_api.html).
  Text placement uses the actual renderer's font metrics. SVG glyph outlines,
  creation-date omission and the framework figure's fixed hash salt preserve
  the original export choices. The results rows quote the paper's Table 1.
  The renderer is an optional Python dependency for this developer tool,
  selected with `--python` or `TE_PYTHON`. Matplotlib loads when a figure command
  runs; Cargo dependencies are unchanged.
  There are no original tests for these authoring scripts. The dedicated probe
  instead runs the unchanged scripts into owned directories and compares the
  native outputs with those fresh renders. It passes 100 checks, including
  exact PNG pixels and bytes, exact SVG bytes, XML parsing, paper labels,
  repeated exports, working-directory defaults, relocation with an empty PATH,
  interpreter precedence, help, invalid arguments and missing-interpreter
  refusal. Results renders match at 100 and 137 figure DPI; framework renders
  match at 100 and 200. At 137 DPI, both framework implementations reject
  `card 1 body is 13.01 units, room 13.00` before writing output. The collision
  guard remains enabled. Source and native console output match after replacing
  only their fixture output-directory names. The results SVG comparisons set
  a fixed renderer hash salt; its default random IDs remain the source behaviour.
  Comparisons use Python 3.15.2, Matplotlib 3.11.2, NumPy 2.5.5 and Pillow 13.0.0
  on macOS. Native PNG previews were also inspected. Generated assets remain
  under `validation/results`; all 256 archived files still match the pinned tree.
  The full unchanged upstream suite passes with 1,561 passed and 30 skipped
  (83.84 seconds), including installed opencode against its loopback fixture.
  All 29 Rust tests, Clippy, formatting and debug/release builds pass.
  Each Linux helper build passes 28 library tests. The helpers retain all 26
  scratch-container checks across arm64 and amd64. The relocated experiment
  bundle retains all 261 checks, 20 SDK files, 39 manifest hashes and 40 physical
  files; this optional developer executable is built separately by Cargo.
  Logs are under `validation/results/`: `figures-probe.log`, `figures-full.log`,
  `figures-full-calls.json`, `figures-rust-tests.log`, `figures-clippy.log`,
  `figures-debug.log`, `figures-release.log`, `figures-reference.log`,
  `figures-container-build.log`, `figures-container-probe.log` and
  `figures-bundle.log`.
  The animated teaser, other renderer environments, Windows execution and
  remaining production-parity audits stay open. Formal simplification and
  performance passes remain at 0/3 each; original-test conversion is pending.

- The animated teaser now runs through `te-figures teaser`. Rust owns recursive
  font selection, colours, text, timeline, easing, bar/card geometry, examples,
  title and results panels, crossfades, held frames, poster generation and
  encoder arguments. The paper rows come from the same Rust constant used by
  the static results figure. Both embedded logos match the pinned PNG assets.
  The shared renderer binding invokes Matplotlib or Pillow APIs; numeric tuples
  retain their Python tuple representation for Pillow colours. Rust releases
  temporary SDK objects after each frame while retaining live images and font
  and picture caches. This keeps the frame pipeline's image ownership bounded.
  `--out` exports GIF and PNG; `--page` exports VP9 WebM, H.264 MP4 and PNG.
  `--still` exposes the original poster operation without running the encoder.
  Full exports preserve the original codec flags, overwrite behaviour and
  output summaries. The standalone default directory is `./docs/assets`.
  Font files remain external inputs; `.ttf` candidates precede `.woff`.
  Dedicated animation validation passes 1,191 checks. Both the 1.25-scale GIF
  sequence and 2-scale page sequence contain 581 frames, with every PNG frame
  byte matching a fresh run of the unchanged source. Posters also match in PNG
  dimensions, pixels and bytes. A Java encoder fixture records the source's
  exact arguments and copies its input frames. The native run uses the same
  fixture to record inputs before invoking real ffmpeg. Encoder arguments match
  after replacing only input/output fixture paths. Native GIF, WebM and MP4 files
  pass ffprobe frame-count and dimension checks: 1400 x 750 for GIF and
  2240 x 1200 for both page formats. Console comparisons normalise output paths
  and the fixture encoder's file-size field; duration text remains compared.
  Another 23 checks cover missing-font and missing-encoder preflight, `.ttf`
  precedence through symlinks, embedded asset bytes, invalid arguments and an
  encoder exiting with status 23. Both source and native exports then stop
  before writing the poster and remove their temporary frame directories.
  Relocated native runs use the selected Python interpreter and embedded logos.
  Static-figure regression retains all 100 checks after sharing the binding.
  These comparisons use Python 3.15.2, Pillow 13.0.0, IBM Plex Sans 2.0.0 and
  Mono 2.5.0 package fonts, and ffmpeg 9.0.2 on macOS. Font packages and generated
  outputs stay under ignored validation directories. All 256 archived files
  still match the pinned tree. The full unchanged suite passes with 1,561 passed
  and 30 skipped (85.92 seconds), including installed opencode against its
  loopback fixture. All 29 Rust tests, formatting, Clippy and debug/release
  builds pass. Each Linux helper build passes 28 library tests, and both helpers
  retain all 26 scratch-container checks across arm64 and amd64.
  The relocated experiment bundle retains all 261 checks, 20 SDK files,
  39 manifest hashes and 40 physical files; the figure tool is built separately
  by Cargo.
  Logs are under `validation/results/`: `teaser-animations.log`,
  `teaser-failures.log`, `teaser-static-regression.log`, `teaser-full.log`,
  `teaser-full-calls.json`, `teaser-rust-tests.log`, `teaser-clippy.log`,
  `teaser-debug.log`, `teaser-release.log`, `teaser-reference.log`,
  `teaser-container-build.log`, `teaser-container-probe.log` and
  `teaser-bundle.log`.
  Other font/encoder environments, Windows execution and the remaining
  production-parity audits stay open. Formal simplification and performance
  passes remain at 0/3 each; original-test conversion is pending.

- The shared Claude/Droid timestamp helper now follows CPython 3.15.2's
  [ISO parser and local-time mapping](https://github.com/python/cpython/blob/v3.15.2/Modules/_datetimemodule.c)
  in Rust. The initial format-list implementation differed in 5,355 of 7,240
  source comparisons: it missed week dates, abbreviated clocks, Unicode
  separators and fractional timezone offsets, accepted malformed field widths,
  and differed at precision and date boundaries. Claude/Droid heads and Droid
  session discovery now retain the same `started` values as the original helper.
  Parsing preserves the wrapper's final-`Z` replacement, basic and extended week
  dates, its numeric-separator disambiguation, comma fractions, microsecond
  truncation, offset range checks, 24:00 normalisation and terminal-NUL rules.
  Aware timestamps use exact integer-microsecond division through the existing
  rational-number dependency. Naive timestamps use the original `fold=0`
  wall-time equation, including gaps, overlaps and year-boundary failures.
  Invalid timestamps continue to return no value. Production parsing uses Rust
  and native time-zone data; it does not start a Python interpreter.
  The expanded probe passes 32,360 source comparisons across UTC,
  America/New_York, Europe/Paris, Australia/Lord_Howe and Pacific/Apia. It covers
  deterministic mutations, long fractions, aware/naive dates, DST transitions,
  the Apia skipped day, actual trace heads and Droid discovery rows. Numeric
  outputs are compared exactly after the same JSON float formatting. Each zone
  uses fresh source and native processes. Two Rust regression tests also check
  accepted forms, rounding and malformed clock/offset rejection.
  The official CPython licence is preserved in `src/trace/LICENSE-CPYTHON` and
  copied into generated bundle notices with the source attribution and a summary
  of the Rust adaptation. Notice verification covers the new text and all 104
  locked registry crates. Cargo dependencies and the lockfile are unchanged.
  All 31 Rust tests, formatting, Clippy and debug/release builds pass. Each Linux
  helper build passes 30 library tests. Both helpers retain all 26 scratch-container
  checks. The relocated experiment bundle retains all 261 checks, 20 SDK files,
  39 manifest hashes and 40 physical files, including the updated notices.
  The full unchanged original suite passes
  with 1,561 passed and 30 skipped (92.13 seconds), including installed opencode
  against its loopback fixture. All 256 archived files still match the pinned tree.
  Logs are under `validation/results/`: `timestamp-before.log`,
  `timestamp-probe.log`, `timestamp-full.log`, `timestamp-full-calls.json`,
  `timestamp-rust-tests.log`, `timestamp-clippy.log`, `timestamp-debug.log`,
  `timestamp-release.log`, `timestamp-notices.log`, `timestamp-notices-check.log`,
  `timestamp-reference.log`, `timestamp-container-build.log`,
  `timestamp-container-probe.log` and `timestamp-bundle.log`.
  Timestamp comparisons use Python 3.15.2 on macOS. Other platform/time-zone
  databases and surrogate-containing input remain in the platform and JSON
  audits. Formal simplification and performance passes remain at 0/3 each;
  original-test conversion is pending.

- Claude history now uses the same tilde expansion helper as side-call CLI and
  Pi extension paths. The removed local shortcut skipped named users. The shared
  helper also reports the original `embedded null byte` error for NUL-containing
  Unix usernames. Its Windows branch retains trailing profile separators and
  distinguishes an absent `USERNAME` from an empty value, following
  [Python's path expansion contract](https://docs.python.org/3/library/os.path.html#os.path.expanduser).
  The history test adapter now uses `str()` for extra directories, matching the
  source's conversion of path objects and other values.
  The new path probe initially found 92 differences in 450 comparisons; all 450
  now pass. Cases cover known/unknown users, empty/root/trailing-slash homes,
  Unicode and NUL bytes, actual named-user history discovery, symlinks, duplicate
  directories, `projects` fallback and non-string extra directories. The environment
  probe passes 294 comparisons, including injected POSIX and Windows `join` and
  `dirname` functions with local/parent-file existence branches. The loader already
  used those functions, so its production code did not change.
  All 31 Rust tests, formatting, Clippy and debug/release builds pass. The unchanged
  original suite passes with 1,561 passed and 30 skipped (92.67 seconds), with 32
  calls to native history. All 256 archived files still match the pinned tree.
  Both Linux helper builds pass 30 library tests. The helpers pass 26 scratch-container
  checks, and the relocated experiment bundle passes all 261 checks.
  Logs are under `validation/results/`: `path-before.log`, `path-probe.log`,
  `path-environment.log`, `path-full.log`, `path-full-calls.json`,
  `path-rust-tests.log`, `path-clippy.log`, `path-debug.log`, `path-release.log`,
  `path-reference.log`, `path-container-build.log`, `path-container-probe.log`
  and `path-bundle.log`.
  Windows-specific Rust regression cases are included but cannot run on this Mac;
  native Windows execution and OS paths outside UTF-8 remain unverified. Formal
  simplification and performance passes remain at 0/3 each; original-test conversion
  is pending.

- The production-entry-point inventory found `te_harness.capture_catalog`, which
  had no original tests. Its replacement is `te-bench capture-catalog`. Rust owns
  option selection, container setup and cleanup, first-party and recorded probes,
  deferred ToolSearch requests, longest-description merging, truncation and console
  output. The container recorder uses `te-net-guard --catalog-recorder` for snapshot
  state, request/response header filtering, credential replacement and error replies.
  A standard-library Python binding supplies threaded HTTP, TLS and process I/O.
  The shared static-helper installer selects and verifies the container architecture.
  Existing snapshots retain longer descriptions; malformed prior JSON falls back
  to an empty snapshot. Description limits larger than the native integer range
  retain Python's slicing result. No Cargo dependencies were added.
  New source comparisons cover 133 capture cases, including command arguments,
  readiness retries, deferred tools, existing snapshots, Unicode, malformed streams,
  errors and exception identity through cleanup. The recorder passes 166 comparisons
  over real loopback HTTP, including streaming, header filtering, HTTP errors,
  connection failures, UTF-8/16/32 bodies, non-finite metadata, surrogate text,
  failed snapshot writes and concurrent requests. The standalone CLI matches source
  console output and snapshot bytes with a Java Docker fixture. Both commands also
  remove their container fixture after an actual Ctrl-C and return the same status.
  All 31 Rust tests, formatting, Clippy and debug/release builds pass. The final
  unchanged original suite passes with 1,561 passed and 30 skipped (88.65 seconds).
  Both Linux helper builds pass 30 library tests and all 28 scratch-container checks.
  The relocated bundle passes 270 checks, including capture and interruption with
  its own helper files and no upstream import path. Its manifest covers 40 payload
  files; the bundle contains 41 files including the manifest. All 256 archived
  source, test and asset files still match the pinned tree.
  Logs are under `validation/results/`: `catalog-probe.log`, `catalog-recorder.log`,
  `catalog-cli-probe.log`, `catalog-full.log`, `catalog-full-calls.json`,
  `catalog-rust-tests.log`, `catalog-clippy.log`, `catalog-debug.log`,
  `catalog-release.log`, `catalog-container-build.log`, `catalog-container-probe.log`,
  `catalog-bundle.log` and `catalog-reference.log`.
  Paid agent execution and native Windows execution were not run. Tool names and
  descriptions outside their string schemas remain in the JSON/schema audits.
  Formal simplification and performance passes remain at 0/3 each; original-test
  conversion is pending.

## Dependencies

Build with Rust 1.89 or newer and a C compiler for bundled SQLite and PCRE2. Validation uses
Rust 1.99.1. The SQLite wrapper requires the minimum Rust version change; its
unused cache and WebAssembly default features are disabled. Clippy's conditional
syntax fixes after that version change are part of the port, not one of the
three requested simplification passes.

[Chrono](https://docs.rs/chrono/0.4.45/chrono/) supplies calendar parsing and local
timezone handling. [Rusqlite](https://docs.rs/rusqlite/0.43.0/rusqlite/) supplies
SQLite connections opened explicitly with `SQLITE_OPEN_READ_ONLY`. Trace readers
use a five-second busy timeout; gateway lookups retain the SDK's two-second limit.
The lockfile pins both libraries and their transitive dependencies.
`fancy-regex` 0.20.2 supplies existing fixed-pattern matching elsewhere in the port.
Detector and extractor patterns use `pcre2` 0.2.11 with bundled PCRE2 10.46. The
native compiler expands Python character classes, case folding and boundaries
before matching, and retains the original pattern text in saved metadata.
Caseless backreferences compare captured substrings in a Rust callback using
Unicode 17 simple lowercase mappings or ASCII rules. The already-locked
`pcre2-sys` 0.2.10 dependency is now declared directly for this C API. Each match
owns its callback state, PCRE2 context and capture offsets. `.cargo/config.toml`
selects static PCRE2 linking for host builds; the
dependency selects static linking for the Linux musl helpers too. `regex-syntax`
was already locked transitively and now supplies character ranges and UTF-8
encoding directly. The notice generator includes embedded C licence headers.
Generated Unicode 17 data extends regex-syntax's Unicode 16 classes and simple
folds and supplies lowercase mappings. Reproduce it with
`jbang scripts/UnicodeRegexTables.java`; add
`--check` to verify the committed data. The script caches five versioned official
inputs under `validation/results/unicode-ucd` and checks their SHA-256 hashes on
every run. `unicode-ident` 1.0.24 was already locked and now supplies Unicode 17
group-name properties directly.
`unicode_names2` 4.0.0 supplies compressed Unicode 17 character names. The compiler
checks canonical spellings and the official alias table to preserve Python's
spaces and hyphens. It derives Tangut names for the two ranges omitted by the
crate. Name data is compiled into executables; runtime lookup needs no files or
Python process. The notice generator includes the crate's Unicode data licence.
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
jbang validation/SpiralProbe.java --sdk
jbang validation/NudgeProbe.java
jbang validation/NudgeProbe.java --sdk
jbang validation/FacadeProbe.java
jbang validation/UsageProbe.java
jbang validation/UsageProbe.java --sdk
jbang validation/ProviderProbe.java
jbang validation/ProviderProbe.java --sdk
jbang validation/OptimizerHttpProbe.java --sdk
jbang validation/OptimizerCliProbe.java --sdk
jbang validation/OptimizerContextProbe.java --sdk
jbang validation/OptimizerLogProbe.java --sdk
jbang validation/PreflightProbe.java --sdk
jbang validation/CopilotHostProbe.java --sdk
jbang validation/InvocationProbe.java --sdk
jbang validation/ScaffoldBaseProbe.java --sdk
jbang validation/SerenaScaffoldProbe.java --sdk
jbang validation/RtkScaffoldProbe.java --sdk
jbang validation/RepoMapScaffoldProbe.java --sdk
jbang validation/ScaffoldRegistryProbe.java --sdk
jbang validation/TaskProbe.java --sdk
jbang validation/ContainerProbe.java --sdk
jbang validation/RunnerControlProbe.java --sdk
jbang validation/CodexConfigProbe.java --sdk
jbang validation/RunnerSetupProbe.java --sdk
jbang validation/WebSetupProbe.java --sdk
jbang validation/GuardInstallProbe.java --sdk
jbang validation/RunnerCallsProbe.java --sdk
jbang validation/WebCleanupProbe.java swebench --sdk
jbang validation/SweSegmentProbe.java --sdk
jbang validation/SweSegmentProcessProbe.java --sdk
jbang validation/SweScaffoldContextProbe.java --sdk
jbang validation/SweHostProbe.java --sdk
jbang validation/WebTaskPipelineProbe.java --sdk
jbang validation/WebCleanupProbe.java --sdk
jbang validation/TaskProbe.java
jbang validation/WebSegmentProbe.java
jbang validation/WebAgentProbe.java
jbang validation/WebGuardProbe.java
jbang validation/NativeWebGuardProbe.java
jbang validation/WebScaffoldProbe.java
jbang validation/ScaffoldBaseProbe.java
jbang validation/SerenaScaffoldProbe.java
jbang validation/RtkScaffoldProbe.java
jbang validation/ScaffoldRegistryProbe.java
jbang validation/RepoMapScaffoldProbe.java
jbang validation/SweScaffoldContextProbe.java
jbang validation/SweSegmentProbe.java
jbang validation/SweSegmentProcessProbe.java
jbang validation/ScaffoldMergeHelperProbe.java
jbang validation/ScaffoldMergeHelperProbe.java --context
jbang validation/ScaffoldMergeStagingProbe.java --docker
jbang scripts/BuildContainerHelpers.java
jbang validation/ContainerHelperProbe.java
jbang validation/InvocationProbe.java
jbang validation/WebSegmentProcessProbe.java
jbang validation/WebTaskPipelineProbe.java
jbang validation/GraderProbe.java
jbang validation/SettingsProbe.java
jbang validation/ContainerProbe.java
jbang validation/CodexConfigProbe.java
jbang validation/CodexInstallProbe.java
jbang validation/CodexInstallProbe.java --sdk
jbang validation/RunnerWrapperProbe.java
jbang validation/NetGuardProbe.java
jbang validation/GuardInstallProbe.java --docker
jbang validation/NativeGuardStagingProbe.java --docker
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
jbang validation/ExperimentBundleProbe.java
jbang validation/ExperimentAssetsProbe.java
jbang validation/WebPreloadProbe.java
jbang validation/DriverSelectionProbe.java
jbang validation/DriverTasksProbe.java
jbang validation/ArmRegistryProbe.java
jbang validation/EnvironmentProbe.java
jbang validation/DriverMainProbe.java
jbang validation/AggregateProbe.java
jbang validation/WorkloadProbe.java
jbang validation/SweepProbe.java
jbang validation/DatasetExportProbe.java
jbang validation/WebExportProbe.java
jbang validation/YamlMirrorProbe.java
jbang validation/CompareProbe.java
jbang validation/LaunchProbe.java
jbang validation/MainSweepProbe.java
jbang validation/CodexSweepProbe.java
jbang validation/DeadCodeProbe.java
jbang validation/FiguresProbe.java
jbang validation/TeaserProbe.java --animations
jbang validation/TeaserFailureProbe.java --encoder
jbang validation/TimestampProbe.java
jbang validation/PathExpansionProbe.java
jbang validation/CatalogProbe.java
jbang validation/CatalogRecorderProbe.java
jbang validation/CatalogCliProbe.java
jbang validation/StandaloneBenchProbe.java
jbang validation/StandaloneTransportProbe.java
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
