"""Filesystem, subprocess and mapping fixtures for Copilot host policy."""
import builtins
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import types

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
from te_harness import copilot as cp
if "--native" in sys.argv:
    from native_experiment_providers import install
    install()

for line in sys.stdin:
    request = json.loads(line)
    calls = []
    saved = {name:getattr(cp,name) for name in ("os","shutil","subprocess","copilot_bin","copilot_cache","copilot_config","auth_env","provider_env")}
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,dict): return {key:encode(item) for key,item in value.items()}
        if isinstance(value,(tuple,list)): return [encode(item) for item in value]
        if isinstance(value,bytes): return {"bytes":list(value)}
        return value
    def record(name,args=None):
        calls.append([name,encode(args)])
        if request.get("fail") == name:
            kind = request.get("error","RuntimeError")
            if kind == "TimeoutExpired": raise subprocess.TimeoutExpired("fixture",1)
            if kind == "SubprocessError": raise subprocess.SubprocessError("fixture failure")
            if kind == "ShadowOSError": raise type("OSError",(BaseException,),{})("fixture failure")
            raise getattr(builtins,kind)("fixture failure")
    class Token:
        def __init__(self,name): self.name = name
        def __bool__(self): record("bool:"+self.name);return request.get("truthy",True)
        def strip(self): record("strip");return self
        def splitlines(self): record("splitlines");return [self]
        def __getitem__(self,key): record("slice",[key.start,key.stop,key.step]);return self
    class Environment(dict):
        def get(self,*args): record("get",args);return super().get(*args)
        def __getitem__(self,key): record("item",key);return super().__getitem__(key)
    environment = Environment(request.get("env",{}))
    arguments = request.get("arguments",{})
    for key in request.get("objects",[]): environment[key] = Token(key)
    for key in request.get("argument_objects",[]): arguments[key] = Token(key)
    def realpath(path):
        record("realpath",path)
        if request.get("real_paths"): return os.path.realpath(path)
        return path if isinstance(path,Token) else request.get("realpath","/resolved/copilot")
    def expand(path): record("expanduser",path);return request.get("expanded","/home/fixture/.local/bin/copilot")
    def access(path,mode): record("access",[path,mode]);return os.access(path,mode) if request.get("real_paths") else request.get("accessible",False)
    def which(name): record("which",name);return shutil.which(name,path=request.get("path")) if request.get("real_paths") else request.get("which")
    def exists(path): record("exists",path);return os.path.exists(path) if request.get("real_paths") else path not in request.get("missing",[])
    class Completed:
        @property
        def stdout(self):
            record("stdout")
            if request.get("paired_stdout"): return request.get("stdout","")+"\ud800\udc00"
            return Token("stdout") if request.get("opaque_stdout") else request.get("stdout","fixture 1.2.3\nextra")
        @property
        def stderr(self): record("stderr");return request.get("stderr","")
    def run(argv,**kwargs):
        record("run",[argv,kwargs])
        assert argv[0] is arguments["binary"],"binary identity changed"
        return subprocess.run(argv,**kwargs) if request.get("real_process") else Completed()
    helper_values = {"copilot_bin":"/bin/copilot","copilot_cache":"/cache","copilot_config":"/config","auth_env":{"AUTH":"auth","SAME":"auth"},"provider_env":{"PROVIDER":"provider","SAME":"provider"},**request.get("helper_values",{})}
    if request.get("opaque_helpers"): helper_values.update(copilot_bin=Token("binary"),auth_env={"AUTH":Token("auth")})
    def helper(name):
        def call(): record(name);return helper_values[name]
        return call
    method = request.get("method","copilot_bin")
    try:
        cp.os = types.SimpleNamespace(environ=environment,path=types.SimpleNamespace(realpath=realpath,expanduser=expand,exists=exists),access=access,X_OK=os.X_OK)
        cp.shutil = types.SimpleNamespace(which=which)
        cp.subprocess = types.SimpleNamespace(run=run,SubprocessError=subprocess.SubprocessError)
        if request.get("stub_helpers",True):
            for name in helper_values:
                if name != method: setattr(cp,name,helper(name))
        value = getattr(cp,method)(**arguments)
        if method == "require_install": assert value is helper_values["copilot_bin"],"install result identity changed"
        if method == "run_env" and request.get("opaque_helpers"): assert value["AUTH"] is helper_values["auth_env"]["AUTH"],"credential identity changed"
        if method == "cli_version" and request.get("opaque_stdout"): assert isinstance(value,Token),"foreign string protocol result lost"
        result = {"value":encode(value)}
        if request.get("paired_stdout"): result["points"] = list(map(ord,value))
    except BaseException as error: result = {"raised":type(error).__name__,"message":str(error)}
    finally:
        for name,value in saved.items(): setattr(cp,name,value)
    result["calls"] = calls
    print(json.dumps(result,sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
