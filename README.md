# TokenEconomizer native

Rust port of [Microsoft TokenEconomizer](https://github.com/microsoft/TokenEconomizer),
pinned at `583fff78bf504b59d542f953c31e87d01d02ab42`.

The port is in progress. The original implementation and tests are preserved in
`reference/upstream`. See [the migration record](docs/migration.md) for scope,
validation commands and the required order of work.

The Apache-2.0 licence and upstream attribution are preserved.

Build with Rust 1.89 or newer and a C compiler for bundled SQLite.

The native CLI runs the implemented command services directly:

```sh
cargo build --locked
target/debug/te --host claude --cwd /path/to/repo status
target/debug/te --host claude --cwd /path/to/repo config show
```

It reads and writes the existing TokenEconomizer state formats. Background folds,
judges and map refreshes use the same native executable.

The native hook reads the host's JSON payload from stdin and writes its JSON
response to stdout:

```sh
printf '%s\n' '{"hook_event_name":"SessionStart","session_id":"example","cwd":"/path/to/repo"}' \
  | target/debug/te-hook --host claude
```

Keep `te` beside `te-hook`: the hook uses it for background work and creates the
state directory's `bin/te` launcher. Hook diagnostics go to stderr.

The native repository-map generator is available now. It uses an optional aider
installation in the interpreter selected with `--python`:

```sh
cargo build --bin te-repomap-gen
target/debug/te-repomap-gen --python /path/to/python /path/to/repo 2048 \
  'Engine,parse_config' --git --cache /path/to/state/tags
```

Build a marketplace bundle or a separate Droid bundle for the binaries' platform:

```sh
cargo build --release --locked
target/release/te-package --out dist/marketplace --zip
target/release/te-package --droid --out dist/droid
```

The marketplace bundler reads the pinned host assets from `reference/upstream`
and copies
`te`, `te-hook` and `te-repomap-gen` from beside itself. `--binaries` selects another
binary directory. The resulting plugin directory can move independently of this
checkout and carries no Python core package or bootstrap.

Hermes keeps its SDK binding in Python and runs gateway session, redirect and
notice policy in Rust. Concurrent callbacks retain their profile and repository.
Pi keeps its TypeScript SDK binding and runs session sections, prompt turns,
tool-result steering and notice policy in Rust.
opencode v1 and v2 keep TypeScript bindings and run manuals, prompt turns,
tool-result redirects and session exports in Rust.
The four experiment dataset adapters also run in Rust and implement the native
tooling interface. The experiment facade, spiral stage, generic nudge controller
and Codex continuation controller also run in Rust.
Task-runner usage accounting and interrupted-session recovery run in Rust for
Claude, Codex and Copilot.
The shared SWE-bench task lifecycle and grading flow also run in Rust, with host
callbacks for container commands, scaffolds, streams and the external grader.
The grader wrapper loads and normalises tables, builds predictions and selects
reports in Rust. Its Python binding imports and calls the official SWE-bench SDK.
SWE-bench run configuration, CPU and thread limits, environment helpers and scaffold
patch exclusions also run in Rust. Both benchmark harnesses use the native network
tool block.
Per-task SWE-bench container starts, cleanup, command construction and measured
environment activation run in Rust. Process execution uses a Python I/O binding.
Codex request configuration, capability toggles, prompt-section trimming and
SWE-bench configuration installation also run in Rust, along with scaffold result
handling and MCP configuration wiring.
SWE-bench scaffold context assembly and its retained Docker execution, copy and
binary-read callbacks also use native policy. SDK bindings retain the raw handles
and supply process I/O.
SWE-bench agent segments now run in Rust for Claude, Copilot and Codex, including
Docker arguments, credential forwarding, transcripts, guard stops, deadlines,
container-side termination, usage recovery and Claude session-log reads. Both
benchmark harnesses share the native stream loop and retain their launch and
termination policy. SDK bindings supply processes, pipes, clocks and object APIs.
The Copilot and Codex SWE-bench task wrappers also run in Rust, including debug-log
persistence, task state resets and transcript-based RTK and MCP measurements.
The standalone benchmark answer-key guard is `target/debug/te-net-guard`. It reads
hook JSON from stdin, refuses matching commands with exit status 2 and a reason on
stderr, and appends telemetry to `TE_NET_GUARD_LOG` when set. Payload selection,
host-list generation and transcript attempt counting also run in Rust.
`--install-hook <agent> <path> <command>` appends a guard entry for Claude, Codex or
Copilot while preserving existing hooks. Unreadable or malformed JSON starts from
an empty object. SWE-bench DNS checks, refusal probes, refusal counts and patch
exclusion commands also run in Rust.
`--merge-json <path> <patch-file>` performs the scaffold's one-level dictionary
merge natively, preserving unrelated keys. Build static Linux arm64 and amd64
helpers with `jbang scripts/BuildContainerHelpers.java` using Docker. Artifacts,
checksums, compiler details and third-party notices land under
`target/container-helpers/<architecture>/`. The normal SWE-bench guard installer
selects the container architecture, validates the manifest hash and static ELF,
copies the helper and registers its hook after scaffold setup. Registration
preserves existing hooks and verifies an actual refusal before reporting success.
The validation binding supplies this build directory. The experiment runtime
bundle ships the same helper layout beside its executables. Scaffold merges
invoke this helper too; SWE-bench deploys it
when a merge needs it, and WebBench uses the host binary beside the experiment
executable. WebBench guards use a per-task executable `net_guard.py` launcher
that calls the native helper. The filename preserves the existing hook contract;
the launcher needs a POSIX shell and the native helper beside the experiment
executable. Verification executes that launcher and removes its probe telemetry.

Package the experiment runtime engine and assets:

```sh
jbang scripts/BuildContainerHelpers.java
cargo build --release --locked
target/release/te-package --experiments --container-helpers target/container-helpers \
  --out dist/experiments --zip
```

This mode embeds the pinned Codex prompt, WebBench Node preload, licence and
notices. It copies five native executables and both static Linux helpers, records
file hashes and preserves executable permissions. It requires a fresh output
directory. Bundled Codex engines read captures from
`assets/codex_base_prompt/<version>.md`; development engines use the embedded
pinned capture. Repo-map scaffolds invoke the companion `te-repomap-gen` and pass
their Python interpreter to its aider SDK binding. Standalone host bindings
and a benchmark command remain pending. WebBench selects the bundled Node
preload; unbundled development engines use `src/experiments/assets` in the build
checkout.

SWE-bench controller selection, constructor parameters, continuation dispatch and
telemetry fields run in Rust for all three agents. SDK bindings retain controller,
callback and telemetry objects across the native calls.
Live-container sweeping and signal cleanup also run in Rust, preserving snapshot
iteration, command arguments, registry removal and the signal exit status.
Codex installation discovery, captured-prompt loading, offline skills discovery
and configuration validation also run in Rust, including process-group cleanup
and temporary-directory retries. SDK bindings supply filesystem and process I/O.
Constructor coordination also runs in Rust for Claude, Copilot and Codex in both
domains, including validation order, scaffold setup and configuration preflights.
Web-Bench's shared stream-state initialisation also runs in Rust.
Its stream start, session ownership checks and session close coordination also
run in Rust. Evaluator event and request waits also use native deadline, polling
and event-selection policy, with SDK bindings for clocks, queues and object protocols.
Evaluator stdout logging, attempt parsing, event publication and exit-state
updates also run in Rust. Evaluator startup uses native configuration, subset
warnings, environment assembly, process launch and session registration. SDK
bindings supply queues, HTTP servers, threads, files and process I/O. HTTP request
parsing, queue rendezvous, timeout replies, response headers and broken-pipe logging
also use native policy. Evaluator cleanup, live-session sweeping and signal
registration also run in Rust. A separate SDK bridge connection allows cleanup
while the main RPC lock is held, preserving the original signal exit status.
Workspace materialisation and answer collection use native path checks, directory
pruning, change detection, size limits and deletion reporting. Ignore-file loading,
path ancestors and wildcard matching also run in Rust, with SDK bindings for
filesystem I/O, OS path normalisation and object protocols.
Evaluator task-file loading uses native path assembly, line filtering, ID
collection and file-error handling. SDK bindings provide file contexts and JSON
library decoding while retaining ID values by reference.
Web-Bench repository and Node discovery, evaluator abort and task-timeout policy,
preload environment assembly and namespace-probe decisions also run in Rust.
SDK bindings supply paths, process I/O and Python object protocols.
The shared Web-Bench task loop uses native retry and stream-failure decisions,
per-attempt guard coordination, evaluator replies, artifact order, resolve metrics
and telemetry selection. SDK bindings retain session, queue, file and result
objects. Copilot debug log persistence and Codex capability metadata, compliance
coordination and task home pruning also run in Rust. Web-Bench agent segments use
native environment assembly, streaming, guard stops, deadlines, process cleanup
and usage-recovery coordination for Claude, Copilot and Codex. Claude session-log
discovery and read-error handling also run in Rust. SDK bindings supply processes,
pipes, clocks, files and object protocols. Per-task homes, Copilot configuration
seeding and cache links, the Web-Bench Codex configuration writer and accounting
hooks also use native policy. Copilot cache and configuration-directory discovery
run in Rust too. Web-Bench hook installation, refusal probes, refusal counts,
hosts-file generation and namespace path quoting also use native policy.
Copilot and Codex argument builders, Codex resume arguments and environment
additions, and Copilot configuration-refusal detection also run in Rust.
Web-Bench scaffold profiles, context construction, retained execution and copy
callbacks, setup coordination and answer exclusions also run in Rust. Scaffold
profile and context constructors, instructions, MCP configuration and merge-helper
staging use native policy too. Merges stage a shell invocation with a quoted JSON
payload and execute the native helper. Experiment helper packaging remains open.
Scaffold registry lookup,
repo-map identifier selection, archive filtering, generator orchestration and
cleanup also use native policy. SDK bindings supply archive extraction, Unicode
properties, process I/O and the generator command until native experiment packaging
supplies it. RTK cache selection, host staging, agent-specific hook registration,
rewrite probes and Codex instruction inlining also use native policy.
Serena installation policy, MCP server configuration, Codex server handoff and
navigation instructions also run in Rust. Serena remains an external Python tool.
Run configuration, prompt construction, transport-instruction filtering
and both guard constructors now use native policy. The pinned prompt text is
embedded in the native library; SDK bindings retain runtime values and object APIs.
Provider routing, credential filtering, Copilot model normalisation and effort
selection also run in Rust. Codex catalogue parsing, caching and model verification
use native policy with mapping and HTTP bindings.
Both shared endpoint preflight functions also run in Rust, including their gate,
probe arguments, rejection messages and Codex success log. The JSON reader reports
syntax messages and positions that match the source for catalogue and grader errors.
Messages and Responses optimizer calls also use native policy for request bodies,
authentication, retries, text extraction, warnings and usage accounting. SDK bindings
supply HTTP I/O, clocks, observers and pricing calls.
Copilot CLI optimizer coordination also runs in Rust, including tool-isolation
flags, model selection, cleanup, event parsing and billing order. Copilot host
binary discovery, version reporting, installation validation and environment
assembly also run in Rust. Shared optimizer metadata uses native log-path
selection, cell keys, context updates and prompt classification, including the
Web-Bench aliases. Shared optimizer log records and verbosity control also use
native policy, with SDK bindings for files, clocks, streams and JSON encoding.
The shared string binding preserves individual Python code points, including
adjacent surrogates, during log truncation, classification and version reporting.
Both domains' optimizer and preflight methods also use native routing, and their
runner factories use native defaults and scaffold validation. SDK bindings retain
argument, helper and constructor references, including when Web-Bench loads later.

Installed host verification and the remaining migration gates are still in progress.
