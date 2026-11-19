"""Environment, path and file bindings for native environment-loader comparisons."""
import builtins
import json
import ntpath
import os
from pathlib import Path
import sys
import types

ROOT=Path(__file__).resolve().parents[1]
sys.path[:0]=[str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments/scripts")]
import _env as module
if "--native" in sys.argv:
    from native_experiment_providers import install
    install(driver_module=module)

for line in sys.stdin:
    request=json.loads(line)
    calls=[]
    errors=[]
    saved={key:getattr(module,key) for key in ("os","_REPO_ROOT")}
    def record(name,args=None):
        calls.append([name,args])
        if request.get("fail") == name:
            error=getattr(builtins,request.get("error","RuntimeError"))("fixture failure")
            errors.append(error)
            raise error
    class Environment(dict):
        def get(self,key,*default): record("get",[key,*default]);return super().get(key,*default)
        def __setitem__(self,key,value): record("set",[key,value]);return super().__setitem__(key,value)
    process_environment=dict(os.environ) if request.get("real_env") else None
    if process_environment is not None:
        os.environ.clear()
        os.environ.update(request.get("env",{}))
        environment=os.environ
    else: environment=Environment(request.get("env",{}))
    def exists(path):
        record("exists",path)
        return path in request["existing"] if "existing" in request else os.path.exists(path)
    class Input:
        def __enter__(self): record("enter");return self
        def read(self): record("read");return request.get("document","")
        def __exit__(self,kind,error,traceback):
            record("exit",[kind.__name__ if kind else None,type(sys.exception()).__name__ if sys.exception() else None])
            return request.get("suppress",False)
    def opener(path,**kwargs): record("open",[path,kwargs]);return Input()
    try:
        paths = ntpath if request.get("windows") else os.path
        module.os=types.SimpleNamespace(environ=environment,path=types.SimpleNamespace(join=paths.join,dirname=paths.dirname,exists=exists))
        module._REPO_ROOT=request.get("root","/fixture/experiments")
        if request.get("file_fixture"): module.open=opener
        operation=request.get("operation","parse")
        if operation == "parse":
            if request.get("direct") and "--native" in sys.argv:
                from native_plugin import invoke
                value=invoke("experiment.environment.parse",{"text":request.get("text","")})
            else: value=module._parse(request.get("text",""))
        elif operation == "path": value=module.env_path(request.get("path"))
        elif operation == "load": value=module.load_env(request.get("path"),request.get("override",False))
        else: value=module.require_keys(*request.get("names",[]))
        output={"value":value}
    except BaseException as error: output={"raised":type(error).__name__,"message":str(error),"same_error":bool(errors) and error is errors[-1]}
    finally:
        final_environment=dict(environment)
        if process_environment is not None:
            os.environ.clear()
            os.environ.update(process_environment)
        if request.get("file_fixture"): del module.open
        for key,value in saved.items(): setattr(module,key,value)
    output.update(calls=calls,env=final_environment)
    print(json.dumps(output,sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
