"""File fixtures and original/native WebBench exporter oracle."""
import contextlib
import importlib.util
import io
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT / "reference/upstream"), str(ROOT / "reference/upstream/experiments")]
spec = importlib.util.spec_from_file_location("export_webbench_tasks", ROOT / "reference/upstream/experiments/scripts/export_webbench_tasks.py")
exporter = importlib.util.module_from_spec(spec)
spec.loader.exec_module(exporter)
SOURCE_BUILD = exporter.build
if "--native" in sys.argv:
    from native_web_export import install
    install(exporter)
BASE = os.getcwd()


def fixtures(base):
    projects = base / "projects"
    for project, ids in exporter.workloads.WEBBENCH_TASKS.items():
        directory = projects / project
        directory.mkdir(parents=True)
        rows = [{"id": iid, "description": f"{iid}: é😀\r\n\t\\\""} for iid in reversed(ids)]
        rows.extend([{"id": "unknown"}, {"id": ids[0], "description": "last duplicate é😀"}])
        (directory / "tasks.jsonl").write_text("\u001c \u001f\n" + "\r\n".join(json.dumps(row, ensure_ascii=False) for row in rows) + "\r", encoding="utf-8")
    return projects


for line in sys.stdin:
    request = json.loads(line)
    stdout, stderr = io.StringIO(), io.StringIO()
    base = Path(request["base"])
    saved_argv, saved_env = sys.argv, os.environ.copy()
    out = base / "out.json"
    before = None
    try:
        if base.exists(): shutil.rmtree(base)
        base.mkdir(parents=True)
        projects = fixtures(base)
        os.chdir(base)
        for key in (exporter.PROJECTS_ENV, exporter.TASKS_ENV): os.environ.pop(key, None)
        state = request.get("state", "missing")
        if state in ("good", "crlf", "stale", "invalid-utf8"):
            want = json.dumps(SOURCE_BUILD(projects), indent=2, ensure_ascii=False) + "\n"
            out.write_bytes(b"\xff" if state == "invalid-utf8" else ("old\n" if state == "stale" else want.replace("\n", "\r\n") if state == "crlf" else want).encode())
            os.utime(out, ns=(1000000000, 1000000000))
            before = out.stat().st_mtime_ns
        if state == "directory": out.mkdir()
        target = projects / request.get("project", "calculator") / "tasks.jsonl"
        if request.get("mutation") == "missing": target.unlink()
        elif request.get("mutation") == "directory":
            target.unlink()
            target.mkdir()
        elif "content" in request: target.write_text(request["content"], encoding="utf-8")
        elif "bytes" in request: target.write_bytes(bytes(request["bytes"]))
        elif "description" in request or request.get("drop_description"):
            rows = [json.loads(line) for line in target.read_text().splitlines() if line.strip()]
            for row in rows:
                if request.get("drop_description"): row.pop("description", None)
                else: row["description"] = request["description"]
            target.write_text("\n".join(json.dumps(row) for row in rows), encoding="utf-8")
        for key, value in request.get("env", {}).items():
            os.environ[key] = value.replace("@PROJECTS@", str(projects)).replace("@OUT@", str(out))
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            if request["method"] == "fixtures": value = str(projects)
            elif request["method"] == "build": value = exporter.build(request.get("root", "projects"))
            else:
                sys.argv = ["export_webbench_tasks.py", *request.get("argv", ["--projects-root", "projects", "--out", "out.json"])]
                value = exporter.main()
        response = {"value": value}
    except BaseException as error:
        response = {"raised": type(error).__name__, "message": str(error)}
    finally:
        sys.argv = saved_argv
        os.environ.clear()
        os.environ.update(saved_env)
        os.chdir(BASE)
    response.update(stdout=stdout.getvalue(), stderr=stderr.getvalue())
    if request["method"] == "main":
        response["output_sha256"] = hashlib.sha256(out.read_bytes()).hexdigest() if out.is_file() else None
        response["untouched"] = out.stat().st_mtime_ns == before if before is not None else None
    print(json.dumps(response, sort_keys=True, separators=(",", ":"), ensure_ascii=False), flush=True)
