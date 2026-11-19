# TokenEconomizer experiment runtime

This payload contains the Rust executables, static Linux arm64 and amd64 task
helpers, the captured Codex base prompt and the WebBench Node preload. The
manifest records every payload file's SHA-256 and the dependency lockfile hash.

Keep the executables together in `bin/`. Container helper discovery uses
`bin/container-helpers/`. The payload can move independently of its build checkout.
`te-bridge` exposes the native JSON-lines service protocol.

Run a task stream with the native benchmark command:

```sh
bin/te-bench --arm baseline --tasks-file /path/to/tasks.json --results-root /path/to/results
```

The default stub produces synthetic transcripts and grades for wiring checks.
Use `--runner module:factory` for an external task runner. Python 3.10 or newer supplies the
standard-library I/O binding; `TE_BENCH_PYTHON` selects an interpreter with the
runner's dependencies. The SDK and engine run independently of the original
checkout. The loader uses this directory's `.env`, or its parent's when the local file is absent;
`TOKENECONOMIZER_ENV_FILE` selects an explicit file.

The SDK includes the generic streamed-rollout controller and Codex policy, state,
resume-guard and envelope-tracker objects. Their state transitions execute in Rust.
The Codex installation binding covers binary and vendor discovery, version
reporting, captured prompts, bundled skill discovery and configuration validation.
Accounting bindings cover Claude, Codex and Copilot, including Claude's
incomplete-stream recovery.
The SDK also includes the shared object/I/O binding and provider routing. Rust
supplies credential names and endpoint defaults and validates model pricing.
Optimizer bindings cover all three agents, endpoint preflights, context and logging,
Copilot discovery, and sealed Copilot and Codex invocation builders.
Scaffold bindings include the shared profile/context classes, RTK, Serena, repo-map
and their registry. Rust supplies their metadata, selects scaffolds and drives setup.
Task-lifecycle and constructor bindings accept caller-provided harness namespaces,
including opaque SDK references and subclass callbacks. Built-in SWE-bench and
WebBench class assembly remains pending.
Guard convenience APIs and hook-entry merging execute in Rust. The legacy
merge-program API returns a Python I/O launcher for the bundled native installer.
External agents, the official grader SDK, aider and Node remain caller-provided
dependencies where the selected experiment uses them.

This port retains the Apache-2.0 licence. See `LICENSE` and
`THIRD-PARTY-NOTICES.txt` for attribution and dependency licences.
