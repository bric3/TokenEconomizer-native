"""Constructor, object and filesystem bindings for scaffold integration comparisons."""
import builtins
import inspect
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import types

ROOT=Path(__file__).resolve().parents[1]
sys.path[:0]=[str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
from te_harness import swebench
if "--native" in sys.argv:
    from native_experiment_providers import install
    install()
from te_harness import webbench as wb
if "--native" in sys.argv:
    install(wb)
    previous=wb.WebBenchRunner._scaffold_context,wb.WebBenchRunner._apply_scaffold,wb.CodexWebBenchRunner.scaffold_profile_for
    install(wb)
    assert previous == (wb.WebBenchRunner._scaffold_context,wb.WebBenchRunner._apply_scaffold,wb.CodexWebBenchRunner.scaffold_profile_for)

for line in sys.stdin:
    request=json.loads(line)
    calls,counts,profiles,contexts,placed=[],{},[],[],[]
    absent=object();saved={name:getattr(wb,name,absent) for name in ("os","subprocess","shutil","scaffolds_pkg","_log")}
    class Token:
        def __init__(self,name): self.name=name
        def __bool__(self): record("bool:"+self.name);return request.get("booleans",{}).get(self.name,True)
        def __str__(self): record("str:"+self.name);return "opaque "+self.name
        def __format__(self,spec): record("format:"+self.name,spec);return "formatted "+self.name
        def __fspath__(self): record("fspath:"+self.name);return request.get("path","/run/path")
        def __iter__(self): record("iter:"+self.name);return iter(request.get("iterables",{}).get(self.name,["custom","*.tmp","?"]))
        def __hash__(self): return 123
        def __contains__(self,value): record("contains:"+self.name,value);return request.get("contains",False)
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,dict): return {key:encode(item) for key,item in dict.items(value)}
        if isinstance(value,(tuple,list)): return [encode(item) for item in value]
        if isinstance(value,(set,frozenset)): return sorted([encode(item) for item in value],key=lambda item:json.dumps(item,sort_keys=True))
        if isinstance(value,(Profile,Context)): return {"object":type(value).__name__,"fields":encode(vars(value))}
        if isinstance(value,subprocess.CompletedProcess): return {"args":encode(value.args),"code":value.returncode,"stdout":value.stdout,"stderr":value.stderr}
        if callable(value): return {"callable":value.__name__,"qualname":value.__qualname__,"module":value.__module__,"signature":str(inspect.signature(value))}
        return value
    class DetailError(Exception):
        def __str__(self):
            record("error.str")
            values=request.get("error_strings",["scaffold failure é😀"])
            return values[min(counts["error.str"]-1,len(values)-1)]
    class ObservedOSError(OSError):
        def __str__(self): record("error.str");return "file failure é😀"
    def record(name,args=None):
        calls.append([name,encode(args),type(sys.exception()).__name__ if sys.exception() is not None else None]);counts[name]=counts.get(name,0)+1
        fault=next((item for item in [request,*request.get("failures",[])] if item.get("fail") == name and counts[name] == item.get("fail_at",1)),None)
        if fault:
            kind=fault.get("error","RuntimeError")
            if kind == "DetailError": raise DetailError()
            if kind == "ObservedOSError": raise ObservedOSError()
            if kind.startswith("Shadow"): raise type(kind.removeprefix("Shadow"),(Exception,),{})("fixture failure")
            raise getattr(builtins,kind)("fixture failure")
    class Namespace:
        def __init__(self,tag,**values): self.__dict__.update(values);self.tag=tag
        def __getattribute__(self,name):
            if name not in ("tag","__dict__","__class__"): record(object.__getattribute__(self,"tag")+"."+name)
            return object.__getattribute__(self,name)
    def callback(name,value=None,implementation=None):
        def call(*args,**kwargs):
            record(name,[args,list(kwargs),kwargs])
            return implementation(*args,**kwargs) if implementation else value
        return call
    class Profile:
        def __init__(self,**kwargs): self.__dict__.update(kwargs)
        def __getattribute__(self,name):
            if name == "patch_excludes": record("profile.patch_excludes")
            return object.__getattribute__(self,name)
    def profile(**kwargs):
        assert isinstance(kwargs["nav_tools"],tuple) and isinstance(kwargs["patch_excludes"],tuple),"profile tuple argument changed"
        record("profile",[list(kwargs),kwargs]);value=Profile(**kwargs);profiles.append(value);return value
    class Context:
        def __init__(self,**kwargs): self.__dict__.update(kwargs)
    def context(**kwargs):
        record("context",[list(kwargs),kwargs]);value=Context(**kwargs);contexts.append(value);return value
    class Scaffold:
        def __getattribute__(self,name):
            if name in ("name","setup","patch_excludes"): record("scaffold."+name)
            return object.__getattribute__(self,name)
        def __bool__(self): record("scaffold.bool");return False
        name="fixture"
        patch_excludes=request.get("excludes",["notes.md","*.txt","?","notes.md"])
        def setup(self,value): record("setup",value);return notes
    class Task(dict):
        def __getitem__(self,key): record("task.item",key);return dict.__getitem__(self,key)
    def path_function(name):
        def call(*args):
            record(name,args)
            if name == "lexists" and not request.get("real_files"): return request.get("lexists",False)
            return getattr(os.path,name)(*args)
        return call
    def filesystem(name):
        def call(*args,**kwargs):
            record(name,[args,list(kwargs),kwargs]);placed.append([name,args])
            if request.get("real_files"): return getattr(shutil if name == "copy2" else os,name)(*args,**kwargs)
        return call
    def run(*args,**kwargs):
        record("run",[args,list(kwargs),kwargs])
        return subprocess.run(*args,**kwargs) if request.get("real_process") else completed
    def complete(*args): record("complete",args);return subprocess.CompletedProcess(*args)
    def log(value):
        record("log",value)
        if request.get("rename_error") and sys.exception() is not None: type(sys.exception()).__name__="RenamedError"
    agent=request.get("agent","claude");operation=request.get("operation","profile")
    base={"claude":wb.WebBenchRunner,"copilot":wb.CopilotWebBenchRunner,"codex":wb.CodexWebBenchRunner}[agent]
    class Runner(base):
        def __getattribute__(self,name):
            if name in ("scaffold","scaffold_bin_dir","scaffold_exec_env","prepare_agent_home","scaffold_profile_for","_scaffold_context"): record("get:"+name)
            return super().__getattribute__(name)
        scaffold_bin_dir=staticmethod(callback("bin_dir",request.get("bin_dir","/run/bin")))
        scaffold_exec_env=staticmethod(callback("exec_env",request.get("exec_env",{"AGENT_ENV":"yes"})))
        prepare_agent_home=staticmethod(callback("prepare"))
    runner=object.__new__(Runner)
    object.__setattr__(runner,"scaffold",Scaffold() if request.get("scaffold",True) else None)
    task=Task(instance_id="task-1")
    notes=Token("notes");completed=Token("completed")
    inputs={"home":request.get("home","/run/home"),"workspace":request.get("workspace","/run/work"),"run_dir_path":request.get("run_dir","/run"),"cfg":request.get("cfg","/run/mcp.json"),"script":request.get("script","fixture script"),"timeout":request.get("timeout",17),"env":request.get("env",{"EXTRA":42,"HOME":"/override"}),"host_path":request.get("src","/source/tool"),"dest_path":request.get("dest","/dest/tool"),"argv":request.get("argv",["fixture","--version"])}
    for name in request.get("objects",[]):
        if name in inputs: inputs[name]=Token(name)
        elif name == "patch_excludes": object.__setattr__(object.__getattribute__(runner,"scaffold"),name,Token(name))
        elif name == "exclude_name": object.__setattr__(object.__getattribute__(runner,"scaffold"),"patch_excludes",[Token("exclude_name")])
    try:
        paths=Namespace("path",**{name:path_function(name) for name in ("join","dirname","basename","lexists")})
        wb.os=Namespace("os",path=paths,pathsep=os.pathsep,environ=request.get("environment",{"HOME":"/operator","PATH":"/usr/bin","KEEP":"yes"}),**{name:filesystem(name) for name in ("makedirs","unlink","link","chmod")})
        wb.shutil=Namespace("shutil",copy2=filesystem("copy2"))
        wb.subprocess=Namespace("subprocess",run=run,CompletedProcess=complete)
        wb.scaffolds_pkg=Namespace("scaffolds_pkg",AgentProfile=profile,ScaffoldContext=context)
        wb._log=log
        if operation == "profile":
            value=runner.scaffold_profile_for(inputs["home"])
            results=[value.mcp_args(cfg) for cfg in (inputs["cfg"],None,inputs["cfg"])]
            result={"value":value,"mcp":results}
        elif operation == "excludes": result={"value":runner.scaffold_answer_excludes()}
        elif operation == "apply":
            value=runner._apply_scaffold(inputs["workspace"],inputs["home"],task,inputs["run_dir_path"])
            if request.get("fail") is None and request.get("scaffold",True): assert value is notes,"setup result identity changed"
            result={"value":value}
        else:
            value=runner._scaffold_context(inputs["workspace"],inputs["home"],task,inputs["run_dir_path"])
            assert value.task is task and value.run_dir is inputs["run_dir_path"],"context argument identity changed"
            if request.get("retarget_home"): inputs["home"]="/changed/home"
            if operation == "exec": value=value.exec_(inputs["script"],inputs["timeout"],inputs["env"])
            elif operation == "host": value=value.host_run(inputs["argv"],inputs["timeout"])
            elif operation == "copy": value=value.copy_in(inputs["host_path"],inputs["dest_path"])
            result={"value":value}
    except BaseException as error:
        result={"raised":type(error).__name__,"message":str(error)}
        if request.get("exception_chain"):
            chain,current=[],error.__context__
            while current is not None and len(chain)<5: chain.append(type(current).__name__);current=current.__context__
            result["context"]=chain
    finally:
        for name,value in saved.items():
            if value is absent: vars(wb).pop(name,None)
            else: setattr(wb,name,value)
    result.update(calls=calls,placed=placed)
    print(json.dumps(encode(result),sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
