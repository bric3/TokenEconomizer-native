# Rust and Python hook performance

On an Apple M4 Max, the release Rust hook ran 1.9x to 4.8x faster and used
28% to 64% less peak resident memory than the pinned Python hook across four
offline Claude Code workloads in the controlled rerun. The largest fold used
60 ms of median CPU time in Rust and 140 ms in Python.

## Results

Both runs on 20 November 2026 used arm64 macOS 27.2, CPython 3.15.2,
Rust 1.99.1 and Temurin 25.0.5. The Rust release build used revision
`f9d13b5999d715235ecaa67bd6c9440d89831dc7` and the checked-in Cargo lockfile.
The Python process imported the archived Microsoft TokenEconomizer revision
`583fff78bf504b59d542f953c31e87d01d02ab42`. [Reference pin](../reference/revision.txt)

### Controlled rerun

The documented commands ran in a fresh workspace with a new Python environment
and a temporary bytecode cache. They reused the release binaries built from
the revision above.

| Workload          | Pairs | Python, ms | Rust, ms | Speed-up | Python RSS, MiB | Rust RSS, MiB |
| ----------------- | ----: | ---------: | -------: | -------: | --------------: | ------------: |
| Start a session   |    21 |       74.3 |     15.5 |     4.8x |            25.9 |           9.4 |
| Record one result |    21 |       70.6 |     15.4 |     4.6x |            25.9 |          10.2 |
| Fold 96 calls     |    15 |       84.3 |     23.2 |     3.6x |            26.3 |          14.9 |
| Fold 3,072 calls  |    11 |      166.7 |     89.5 |     1.9x |            40.8 |          29.2 |

### Original run

| Workload          | Pairs | Python, ms | Rust, ms | Speed-up | Python RSS, MiB | Rust RSS, MiB |
| ----------------- | ----: | ---------: | -------: | -------: | --------------: | ------------: |
| Start a session   |    21 |       82.9 |     16.9 |     4.9x |            25.8 |           9.3 |
| Record one result |    21 |       74.3 |     15.2 |     4.9x |            25.9 |          10.1 |
| Fold 96 calls     |    15 |       82.8 |     23.1 |     3.6x |            26.3 |          15.2 |
| Fold 3,072 calls  |    11 |      171.6 |     83.7 |     2.1x |            40.9 |          29.3 |

Time and peak resident set size (RSS) are medians from separate passes over the
same workloads. Speed-up is the Python median wall time divided by the Rust
median. The large fold used 57% less CPU time in the controlled rerun and 60%
less in the original run. macOS `time` reports user and system time in
hundredths of a second, which is too coarse to compare shorter hooks reliably.

## Reproduce

Use a macOS checkout with the benchmark driver, Git, a Rust toolchain and
C compiler, CPython 3.15.2, and JBang with Java 25. The Python hook loads
directly from the archive and uses the standard library. From the checkout
root, create the local Python environment if needed and build both release
executables. [Benchmark driver](../validation/PythonRustHookBenchmark.java)

```sh
python3.15 -m venv --without-pip .venv
cargo build --release --locked --target-dir target --bin te --bin te-hook
jbang validation/PythonRustHookBenchmark.java --direct
jbang validation/PythonRustHookBenchmark.java
```

The first benchmark command times each hook directly and writes
`validation/results/python-vs-rust-direct-samples.tsv`. The second also collects
CPU time and peak RSS with macOS `/usr/bin/time -l` and writes
`validation/results/python-vs-rust-samples.tsv`. Each command prints medians
and records the workload, transcript bytes, pair number, runtime and measurements
in the TSV. Direct samples leave the CPU and RSS fields empty. The counter pass
requires access to macOS process counters. Some sandboxes block that access.

## Method

The driver launches `.venv/bin/python -m tokeneconomizer.plugin.hook --host
claude` with the archive on `PYTHONPATH`, and `target/release/te-hook --host
claude`. Both receive the same JSON payloads on stdin. Every trial gets a
fresh temporary home and TokenEconomizer state directory. Both use the same
synthetic repository and, for each fold workload, the same JSONL transcript.

- `SessionStart` creates a session with fresh TokenEconomizer state. The
  measurement includes the hook's state initialisation and writes.
- `PostToolUse` records one successful `Read`. The driver prepares the
  session and prompt before timing, then sends `Stop` afterwards to check that
  the turn recorded exactly one tool call.
- `SessionEnd` closes a prepared session and folds its transcript inline.
  The small fold has eight prompts with 12 calls each (about 84 kB). The
  large fold has 128 prompts with 24 calls each (about 2.9 MB). Each prompt
  contains an edit, a failed test command, a successful retry, and reads.
  These are transcript records. Each fold must write one ledger row with
  the expected prompt and request counts and 435 effective tokens per request.

The driver sets `TE_MODE=headless`, `TE_FOLD_SYNC=1`,
`TE_MICROSKILLS=envfacts`, `TE_TOOLPRUNE=off`, and `TE_LOCAL_MODEL=off`.
This runs deterministic environment-fact learning during folds. It checks
that each hook exits successfully, replies with `{}`, and leaves no
`errors.log`. It also checks that the session, turn, or ledger state was
written, since a hook can reply with `{}` after an internal failure.

Each pass discards two warm-up pairs per workload and alternates runtime
order across the measured pairs. A monotonic clock measures launch-to-exit
wall time, including process startup and the hook's I/O. Input generation,
setup hooks and output checks sit outside that interval. The direct pass
avoids the timing wrapper. The counter pass reads user plus system CPU time
and maximum RSS from `time -l`. Both passes use fresh hook processes and
reuse files, so their file caches warm up. The checked-in driver gives
Python a fresh temporary bytecode cache per pass, which the warm-ups populate.

## Coverage gaps

- **Transcript variety.** The two fold sizes use regular synthetic traces with
  short tool results and a fixed usage pattern. Test anonymised production
  traces with different lengths, large results, interruptions and malformed
  records to see how the speed-up varies with the input.
- **Active mechanisms and other hosts.** The run covers one Claude Code
  hook protocol and local learning. Warm repositories with prior learned
  state, guard redirects, tool pruning, history import, repository maps,
  and other host adapters need their own scenarios. Measure external model
  calls separately when evaluating whole-session time.
- **Scheduling and contention.** Inline folding measures the cost of the
  fold and the hook together. With the default detached fold, the caller
  sees the time to schedule the job and the worker incurs the processing
  cost. Measure both, plus queue delay and contention under concurrent hooks,
  to compare that workflow.
- **Startup and tails.** Fresh processes ran after warm-up on one Mac.
  The original run could reuse Python bytecode from earlier tests. The
  checked-in driver controls that cache. The timer starts at the hook
  executable or Python interpreter, so the installed plugin launchers,
  first launch after installation, cold file caches, and tail latency
  still need dedicated runs.
- **Portability and resource scope.** Peak RSS describes the measured hook
  process. Whole-agent memory, detached-worker memory, and end-to-end
  SWE-bench or Web-Bench time need separate measurements. Repeat the
  comparison on Linux and Windows before applying these ratios there.
