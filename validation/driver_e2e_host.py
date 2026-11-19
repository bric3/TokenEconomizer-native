"""Real driver/facade/adapter/stub wiring with owned data and no agent calls."""
import contextlib
import io
import json
import os
from pathlib import Path
import shutil
import sys
import time
import types
from driver_artifacts import numeric_values

ROOT=Path(__file__).resolve().parents[1]
sys.path[:0]=[str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments"),str(ROOT/"reference/upstream/experiments/scripts")]
import _env
import run_experiment as rx
if "--native" in sys.argv:
    from native_experiment_arms import install as arms_install
    from te_bench import arms
    arms_install(arms)
    from native_experiment_providers import install
    install(driver_module=_env)
    rx.load_env=_env.load_env
    install(driver_module=rx)
    from native_experiment_facade import install as facade_install
    facade_install()
    from native_plugin import install_experiment_adapters
    install_experiment_adapters()

for line in sys.stdin:
    request=json.loads(line)
    root=Path(request["root"])
    if root.exists(): shutil.rmtree(root)
    root.mkdir(parents=True)
    saved_environment=dict(os.environ)
    argv=sys.argv
    clock=rx.time
    stdout,stderr=io.StringIO(),io.StringIO()
    try:
        env_file=root/"fixture.env";env_file.write_text("DRIVER_LITERAL='$HOME $(echo literal)'\n",encoding="utf-8")
        os.environ.clear()
        os.environ.update(HOME=str(root),PATH="",TOKENECONOMIZER_ENV_FILE=str(env_file))
        tasks=root/"tasks.json"
        tasks.write_text(json.dumps([dict(instance_id=f"pkg__{index}",problem_statement="Offline fixture",repo="org/pkg") for index in range(request.get("count",3))]),encoding="utf-8")
        rx.time=types.SimpleNamespace(strftime=time.strftime,gmtime=lambda:time.gmtime(0))
        sys.argv=["te-bench","--arm",request.get("arm","baseline"),"--agent",request.get("agent","claude"),"--repo","org/pkg","--tasks-file",str(tasks),"--results-root",str(root/"results"),"--fresh"]
        with contextlib.redirect_stdout(stdout),contextlib.redirect_stderr(stderr): value=rx.main()
        output={"value":value}
    except BaseException as error: output={"raised":type(error).__name__,"message":str(error)}
    finally:
        final_environment={key:os.environ.get(key) for key in ("DRIVER_LITERAL","TE_OPTIMIZER_PREFLIGHT")}
        os.environ.clear();os.environ.update(saved_environment)
        sys.argv=argv;rx.time=clock
    files={}
    for path in sorted((root/"results").rglob("*")):
        if not path.is_file(): continue
        text=path.read_text(encoding="utf-8")
        if path.name == "row.json": files[str(path.relative_to(root))]=json.loads(text)
        elif path.parent.name == "_streams": files[str(path.relative_to(root))]=[json.loads(row) for row in text.splitlines()]
        elif path.name == "transcript.jsonl": files[str(path.relative_to(root))]=text
    output.update(stdout=stdout.getvalue(),stderr=stderr.getvalue(),env=final_environment,files=numeric_values(files))
    print(json.dumps(output,sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
