"""Standard-library AST and object transport; scan and deletion policy lives in Rust."""
import ast
import contextlib
import importlib
import io
import json
import math
import sys

objects = []


def retain(value):
    objects.append(value)
    return len(objects) - 1


def flatten(tree):
    nodes, indices, encoded = [], {}, []

    def value(item):
        if isinstance(item, ast.AST):
            identity = id(item)
            if identity not in indices:
                indices[identity] = len(nodes)
                nodes.append(item)
            return {"node": indices[identity]}
        if isinstance(item, list):
            return [value(part) for part in item]
        if isinstance(item, str) and any(0xD800 <= ord(c) <= 0xDFFF for c in item):
            return {"scalar": "str", "points": list(map(ord, item))}
        if isinstance(item, int) and not -(2**63) <= item < 2**64:
            return {"scalar": "int", "hex": hex(item)}
        if isinstance(item, float) and not math.isfinite(item):
            return {"scalar": "float", "hex": item.hex()}
        if item is None or isinstance(item, (str, int, float, bool)):
            return item
        return {"scalar": type(item).__name__, "repr": repr(item)}

    value(tree)
    for node in nodes:
        record = {"_type": type(node).__name__}
        record.update((name, value(member)) for name, member in ast.iter_fields(node))
        for name in ("lineno", "end_lineno"):
            if hasattr(node, name):
                record[name] = getattr(node, name)
        encoded.append(record)
    return {"nodes": encoded}


def call(op, args):
    if op == "parse":
        return flatten(ast.parse(args[0], filename=args[1]))
    if op == "stdlib":
        return list(sys.stdlib_module_names)
    if op == "import":
        return retain(importlib.import_module(args[0]))
    if op == "getattr":
        return retain(getattr(objects[args[0]], args[1]))
    if op == "is_type":
        return isinstance(objects[args[0]], type)
    if op == "hasattr":
        return hasattr(objects[args[0]], args[1])
    if op == "issubclass":
        return issubclass(objects[args[0]], objects[args[1]])
    raise ValueError("unknown AST binding operation: " + op)


for line in sys.stdin:
    request = json.loads(line)
    stdout, stderr = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
        try:
            response = {"value": call(request["op"], request["args"])}
        except BaseException as error:
            response = {"error": {"kind": type(error).__name__, "message": str(error),
                "bases": [base.__name__ for base in type(error).__mro__]}}
    response.update(stdout=stdout.getvalue(), stderr=stderr.getvalue())
    print(json.dumps(response, ensure_ascii=True), flush=True)
