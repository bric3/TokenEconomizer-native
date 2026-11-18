"""Object, constructor, file and codec bindings for Web-Bench run contracts."""
import builtins
import hashlib
import io
import json
import os
from pathlib import Path
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
    previous = wb.WebBenchRunner._write_run_config,wb.WebBenchRunner._make_guard,wb.CodexWebBenchRunner._make_guard,wb.build_prompt,wb.strip_transport_instructions
    install(wb)
    assert previous == (wb.WebBenchRunner._write_run_config,wb.WebBenchRunner._make_guard,wb.CodexWebBenchRunner._make_guard,wb.build_prompt,wb.strip_transport_instructions)

for line in sys.stdin:
    request = json.loads(line)
    operation = request.get("operation","config")
    calls,counts,contexts,written = [],{},[],{}
    absent = object()
    saved = {name:getattr(wb,name,absent) for name in ("os","json","open","hashlib","NudgeGuard","codex_spiral","guard_accepts","network_tool_block","TASK_TEMPLATE","ERROR_TEMPLATE","_INIT_FORMAT_LINES")}
    class Token:
        def __init__(self,name): self.name = name
        def __bool__(self):
            record("bool:"+self.name)
            value = request.get("booleans",{}).get(self.name,True)
            return value[min(counts["bool:"+self.name]-1,len(value)-1)] if isinstance(value,list) else value
        def __iter__(self): record("iter:"+self.name);return iter(request.get("iterables",{}).get(self.name,["Bash","WebSearch"]))
        def __contains__(self,value): record("contains:"+self.name,value);return False
        def __len__(self): record("len:"+self.name);return request.get("length",3)
        def __int__(self): record("int:"+self.name);return request.get("integer",2)
        def __str__(self): record("str:"+self.name);return "opaque "+self.name
        def __format__(self,spec): record("format:"+self.name,spec);return "opaque "+self.name+str(counts["format:"+self.name])
        def __eq__(self,value):
            record("eq:"+self.name,value)
            return Token("comparison") if request.get("opaque_comparison") else request.get("equals",{}).get(self.name,False)
        def __fspath__(self): record("fspath:"+self.name);return request.get("path","/run")
        def encode(self): record("encode:"+self.name);return b"fixture manual"
        def strip(self): record("strip:"+self.name);return Token("stripped") if request.get("opaque_stripped") else "fixture description"
        def rstrip(self): record("rstrip:"+self.name);return request.get("rstrip","manual")
        def splitlines(self): record("splitlines:"+self.name);return [Token("line")] if request.get("opaque_lines") else request.get("lines",[" kept ","Add filename after code block."])
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,Manager): return {"file":value.path}
        if isinstance(value,bytes): return {"bytes":list(value)}
        if isinstance(value,dict): return {key:encode(item) for key,item in dict.items(value)}
        if isinstance(value,(list,tuple)): return [encode(item) for item in value]
        if callable(value): return "str" if value is str else "callable"
        return value
    class DetailError(Exception):
        def __str__(self): record("error.str");return "fixture contract failure é😀"
    def record(name,args=None):
        calls.append([name,encode(args)])
        counts[name] = counts.get(name,0)+1
        if request.get("fail") == name and counts[name] == request.get("fail_at",1):
            kind = request.get("error","RuntimeError")
            if kind == "DetailError": raise DetailError()
            if kind.startswith("Shadow"): raise type(kind.removeprefix("Shadow"),(Exception,),{})("fixture failure")
            raise getattr(builtins,kind)("fixture failure")
    class Mapping(dict):
        def __getattribute__(self,name):
            if name == "get": record(object.__getattribute__(self,"tag")+".get_function")
            return dict.__getattribute__(self,name)
        def get(self,*args): record(self.tag+".get",args);return dict.get(self,*args)
    def mapping(tag,values):
        result=Mapping(values);result.tag=tag;return result
    class Manager:
        def __init__(self,path): self.path,self.closed,self.actual = path,False,None
        def __enter__(self):
            record("enter",self.path)
            self.actual = builtins.open(self.path,"w",encoding="utf-8") if request.get("real_files") else io.StringIO()
            written[self.path] = ""
            return self
        def __exit__(self,kind,error,tb):
            record("exit",[kind.__name__ if kind else None,sys.exception() is error,error is None or error.__traceback__ is tb])
            self.closed = True
            if self.actual is not None: self.actual.close()
            return request.get("suppress",False)
        def write(self,value):
            record("write",value)
            result = self.actual.write(value);written[self.path] += value
            return result
    def open_file(path,*args,**kwargs):
        record("open",[path,args,list(kwargs),kwargs])
        value=Manager(path);contexts.append(value);return value
    def dump(*args,**kwargs): record("json.dump",[args,list(kwargs),kwargs]);json.dump(*args,**kwargs)
    def join(*args): record("join",args);return os.path.join(*args)
    class Paths:
        def __getattribute__(self,name):
            if name == "join": record("path.join")
            return object.__getattribute__(self,name)
    paths=Paths();paths.join=join
    def sha256(value): record("sha256",value);return Hash(hashlib.sha256(value))
    class Hash:
        def __init__(self,actual): self.actual=actual
        def __getattribute__(self,name):
            if name == "hexdigest": record("hash.hexdigest")
            return object.__getattribute__(self,name)
        def hexdigest(self): record("hexdigest");return self.actual.hexdigest()
    def accepts(name): record("guard_accepts",name);return Token("accepts") if request.get("opaque_accepts") else request.get("accepts",True)
    def network(): record("network_tool_block");return request.get("network",["WebFetch","WebSearch","WebSearch"])
    def nudge(**options):
        record("nudge_guard",[list(options),options])
        for key,source in (("envelope","abort_envelope"),("judge","spiral_judge"),("thr","spiral_thr"),("nudge_writer","spiral_nudge_writer")):
            assert options[key] is dict.get(kwargs,source),"guard argument lost its identity"
        return {"guard":"nudge","kwargs":options}
    def codex(**options):
        record("codex_guard",[list(options),options])
        assert options["policy"] is dict.get(kwargs,"codex_spiral_policy"),"policy lost its identity"
        return {"guard":"codex","kwargs":options}
    class Template:
        def __init__(self,name): self.name=name
        def __getattribute__(self,name):
            if name == "format": record("template.format:"+object.__getattribute__(self,"name"))
            return object.__getattribute__(self,name)
        def format(self,**kwargs): record("template.call:"+self.name,[list(kwargs),kwargs]);return request.get(self.name+"_value","custom "+self.name)
    class Scaffold:
        def __getattribute__(self,name):
            if name == "name": record("scaffold.name")
            return object.__getattribute__(self,name)
        name="fixture-scaffold"
    class Session:
        def __getattribute__(self,name):
            if name in ("config_path","log_path","hash"): record("session."+name)
            return object.__getattribute__(self,name)
        config_path,log_path,hash="/session/config.json","/session/evaluator.log","fixture-hash"
    tracked=("agent","model","effort","cli_version","agent_bin","routing_base","optimizer_model","optimizer_transport","protected_tools","scaffold","scaffold_answer_excludes","repo","retry","max_workers","node","task_timeout","verdict_timeout","start_timeout")
    class Runner(wb.WebBenchRunner):
        def __getattribute__(self,name):
            if name in tracked: record("get:"+name)
            value=super().__getattribute__(name)
            if name == "protected_tools" and request.get("mutate_protected"): object.__setattr__(self,name,[])
            return value
        def scaffold_answer_excludes(self): record("scaffold_excludes");return request.get("scaffold_excludes",["RTK.md","AGENTS.md"])
    runner=object.__new__(Runner)
    fields={"agent":"claude","model":"claude-opus-4-8","effort":"high","cli_version":"fixture-cli","agent_bin":"/agent","routing_base":"https://fixture.invalid","optimizer_model":"fixture-model","protected_tools":["Bash"],"scaffold":Scaffold() if request.get("scaffold") else None,"repo":"/repo","retry":2,"max_workers":3,"node":"/node","task_timeout":240,"verdict_timeout":270,"start_timeout":900,**request.get("fields",{})}
    for key,value in fields.items(): object.__setattr__(runner,key,value)
    for key in request.get("missing",[]): object.__delattr__(runner,key)
    task=mapping("task",request.get("task",{"instance_id":"task-1","repo":"@web-bench/project","problem_statement":"driver description"}))
    data=mapping("request",request.get("request",{"task":" evaluator description ","error":" previous failure "}))
    kwargs=mapping("kwargs",request.get("kwargs",{"spiral_guard":True,"spiral_nudge":True,"spiral_never_abort":True}))
    environment=mapping("env",request.get("env",{}))
    inputs={"injected":request.get("injected","manual\n"),"exclude_tools":request.get("excluded",["Bash","WebFetch"]),"project":request.get("project","project"),"workspace":request.get("workspace","."),"description":request.get("description"," first\nAdd filename after code block.\n last "),"run_dir_path":request.get("run_dir","/run")}
    for key in request.get("objects",[]):
        if key in inputs: inputs[key]=Token(key)
        elif key in fields: object.__setattr__(runner,key,Token(key))
        elif key in ("task_text","error_text"): dict.__setitem__(data,"task" if key == "task_text" else "error",Token(key))
        elif key == "verbosity": dict.__setitem__(environment,"TE_VERBOSE",Token(key))
        else: dict.__setitem__(kwargs,key,Token(key))
    for key,object_key in (("injected_points","injected"),("description_points","description")):
        if key in request: inputs[object_key]="".join(map(chr,request[key]))
    if "error_points" in request: dict.__setitem__(data,"error","".join(map(chr,request["error_points"])))
    try:
        wb.os,wb.json,wb.open=types.SimpleNamespace(path=paths,environ=environment),types.SimpleNamespace(dump=dump),open_file
        wb.hashlib=types.SimpleNamespace(sha256=sha256)
        wb.guard_accepts,wb.network_tool_block=accepts,network
        wb.NudgeGuard,wb.codex_spiral=nudge,types.SimpleNamespace(CodexResumeGuard=codex)
        for name in ("TASK_TEMPLATE","ERROR_TEMPLATE","_INIT_FORMAT_LINES"):
            if name in request: setattr(wb,name,request[name])
        for name in request.get("foreign_templates",[]): setattr(wb,name,Template(name))
        if operation == "strip": value=wb.strip_transport_instructions(inputs["description"])
        elif operation == "prompt": value=wb.build_prompt(task,inputs["injected"],data,inputs["project"],workspace=inputs["workspace"])
        elif operation in ("guard","codex_guard"):
            cls=wb.CodexWebBenchRunner if operation == "codex_guard" else wb.WebBenchRunner
            value=cls._make_guard(runner,kwargs)
        else: value=runner._write_run_config(inputs["run_dir_path"],task,"baseline",inputs["injected"],inputs["exclude_tools"],request.get("max_turns",80),kwargs,Session(),inputs["project"])
        result={"value":encode(value)}
        if request.get("observe_points") and isinstance(value,str): result["points"]=list(map(ord,value))
    except BaseException as error:
        result={"raised":type(error).__name__,"message":str(error)}
        if request.get("exception_chain"):
            chain=[];current=error.__context__
            while current is not None and len(chain)<5: chain.append(type(current).__name__);current=current.__context__
            result["context"]=chain
    finally:
        closed=[manager.closed for manager in contexts]
        for manager in contexts:
            if manager.actual is not None: manager.actual.close()
        for key,value in saved.items():
            if value is absent: vars(wb).pop(key,None)
            else: setattr(wb,key,value)
    result.update(calls=calls,written=written,closed=closed)
    print(json.dumps(result,sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
