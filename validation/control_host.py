"""SDK objects and identity checks for native runner controller comparisons."""
import dataclasses
import contextlib
import io
import inspect
import json
import os
from pathlib import Path
import sys
import types

ROOT = Path(__file__).resolve().parents[1]
if "--sdk" in sys.argv:
    from standalone_harness import sw
else:
    sys.path[:0] = [str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
    from te_harness import swebench as sw
    if "--native" in sys.argv:
        from native_experiment_task import install
        install()
        from native_experiment_nudge import install
        install()
        from native_experiment_spiral import install
        install()


for line in sys.stdin:
    request = json.loads(line)
    calls = []
    saved = sw.NudgeGuard,sw.codex_spiral.CodexResumeGuard,sw.inspect,os.environ.get("TE_VERBOSE"),sys.get_int_max_str_digits()
    def record(name,args=None):
        calls.append([name,args])
        if request.get("fail") == name:
            kind = request.get("error","RuntimeError")
            if kind == "ShadowTypeError": raise type("TypeError",(Exception,),{})("fixture failure")
            raise getattr(__import__("builtins"),kind)("fixture failure")
    class Token:
        def __init__(self,name): self.name = name
        def __bool__(self):
            record("bool:"+self.name)
            return request.get("truth",True)
        def __int__(self): record("int:"+self.name);return request.get("integer",7)
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if dataclasses.is_dataclass(value): return dataclasses.asdict(value)
        if isinstance(value,dict): return {key:encode(item) for key,item in dict.items(value)}
        if isinstance(value,(list,tuple)): return [encode(item) for item in value]
        if isinstance(value,float) and value.is_integer(): return int(value)
        return value
    class Kwargs(dict):
        def get(self,*args): record("get",encode(args));return super().get(*args)
    class Tel(dict):
        def get(self,*args): record("tel.get",encode(args));return super().get(*args)
        def __getitem__(self,key): record("tel.item",key);return super().__getitem__(key)
    class Snapshot(dict):
        def __bool__(self): record("snapshot.bool");return not request.get("falsey_snapshot",False) and len(self)>0
        def get(self,*args): record("snapshot.get",encode(args));return super().get(*args)
    kwargs = Kwargs(request.get("kwargs",{}))
    for key in request.get("objects",[]): kwargs[key] = Token(key)
    policy = request.get("telemetry",{"snapshot":{"tool_turns":3},"nudges":2,"events":[{"kind":"nudge"}],"thread_id_captured":True,"paid_judge_effective_tokens":7,"policy":{"task":"policy"}})
    if isinstance(policy,dict):
        policy = Tel(policy)
        if isinstance(dict.get(policy,"snapshot"),dict): policy["snapshot"] = Snapshot(dict.__getitem__(policy,"snapshot"))
    prompt,continuation = Token("prompt"),{"mode":"resume","keep":["identity"]}
    class Guard:
        def __init__(self):
            self.aborted,self.nudges_done,self.events,self.judge_tokens,self.nudge_writer_tokens = False,2,[{"kind":"event"}],3,4
            if not request.get("missing_suppressed"): self.abort_suppressed = True
            self.aborted_reads = 0
        def __getattribute__(self,key):
            if key in ("aborted","nudges_done","events","judge_tokens","nudge_writer_tokens","abort_suppressed"):
                record("attr:"+key)
                if key == "aborted" and request.get("changing_aborted"):
                    self.aborted_reads += 1
                    return self.aborted_reads > 1
            return object.__getattribute__(self,key)
        def telemetry(self): record("telemetry");return policy
        def continuation(self,*args):
            record("continuation",encode(args))
            assert args == (() if request.get("agent") == "codex" else (prompt,)),"prompt identity or argument count changed"
            return continuation
    made = []
    class Constructor:
        def __init__(self,**parameters):
            record("constructor",[list(parameters),encode(parameters)])
            for name,key in (("envelope","abort_envelope"),("judge","spiral_judge"),("thr","spiral_thr"),("nudge_writer","spiral_nudge_writer"),("policy","codex_spiral_policy")):
                if name in parameters: assert parameters[name] is dict.get(kwargs,key),"constructor copied "+name
            if "nudge_budget" in parameters: assert type(parameters["nudge_budget"]) is int,"budget was not converted to int"
            self.parameters = parameters
            made.append(self)
    parameters = [inspect.Parameter("self",inspect.Parameter.POSITIONAL_OR_KEYWORD)]
    for name in ("envelope","judge","thr","nudge_budget","verbose","nudge_writer","resume_header"):
        parameters.append(inspect.Parameter(name,inspect.Parameter.KEYWORD_ONLY,default=None))
    if request.get("supports_never_abort"): parameters.append(inspect.Parameter("never_abort",inspect.Parameter.KEYWORD_ONLY,default=None))
    Constructor.__init__.__signature__ = inspect.Signature(parameters)
    original_signature = inspect.signature
    def signature(value): record("signature");return original_signature(value)
    try:
        sw.NudgeGuard,sw.codex_spiral.CodexResumeGuard,sw.inspect = Constructor,Constructor,types.SimpleNamespace(signature=signature)
        if "verbose" in request: os.environ["TE_VERBOSE"] = request["verbose"]
        else: os.environ.pop("TE_VERBOSE",None)
        if "int_max_digits" in request: sys.set_int_max_str_digits(request["int_max_digits"])
        agent = request.get("agent","claude")
        runner = object.__new__({"claude":sw.ClaudeSwebenchRunner,"copilot":sw.CopilotSwebenchRunner,"codex":sw.CodexSwebenchRunner}[agent])
        mode = request.get("mode","build")
        if mode == "real":
            sw.NudgeGuard,sw.codex_spiral.CodexResumeGuard,sw.inspect = saved[:3]
            if agent == "codex" and "codex_spiral_policy" in kwargs:
                kwargs["codex_spiral_policy"] = sw.codex_spiral.CodexSpiralPolicy(**dict.__getitem__(kwargs,"codex_spiral_policy"))
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                guard = runner._build_guard(kwargs)
                if guard is None: value = None
                else:
                    if agent == "codex":
                        guard.state.thread_id = "thread-fixture"
                        guard.pending_reason = request.get("reason","post_green_finalize")
                    else: guard.pending_action = request.get("action","nudge")
                    continuation_value = runner._guard_continuation(guard,"Fix é😀")
                    value = {"continuation":continuation_value,"row":runner._guard_row(guard)}
            value = {"result":value,"output":output.getvalue()}
        elif mode == "accepts": value = sw.guard_accepts(request.get("param","never_abort"))
        elif mode == "build":
            value = runner._build_guard(kwargs)
            if value is not None:
                assert value is made[-1],"constructor result identity changed"
                value = {"guard":encode(value.parameters)}
        elif mode == "row":
            guard = Guard()
            value = runner._guard_row(guard)
            if agent == "codex":
                assert value["codex_spiral_events"] is dict.__getitem__(policy,"events"),"telemetry events were copied"
                assert value["codex_spiral_policy"] is dict.__getitem__(policy,"policy"),"telemetry policy was copied"
            else: assert value["spiral_events"] is object.__getattribute__(guard,"events"),"guard events were copied"
        else:
            value = runner._guard_continuation(Guard(),prompt)
            assert value is continuation,"continuation result was copied"
        response = {"value":encode(value)}
    except BaseException as error: response = {"raised":type(error).__name__,"message":str(error)}
    finally:
        sw.NudgeGuard,sw.codex_spiral.CodexResumeGuard,sw.inspect = saved[:3]
        if saved[3] is None: os.environ.pop("TE_VERBOSE",None)
        else: os.environ["TE_VERBOSE"] = saved[3]
        sys.set_int_max_str_digits(0)
    response["calls"] = calls
    try: print(json.dumps(response,sort_keys=True,separators=(",",":"),ensure_ascii=False),flush=True)
    finally: sys.set_int_max_str_digits(saved[4])
