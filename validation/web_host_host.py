"""Path, environment, numeric and subprocess fixtures for Web-Bench host helpers."""
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
from te_harness import swebench
if "--native" in sys.argv:
    from native_experiment_providers import install
    install()
from te_harness import webbench as wb
if "--native" in sys.argv: install(wb)

for line in sys.stdin:
    request = json.loads(line)
    calls = []
    constants = ("WEBBENCH_UNDICI_ABORT_DEFAULT_S","WEBBENCH_TASK_TIMEOUT_DEFAULT","WEBBENCH_TASK_TIMEOUT_HEADROOM_S","UNDICI_PRELOAD")
    saved = {name:getattr(wb,name) for name in ("os","shutil","subprocess","Path","_log","webbench_undici_abort",*constants)}
    integer_limit = sys.get_int_max_str_digits()
    if "int_max_digits" in request: sys.set_int_max_str_digits(request["int_max_digits"])
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,dict): return {key:encode(item) for key,item in dict.items(value)}
        if isinstance(value,(tuple,list)): return [encode(item) for item in value]
        return value
    class SilentError(OSError):
        def __str__(self): record("error.str");return "fixture error"
    def record(name,args=None):
        calls.append([name,encode(args)])
        if request.get("fail") == name:
            kind = request.get("error","RuntimeError")
            if kind == "TimeoutExpired": raise subprocess.TimeoutExpired("fixture",60)
            if kind == "SilentError": raise SilentError()
            if kind == "ShadowValueError": raise type("ValueError",(Exception,),{})("fixture failure")
            if kind == "ShadowOSError": raise type("OSError",(Exception,),{})("fixture failure")
            raise getattr(builtins,kind)("fixture failure")
    class Token:
        def __init__(self,name): self.name = name
        def __bool__(self): record("bool:"+self.name);return request.get("truthy",True)
        def __int__(self): record("int:"+self.name);return request.get("integer",800)
        def __format__(self,spec):
            record("format:"+self.name,spec)
            if request.get("mutate_log"): wb._log = lambda message:record("new.log",message)
            return "formatted-"+self.name
        def __str__(self): record("str:"+self.name);return "string-"+self.name
        def __fspath__(self): record("fspath:"+self.name);return request.get("fspath","/fixture/projects")
        def __le__(self,other): record("le:"+self.name,other);return request.get("less",False)
        def __gt__(self,other): record("gt:"+self.name,other);return request.get("greater",True)
        def __ne__(self,other): record("ne:"+self.name,other);return request.get("different",False)
        def __sub__(self,other): record("sub:"+self.name,other);return request.get("cap",260)
        def __mul__(self,other): record("mul:"+self.name,other);return request.get("millis",800000)
        def __add__(self,other): record("add:"+self.name,other);return self
        def __radd__(self,other): record("radd:"+self.name,other);return self
        def strip(self): record("strip:"+self.name);return self
        def rstrip(self,suffix):
            record("rstrip:"+self.name,suffix)
            if request.get("mutate_paths"):
                wb.os.path.realpath = lambda value:(record("new.realpath",value),"changed")[1]
                wb.os.path.dirname = lambda value:(record("new.dirname",value),"changed")[1]
            return request.get("stripped","/fixture/projects")
    class Environment(dict):
        def get(self,*args): record("get",args);return super().get(*args)
    environment = Environment(request.get("env",{}))
    for key in request.get("objects",[]): environment[key] = Token(key)
    explicit_env = Environment(request.get("explicit_env",{})) if "explicit_env" in request else environment
    arguments = dict(request.get("arguments",{}))
    for key in request.get("argument_objects",[]): arguments[key] = Token(key)
    method = request.get("method","webbench_task_timeout")
    if request.get("explicit"): arguments["env"] = explicit_env
    def realpath(value): record("realpath",value);return os.path.realpath(value)
    def dirname(value): record("dirname",value);return os.path.dirname(value)
    def which(name): record("which",name);return shutil.which(name,path=request["path"]) if request.get("real_paths") else Token("which") if request.get("opaque_which") else request.get("which")
    class Uri:
        def as_uri(self): record("as_uri");return Token("uri") if request.get("opaque_uri") else request.get("uri","file:///fixture/preload.mjs")
    def path(value): record("Path",value);return Path(value) if request.get("real_uri") else Uri()
    def abort(env): record("abort",env);return Token("abort") if request.get("opaque_abort") else request["abort"]
    class Completed:
        @property
        def returncode(self): record("returncode");return Token("returncode") if request.get("opaque_returncode") else request.get("returncode",0)
    def run(argv,**kwargs):
        record("run",[argv,list(kwargs),kwargs])
        if request.get("real_process"):
            return subprocess.run([request["binary"],*argv[1:]],**kwargs,env={"FIXTURE_EXIT":str(request.get("fixture_exit",0))})
        return Completed()
    def logger(message): record("log",message);return Token("log_result")
    try:
        wb.os = types.SimpleNamespace(environ=environment,path=types.SimpleNamespace(realpath=realpath,dirname=dirname))
        wb.shutil = types.SimpleNamespace(which=which)
        wb.subprocess = types.SimpleNamespace(run=run,TimeoutExpired=subprocess.TimeoutExpired)
        wb.Path,wb._log = path,logger
        for key,value in request.get("constants",{}).items(): setattr(wb,key,value)
        if "abort" in request or request.get("opaque_abort"): wb.webbench_undici_abort = abort
        if method == "_probe_namespace_guard": value = wb.CodexWebBenchRunner._probe_namespace_guard(object.__new__(wb.CodexWebBenchRunner))
        else: value = getattr(wb,method)(**arguments)
        if method == "node_bin" and "TE_NODE_BIN" in request.get("objects",[]) and request.get("truthy",True): assert value is environment["TE_NODE_BIN"],"node override lost identity"
        result = {"value":encode(value)}
    except BaseException as error: result = {"raised":type(error).__name__,"message":str(error)}
    finally:
        for name,value in saved.items(): setattr(wb,name,value)
        sys.set_int_max_str_digits(integer_limit)
    result["calls"] = calls
    print(json.dumps(result,sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
