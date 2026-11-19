"""Original/native YAML mirror oracle with disposable project files."""
import contextlib
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import sys
import yaml

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT / "reference/upstream"), str(ROOT / "reference/upstream/experiments")]
spec = importlib.util.spec_from_file_location("webbench_tasks_from_yaml", ROOT / "reference/upstream/experiments/scripts/webbench_tasks_from_yaml.py")
mirror = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mirror)
SOURCE_RENDER = mirror.render
if "--native" in sys.argv:
    from native_yaml_mirror import install
    install(mirror)
BASE = os.getcwd()

for line in sys.stdin:
    request = json.loads(line)
    base = Path(request["base"])
    saved_env, saved_argv = os.environ.copy(), sys.argv
    saved_load = yaml.safe_load
    stdout, stderr = io.StringIO(), io.StringIO()
    errors, before = [], {}
    try:
        if base.exists(): shutil.rmtree(base)
        base.mkdir(parents=True)
        projects = base / "projects"
        projects.mkdir()
        for name, text in request.get("projects", {"calculator": "- id: init\n  description: |\n    é😀\n"}).items():
            directory = projects / name
            directory.mkdir(parents=True)
            path = directory / "tasks.yml"
            if text is not None: path.write_text(text, encoding="utf-8")
            state = request.get("state", {}).get(name)
            out = directory / "tasks.jsonl"
            if state in ("good", "crlf", "stale", "invalid-utf8"):
                want = SOURCE_RENDER(path)
                out.write_bytes(b"\xff" if state == "invalid-utf8" else ("old\n" if state == "stale" else want.replace("\n", "\r\n") if state == "crlf" else want).encode())
                os.utime(out, ns=(1000000000, 1000000000))
                before[name] = out.stat().st_mtime_ns
            elif state == "directory": out.mkdir()
        os.chdir(base)
        os.environ.pop(mirror.PROJECTS_ROOT_ENV, None)
        if "env" in request:
            os.environ[mirror.PROJECTS_ROOT_ENV] = request["env"].replace("@ROOT@", str(projects))
        if "fail" in request:
            error = getattr(__import__("builtins"), request["fail"])("fixture failure")
            errors.append(error)
            def failed_load(stream): raise error
            yaml.safe_load = failed_load
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            method = request["method"]
            if method == "render":
                path = Path(request.get("path", "projects/calculator/tasks.yml"))
                value = mirror.render(path if request.get("path_object") else str(path))
            elif method == "root":
                explicit = Path(request["explicit"]) if request.get("path_object") else request.get("explicit")
                root = mirror.projects_root(explicit)
                value = {"value": os.fspath(root) if isinstance(root, os.PathLike) else root, "type": type(root).__name__, "same": root is explicit}
            elif method == "projects":
                value = list(mirror.swept_yml_projects("projects"))
            elif method == "lazy":
                iterator = mirror.swept_yml_projects("projects")
                (projects / "calculator/tasks.yml").unlink()
                value = [next(iterator)]
                (projects / "react/tasks.yml").write_text("[]\n")
                value.extend(iterator)
            else:
                sys.argv = ["webbench_tasks_from_yaml.py", *request.get("argv", ["--projects-root", "projects", "--all"])]
                value = mirror.main()
        response = {"value": value}
    except BaseException as error:
        response = {"raised": type(error).__name__, "message": str(error), "same_error": any(error is item for item in errors)}
    finally:
        yaml.safe_load = saved_load
        os.environ.clear()
        os.environ.update(saved_env)
        sys.argv = saved_argv
        os.chdir(BASE)
    response.update(stdout=stdout.getvalue(), stderr=stderr.getvalue())
    response["files"] = {str(path.relative_to(base)): hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(base.rglob("*")) if path.is_file()}
    response["untouched"] = {name: (projects / name / "tasks.jsonl").stat().st_mtime_ns == time for name, time in before.items()}
    print(json.dumps(response, sort_keys=True, separators=(",", ":"), ensure_ascii=False), flush=True)
