# TokenEconomizer native plugin

This bundle contains Rust executables for the platform that built it. Keep the
three executables together in `scripts/`: `te`, `te-hook` and `te-repomap-gen`
(with `.exe` on Windows).

The hook reads the host's JSON payload from stdin and writes its JSON response
to stdout. Diagnostics go to stderr. The CLI and background work use `te`.
Core processing needs no Python interpreter. Repository maps use an optional
external aider installation. Hermes loads its adapter inside its Python host.

The bundle retains the host manifests, hook deadlines and command locations of
the pinned upstream plugin. The Droid bundle uses `.factory-plugin`, its own
hook layout and `commands/tokeneconomizer-*.md` commands.

The marketplace metadata lives beside the `tokeneconomizer` directory. Install
the directory with the matching host's plugin installer, or load it for a Claude
Code session with `claude --plugin-dir /path/to/tokeneconomizer`.

The bundle carries its runtime and can move independently of the build checkout.
It reads and writes the existing TokenEconomizer state formats.

`LICENSE` preserves the upstream Apache-2.0 licence. `THIRD-PARTY-NOTICES.txt`
contains the legal texts for locked Rust dependencies, Unicode data and the
CPython-derived timestamp code.
