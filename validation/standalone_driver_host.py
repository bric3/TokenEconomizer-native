"""Standalone SDK integration fixture that rejects reference and validation imports."""
import builtins
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
sys.path.insert(0,str(ROOT/"src/experiments/sdk"))
original_import=builtins.__import__
def guarded(name,*args,**kwargs):
    if name.split(".")[0] in ("tokeneconomizer","te_bench","te_harness","native_plugin"):
        raise AssertionError("standalone binding imported reference/test code: "+name)
    return original_import(name,*args,**kwargs)
builtins.__import__=guarded
from te_runtime.driver import Driver
from te_runtime.transport import Bridge

for line in sys.stdin:
    request=json.loads(line);root=Path(request["root"])
    if root.exists(): shutil.rmtree(root)
    root.mkdir(parents=True)
    saved_environment=dict(os.environ);argv=sys.argv
    stdout,stderr=io.StringIO(),io.StringIO()
    try:
        env_file=root/"fixture.env";env_file.write_text("DRIVER_LITERAL='$HOME $(echo literal)'\n",encoding="utf-8")
        os.environ.clear();os.environ.update(HOME=str(root),PATH="",TOKENECONOMIZER_ENV_FILE=str(env_file))
        tasks=root/"tasks.json"
        tasks.write_text(json.dumps([dict(instance_id=f"pkg__{index}",problem_statement="Offline fixture",repo="org/pkg") for index in range(request.get("count",3))]),encoding="utf-8")
        sys.argv=["te-bench","--arm",request.get("arm","baseline"),"--agent",request.get("agent","claude"),"--repo","org/pkg","--tasks-file",str(tasks),"--results-root",str(root/"results"),"--fresh"]
        with Bridge(ROOT/"target/debug/te-bridge") as bridge:
            driver=Driver(bridge,root)
            driver.module.time=types.SimpleNamespace(strftime=time.strftime,gmtime=lambda:time.gmtime(0))
            with contextlib.redirect_stdout(stdout),contextlib.redirect_stderr(stderr): value=driver.main()
        output={"value":value}
    except BaseException as error: output={"raised":type(error).__name__,"message":str(error)}
    finally:
        environment={key:os.environ.get(key) for key in ("DRIVER_LITERAL","TE_OPTIMIZER_PREFLIGHT")}
        os.environ.clear();os.environ.update(saved_environment);sys.argv=argv
    files={}
    for path in sorted((root/"results").rglob("*")):
        if not path.is_file(): continue
        text=path.read_text(encoding="utf-8")
        if path.name == "row.json": files[str(path.relative_to(root))]=json.loads(text)
        elif path.parent.name == "_streams": files[str(path.relative_to(root))]=[json.loads(row) for row in text.splitlines()]
        elif path.name == "transcript.jsonl": files[str(path.relative_to(root))]=text
    output.update(stdout=stdout.getvalue(),stderr=stderr.getvalue(),env=environment,files=numeric_values(files))
    print(json.dumps(output,sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
