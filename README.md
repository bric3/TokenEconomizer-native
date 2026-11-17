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

The native repository-map generator is available now. It uses an optional aider
installation in the interpreter selected with `--python`:

```sh
cargo build --bin te-repomap-gen
target/debug/te-repomap-gen --python /path/to/python /path/to/repo 2048 \
  'Engine,parse_config' --git --cache /path/to/state/tags
```

Native hook integration, plugin packaging and the remaining migration gates are
still in progress.
