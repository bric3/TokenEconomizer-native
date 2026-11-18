"""Clock, queue and opaque-event fixtures for Web-Bench evaluator waits."""
import builtins
import json
from pathlib import Path
import queue
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
    previous = wb._EvalSession.next_event
    install(wb)
    assert wb._EvalSession.next_event is previous,"session installation is not idempotent"

for line in sys.stdin:
    request = json.loads(line)
    calls,returned = [],[]
    saved = {name:getattr(wb,name) for name in ("time","_log")}
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,dict): return {key:encode(item) for key,item in dict.items(value)}
        if isinstance(value,(tuple,list)): return [encode(item) for item in value]
        return value
    class DetailError(Exception):
        def __str__(self): calls.append(["error.str",None]);return "fixture failure é😀"
    class SilentEmpty(queue.Empty):
        def __str__(self): calls.append(["empty.str",None]);return "empty fixture"
    def failure(kind):
        if kind == "Empty": raise queue.Empty()
        if kind == "SilentEmpty": raise SilentEmpty()
        if kind == "ShadowEmpty": raise type("Empty",(Exception,),{})("shadow fixture")
        if kind == "DetailError": raise DetailError()
        raise getattr(builtins,kind)("fixture failure")
    def record(name,args=None):
        calls.append([name,encode(args)])
        if request.get("fail") == name: failure(request.get("error","RuntimeError"))
    class Token:
        def __init__(self,name): self.name = name
        def __bool__(self):
            record("bool:"+self.name)
            return request.get("booleans",{}).get(self.name,self.name != "expired")
        def __add__(self,other): record("add:"+self.name,other);return Token("deadline")
        def __radd__(self,other): record("radd:"+self.name,other);return Token("deadline")
        def __sub__(self,other): record("sub:"+self.name,other);return Token("left")
        def __le__(self,other): record("le:"+self.name,other);return Token("expired")
        def __gt__(self,other):
            record("gt:"+self.name,other)
            if request.get("mutate_get"): events.get = requests.get = lambda **kwargs:(record("new.get",kwargs),{"kind":"exit"})[1]
            return Token("capped")
        def __eq__(self,other):
            record("eq:"+self.name,other)
            return request.get("equal",other == "started") if not request.get("opaque_comparison") else Token("equal_exit" if other == "exit" else "equal_kind")
        def __format__(self,spec):
            record("format:"+self.name,spec)
            if request.get("mutate_log"): wb._log = lambda message:record("new.log",message)
            return "formatted-"+self.name
    class Event(dict):
        def __getitem__(self,key):
            record("event.item",key)
            if key == "kind" and kinds: return kinds.pop(0)
            return super().__getitem__(key)
        def get(self,*args): record("event.get",args);return super().get(*args)
    kinds = list(request.get("kind_values",[]))
    def event(value):
        if isinstance(value,dict):
            value = Event(value) if request.get("opaque_event") else dict(value)
            for key in request.get("event_objects",[]): value[key] = Token(key)
        return value
    values = [event(value) for value in request.get("queue",[{"kind":"started","task":"task-1","attempt":1}])]
    class Queue:
        def __init__(self,name): self.name = name;self.items = list(values);self.actual = queue.Queue()
        def __getattribute__(self,name):
            if name == "get": record("queue.get_function")
            return object.__getattribute__(self,name)
        def get(self,**kwargs):
            record("queue.get",[self.name,kwargs])
            if request.get("real_queue"):
                value = self.actual.get(**kwargs)
            else:
                if not self.items: raise queue.Empty()
                value = self.items.pop(0)
                if isinstance(value,dict) and "raise" in value: failure(value["raise"])
            returned.append(value)
            return value
    events,requests = Queue("events"),Queue("requests")
    if request.get("real_queue"):
        for value in values:
            events.actual.put(value)
            requests.actual.put(value)
    times = list(request.get("times",[10,10,11,12,20]))
    def clock(): record("time");return Token("now") if request.get("opaque_clock") else times.pop(0) if times else 20
    def logger(message): record("log",message);return Token("log_result")
    fields = {"project":"project","events":events,"requests":requests,"finished":True,**request.get("fields",{})}
    for key in request.get("field_objects",[]): fields[key] = Token(key)
    for key in request.get("missing",[]): fields.pop(key,None)
    class Session(wb._EvalSession):
        def __getattribute__(self,name):
            if name in fields or name in request.get("missing",[]): record("read:"+name)
            return super().__getattribute__(name)
    session = object.__new__(Session)
    for key,value in fields.items(): setattr(session,key,value)
    arguments = {"timeout":10,**request.get("arguments",{})}
    method = request.get("method","next_event")
    if method == "next_event": arguments.setdefault("kind","started")
    for key in request.get("omit",[]): arguments.pop(key,None)
    for key in request.get("objects",[]): arguments[key] = Token(key)
    try:
        wb.time,wb._log = types.SimpleNamespace(time=clock),logger
        value = getattr(session,method)(**arguments)
        if value is not None and value != (None,None): assert any(value is candidate for candidate in returned),"queue result identity changed"
        result = {"value":encode(value)}
    except BaseException as error:
        result = {"raised":type(error).__name__,"message":str(error)}
        if request.get("binding_error"): result["context"] = type(error.__context__).__name__ if error.__context__ is not None else None
    finally:
        for key,value in saved.items(): setattr(wb,key,value)
    result["calls"] = calls
    print(json.dumps(result,sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
