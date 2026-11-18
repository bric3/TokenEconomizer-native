"""HTTP, clock, pricing and observer stand-ins for native optimizer comparisons."""
import builtins
import io
import json
import os
from pathlib import Path
import sys
import types
import urllib.error

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
from te_harness import swebench as sw, codex
if "--native" in sys.argv:
    from native_experiment_providers import install
    install()

for line in sys.stdin:
    request = json.loads(line)
    calls = []
    saved = sw.urllib,codex.urllib,sw.time,codex.time,sw.pricing,codex.pricing,sw._log,sw._log_optimizer_call,codex._responses_text
    integer_limit = sys.get_int_max_str_digits()
    if "int_max_digits" in request: sys.set_int_max_str_digits(request["int_max_digits"])
    data_seen = {}
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,bytes): return {"bytes":list(value)}
        if isinstance(value,dict): return {key:encode(item) for key,item in value.items()}
        if isinstance(value,(tuple,list)): return [encode(item) for item in value]
        return value
    def record(name,args=None):
        calls.append([name,encode(args)])
        if request.get("fail") == name:
            kind = request.get("error","RuntimeError")
            if kind == "ShadowException": raise type("Exception",(BaseException,),{})("fixture failure")
            raise getattr(builtins,kind)("fixture failure")
    class Token:
        def __init__(self,name): self.name = name
        def __bool__(self): record("bool:"+self.name);return True
        def __format__(self,spec): record("format:"+self.name,spec);return "formatted-"+self.name
    agent,mode = request.get("agent","claude"),request.get("mode","call")
    arguments = {"prompt":"fixture é😀", "configured_model":"gpt-5.6-sol" if agent == "codex" else "claude-sonnet-5",**request.get("arguments",{})}
    if agent == "codex": arguments.setdefault("routing",{"base_url":"http://fixture/v1/","api_key":"fixture-key"})
    else: arguments.setdefault("api_key","fixture-key")
    class URL(str):
        def rstrip(self,*args):
            record("url.rstrip",args)
            if request.get("mutate_request"):
                def replacement(*args,**kwargs): record("replaced.request");return saved[0].request.Request(*args,**kwargs)
                sw.urllib.request.Request = replacement
            return super().rstrip(*args)
    if request.get("mutate_request"):
        if agent == "codex": arguments["routing"]["base_url"] = URL(arguments["routing"]["base_url"])
        else: arguments["base_url"] = URL(arguments.get("base_url","http://fixture/"))
    for name in request.get("objects",[]): arguments[name] = Token(name)
    expected_mid = arguments["configured_model"]
    if agent != "codex" and isinstance(arguments.get("model"),str) and arguments["model"].startswith("claude-"): expected_mid = arguments["model"]
    effective = Token("effective") if request.get("opaque_effective") else request.get("effective",1.5)
    default_data = {"output_text":"answer é😀","usage":{"input_tokens":10,"input_tokens_details":{"cached_tokens":3},"output_tokens":2}} if agent == "codex" else {"content":[{"type":"text","text":"answer é😀"}],"usage":{"input_tokens":7,"output_tokens":2}}
    replies = request.get("replies",[{"data":default_data}])
    current = {}
    attempts = 0
    previous_request = None
    def logger(message): record("log",message);return Token("log_result")
    def observed(model,prompt,text,usage,stop_reason=None):
        record("record",[model,prompt,text,usage,stop_reason])
        assert model is expected_mid,"billed model identity changed"
        assert prompt is arguments["prompt"],"prompt identity changed"
        if request.get("mutate_usage") and isinstance(usage,dict): usage.update(request["mutate_usage"])
        if request.get("mutate_raw_usage"): data_seen["data"]["usage"].update(request["mutate_raw_usage"])
        return Token("record_result")
    def pricing(entry): record("pricing",entry);assert entry["model"] is expected_mid,"pricing model identity changed";return effective
    def sleep(seconds): record("sleep",seconds)
    def response_text(data):
        record("response_text",data)
        data_seen["data"] = data
        if request.get("mutate_response"): data.update(request["mutate_response"])
        return saved[8](data)
    class Response:
        def __enter__(self): record("enter");return self
        def __exit__(self,kind,error,tb):
            record("exit",None if kind is None else {"kind":kind.__name__,"message":str(error)})
            if "exit_error" in current: raise getattr(builtins,current["exit_error"])("fixture close failure")
            return current.get("suppress",False)
        def read(self):
            record("read")
            if "bytes" in current: return bytes(current["bytes"])
            return current["body"].encode() if "body" in current else json.dumps(current.get("data",default_data)).encode()
    def urlopen(req,timeout=None):
        global attempts,current,previous_request
        record("urlopen",[req.full_url,req.data.decode(),req.get_method(),req.header_items(),timeout])
        if previous_request is not None: assert previous_request is req,"request was rebuilt during retry"
        previous_request = req
        current = replies[min(attempts,len(replies)-1)]
        attempts += 1
        if request.get("mutate_log"):
            sw._log = lambda message:record("new.log",message)
            sw._log_optimizer_call = lambda *args,**kwargs:record("new.record",[args,kwargs])
        if "http" in current:
            raise urllib.error.HTTPError(req.full_url,current["http"],"fixture",{},io.BytesIO(current.get("error_body","failed é😀").encode()))
        if "error" in current:
            kind = current["error"]
            if kind == "URLError": raise urllib.error.URLError("fixture failure")
            raise getattr(builtins,kind)("fixture failure")
        return Response()
    try:
        sw._log,sw._log_optimizer_call = logger,observed
        codex._responses_text = response_text
        sw.time = codex.time = types.SimpleNamespace(sleep=sleep)
        sw.pricing = codex.pricing = types.SimpleNamespace(effective_tokens=pricing)
        if not request.get("real_http"):
            transport = types.SimpleNamespace(request=types.SimpleNamespace(Request=saved[0].request.Request,urlopen=urlopen),error=urllib.error)
            sw.urllib = codex.urllib = transport
        if mode == "text": value = saved[8](request.get("data"))
        else:
            value = (codex.optimizer_call if agent == "codex" else sw.anthropic_optimizer_call)(**arguments)
            if isinstance(value,tuple) and value[1] is not None and value[0]: assert value[1] is effective,"effective-token result identity changed"
        result = {"value":encode(value)}
    except BaseException as error: result = {"raised":type(error).__name__,"message":str(error)}
    finally:
        sw.urllib,codex.urllib,sw.time,codex.time,sw.pricing,codex.pricing,sw._log,sw._log_optimizer_call,codex._responses_text = saved
        sys.set_int_max_str_digits(integer_limit)
    result["calls"] = calls
    print(json.dumps(result,sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
