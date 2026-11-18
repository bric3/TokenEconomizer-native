# TokenEconomizer experiment runtime

This payload contains the Rust executables, static Linux arm64 and amd64 task
helpers, the captured Codex base prompt and the WebBench Node preload. The
manifest records every payload file's SHA-256 and the dependency lockfile hash.

Keep the executables together in `bin/`. Container helper discovery uses
`bin/container-helpers/`. The payload can move independently of its build checkout.
`te-bridge` exposes the native JSON-lines service protocol.

Standalone experiment host bindings and a benchmark command are still pending.
The payload currently supplies the runtime engine and assets for that integration.
External agents, the official grader SDK, aider and Node remain caller-provided
dependencies where the selected experiment uses them.

This port retains the Apache-2.0 licence. See `LICENSE` and
`THIRD-PARTY-NOTICES.txt` for attribution and dependency licences.
