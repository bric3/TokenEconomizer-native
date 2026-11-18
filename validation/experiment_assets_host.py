"""Object and process bindings for native experiment asset selection checks."""
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT=Path(__file__).resolve().parents[1]
sys.path[:0]=[str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
from te_bench.baselines import repomap
from te_harness import codex, webbench
if "--native" in sys.argv:
    from native_codex_install import install as install_codex
    from native_experiment_providers import install
    install_codex()
    install(scaffold_module=repomap)
    install(webbench)

for line in sys.stdin:
    request=json.loads(line)
    replacement=None
    previous_process=None
    saved={key:os.environ.get(key) for key in ("PYTHONPATH","TE_REPOMAP_TOKENS","ASSET_SDK_FAIL")}
    try:
        if "sdk" in request: os.environ["PYTHONPATH"]=request["sdk"]
        if "budget" in request: os.environ["TE_REPOMAP_TOKENS"]=request["budget"]
        if request.get("fail"): os.environ["ASSET_SDK_FAIL"]="1"
        if "bridge" in request and "--native" in sys.argv:
            import native_plugin
            previous_process=native_plugin.PROCESS
            replacement=subprocess.Popen([request["bridge"]],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,encoding="utf-8")
            native_plugin.PROCESS=replacement
        if request.get("operation") == "prompt": value=codex.base_prompt_for(request.get("version"))
        elif request.get("operation") == "web": value=webbench.webbench_undici_env(request.get("env",{}),repo=request.get("repo"))
        else: value=repomap.RepoMapScaffold().generate(request["directory"],request.get("idents",["Widget"]))
        output={"value":value}
    except BaseException as error: output={"raised":type(error).__name__,"message":str(error)}
    finally:
        if replacement is not None:
            replacement.stdin.close()
            try: replacement.wait(timeout=5)
            except subprocess.TimeoutExpired: replacement.kill();replacement.wait()
            native_plugin.PROCESS=previous_process
        for key,value in saved.items():
            if value is None: os.environ.pop(key,None)
            else: os.environ[key]=value
    print(json.dumps(output,sort_keys=True,separators=(",",":"),ensure_ascii=False),flush=True)
