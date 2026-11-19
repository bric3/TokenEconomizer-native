"""Object, import and process fixtures for built-in SWE-bench host helpers."""
import builtins
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
from types import SimpleNamespace

ROOT=Path(__file__).resolve().parents[1]
if "--sdk" in sys.argv:
    from standalone_harness import sw
else:
    sys.path[:0]=[str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
    from te_harness import swebench as sw
    if "--native" in sys.argv:
        from native_experiment_providers import install
        install()

for line in sys.stdin:
    request=json.loads(line)
    calls=[]
    counts={}
    saved={name:getattr(sw,name) for name in ("os","shutil","_run","sys","swebench_repo")}
    original_import=builtins.__import__
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,dict): return {key:encode(item) for key,item in value.items()}
        if isinstance(value,(list,tuple)): return [encode(item) for item in value]
        return value
    class DetailError(Exception):
        def __str__(self): calls.append(["error.str",None]);return "fixture host failure"
    def record(name,args=None):
        calls.append([name,encode(args)])
        counts[name]=counts.get(name,0)+1
        if request.get("fail") == name and counts[name] == request.get("fail_at",1):
            kind=request.get("error","RuntimeError")
            if kind == "DetailError": raise DetailError()
            raise getattr(builtins,kind)("fixture failure")
    class Token:
        def __init__(self,name): self.name=name
        def __bool__(self): record("bool:"+self.name);return request.get("truthy",True)
        def strip(self): record("strip");return self
    def option(name,default=None): return Token(name) if name in request.get("objects",[]) else request.get(name,default)
    class Environment(dict):
        def get(self,*args): record("env.get",args);return option("binary")
    class Process:
        @property
        def stdout(self): record("stdout");return option("stdout","  fixture 1.2.3\n")
    class Paths:
        def __contains__(self,item): record("contains",item);return request.get("present",False)
        def insert(self,index,item): record("insert",[index,item])
    def which(name): record("which",name);return option("which","/fixture/claude")
    def realpath(path): record("realpath",path);return os.path.realpath(path) if request.get("real") else path
    def run(argv,**kwargs):
        record("run",[argv,kwargs])
        return subprocess.run(argv,capture_output=True,text=True,**kwargs) if request.get("real") else Process()
    def repo(): record("repo");return option("repo","/fixture/grader")
    def importer(name,*args,**kwargs):
        if name.startswith("swebench.harness."):
            record("import",name)
            if request.get("real_import"): return original_import(name,*args,**kwargs)
            if name.endswith("grading"): return SimpleNamespace(get_eval_report=Token("report"))
            return SimpleNamespace(make_test_spec=Token("spec"))
        return original_import(name,*args,**kwargs)
    response={}
    try:
        sw.os=SimpleNamespace(environ=Environment(),path=SimpleNamespace(realpath=realpath))
        sw.shutil=SimpleNamespace(which=which)
        sw._run,sw.sys,sw.swebench_repo=run,sys if request.get("real_import") else SimpleNamespace(path=Paths()),repo
        builtins.__import__=importer
        method=request.get("method","claude_bin")
        if method == "profiles":
            value={}
            for agent,cls in sw.RUNNERS.items():
                profile=cls.scaffold_profile
                value[agent]={key:encode(getattr(profile,key)) for key in ("kind","home","instructions_file","nav_tools","patch_excludes","bin_dir","env")}
                value[agent]["mcp"]=profile.mcp_args("/fixture/mcp.json")
                value[agent]["probe"]=cls.NET_GUARD_PROBE
        else:
            value=getattr(sw,method)(option("argument","fixture")) if method == "cli_version" else getattr(sw,method)()
            if request.get("real_import"): value=[function() for function in value]
        response["value"]=encode(value)
    except BaseException as error: response.update(raised=type(error).__name__,message=str(error))
    finally:
        builtins.__import__=original_import
        for name,value in saved.items(): setattr(sw,name,value)
    print(json.dumps(dict(response,calls=calls),sort_keys=True,ensure_ascii=True),flush=True)
