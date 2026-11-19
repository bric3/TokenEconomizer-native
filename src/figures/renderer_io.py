"""Renderer object transport. Figure data, layout and export options belong to Rust."""
import contextlib
import json
import io
import sys

with contextlib.redirect_stdout(sys.stderr):
    if sys.argv[1] == "matplotlib":
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
        from matplotlib.lines import Line2D
        from matplotlib.patches import Circle, FancyArrowPatch, FancyBboxPatch, Rectangle
        roots = {"plt": plt, "Line2D": Line2D, "FancyBboxPatch": FancyBboxPatch,
                 "Circle": Circle, "FancyArrowPatch": FancyArrowPatch, "Rectangle": Rectangle}
    elif sys.argv[1] == "pillow":
        from PIL import Image, ImageDraw, ImageFont
        roots = {"Image": Image, "ImageDraw": ImageDraw, "ImageFont": ImageFont,
                 "BytesIO": io.BytesIO, "bytes": bytes}
    else:
        raise ValueError("unknown figure renderer")
objects = dict(roots)
next_id = 0


def lookup(path):
    root, *parts = path.split(".")
    value = objects[root]
    for name in parts:
        value = getattr(value, name)
    return value


def unpack(value):
    if isinstance(value, dict):
        if set(value) == {"$ref"}:
            return lookup(value["$ref"])
        if set(value) == {"$tuple"}:
            return tuple(unpack(item) for item in value["$tuple"])
        return {key: unpack(item) for key, item in value.items()}
    if isinstance(value, list):
        return [unpack(item) for item in value]
    return value


for line in sys.stdin:
    request = json.loads(line)
    with contextlib.redirect_stdout(sys.stderr):
        try:
            if request["op"] == "keep":
                objects = {key: objects[key] for key in request["handles"]}
                objects.update(roots)
                print(json.dumps({"value": None}), file=sys.__stdout__, flush=True)
                continue
            target = lookup(request["target"])
            if request["op"] == "call":
                result = target(*unpack(request["args"]), **unpack(request["kwargs"]))
                key = str(next_id)
                next_id += 1
                objects[key] = result
                response = {"value": key}
            elif request["op"] == "value":
                response = {"value": target.tolist() if hasattr(target, "tolist") else target}
            else:
                raise ValueError("unknown renderer binding operation")
        except Exception as error:
            response = {"error": f"{type(error).__name__}: {error}"}
    print(json.dumps(response, ensure_ascii=True), flush=True)
