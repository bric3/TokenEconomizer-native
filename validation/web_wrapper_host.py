"""Superclass, file and filesystem bindings for Web-Bench agent wrappers."""
import builtins
import io
import json
import os
from pathlib import Path
import shutil
import sys
import types

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
from te_harness import swebench
if "--native" in sys.argv:
    from native_experiment_providers import install
    install()
from te_harness import webbench as wb
if "--native" in sys.argv:
    install(wb)
    previous = wb.CodexWebBenchRunner.run_task,wb.CopilotWebBenchRunner.run_task,wb.CodexWebBenchRunner._prune_agent_home
    install(wb)
    assert previous == (wb.CodexWebBenchRunner.run_task,wb.CopilotWebBenchRunner.run_task,wb.CodexWebBenchRunner._prune_agent_home)

for line in sys.stdin:
    request = json.loads(line)
    operation = request.get("operation","codex")
    calls,counts,written,contexts,removed = [],{},{},[],[]
    absent = object()
    saved = {name:getattr(wb,name,absent) for name in ("os","shutil","open","_log")}
    saved_parent,saved_compliance = wb.WebBenchRunner.run_task,wb.codex.rtk_compliance
    class Token:
        def __init__(self,name): self.name = name
        def __bool__(self): record("bool:"+self.name);return request.get("booleans",{}).get(self.name,True)
        def __iter__(self): record("iter:"+self.name);return iter(request.get("iterables",{}).get(self.name,["skills","goals"]))
        def __int__(self): record("int:"+self.name);return request.get("integer",42)
        def __fspath__(self): record("fspath:"+self.name);return "/out" if self.name == "out_dir" else "/run"
        def __format__(self,spec): record("format:"+self.name,spec);return "opaque "+self.name
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,dict): return {key:encode(item) for key,item in dict.items(value)}
        if isinstance(value,(list,tuple)): return [encode(item) for item in value]
        return value
    class DetailError(Exception):
        def __str__(self): record("error.str");return "fixture wrapper error é😀"
    class ObservedOSError(OSError):
        def __str__(self): record("error.str",type(sys.exception()).__name__ if sys.exception() is not None else None);return "fixture file error é😀"
    def record(name,args=None):
        calls.append([name,encode(args)])
        counts[name] = counts.get(name,0)+1
        fault = next((fault for fault in [request,*request.get("failures",[])] if fault.get("fail") == name and counts[name] == fault.get("fail_at",1)),None)
        if fault is not None:
            kind = fault.get("error","RuntimeError")
            if kind == "DetailError": raise DetailError()
            if kind == "ObservedOSError": raise ObservedOSError()
            if kind.startswith("Shadow"): raise type(kind.removeprefix("Shadow"),(Exception,),{})("fixture failure")
            raise getattr(builtins,kind)("fixture failure")
    class Row(dict):
        def __getattribute__(self,name):
            if name in ("pop","update"): record("row."+name)
            return dict.__getattribute__(self,name)
        def pop(self,*args): record("pop",args);return dict.pop(self,*args)
        def update(self,*args): record("update",args);return dict.update(self,*args)
        def __setitem__(self,key,value): record("row.set",[key,value]);dict.__setitem__(self,key,value)
    parent_row = Row(request.get("row",{"_debug_log":"debug é😀\n","net_guard_installed":True}))
    if request.get("opaque_debug"): dict.__setitem__(parent_row,"_debug_log",Token("debug"))
    out_dir = Token("out_dir") if request.get("opaque_out") else request.get("out_dir","/out")
    task = Token("task") if request.get("opaque_task") else {"instance_id":"task-1"}
    excluded,injected = Token("excluded"),Token("injected")
    run_dir = Token("run_dir") if request.get("opaque_path") else request.get("run_dir","/run")
    kwargs = request.get("kwargs",{"codex_config_capabilities":["skills","goals"],"extra":"value"}).copy()
    if request.get("opaque_caps"): kwargs["codex_config_capabilities"] = Token("caps")
    def parent(self,*args,**options):
        record("parent",[args,list(options),options])
        assert args[0] is task and options["injected"] is injected and options["exclude_tools"] is excluded and options["run_dir_path"] is run_dir
        if "rewrite_caps" in request: object.__setattr__(self,"_capabilities",request["rewrite_caps"])
        return request["parent_result"] if "parent_result" in request else (parent_row,out_dir)
    class ExitValue:
        def __bool__(self):
            record("exit.bool",type(sys.exception()).__name__ if sys.exception() is not None else None)
            return request.get("suppress",False)
    class Manager:
        def __init__(self,path): self.path,self.actual,self.closed = path,None,False
        def __enter__(self):
            record("enter",self.path)
            self.actual = builtins.open(self.path,"w",encoding="utf-8") if request.get("real_files") else io.StringIO()
            written[self.path] = ""
            return self
        def __exit__(self,kind,error,tb):
            record("exit",[kind.__name__ if kind else None,sys.exception() is error,error is None or error.__traceback__ is tb])
            self.closed = True
            if self.actual is not None: self.actual.close()
            return ExitValue() if request.get("opaque_exit") else request.get("suppress",False)
        def write(self,value):
            record("write",value)
            count = self.actual.write(value)
            written[self.path] += value
            return count
    def open_file(path,*args,**options):
        record("open",[path,args,list(options),options])
        value = Manager(path);contexts.append(value)
        return value
    def path_function(name):
        def call(*args):
            record(name,args)
            if name == "isdir" and not request.get("real_files"):
                defaults = {"/run/.codex-home/state.sqlite":False,"/run/.codex-home/notes.txt":False}
                return request.get("isdir",{}).get(args[0],defaults.get(args[0],True))
            return getattr(os.path,name)(*args)
        return call
    class Paths:
        def __getattribute__(self,name):
            if name in ("join","isdir"): record("path."+name)
            return object.__getattribute__(self,name)
    paths = Paths()
    paths.join,paths.isdir = path_function("join"),path_function("isdir")
    def names(path):
        record("listdir",path)
        return os.listdir(path) if request.get("real_files") else request.get("names",["config.toml","instructions.md","AGENTS.md","RTK.md","sessions","state.sqlite","notes.txt"])
    def remove(path):
        record("remove",path);removed.append(path)
        if request.get("real_files"): os.remove(path)
    def rmtree(path):
        record("rmtree",path);removed.append(path)
        if request.get("real_files"): shutil.rmtree(path)
    def compliance(path): record("compliance",path);return request.get("compliance",{"rtk_seen":True})
    def log(text):
        record("log",[text,type(sys.exception()).__name__ if sys.exception() is not None else None])
    class Middle(wb.WebBenchRunner):
        def run_task(self,*args,**options): record("middle");return super().run_task(*args,**options)
    selected = wb.CopilotWebBenchRunner if operation == "copilot" else wb.CodexWebBenchRunner
    tracked = ("_capabilities","_lean_instructions_tokens","answer_key_ns","scaffold_name","_prune_agent_home","agent_home","keep_workspaces","_AGENT_HOME_KEEP")
    class Runner(selected,Middle):
        def __getattribute__(self,name):
            if name in tracked: record("get:"+name)
            return super().__getattribute__(name)
        def __setattr__(self,name,value): record("set:"+name,value);object.__setattr__(self,name,value)
        def agent_home(self,path): record("agent_home",path);return os.path.join(os.fspath(path),".codex-home")
        def _prune_agent_home(self,path):
            record("prune_agent_home",path)
            if request.get("real_prune"): return super()._prune_agent_home(path)
    runner = object.__new__(Runner)
    values = {"_capabilities":(),"_lean_instructions_tokens":0,"answer_key_ns":True,"scaffold_name":None,"keep_workspaces":False,**request.get("fields",{})}
    for key in request.get("objects",[]): values[key] = Token(key)
    for key,value in values.items(): object.__setattr__(runner,key,value)
    for key in request.get("missing",[]): object.__delattr__(runner,key)
    try:
        wb.WebBenchRunner.run_task = parent
        wb.codex.rtk_compliance = compliance
        wb.os,wb.shutil,wb.open,wb._log = types.SimpleNamespace(path=paths,listdir=names,remove=remove),types.SimpleNamespace(rmtree=rmtree),open_file,log
        if operation == "prune": value = wb.CodexWebBenchRunner._prune_agent_home(runner,run_dir)
        else:
            value = runner.run_task(task,arm="baseline",injected=injected,exclude_tools=excluded,run_dir_path=run_dir,**kwargs)
            assert value[0] is parent_row and value[1] is out_dir,"parent result lost its identity"
        result = {"value":encode(value)}
    except BaseException as error:
        result = {"raised":type(error).__name__,"message":str(error)}
        if request.get("exception_chain"):
            chain=[];current=error.__context__
            while current is not None and len(chain)<5: chain.append(type(current).__name__);current=current.__context__
            result["context"] = chain
    finally:
        closed = [manager.closed for manager in contexts]
        for manager in contexts:
            if manager.actual is not None: manager.actual.close()
        wb.WebBenchRunner.run_task,wb.codex.rtk_compliance = saved_parent,saved_compliance
        for key,value in saved.items():
            if value is absent: vars(wb).pop(key,None)
            else: setattr(wb,key,value)
    result.update(calls=calls,written=written,closed=closed,removed=removed,row=encode(parent_row),state={key:encode(vars(runner).get(key)) for key in ("_capabilities",)})
    print(json.dumps(result,sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
