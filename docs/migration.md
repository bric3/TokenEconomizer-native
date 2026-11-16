# Migration record

The deliverable is a Rust implementation of TokenEconomizer's shipped runtime:
micro-skills, tool pruning, loop guard, trace readers, pricing, accounting,
persistent state, hook processing, CLI and agent integrations. Preserve observable
behaviour, state formats, error recovery and integration contracts. The reference
experiment harness and documentation remain available as compatibility consumers
and fixtures; they are not the shipped runtime.

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
  and the fact lifecycle's promotion, retirement, scoring and firing decisions.
- The test adapter replaces these functions and lifecycle methods in the reference
  process. It records calls reaching Rust in `validation/results/native-calls.json`.
  Other reference code, including Python child processes, still runs in Python.
- Remaining production conversion, six improvement passes and final test conversion
  pending. No performance claim is made from these compatibility runs.

## Verified milestones

- Reference baseline: 1,558 passed, 33 skipped (28.06 seconds).
- First native substitution run: 1,558 passed, 33 skipped (26.52 seconds).
  For example, 523 lifecycle updates and 462 live pricing calls executed in Rust.
  This establishes parity only for the functions substituted by the adapter.
- `cargo build --offline` and `cargo clippy --offline --all-targets -- -D warnings`
  passed on rustc 1.99.1. Cargo's lockfile pins the dependencies used.

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

IntelliJ has no open project for this checkout. Use Cargo's compiler, formatter
and Clippy until IDE access becomes available.
