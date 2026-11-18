"""Object, filesystem and helper bindings for Web-Bench agent support comparisons."""
import builtins
import io
import json
import os
from pathlib import Path
import shutil
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT/"reference/upstream"), str(ROOT/"reference/upstream/experiments")]
from te_harness import swebench
if "--native" in sys.argv:
    from native_experiment_providers import install
    install()
from te_harness import webbench as wb
if "--native" in sys.argv:
    install(wb)
    previous = wb.WebBenchRunner.scaffold_bin_dir, wb.CopilotWebBenchRunner.scaffold_copilot_home, wb.CodexWebBenchRunner._write_config, wb.CodexWebBenchRunner._row_from_segment
    install(wb)
    assert previous == (wb.WebBenchRunner.scaffold_bin_dir, wb.CopilotWebBenchRunner.scaffold_copilot_home, wb.CodexWebBenchRunner._write_config, wb.CodexWebBenchRunner._row_from_segment)

for line in sys.stdin:
    request = json.loads(line)
    calls, counts, managers, written, copied, links, mutations = [], {}, [], {}, [], [], {}
    absent = object()
    saved = {name: getattr(wb, name, absent) for name in ("os", "shutil", "open", "copilot", "codex", "_merge_usage", "_row_from_result", "_log", "dict")}
    class Token:
        def __init__(self, name): self.name = name
        def __bool__(self):
            record("bool:"+self.name)
            value = request.get("booleans", {}).get(self.name, True)
            return value[min(counts["bool:"+self.name]-1, len(value)-1)] if isinstance(value,list) else value
        def __len__(self): record("len:"+self.name);return request.get("length", 19)
        def __fspath__(self): record("fspath:"+self.name);return request.get("path", "/fixture/path")
        def __format__(self, spec): record("format:"+self.name, spec);return "opaque "+self.name
        def __ne__(self, value): record("ne:"+self.name, value);return Token("comparison") if request.get("opaque_comparison") else request.get("not_equal", True)
        def __add__(self, other): record("add:"+self.name, other);return Token("added")
        def get(self, *args): record("get:"+self.name, args);return Token("value") if request.get("opaque_get") else request.get("get_value", 7)
    def encode(value):
        if isinstance(value, Token): return {"ref": value.name}
        if isinstance(value, dict): return {key: encode(item) for key, item in dict.items(value)}
        if isinstance(value, (list,tuple)): return [encode(item) for item in value]
        return value
    class DetailError(Exception):
        def __str__(self): record("error.str", type(sys.exception()).__name__ if sys.exception() else None);return "fixture support failure é😀"
    class ObservedOSError(OSError):
        def __str__(self): record("error.str", type(sys.exception()).__name__ if sys.exception() else None);return "fixture file failure é😀"
    def record(name, args=None):
        calls.append([name, encode(args)])
        counts[name] = counts.get(name, 0)+1
        fault = next((item for item in [request,*request.get("failures", [])] if item.get("fail") == name and counts[name] == item.get("fail_at", 1)), None)
        if fault:
            kind = fault.get("error", "RuntimeError")
            if kind == "DetailError": raise DetailError()
            if kind == "ObservedOSError": raise ObservedOSError()
            if kind.startswith("Shadow"): raise type(kind.removeprefix("Shadow"), (Exception,), {})("fixture failure")
            raise getattr(builtins, kind)("fixture failure")
    class Namespace:
        def __init__(self, tag, **values): self.__dict__.update(values);self.tag=tag
        def __getattribute__(self, name):
            if name not in ("tag", "__dict__", "__class__"): record(object.__getattribute__(self,"tag")+"."+name)
            return object.__getattribute__(self,name)
    def callback(name, value=None, implementation=None):
        def call(*args, **kwargs):
            record(name, [args, list(kwargs), kwargs])
            return implementation(*args, **kwargs) if implementation else value
        return call
    class Mapping(dict):
        def __getattribute__(self, name):
            if name == "get": record(object.__getattribute__(self,"tag")+".get_function")
            return dict.__getattribute__(self,name)
        def __bool__(self): record(self.tag+".bool");return len(self) != 0
        def get(self,*args): record(self.tag+".get", args);return dict.get(self,*args)
        def __setitem__(self,key,value): record(self.tag+".put", [key,value]);return dict.__setitem__(self,key,value)
    def mapping(tag, value): result=Mapping(value);result.tag=tag;return result
    class Row(Mapping):
        def __getitem__(self,key): record("row.item",key);return dict.__getitem__(self,key)
        def __getattribute__(self,name):
            if name == "setdefault": record("row.setdefault_function")
            return super().__getattribute__(name)
        def setdefault(self,*args): record("row.setdefault",args);return dict.setdefault(self,*args)
    def row_dict(value):
        record("dict",value)
        result=Row(value);result.tag="row";return result
    class ExitValue:
        def __bool__(self): record("exit.bool",type(sys.exception()).__name__ if sys.exception() else None);return request.get("suppress",False)
    class Manager:
        def __init__(self,path): self.path,self.closed,self.actual=path,False,None
        def __enter__(self):
            record("enter",self.path)
            self.actual=builtins.open(self.path,"w",encoding="utf-8") if request.get("real_files") else io.StringIO()
            written[self.path]=""
            return self
        def __exit__(self,kind,error,tb):
            record("exit",[kind.__name__ if kind else None,sys.exception() is error,error is None or error.__traceback__ is tb])
            self.closed=True
            if self.actual is not None: self.actual.close()
            return ExitValue() if request.get("opaque_exit") else request.get("suppress",False)
        def write(self,value):
            record("write",value)
            count=self.actual.write(value);written[self.path]+=value;return count
    def open_file(path,*args,**kwargs): record("open",[path,args,list(kwargs),kwargs]);manager=Manager(path);managers.append(manager);return manager
    def mkdir(*args,**kwargs):
        record("mkdir",[args,list(kwargs),kwargs])
        if request.get("real_files"): os.makedirs(*args,**kwargs)
    def copy2(*args):
        record("copy2",args);copied.append(args)
        if request.get("real_files"): return shutil.copy2(*args)
    def symlink(*args):
        record("symlink",args);links.append(args)
        if request.get("real_files"): return os.symlink(*args)
    def path_function(name):
        def call(*args):
            record(name,args)
            if name in ("isdir","isfile","exists") and not request.get("real_files"): return request.get(name,{}).get(args[0],name == "isfile")
            if name == "expanduser": return request.get("operator_home","/operator")
            return getattr(os.path,name)(*args)
        return call
    agent=request.get("agent","codex")
    base={"claude":wb.WebBenchRunner,"copilot":wb.CopilotWebBenchRunner,"codex":wb.CodexWebBenchRunner}[agent]
    tracked=("base_prompt","_capabilities","routing","model","effort","system_skills","agent_home","scaffold_copilot_home")
    class Runner(base):
        def __getattribute__(self,name):
            if name in tracked: record("get:"+name)
            value=super().__getattribute__(name)
            if name == "_capabilities" and request.get("mutate_capabilities"): object.__setattr__(self,name,["later"])
            return value
        def __setattr__(self,name,value): record("set:"+name,value);mutations[name]=value;object.__setattr__(self,name,value)
    runner=object.__new__(Runner)
    for name,value in {"base_prompt":"fixture base prompt","_capabilities":["skills"],"routing":{"base_url":"https://fixture.invalid"},"model":"fixture-model","effort":"high","system_skills":["fixture-skill"],**request.get("fields",{})}.items(): object.__setattr__(runner,name,value)
    for name in request.get("missing",[]): object.__delattr__(runner,name)
    inputs={"home":request.get("home","/run/home"),"run_dir_path":request.get("run_dir","/run"),"workspace":request.get("workspace","/run/work space"),"result":request.get("result",{"tokens_source":"stream_output_only","output_tokens":4,"usage":{"input_tokens":11}}),"a":request.get("a",{"_debug_log":"first"}),"b":request.get("b",{"_debug_log":"second"}),"lean":request.get("lean","lean instructions é😀\n"),"cfg":request.get("cfg","fixture config é😀\n")}
    for name in ("result","a","b"):
        if isinstance(inputs[name],dict): inputs[name]=mapping(name,inputs[name])
    for name in request.get("objects",[]):
        if name in inputs: inputs[name]=Token(name)
        elif name in ("source","usage","output_tokens","_debug_log"):
            target=inputs["a"] if name == "_debug_log" else inputs["result"]
            dict.__setitem__(target,"tokens_source" if name == "source" else name,Token(name))
        else: object.__setattr__(runner,name,Token(name))
    merged=mapping("merged",request.get("merged",{}));claude_row=mapping("claude_row",request.get("claude_row",{}))
    try:
        paths=Namespace("path",**{name:path_function(name) for name in ("join","dirname","expanduser","isdir","isfile","exists")})
        wb.os=Namespace("os",path=paths,makedirs=mkdir,symlink=symlink)
        wb.shutil=Namespace("shutil",copy2=copy2)
        wb.copilot=Namespace("copilot",copilot_config=callback("copilot_config",request.get("src","/operator/.copilot")),merge_usage=callback("merge",merged))
        wb.codex=Namespace("codex",lean_instructions=callback("lean",inputs["lean"]),config_toml=callback("config_toml",inputs["cfg"]),merge_usage=callback("merge",merged))
        wb._merge_usage=callback("merge",merged);wb._row_from_result=callback("claude_row",claude_row)
        wb.open=open_file;wb._log=callback("log")
        if request.get("opaque_dict"): wb.dict=row_dict
        operation=request.get("operation","config")
        if operation == "config": value=runner._write_config(inputs["home"],inputs["workspace"]);assert value is inputs["cfg"]
        elif operation == "home": value=runner.agent_home(inputs["run_dir_path"])
        elif operation == "bin": value=runner.scaffold_bin_dir(inputs["home"])
        elif operation == "class_bin": value=base.scaffold_bin_dir(inputs["home"])
        elif operation == "env": value=runner.scaffold_exec_env(inputs["run_dir_path"])
        elif operation == "prepare": value=runner.prepare_agent_home(inputs["home"])
        elif operation == "copilot_home": value=runner.scaffold_copilot_home(inputs["run_dir_path"])
        elif operation == "merge": value=runner._merge_segments(inputs["a"],inputs["b"]);assert value is merged
        elif operation == "usage": value=runner._segment_usage(inputs["result"])
        else: value=runner._row_from_segment(inputs["result"])
        result={"value":value}
    except BaseException as error:
        result={"raised":type(error).__name__,"message":str(error)}
        if request.get("exception_chain"):
            chain,current=[],error.__context__
            while current is not None and len(chain)<5: chain.append(type(current).__name__);current=current.__context__
            result["context"]=chain
    finally:
        for manager in managers:
            if manager.actual is not None: manager.actual.close()
        for name,value in saved.items():
            if value is absent: vars(wb).pop(name,None)
            else: setattr(wb,name,value)
    result.update(calls=calls,written=written,closed=[manager.closed for manager in managers],copied=copied,links=links,mutations=mutations,inputs={key:inputs[key] for key in ("result","a","b")})
    print(json.dumps(encode(result),sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
