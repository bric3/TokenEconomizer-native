"""SDK stand-ins and real process I/O for native Codex installation comparisons."""
import builtins
import contextlib
import json
import os
from pathlib import Path
import subprocess
import sys
import types
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
from te_harness import codex as module
if "--native" in sys.argv:
    from native_codex_install import install
    install()

for line in sys.stdin:
    request = json.loads(line)
    mode = request.get("mode","codex_bin")
    kwargs = dict(request.get("kwargs",{}))
    calls,counts = [],{}
    saved = {name:getattr(module,name) for name in ("os","shutil","subprocess","time","tempfile","__file__")}
    def record(name,args=None):
        calls.append([name,args])
        counts[name] = counts.get(name,0)+1
        if request.get("fail") == name and counts[name] <= request.get("fail_count",1):
            error = request.get("error","RuntimeError")
            if error == "TimeoutExpired": raise subprocess.TimeoutExpired("fixture",2)
            if error == "ShadowOSError": raise type("OSError",(Exception,),{})("fixture failure")
            raise getattr(builtins,error)("fixture failure")
    class Environment(dict):
        def get(self,*args): record("env.get",list(args));return super().get(*args)
    class Process:
        @property
        def pid(self): record("pid");return 43210
        def poll(self):
            record("poll")
            values = request.get("polls",[None,0])
            return values[min(counts["poll"]-1,len(values)-1)]
        def wait(self,**kw): record("wait",kw)
        def communicate(self,**kw):
            record("communicate",kw)
            return request.get("stdout",""),request.get("stderr","")
    def spawn(*args,**kw): record("spawn",[args,kw]);return Process()
    def run(*args,**kw):
        record("run",[args,kw])
        return subprocess.CompletedProcess(args,1,request.get("stdout",""),request.get("stderr",""))
    def rmtree(*args,**kw): record("rmtree",[args,kw])
    def which(*args): record("which",args);return request.get("which")
    def clock():
        record("clock")
        return 100.0+counts["clock"]
    def sleep(*args): record("sleep",args)
    def killpg(*args): record("killpg",args)
    def mkdtemp(**kw): record("mkdtemp",kw);return "/fixture/tmp"
    class File:
        def __enter__(self): return self
        def __exit__(self,*args): record("close")
        def read(self): record("read");return request.get("contents","fixture é😀\n")
        def write(self,value): record("write",value)
    def opened(*args,**kw): record("open",[args,kw]);return File()
    def path_operation(name):
        def operation(*args):
            record(name,args)
            if name in ("isfile","isdir","exists"):
                return args[0] in request.get(name,[])
            return getattr(os.path,name)(*args)
        return operation
    def listdir(*args):
        record("listdir",args)
        entries = request.get("directories",{})
        if args[0] not in entries: raise FileNotFoundError("fixture directory missing")
        return entries[args[0]]
    try:
        if request.get("real"):
            # A deterministic temporary path keeps subprocess error messages comparable.
            def real_temp(*,dir):
                temporary = os.path.join(dir,"work")
                os.mkdir(temporary)
                return temporary
            module.tempfile = types.SimpleNamespace(mkdtemp=real_temp)
            opened_context = contextlib.nullcontext()
        else:
            module.__file__ = "/fixture/te_harness/codex.py"
            module.os = types.SimpleNamespace(environ=Environment(request.get("env",{})),listdir=listdir,killpg=killpg,
                path=types.SimpleNamespace(**{name:path_operation(name) for name in ("dirname","join","realpath","abspath","isfile","isdir","exists")}))
            module.shutil = types.SimpleNamespace(which=which,rmtree=rmtree)
            module.subprocess = types.SimpleNamespace(Popen=spawn,run=run,DEVNULL=subprocess.DEVNULL,PIPE=subprocess.PIPE,TimeoutExpired=subprocess.TimeoutExpired)
            module.time = types.SimpleNamespace(time=clock,sleep=sleep)
            module.tempfile = types.SimpleNamespace(mkdtemp=mkdtemp)
            opened_context = patch("builtins.open",opened)
        if mode == "_kill_process_group": kwargs["proc"] = Process()
        with opened_context:
            value = getattr(module,mode)(**kwargs)
        response = {"value":value}
    except BaseException as error:
        response = {"raised":type(error).__name__,"message":str(error)}
    finally:
        for name,value in saved.items(): setattr(module,name,value)
    response["calls"] = calls
    print(json.dumps(response,sort_keys=True,separators=(",",":"),ensure_ascii=False),flush=True)
