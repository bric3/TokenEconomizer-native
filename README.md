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

The bundler reads the pinned host assets from `reference/upstream` and copies
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

Installed host verification and the remaining migration gates are still in progress.
