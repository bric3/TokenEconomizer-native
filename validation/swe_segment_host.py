"""Object, process and clock bindings for SWE-bench streamed segments."""
import builtins
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT=Path(__file__).resolve().parents[1]
if "--sdk" in sys.argv:
    from standalone_harness import sw as swe, bridge
    from te_runtime.swebench_runner import bind
    previous=swe.ClaudeSwebenchRunner._segment
    bind(bridge,swe)
    assert previous is swe.ClaudeSwebenchRunner._segment
else:
    sys.path[:0]=[str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
    from te_harness import swebench as swe
if "--native" in sys.argv and "--sdk" not in sys.argv:
    from native_experiment_providers import install
    install()
    previous=swe.ClaudeSwebenchRunner._segment,swe.CopilotSwebenchRunner._segment,swe.CodexSwebenchRunner._segment,swe.ClaudeSwebenchRunner._session_log_text
    install()
    assert previous == (swe.ClaudeSwebenchRunner._segment,swe.CopilotSwebenchRunner._segment,swe.CodexSwebenchRunner._segment,swe.ClaudeSwebenchRunner._session_log_text)

for line in sys.stdin:
    request=json.loads(line);calls,counts=[],{}
    absent=object()
    saved={name:getattr(swe,name,absent) for name in ("os","time","subprocess","json","copilot","codex","net_guard","network_tool_block","_UsageRecovery","_run","_log","Exception")}
    class Token:
        def __init__(self, name): self.name = name
        def __bool__(self): record("bool:"+self.name);return request.get("booleans", {}).get(self.name, True)
        def __str__(self): record("str:"+self.name);return "opaque "+self.name
        def __repr__(self): record("repr:"+self.name);return "opaque "+self.name
        def __format__(self, spec): record("format:"+self.name, spec);return "opaque "+self.name
        def __fspath__(self): record("fspath:"+self.name);return request.get("path", "/run/work space")
        def __iter__(self): record("iter:"+self.name);return iter(request.get("iterables", {}).get(self.name, ["Bash", "WebFetch"]))
        def __contains__(self, value): record("contains:"+self.name, value);return False
        def __ne__(self, value): record("ne:"+self.name, value);return request.get("not_equal", False)
        def __eq__(self, value): record("eq:"+self.name, value);return False

    def encode(value):
        if isinstance(value, Token): return {"ref": value.name}
        if isinstance(value, (list, tuple)): return [encode(item) for item in value]
        if isinstance(value, dict): return {key: encode(item) for key, item in dict.items(value)}
        if isinstance(value, type): return value.__name__
        return value

    class DetailError(Exception):
        def __str__(self): record("error.str", type(sys.exception()).__name__ if sys.exception() else None);return "segment fixture é😀"

    def record(name, args=None):
        calls.append([name, encode(args)])
        counts[name] = counts.get(name, 0)+1
        if (request.get("fail") == name and counts[name] == request.get("fail_at", 1)) or (name in request.get("failures", {}) and counts[name] == 1):
            kind = request.get("failures", {}).get(name, request.get("error", "RuntimeError"))
            if kind == "DetailError": raise DetailError()
            if kind == "TimeoutExpired": raise subprocess.TimeoutExpired("agent", 60)
            if kind.startswith("Shadow"): raise type(kind.removeprefix("Shadow"), (Exception,), {})("fixture failure")
            raise getattr(builtins, kind)("fixture failure")

    class Namespace:
        def __init__(self, tag, **values): self.__dict__.update(values);self.tag = tag
        def __getattribute__(self, name):
            if name not in ("tag", "__dict__", "__class__"): record(object.__getattribute__(self, "tag")+"."+name)
            return object.__getattribute__(self, name)

    def callback(name, value=None, implementation=None):
        def call(*args, **kwargs):
            record(name, [args, list(kwargs), kwargs])
            return implementation(*args, **kwargs) if implementation else value
        return call

    class Mapping(dict):
        def __getattribute__(self, name):
            if name in ("get", "update"): record(object.__getattribute__(self, "tag")+"."+name+"_function")
            return dict.__getattribute__(self, name)
        def get(self, *args): record(self.tag+".get", args);return dict.get(self, *args)
        def __getitem__(self, key): record(self.tag+".item", key);return dict.__getitem__(self, key)
        def __bool__(self): record(self.tag+".bool");return dict.__len__(self) != 0
        def __setitem__(self, key, value): record(self.tag+".put", [key, value]);return dict.__setitem__(self, key, value)

    def mapping(tag, value):
        result = Mapping(value);result.tag = tag;return result

    class Pipe:
        def __init__(self, name): self.name, self.written, self.closed = name, [], False
        def write(self, value): record(self.name+".write", value);self.written.append(value)
        def flush(self): record(self.name+".flush")
        def close(self): record(self.name+".close");self.closed = True
        def read(self): record(self.name+".read");return request.get("stderr", "")
        def __iter__(self): record(self.name+".iter");return iter(request.get("lines", ["  ", " bad json \n", '{"type":"result","num_turns":2}\n']))

    class Process:
        def __init__(self): self.stdin, self.stdout, self.stderr, self.pid = Pipe("stdin"), Pipe("stdout"), Pipe("stderr"), 1234
        def __getattribute__(self, name):
            if name in ("stdin", "stdout", "stderr", "pid", "kill", "wait"): record("proc."+name)
            return object.__getattribute__(self, name)
        def kill(self): record("kill")
        def wait(self, **kwargs): record("wait", [list(kwargs), kwargs]);return 0

    process, transcript = Process(), Pipe("transcript")
    launched = []
    def popen(*args, **kwargs): record("popen", [args, list(kwargs), kwargs]);launched.append([args, kwargs]);return process

    class Guard:
        def __getattribute__(self, name):
            if name in ("segment", "monitor"): record("guard."+name)
            return object.__getattribute__(self, name)
        segment = 3
        def __bool__(self): record("guard.bool");return False
        def monitor(self, value): record("monitor", value);return counts["monitor"] >= request.get("stop_after", 999)

    class Recovery:
        def __init__(self): record("recovery")
        def __getattribute__(self, name):
            if name in ("observe", "synthesize"): record("recovery."+name)
            return object.__getattribute__(self, name)
        def observe(self, event): record("observe", event)
        def synthesize(self, reason, logs): record("synthesize", [reason, logs]);return {"recovered": reason, "logs": logs}

    agent=request.get("agent","claude")
    cls={"claude":swe.ClaudeSwebenchRunner,"copilot":swe.CopilotSwebenchRunner,"codex":swe.CodexSwebenchRunner}[agent]
    tracked=("model","effort","api_key","protected_tools","task_timeout","NET_GUARD_LOG","_env_activation_args","scaffold_profile","_guard_python","_session_log_text","_exec")
    class Runner(cls):
        NET_GUARD_LOG="/run/net-guard.jsonl"
        def __getattribute__(self,name):
            if name in tracked: record("get:"+name)
            return super().__getattribute__(name)
        _env_activation_args=staticmethod(callback("activation",request.get("activation",["-e","BASH_ENV=/testbed/project-env"])))
        _exec=staticmethod(callback("exec",Namespace("exec_result",stdout=request.get("log_text","session logs"))))
    if request.get("operation") != "session_log": Runner._session_log_text=staticmethod(callback("session_log",request.get("log_text","session logs")))
    runner=object.__new__(Runner)
    for name,value in {"model":"fixture-model","effort":"high","api_key":"fixture-key","protected_tools":["Bash"],"task_timeout":10,"_guard_python":"/bin/python",**request.get("fields",{})}.items():object.__setattr__(runner,name,value)
    profile=Namespace("profile",mcp_args=callback("mcp_args",request.get("mcp_args",["--mcp-config","/fixture/mcp.json"])),home=request.get("home","/home/task"))
    object.__setattr__(runner,"scaffold_profile",profile)
    for name in request.get("missing",[]):object.__delattr__(runner,name)
    inputs={"name":request.get("name","fixture-container"),"prompt":request.get("prompt","fix é😀\n"),"exclude_tools":request.get("exclude_tools",["Bash","Read","WebFetch","Read"]),"max_turns":request.get("max_turns",8),"mcp_config":request.get("mcp_config","/fixture/mcp.json")}
    if isinstance(inputs["prompt"],dict):inputs["prompt"]=mapping("resume",inputs["prompt"])
    usage=mapping("usage",request.get("usage",{"num_turns":2,"input_tokens":11}))
    raw_argv=request.get("argv",["agent","exec","-C","/testbed","-"])
    for name in request.get("objects",[]):
        if name in inputs:inputs[name]=Token(name)
        elif name == "argv":raw_argv[2]=Token(name)
        elif name == "home":object.__setattr__(profile,"home",Token(name))
        else:object.__setattr__(runner,name,Token(name))
    def tick():record("clock");return request.get("times",[0,1,2,3])[min(counts["clock"]-1,len(request.get("times",[0,1,2,3]))-1)]
    def log(value):record("log",[value,type(sys.exception()).__name__ if sys.exception() else None])
    def host_run(*args,**kwargs):record("run",[args,list(kwargs),kwargs]);return Namespace("host_result",stdout=request.get("log_text","debug logs"))
    try:
        swe.os=Namespace("os",environ=request.get("env",{"KEEP":"yes","ANTHROPIC_API_KEY":"stale-key"}))
        swe.time=Namespace("time",time=tick,sleep=callback("sleep"))
        swe.subprocess=Namespace("subprocess",Popen=popen,PIPE=-1,DEVNULL=-3,TimeoutExpired=getattr(builtins,request["timeout_type"]) if "timeout_type" in request else subprocess.TimeoutExpired)
        swe.json=Namespace("json",loads=callback("loads",implementation=json.loads))
        swe._log,swe._run,swe._UsageRecovery=log,host_run,Recovery
        swe.net_guard=Namespace("net_guard",LOG_ENV="FIXTURE_GUARD_LOG")
        swe.network_tool_block=callback("network",request.get("network",["WebFetch","WebSearch","WebSearch"]))
        options=dict(build_argv=callback("build_argv",raw_argv),run_env=callback("run_env",request.get("run_env",{"RUN_ENV":"yes"})),row_usage=callback("row_usage",usage))
        swe.copilot=Namespace("copilot",**options,CONTAINER_LOG_DIR="/run/copilot-logs",provider_secret_names=callback("secrets",request.get("secrets",["PROVIDER_KEY"])),fatal_config_error=callback("fatal_config",request.get("fatal","")))
        swe.codex=Namespace("codex",**options,KEY_ENV="FIXTURE_CODEX_KEY",build_resume_argv=callback("resume_argv",raw_argv))
        for name,kind in request.get("exception_types",{}).items():setattr(swe,name,getattr(builtins,kind) if isinstance(kind,str) else kind)
        if request.get("operation") == "session_log":value=runner._session_log_text(inputs["name"])
        else:
            value=runner._segment(inputs["name"],inputs["prompt"],transcript,inputs["exclude_tools"],inputs["max_turns"],Guard() if request.get("guard",True) else None,mcp_config=inputs["mcp_config"])
            if agent != "claude":assert value is usage,"usage identity changed"
            for argv,keywords in launched:
                assert "fixture-key" not in argv,"secret reached argv"
        result={"value":encode(value)}
    except BaseException as error:
        result={"raised":type(error).__name__,"message":str(error)}
        if request.get("exception_chain"):
            chain,current=[],error.__context__
            while current is not None and len(chain)<5:chain.append(type(current).__name__);current=current.__context__
            result["context"]=chain
    finally:
        for name,value in saved.items():
            if value is absent:vars(swe).pop(name,None)
            else:setattr(swe,name,value)
    result.update(calls=calls,transcript=transcript.written,stdin=object.__getattribute__(process,"stdin").written,stdin_closed=object.__getattribute__(process,"stdin").closed,launched=len(launched),usage=encode(usage))
    print(json.dumps(encode(result),sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
