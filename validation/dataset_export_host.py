"""Dataset iterator and filesystem fixtures for original/native exporter comparisons."""
import builtins
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import sys
import types

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT / "reference/upstream"), str(ROOT / "reference/upstream/experiments")]
from te_bench import workloads
spec = importlib.util.spec_from_file_location("export_swebench_tasks", ROOT / "reference/upstream/experiments/scripts/export_swebench_tasks.py")
exporter = importlib.util.module_from_spec(spec)
spec.loader.exec_module(exporter)
if "--native" in sys.argv:
    from native_dataset_exports import install
    install(exporter)

ORDER = [iid for ids in workloads.runnable_instances("swebench").values() for iid in ids]
BASE = os.getcwd()


def fixture_row(iid, tag):
    row = {key: tag + ":" + key + " é😀\r\n" for key in exporter.GRADING_FIELDS}
    row.update(instance_id=iid, repo="fixture/repo", problem_statement=tag + ":issue é😀\n")
    return row


for line in sys.stdin:
    request = json.loads(line)
    calls, errors = [], []
    stdout, stderr = io.StringIO(), io.StringIO()
    saved = sys.argv
    base = Path(request.get("base", BASE))
    rows = request.get("rows", [[fixture_row(iid, "verified") for iid in reversed(ORDER[::2])],
        [fixture_row(iid, "full") for iid in ORDER] + [fixture_row("unknown", "full")]])
    if request.get("missing"):
        rows = [[row for row in split if row.get("instance_id") not in request["missing"]] for split in rows]
    if "drop_field" in request:
        rows[0][-1].pop(request["drop_field"], None)

    def fail(point):
        if request.get("fail") == point:
            error = getattr(builtins, request.get("error", "RuntimeError"))("fixture failure")
            errors.append(error)
            raise error

    class Rows:
        def __init__(self, index): self.index = index
        def __iter__(self):
            calls.append(["rows.iter", self.index])
            fail(f"rows.iter:{self.index}")
            return RowCursor(self.index)

    class RowCursor:
        def __init__(self, index): self.index, self.position = index, 0
        def __iter__(self): return self
        def __next__(self):
            calls.append(["rows.next", self.index, self.position])
            fail(f"rows.next:{self.index}:{self.position}")
            if self.position == len(rows[self.index]): raise StopIteration
            row = rows[self.index][self.position]
            self.position += 1
            return row

    class Streams:
        def __iter__(self):
            calls.append(["outer.iter"])
            fail("outer.iter")
            return StreamCursor()

    class StreamCursor:
        def __init__(self): self.position = 0
        def __iter__(self): return self
        def __next__(self):
            calls.append(["outer.next", self.position])
            fail(f"outer.next:{self.position}")
            if self.position == len(rows): raise StopIteration
            result = Rows(self.position)
            self.position += 1
            return result

    def load_dataset(path, *, split):
        index = exporter.SPLITS.index(path)
        calls.append(["load_dataset", path, split])
        fail(f"load_dataset:{index}")
        return Rows(index)

    try:
        if request["method"] in ("main", "fixtures"):
            if base.exists(): shutil.rmtree(base)
            base.mkdir(parents=True)
        if request["method"] == "fixtures":
            for index, split in enumerate(rows):
                (base / f"split-{index}.json").write_text(json.dumps(split), encoding="utf-8")
            response = {"value": str(base)}
        else:
            with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                if request["method"] == "build":
                    value = exporter.build(Streams(), set(request.get("wanted", ORDER)))
                elif request["method"] == "constants":
                    value = {"splits": exporter.SPLITS, "grading_fields": exporter.GRADING_FIELDS}
                else:
                    os.chdir(base)
                    out = Path(request.get("out", "out"))
                    if request.get("existing"):
                        out.mkdir(parents=True, exist_ok=True)
                        for name in ("swebench_tasks.json", "swebench_grading.json", "image_tags.txt"):
                            (out / name).write_text("old export\n", encoding="utf-8")
                    if "blocked_file" in request:
                        out.mkdir(parents=True, exist_ok=True)
                        blocked = out / request["blocked_file"]
                        if blocked.is_file(): blocked.unlink()
                        blocked.mkdir()
                    sys.modules["datasets"] = types.SimpleNamespace() if request.get("sdk_missing") else types.SimpleNamespace(load_dataset=load_dataset)
                    sys.argv = ["export_swebench_tasks.py", *request.get("argv", ["--out-dir", str(out)])]
                    value = exporter.main()
            response = {"value": value}
    except BaseException as error:
        response = {"raised": type(error).__name__, "message": str(error), "same_error": any(error is item for item in errors)}
    finally:
        sys.argv = saved
        os.chdir(BASE)
    response.update(stdout=stdout.getvalue(), stderr=stderr.getvalue(), calls=calls)
    if request["method"] == "main":
        response["files"] = {str(path.relative_to(base)): path.read_bytes().decode("utf-8") for path in sorted(base.rglob("*")) if path.is_file()}
        response["directories"] = [str(path.relative_to(base)) for path in sorted(base.rglob("*")) if path.is_dir()]
    print(json.dumps(response, sort_keys=True, separators=(",", ":"), ensure_ascii=False), flush=True)
