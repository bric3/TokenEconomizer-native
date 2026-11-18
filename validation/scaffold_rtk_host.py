"""Object, environment and process-result fixtures for the RTK scaffold."""
import builtins
import json
import os
from pathlib import Path
import shlex
import sys

ROOT=Path(__file__).resolve().parents[1]
sys.path[:0]=[str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
from te_bench.baselines import rtk
if "--native" in sys.argv:
    from native_experiment_providers import install
    install(scaffold_module=rtk)
    previous=rtk.cache_dir,rtk.RtkScaffold.setup,rtk.RtkScaffold._claude_hook.__func__
    install(scaffold_module=rtk)
    assert previous == (rtk.cache_dir,rtk.RtkScaffold.setup,rtk.RtkScaffold._claude_hook.__func__)
assert "te_harness.swebench" not in sys.modules and "te_harness.webbench" not in sys.modules

for line in sys.stdin:
    request=json.loads(line);calls,counts=[],{}
    saved={name:getattr(rtk,name) for name in ("os","json","shlex","RTK_URL","RTK_INSTALL_SH","RTK_VERSION")}
    class Token:
        def __init__(self,name): self.name=name
        def __bool__(self):
            record("bool:"+self.name);value=request.get("booleans",{}).get(self.name,True)
            return value[min(counts["bool:"+self.name]-1,len(value)-1)] if isinstance(value,list) else value
        def __format__(self,spec): record("format:"+self.name,spec);return "opaque "+self.name+str(counts["format:"+self.name])+" é😀"
        def __repr__(self): record("repr:"+self.name);return "opaque "+self.name
        def __eq__(self,value): record("eq:"+self.name,value);return Token("comparison") if request.get("opaque_comparison") else request.get("equals",True)
        def __gt__(self,value): record("gt:"+self.name,value);return Token("positive")
        def __contains__(self,value): record("contains:"+self.name,value);return request.get("contains",True)
        def __iter__(self): record("iter:"+self.name);return iter(request.get("iterables",{}).get(self.name,["RTK_HOOK_AT=/first","other","RTK_HOOK_AT=/second"]))
        def __fspath__(self): record("fspath:"+self.name);return "/cache/opaque"
        def __getitem__(self,key): record("slice:"+self.name,[key.start,key.stop,key.step]);return Token("sliced")
        def __add__(self,other): record("add:"+self.name,other);return Token("added")
        def __radd__(self,other): record("radd:"+self.name,other);return Token("added")
        def strip(self): record("strip:"+self.name);return Token("stripped") if request.get("opaque_stripped") else request.get("stripped","2")
        def splitlines(self): record("splitlines:"+self.name);return Token("lines") if request.get("opaque_lines") else request.get("lines",["RTK_HOOK_AT=/fixture"])
    class DetailError(Exception):
        def __str__(self): record("error.str");return "RTK fixture failure é😀"
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,Profile): return "profile"
        if isinstance(value,Context): return "context"
        if isinstance(value,dict): return {key:encode(item) for key,item in dict.items(value)}
        if isinstance(value,(list,tuple)): return [encode(item) for item in value]
        if isinstance(value,bytes): return {"bytes":list(value)}
        return value
    def record(name,args=None):
        calls.append([name,encode(args),type(sys.exception()).__name__ if sys.exception() is not None else None]);counts[name]=counts.get(name,0)+1
        if request.get("fail") == name and counts[name] == request.get("fail_at",1):
            kind=request.get("error","RuntimeError")
            if kind == "DetailError": raise DetailError()
            raise getattr(builtins,kind)("fixture failure")
    def option(name,default): return Token(name) if name in request.get("objects",[]) else request.get(name,default)
    class Namespace:
        def __init__(self,tag,**values): self.__dict__.update(values);self.tag=tag
        def __getattribute__(self,name):
            if name not in ("tag","__dict__","__class__"): record(object.__getattribute__(self,"tag")+"."+name)
            return object.__getattribute__(self,name)
    def callback(name,value=None,implementation=None):
        def call(*args,**kwargs): record(name,[args,list(kwargs),kwargs]);return implementation(*args,**kwargs) if implementation else value
        return call
    class Environment(dict):
        def get(self,*args): record("env.get",args);return dict.get(self,*args)
    class Profile:
        def __getattribute__(self,name):
            if name in ("kind","home","bin_dir","env"): record("profile.get:"+name)
            return object.__getattribute__(self,name)
    class Result:
        def __init__(self,tag): self.tag=tag
        @property
        def returncode(self): record(self.tag+".returncode");return option(self.tag+"_rc",request.get("rc",0))
        @property
        def stdout(self): record(self.tag+".stdout");return option(self.tag+"_stdout",request.get("stdout","RTK_HOOK_AT=/fixture\nrtk ls\n2"))
        @property
        def stderr(self): record(self.tag+".stderr");return option(self.tag+"_stderr",request.get("stderr","fixture stderr é😀"))
    class Context:
        def __getattribute__(self,name):
            if name in ("profile","host_run","exec_","copy_in","merge_json_file","log"): record("context.get:"+name)
            return object.__getattribute__(self,name)
        def host_run(self,*args,**kwargs): record("host",[args,list(kwargs),kwargs]);return Result("host")
        def exec_(self,*args,**kwargs):
            record("exec",[args,list(kwargs),kwargs]);return Result("exec"+str(counts["exec"]))
        def copy_in(self,*args): record("copy",args);return Token("ignored")
        def merge_json_file(self,*args): record("merge",args);return option("registered",True)
        def log(self,*args): record("log",args)
    class Scaffold(rtk.RtkScaffold):
        def __getattribute__(self,name):
            if name in ("bin_path","stage_host_binary","_codex_instructions","_install_copilot_hook","_probe_hook_rewrites","_hook_binary","_claude_hook","_copilot_hook_config","RTK_RULE","COPILOT_HOOK_FILE","_PROBE_PAYLOAD"):
                record("scaffold.get:"+name)
                if name in request.get("stubs",[]):
                    def call(*args): record("stub:"+name,args);return option("stub_"+name,True)
                    return call
            return super().__getattribute__(name)
    ctx=Context();profile=Profile();ctx.profile=profile
    profile.home=option("home","/home/task");profile.bin_dir=option("bin_dir","/home/bin");profile.kind=option("kind","claude");profile.env=request.get("profile_env",{"CODEX_HOME":"/home/codex"})
    environment=Environment(request.get("env",{"TE_SCAFFOLD_CACHE":"/cache"}))
    for name in request.get("env_objects",[]): environment[name]=Token(name)
    if request.get("class_hook"):
        def hook(profile): record("class.hook",profile);return Token("hook_binary")
        Scaffold._hook_binary=staticmethod(hook)
    try:
        rtk.os=Namespace("os",environ=environment,path=Namespace("path",join=callback("join",implementation=os.path.join),dirname=callback("dirname",implementation=os.path.dirname),exists=callback("exists",option("exists",True))),access=callback("access",option("access",True)),X_OK=os.X_OK,makedirs=callback("makedirs"))
        rtk.json=Namespace("json",dumps=callback("dumps",implementation=json.dumps));rtk.shlex=Namespace("shlex",quote=callback("quote",implementation=shlex.quote))
        for name in ("RTK_URL","RTK_INSTALL_SH","RTK_VERSION"):
            if name in request.get("objects",[]): setattr(rtk,name,Token(name))
        scaffold=Scaffold();operation=request.get("operation","setup")
        if operation == "cache": value=rtk.cache_dir()
        elif operation in ("_hook_binary","_claude_hook","_copilot_hook_config","preflight"): value=getattr(scaffold,operation)(profile)
        elif operation == "bin_path": value=scaffold.bin_path()
        elif operation == "_probe_hook_rewrites": value=scaffold._probe_hook_rewrites(ctx,option("agent","claude"))
        else: value=getattr(scaffold,operation)(ctx)
        output={"value":value}
    except BaseException as error:
        output={"raised":type(error).__name__,"message":str(error)}
        if request.get("exception_chain"):
            chain,current=[],error.__context__
            while current is not None and len(chain)<5: chain.append(type(current).__name__);current=current.__context__
            output["context"]=chain
    finally:
        for name,value in saved.items(): setattr(rtk,name,value)
    output.update(calls=calls)
    print(json.dumps(encode(output),sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
