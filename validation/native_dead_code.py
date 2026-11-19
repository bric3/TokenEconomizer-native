"""Original dead-code API argument, object-identity and console transport."""
import builtins
import os
import sys
from native_plugin import invoke


def items(value):
    for item in value:
        for key in ("_span", "_class"):
            if item.get(key) is not None: item[key] = tuple(item[key])
    return value


def call(name, **arguments):
    response = invoke("dead_code." + name, dict(python=sys.executable, **arguments))
    print(response.get("stdout", ""), end="")
    print(response.get("stderr", ""), end="", file=sys.stderr)
    if response.get("exit"):
        raise SystemExit(response["value"])
    if name == "fix" and response.get("items") is not None:
        for original, updated in zip(arguments["items"], items(response["items"])):
            original.clear(); original.update(updated)
    if "error" in response:
        error = response["error"]
        raise getattr(builtins, error["kind"], RuntimeError)(error["message"])
    return response.get("value")


def install(module):
    module.scan = lambda root: items(call("scan", root=os.fspath(root)))
    def fix(root, items):
        call("fix", root=os.fspath(root), items=items)
        return items
    module.fix = fix
    module.sweep = lambda root, max_passes=20: items(call("sweep", root=os.fspath(root), max_passes=max_passes))
    module.counts = lambda items: call("counts", items=items)
    module.markdown = lambda items, fixed: call("markdown", items=items, fixed=fixed)
    module.main = lambda argv=None: call("main", root=os.path.dirname(os.path.dirname(module.__file__)), argv=sys.argv[1:] if argv is None else argv)
