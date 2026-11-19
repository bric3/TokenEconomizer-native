"""Original/native paired-report oracle with disposable result cells."""
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT / "reference/upstream"), str(ROOT / "reference/upstream/experiments")]
spec = importlib.util.spec_from_file_location("compare_arms", ROOT / "reference/upstream/experiments/scripts/compare_arms.py")
comparison = importlib.util.module_from_spec(spec)
spec.loader.exec_module(comparison)
if "--native" in sys.argv:
    from native_compare import install
    install(comparison)
BASE = os.getcwd()


def fixture_row(i, arm):
    return {"instance": f"task-{i}", "tokens_total": 1000 + i * (90 if arm else 100),
            "resolved": i != (1 if arm else 2), "num_turns": 3 + i,
            "model": "claude-sonnet-4-5", "agent": "claude", "effort": "high", "cli_version": "fixture"}


for line in sys.stdin:
    request = json.loads(line)
    stdout, stderr = io.StringIO(), io.StringIO()
    base = Path(request["base"])
    saved = sys.argv
    try:
        if base.exists(): shutil.rmtree(base)
        base.mkdir(parents=True)
        for side in ("baseline", "arm"):
            rows = request.get(side, [fixture_row(i, side == "arm") for i in range(3)])
            cell = base / side
            cell.mkdir()
            for i, row in enumerate(rows):
                row = row.copy() if isinstance(row, dict) else row
                directory = cell / (row.pop("_dir", f"2026-{i:04d}") if isinstance(row, dict) else f"2026-{i:04d}")
                directory.mkdir(parents=True, exist_ok=True)
                config = row.pop("_config", {"routing": {"base_url": "https://fixture.invalid"}}) if isinstance(row, dict) else None
                raw = row.pop("_raw", None) if isinstance(row, dict) else None
                (directory / "row.json").write_text(raw if raw is not None else json.dumps(row), encoding="utf-8")
                if config is not None:
                    (directory / "run_config.json").write_text(config if isinstance(config, str) else json.dumps(config), encoding="utf-8")
        if request.get("facts"):
            from tokeneconomizer.microskills.envfacts import EnvFact, EnvFactsDistiller
            fact = EnvFact("absent_tool:uv", "absent_tool", False, {"tool": "uv"}).to_dict()
            fact["life"].update(state="ACTIVE", fire_num=4, fire_den=4, promoted_at=1)
            state = {"repo": "fixture/repo", "now": 3, "facts": {fact["id"]: fact}, "forgotten": {}}
            path = EnvFactsDistiller._path(base / "arm", "fixture/repo")
            Path(path).write_text(json.dumps(state), encoding="utf-8")
        if "state" in request:
            directory = base / "arm/envfacts_state"
            directory.mkdir(exist_ok=True)
            (directory / "fixture.json").write_text(json.dumps(request["state"]), encoding="utf-8")
        if "stream" in request:
            directory = base / "arm/_streams"
            directory.mkdir()
            (directory / "fixture.jsonl").write_text(request["stream"], encoding="utf-8")
        if request.get("existing_json"): (base / "comparison.json").write_text("old report\n")
        os.chdir(base)
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            if request["method"] == "load_cell":
                value = [list(rows.items()) for rows in comparison.load_cell(request.get("cell", "baseline"))]
            elif request["method"] == "cost": value = comparison.cost(request["row"])
            elif request["method"] == "describe": value = comparison.describe(dict(request.get("rows", [])), dict(request.get("configs", [])), request.get("label", "baseline"))
            else:
                sys.argv = ["compare_arms.py", *request.get("argv", ["--baseline", "baseline", "--arm", "arm", "--json", "comparison.json"])]
                value = comparison.main()
        response = {"value": value}
    except BaseException as error:
        response = {"raised": type(error).__name__, "message": str(error)}
    finally:
        sys.argv = saved
        os.chdir(BASE)
    response.update(stdout=stdout.getvalue(), stderr=stderr.getvalue(), report=(base / "comparison.json").read_text() if (base / "comparison.json").is_file() else None)
    print(json.dumps(response, sort_keys=True, separators=(",", ":"), ensure_ascii=False), flush=True)
