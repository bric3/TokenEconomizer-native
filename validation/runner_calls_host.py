"""Opaque runner, factory and transport fixtures for native call coordination."""
import builtins
import json
from pathlib import Path
import sys
import types

ROOT = Path(__file__).resolve().parents[1]
if "--sdk" in sys.argv:
    from standalone_harness import sw, wb as web
else:
    sys.path[:0] = [str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
    from te_harness import swebench as sw, webbench as web
if "--native" in sys.argv and "--sdk" not in sys.argv:
    from native_experiment_providers import install
    install()
if "--native" in sys.argv and "--sdk" not in sys.argv:
    install(web)
    previous = web.CodexWebBenchRunner.optimizer_call
    install(web)
    assert web.CodexWebBenchRunner.optimizer_call is previous,"late runner installation is not idempotent"

for line in sys.stdin:
    request = json.loads(line)
    calls,references = [],{}
    owner = web if request.get("webbench") else sw
    saved = {name:getattr(owner,name) for name in ("os","anthropic_optimizer_call","preflight_optimizer_endpoint","copilot","codex","RUNNERS","_install_signal_cleanup")}
    supported = getattr(web,"WEBBENCH_SCAFFOLDS")
    http_errors = sw.urllib.error
    dependency_state = [(module,name,getattr(module,name)) for module,names in ((sw,("os","urllib","pricing","_log","_log_optimizer_call")),(saved["codex"],("os","urllib","pricing"))) for name in names]
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,dict): return {str(key):encode(item) for key,item in dict.items(value)}
        if isinstance(value,(tuple,list)): return [encode(item) for item in value]
        if isinstance(value,bytes): return {"bytes":list(value)}
        return value
    class DetailError(Exception):
        def __str__(self): record("error.str");return "fixture error é😀"
    def record(name,args=None):
        calls.append([name,encode(args)])
        if request.get("fail") == name:
            kind = request.get("error","RuntimeError")
            if kind == "DetailError": raise DetailError()
            if kind == "ShadowException": raise type("Exception",(BaseException,),{})("fixture failure")
            raise getattr(builtins,kind)("fixture failure")
    class Token:
        def __init__(self,name): self.name = name
        def __bool__(self): record("bool:"+self.name);return request.get("truthy",True)
        def __eq__(self,other):
            record("eq:"+self.name,other)
            if request.get("mutate_model"): owner.codex.optimizer_call = owner.copilot.optimizer_call = helper("replaced")
            return Token("comparison") if request.get("opaque_comparison") else request.get("equal",False)
        def __repr__(self):
            record("repr:"+self.name)
            if request.get("mutate_repr"): owner.RUNNERS = Registry({"changed":helper("changed")})
            return "token-"+self.name
        def lower(self): record("lower:"+self.name);return request.get("lowered","copilot")
        def __iter__(self):
            record("iter:"+self.name)
            if request.get("mutate_unpack"): owner.anthropic_optimizer_call = helper("replaced")
            return iter((Token("base"),Token("endpoint_key"),Token("bearer")))
    class Environment(dict):
        def get(self,*args): record("env.get",args);return super().get(*args)
    class Registry(dict):
        def __contains__(self,key): record("registered",key);return super().__contains__(key)
        def __getitem__(self,key): record("lookup",key);return super().__getitem__(key)
        def __iter__(self): record("keys");return super().__iter__()
        def __len__(self): record("key_count");return super().__len__()
    class Name(str):
        def __lt__(self,other): record("compare_key",[str(self),str(other)]);return super().__lt__(other)
    returned = Token("result")
    if request.get("result_tuple"): returned = (Token("text"),Token("cost"))
    elif request.get("null_result"): returned = None
    arguments = dict(request.get("arguments",{}))
    for key in request.get("objects",[]): arguments[key] = Token(key)
    fields = {"optimizer_endpoint":None,"optimizer_model":Token("optimizer_model"),"optimizer_max_tokens":Token("cap"),"api_key":Token("key"),"copilot":Token("binary"),"model":Token("runner_model"),"routing":{"api_key":Token("routing_key"),"base_url":"http://fixture"},**request.get("fields",{})}
    if request.get("real_helpers"):
        fields.update(optimizer_model="claude-sonnet-5",optimizer_max_tokens=64,api_key="fixture-key",model="gpt-5.6-sol",routing={"api_key":"fixture-key","base_url":"http://fixture/v1"})
        fields.update(request.get("fields",{}))
    if request.get("opaque_endpoint"): fields["optimizer_endpoint"] = Token("endpoint")
    for key in request.get("missing",[]): fields.pop(key,None)
    endpoints = iter(request.get("endpoints",[]))
    def helper(name):
        def call(*args,**kwargs):
            record(name,[args,list(kwargs),kwargs])
            if args and "prompt" in arguments: assert args[0] is arguments["prompt"],"prompt identity changed"
            references["returned"] = returned
            return returned
        return call
    def cleanup():
        record("cleanup")
        if request.get("mutate_cleanup"): owner.RUNNERS = Registry({"claude":helper("changed")})
        return Token("cleanup_result")
    cost = 1.5
    def make_request(*args,**kwargs): record("request",[args,list(kwargs),kwargs]);return Token("request")
    class Response:
        def __enter__(self): record("enter");return self
        def __exit__(self,kind,error,tb): record("exit",kind.__name__ if kind else None)
        def read(self): record("read");return b'{"content":[{"type":"text","text":"integrated answer"}],"output_text":"integrated answer","usage":{"input_tokens":7,"output_tokens":2}}'
    def urlopen(*args,**kwargs): record("urlopen",[args,kwargs]);return Response()
    def pricing(entry): record("pricing",entry);return cost
    agent = request.get("agent","claude")
    cls_name = ({"claude":"WebBenchRunner","copilot":"CopilotWebBenchRunner","codex":"CodexWebBenchRunner"} if request.get("webbench") else {"claude":"ClaudeSwebenchRunner","copilot":"CopilotSwebenchRunner","codex":"CodexSwebenchRunner"})[agent]
    class Runner(getattr(owner,cls_name)):
        def __getattribute__(self,name):
            if name in fields or name in request.get("missing",[]):
                record("get:"+name)
                if name == request.get("mutate_field"): owner.anthropic_optimizer_call = owner.codex.optimizer_call = owner.copilot.optimizer_call = helper("replaced")
                if name == "optimizer_endpoint" and "endpoints" in request: return next(endpoints)
            return super().__getattribute__(name)
    runner = object.__new__(Runner)
    for name,value in fields.items(): setattr(runner,name,value)
    mode = request.get("mode","optimizer")
    if mode == "optimizer": arguments.setdefault("prompt",Token("prompt"))
    try:
        owner.os = types.SimpleNamespace(environ=Environment(request.get("env",{})))
        owner.anthropic_optimizer_call,owner.preflight_optimizer_endpoint = helper("anthropic"),helper("preflight")
        owner.copilot = types.SimpleNamespace(optimizer_call=helper("copilot"))
        owner.codex = types.SimpleNamespace(optimizer_call=helper("codex"),preflight_optimizer=helper("codex_preflight"))
        owner._install_signal_cleanup = cleanup
        keys = request.get("keys",["claude","copilot","codex"])
        if request.get("foreign_keys"): keys = [Name(key) for key in keys]
        owner.RUNNERS = Registry({key:helper("construct:"+str(key)) for key in keys})
        if request.get("paired_key"): owner.RUNNERS["\ud800\udc00"] = helper("paired")
        web.WEBBENCH_SCAFFOLDS = request.get("supported",supported)
        if request.get("real_helpers"):
            owner.anthropic_optimizer_call,owner.preflight_optimizer_endpoint = saved["anthropic_optimizer_call"],saved["preflight_optimizer_endpoint"]
            owner.codex,owner.copilot = saved["codex"],saved["copilot"]
            transport = types.SimpleNamespace(request=types.SimpleNamespace(Request=make_request,urlopen=urlopen),error=http_errors)
            sw.urllib = saved["codex"].urllib = transport
            sw.pricing = saved["codex"].pricing = types.SimpleNamespace(effective_tokens=pricing)
            sw.os = saved["codex"].os = owner.os
            sw._log = lambda message:record("log",message)
            sw._log_optimizer_call = lambda *args,**kwargs:record("record",[args,kwargs])
        if mode == "factory": value = owner.make_runner(**arguments)
        else: value = getattr(runner,"optimizer_call" if mode == "optimizer" else "_preflight_optimizer_endpoint")(**arguments)
        if mode == "preflight": assert value is None,"preflight helper result was not discarded"
        elif request.get("real_helpers"): assert value[0] == "integrated answer" and value[1] is cost,"integrated transport result changed"
        else: assert value is references["returned"],"helper return identity changed"
        result = {"value":encode(value)}
    except BaseException as error:
        message = str(error)
        result = {"raised":type(error).__name__,"message":message}
        if request.get("paired_key"): result["points"] = list(map(ord,message))
    finally:
        for name,value in saved.items(): setattr(owner,name,value)
        for module,name,value in dependency_state: setattr(module,name,value)
        web.WEBBENCH_SCAFFOLDS = supported
    result["calls"] = calls
    print(json.dumps(result,sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
