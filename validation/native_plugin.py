"""Pytest transport adapter: original assertions call the Rust implementation.

This is test compatibility code, retained only until the final test conversion.
Production calculations live in Rust. Original test files remain unmodified.
"""
import collections
import functools
import importlib
import inspect
import json
import os
from pathlib import Path
import subprocess
import sys
import threading

ROOT = Path(__file__).resolve().parents[1]
CALLS = collections.Counter()
LOCK = threading.Lock()
PROCESS = None


def invoke(operation, arguments):
    global PROCESS
    with LOCK:
        if PROCESS is None:
            PROCESS = subprocess.Popen(
                [str(ROOT / "target/debug/te-bridge")], stdin=subprocess.PIPE,
                stdout=subprocess.PIPE, text=True, encoding="utf-8")
        PROCESS.stdin.write(json.dumps({"op": operation, "args": arguments}) + "\n")
        PROCESS.stdin.flush()
        line = PROCESS.stdout.readline()
        if not line:
            raise RuntimeError(f"Rust bridge exited: {PROCESS.poll()}")
        response = json.loads(line)
        CALLS[operation] += 1
        if "error" in response:
            raise ValueError(response["error"])
        return response["result"]


def wrap(original, operation):
    signature = inspect.signature(original)

    @functools.wraps(original)
    def native(*args, **kwargs):
        bound = signature.bind(*args, **kwargs)
        bound.apply_defaults()
        arguments = dict(bound.arguments)
        obj = arguments.get("self")
        if obj is not None:
            arguments["self"] = obj.to_dict()
        if "files" in arguments:
            arguments["files"] = sorted(arguments["files"])
        result = invoke(operation, arguments)
        if operation == "lifecycle.update":
            obj.__dict__.update(result)
            return None
        if operation == "accounting.effective":
            return tuple(result)
        return result

    return native


def pytest_configure(config):
    sys.path.insert(0, str(ROOT / "reference/upstream"))
    groups = {
        "pricing": ("resolve_model", "weights_for", "has_breakdown", "cache_write_1h_tokens",
                    "effective_tokens", "approx_tokens"),
        "ledger": ("overhead", "effective_exec"),
        "plugin.accounting": ("effective", "overhead_total", "summarize", "fmt_tokens"),
        "names": ("repo_filename",),
    }
    replacements = {}
    for module_name, functions in groups.items():
        module = importlib.import_module("tokeneconomizer." + module_name)
        for name in functions:
            original = getattr(module, name)
            operation = module_name.removeprefix("plugin.") + "." + name
            replacement = wrap(original, operation)
            replacements[original] = replacement
            setattr(module, name, replacement)

    from tokeneconomizer.microskills.lifecycle import Lifecycle
    for name in ("_drifted_confirms", "score", "firing_rate", "injectable", "update"):
        setattr(Lifecycle, name, wrap(getattr(Lifecycle, name), "lifecycle." + name))

    # Package re-exports and imports made before this plugin was configured keep aliases.
    for module in list(sys.modules.values()):
        if module and getattr(module, "__name__", "").startswith("tokeneconomizer"):
            for name, value in list(vars(module).items()):
                if inspect.isfunction(value) and value in replacements:
                    setattr(module, name, replacements[value])


def pytest_sessionfinish(session, exitstatus):
    path = Path(os.environ.get("TE_NATIVE_COUNTS", ROOT / "validation/results/native-calls.json"))
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(dict(CALLS), indent=2) + "\n", encoding="utf-8")
    if PROCESS is not None:
        PROCESS.stdin.close()
        PROCESS.wait(timeout=10)
        PROCESS.stdout.close()
