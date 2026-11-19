"""Bash variable and console transport for original launch-function tests."""
import os
import re
from pathlib import Path
import shlex
import shutil
import sys
import tempfile

from native_plugin import ROOT, invoke


def rewrite(argv, options):
    experiment = ROOT / "reference/upstream/experiments"
    if len(argv) >= 2 and str(argv[1]) == str(experiment / "scripts/launch/fleet_budget.sh"):
        return [str(ROOT / "target/debug/te-bench"), "fleet-budget", *argv[2:]]
    environment = options.get("env") or os.environ
    if Path(options.get("cwd", os.getcwd())).resolve() != experiment and environment.get("REPO") != str(experiment):
        return argv
    for index, argument in enumerate(argv[:-1]):
        if argument not in ("-c", "-uc", "-cu"): continue
        script = argv[index + 1]
        pattern = r'(?P<lead>^|[;\s])(?:\.|source)\s+(?:"\$REPO/scripts/launch/_env\.sh"|scripts/launch/_env\.sh)'
        if re.search(pattern, script):
            binding = shell_binding()
            changed = re.sub(pattern, lambda match: match["lead"] + ". " + shlex.quote(binding), script)
            return [*argv[:index + 1], changed, *argv[index + 2:]]
    return argv


def shell_binding():
    functions = invoke("experiment.launch.constants", {})["functions"]
    directory = ROOT / "validation/results"
    fd, path = tempfile.mkstemp(prefix="launch-binding-", suffix=".sh", dir=directory)
    python = shlex.quote(str(ROOT / ".venv/bin/python"))
    transport = shlex.quote(str(Path(__file__).resolve()))
    mktemp = shlex.quote(shutil.which("mktemp"))
    rm = shlex.quote(shutil.which("rm"))
    body = f"""
_te_native_launch() {{
  local _te_name="$1"; shift
  local _te_state; _te_state=$({mktemp} "${{TMPDIR:-/tmp}}/te-launch.XXXXXXXX") || return 1
  local _TE_NATIVE_STOP=0 _te_rc _te_key
  {{
    printf '%s\\0%s\\0' "$_te_name" "$#"
    if (($#)); then printf '%s\\0' "$@"; fi
    while IFS= read -r _te_key; do printf '%s\\0%s\\0' "$_te_key" "${{!_te_key-}}"; done < <(compgen -v)
  }} | {python} -B {transport} "$_te_state"
  _te_rc=$?
  . "$_te_state"
  {rm} -f "$_te_state"
  if [[ "$_TE_NATIVE_STOP" == 1 ]]; then exit "$_te_rc"; fi
  return "$_te_rc"
}}
"""
    body += "\n".join(name + '() { _te_native_launch ' + name + ' "$@"; }' for name in functions) + "\n"
    with os.fdopen(fd, "w", encoding="utf-8") as output: output.write(body)
    return path


if __name__ == "__main__":
    parts = sys.stdin.buffer.read().decode("utf-8").split("\0")
    name, size = parts[0], int(parts[1])
    arguments = parts[2:2 + size]
    rest = parts[2 + size:-1]
    scope = dict(zip(rest[::2], rest[1::2]))
    value = invoke("experiment.launch.function", {"name": name, "args": arguments,
        "context": {"env": scope, "exported": list(os.environ), "cwd": os.getcwd()}})
    with open(sys.argv[1], "w", encoding="utf-8") as state:
        for key, item in value["changed"].items():
            state.write("unset " + shlex.quote(key) + "\n" if item is None else "export " + shlex.quote(key + "=" + item) + "\n")
        state.write("_TE_NATIVE_STOP=" + str(int(value["stop"])) + "\n")
    print(value["stdout"], end="")
    print(value["stderr"], end="", file=sys.stderr)
    raise SystemExit(value["code"])
