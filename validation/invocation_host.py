"""Object, mapping and library bindings for sealed-agent invocation comparisons."""
import builtins
import json
from pathlib import Path
import sys

ROOT=Path(__file__).resolve().parents[1]
sys.path[:0]=[str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
from te_harness import copilot,codex
if "--native" in sys.argv:
    from native_experiment_providers import install
    install()

for line in sys.stdin:
    request=json.loads(line)
    calls,counts,retained=[],{},[]
    agent=request.get("agent","copilot");method=request.get("method","build_argv")
    module=copilot if agent == "copilot" else codex
    absent=object()
    saved={name:getattr(module,name,absent) for name in ("os","json","str","any","C_BIN","C_HOME","_ISOLATION_ARGS","PROTECTED_TOOLS","_FATAL_CONFIG_ERRORS","effort_applies")}
    class Token:
        def __init__(self,name): self.name=name
        def __bool__(self):
            record("bool:"+self.name)
            value=request.get("booleans",{}).get(self.name,True)
            return value[min(counts["bool:"+self.name]-1,len(value)-1)] if isinstance(value,list) else value
        def __str__(self): record("str:"+self.name);return "opaque "+self.name+" é😀"
        def __format__(self,spec): record("format:"+self.name,spec);return "formatted "+self.name
        def __iter__(self): record("iter:"+self.name);return Iterator(self.name,request.get("iterables",{}).get(self.name,["bash","other"]))
        def __len__(self): record("len:"+self.name);return 2
        def __contains__(self,value): record("contains:"+self.name,value);return request.get("contains",False)
        def __radd__(self,value): record("radd:"+self.name,value);return Token("config_arg")
        def __getitem__(self,key):
            record("item:"+self.name,[key.start,key.stop,key.step] if isinstance(key,slice) else key)
            return Token("sliced" if isinstance(key,slice) else "extra_value")
        def keys(self): record("keys:"+self.name);return ["EXTRA"]
        def splitlines(self): record("splitlines:"+self.name);return Iterable("lines",[Token("line")] if request.get("opaque_lines") else request.get("lines",["model is not available"]))
        def lower(self): record("lower:"+self.name);return Token("low") if request.get("opaque_low") else request.get("lowered","model is not available")
        def strip(self): record("strip:"+self.name);return Token("stripped") if request.get("opaque_stripped") else request.get("stripped","model is not available")
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,dict): return {key:encode(item) for key,item in dict.items(value)}
        if isinstance(value,(tuple,list)): return [encode(item) for item in value]
        return value
    class DetailError(Exception):
        def __str__(self): record("error.str",type(sys.exception()).__name__ if sys.exception() else None);return "invocation failure é😀"
    def record(name,args=None):
        calls.append([name,encode(args)]);counts[name]=counts.get(name,0)+1
        fault=next((item for item in [request,*request.get("failures",[])] if item.get("fail") == name and counts[name] == item.get("fail_at",1)),None)
        if fault:
            kind=fault.get("error","RuntimeError")
            if kind == "DetailError": raise DetailError()
            raise getattr(builtins,kind)("fixture failure")
    class Iterator:
        def __init__(self,tag,items): self.tag,self.iterator=tag,iter(items)
        def __iter__(self): record(self.tag+".iterator.iter");return self
        def __next__(self): record(self.tag+".next");return next(self.iterator)
    class Iterable:
        def __init__(self,tag,items): self.tag,self.items=tag,items
        def __iter__(self): record(self.tag+".iter");return Iterator(self.tag,self.items)
    class Environment(dict):
        def get(self,*args): record("env.get",args);return dict.get(self,*args)
    class Namespace:
        def __init__(self,tag,**values): self.__dict__.update(values);self.tag=tag
        def __getattribute__(self,name):
            if name not in ("tag","__dict__","__class__"): record(object.__getattribute__(self,"tag")+"."+name)
            return object.__getattribute__(self,name)
    def effort_applies(value): record("effort_applies",value);return Token("allowed") if request.get("opaque_allowed") else request.get("effort_allowed",True)
    def dumps(value): record("dumps",value);return Token("encoded") if request.get("opaque_dumps") else json.dumps(value)
    def any_value(generator):
        record("any",[generator.__name__,generator.__qualname__])
        if request.get("retain_generators"):
            retained.append(generator)
            return False if len(retained) == 1 or request.get("consume_after") else next(retained[0])
        return Token("aggregate") if request.get("skip_generator") else builtins.any(generator)
    def string(value): record("string",value);return Token("string")
    defaults={"prompt":"fixture prompt é😀","log_dir":"/run/logs","model":"gpt-5.4","effort":"high","exclude_tools":["bash","view","web_fetch","Read","Read"],"mcp_config":"/run/mcp.json"} if agent == "copilot" else {"hook":True}
    if method == "build_resume_argv": defaults={"session_id":"fixture-session","model":"fixture-model","effort":"high","hook":True}
    elif method == "run_env": defaults={"extra":{"EXTRA":"yes"}}
    elif method == "fatal_config_error": defaults={"stderr_text":"noise\n  MODEL IS NOT AVAILABLE é😀  \nmore"}
    arguments=dict(request.get("arguments",defaults))
    for key,points in request.get("points",{}).items(): arguments[key]="".join(map(chr,points))
    for key in request.get("objects",[]):
        if key in arguments: arguments[key]=Token(key)
        elif key == "isolation": setattr(module,"_ISOLATION_ARGS",Token(key))
        elif key == "protected": setattr(module,"PROTECTED_TOOLS",Token(key))
        elif key == "patterns": setattr(module,"_FATAL_CONFIG_ERRORS",Token(key))
        else: setattr(module,key,Token(key))
    try:
        module.os=Namespace("os",environ=Environment(request.get("env",{})))
        module.json=Namespace("json",dumps=dumps)
        if agent == "copilot" and not request.get("actual_effort"): module.effort_applies=effort_applies
        if request.get("aggregate") or request.get("retain_generators"): module.any=any_value
        if request.get("custom_str"): module.str=string
        for name,key in (("_ISOLATION_ARGS","isolation"),("PROTECTED_TOOLS","protected"),("_FATAL_CONFIG_ERRORS","patterns"),("C_BIN","binary"),("C_HOME","home")):
            if key in request: setattr(module,name,request[key])
        if request.get("observed_patterns"): module._FATAL_CONFIG_ERRORS=Iterable("patterns",request.get("patterns",["missing","is not available","unused"]))
        value=getattr(module,method)(**arguments)
        if request.get("consume_after"): record("retained.consume",[list(generator) for generator in retained])
        if method == "build_argv" and agent == "copilot": assert value[2] is arguments["prompt"] and value[11] is arguments["log_dir"],"raw invocation argument changed"
        if method == "build_resume_argv" and isinstance(arguments.get("model"),Token): assert value[value.index("-m")+1] is arguments["model"],"Codex resume model identity changed"
        result={"value":value}
        if request.get("observe_points"): result["points"]=[list(map(ord,item)) if isinstance(item,str) else None for item in value] if isinstance(value,list) else list(map(ord,value)) if isinstance(value,str) else None
    except BaseException as error:
        result={"raised":type(error).__name__,"message":str(error)}
        if request.get("exception_chain"):
            chain,current=[],error.__context__
            while current is not None and len(chain)<5: chain.append(type(current).__name__);current=current.__context__
            result["context"]=chain
    finally:
        for name,value in saved.items():
            if value is absent: vars(module).pop(name,None)
            else: setattr(module,name,value)
    result["calls"]=calls
    print(json.dumps(encode(result),sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
