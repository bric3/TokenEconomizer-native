"""Dependency, object-protocol and state fixtures for Web-Bench constructors."""
import builtins
import json
from pathlib import Path
import sys
import types

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
from te_harness import swebench as sw
if "--native" in sys.argv:
    from native_runner_setup import install
    install()
    from native_experiment_providers import install as providers
    providers()
from te_harness import webbench as wb
if "--native" in sys.argv:
    install(wb)
    providers(wb)
    original_constructor = wb.CodexWebBenchRunner.__init__
    install(wb)
    assert wb.CodexWebBenchRunner.__init__ is original_constructor,"constructor install is not idempotent"

for line in sys.stdin:
    request = json.loads(line)
    calls,references = [],{}
    names = ("os","shutil","subprocess","scaffolds_pkg","pricing","codex","copilot","resolve_anthropic_key","claude_bin","cli_version","webbench_repo","node_bin","webbench_task_timeout","_log")
    saved = {name:getattr(wb,name) for name in names}
    swebench_os = sw.os
    helper_os = {name:saved[name].os for name in ("copilot","codex")}
    integer_limit = sys.get_int_max_str_digits()
    if "int_max_digits" in request: sys.set_int_max_str_digits(request["int_max_digits"])
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,Scaffold): return {"ref":"scaffold"}
        if isinstance(value,dict): return {key:encode(item) for key,item in value.items()}
        if isinstance(value,(tuple,list)): return [encode(item) for item in value]
        if isinstance(value,(set,frozenset)): return sorted(value)
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
        def __format__(self,spec):
            record("format:"+self.name,spec)
            if request.get("mutate_format"): wb._log = lambda message:record("new.log",message)
            return "formatted-"+self.name
        def __int__(self): record("int:"+self.name);return request.get("integer",17)
        def __eq__(self,other): record("eq:"+self.name,other);return Token("comparison") if request.get("opaque_comparison") else False
    class Model(str): pass
    class Environment(dict):
        def get(self,*args): record("env.get",args);return super().get(*args)
    class Scaffold:
        name = "fixture"
        def __bool__(self): record("scaffold.bool");return not request.get("falsey_scaffold")
        def preflight(self,actual): assert actual is profile,"profile identity changed";record("scaffold.preflight",actual);return Token("preflight_result")
    scaffold,profile,key,repo,node = Scaffold(),Token("profile"),Token("key"),Token("repo"),Token("node")
    claude_binary,copilot_binary,codex_binary,vendor = Token("claude"),Token("copilot"),Token("codex"),Token("vendor")
    routing = request.get("routing",{"base_url":"http://fixture/v1","api_key":key})
    endpoint = request.get("endpoint")
    skills = request.get("skills",["first","second"])
    arguments = dict(request.get("arguments",{}))
    for name in ("model","scaffold"):
        if isinstance(arguments.get(name),str): arguments[name] = Model(arguments[name])
    for name in request.get("objects",[]): arguments[name] = Token(name)
    environment = Environment(request.get("env",{}))
    for name in request.get("env_objects",[]): environment[name] = Token(name)
    def helper(name,implementation):
        def call(*args,**kwargs):
            record(name,[args,list(kwargs),kwargs])
            returned = implementation(*args,**kwargs)
            references[name] = returned
            return returned
        return call
    def normalize(model): return None if isinstance(model,str) and model.lower() == "auto" else model
    def profile_for(self,home):
        record("profile",home)
        if request.get("mutate_profile"): scaffold.preflight = lambda actual:record("new.preflight",actual)
        return profile
    def preflight(self): record("preflight");return Token("preflight_result")
    def namespace(self):
        record("namespace")
        return base._probe_namespace_guard(self) if request.get("real_host_helpers") else request.get("namespace",True)
    def namespace_run(*args,**kwargs): record("namespace.run",[args,list(kwargs),kwargs]);return types.SimpleNamespace(returncode=request.get("namespace_returncode",0))
    agent = request.get("agent","claude")
    base = {"claude":wb.WebBenchRunner,"copilot":wb.CopilotWebBenchRunner,"codex":wb.CodexWebBenchRunner}[agent]
    tracked = {"agent","model","api_key","optimizer_model","effort","effort_requested","scaffold","scaffold_name","copilot","claude","agent_bin","cli_version","routing","routing_base","optimizer_endpoint","base_prompt","system_skills","scaffold_profile_for","_init_stream_state","_probe_namespace_guard","_preflight_optimizer_endpoint"}
    class Runner(base):
        scaffold_profile_for = profile_for
        _preflight_optimizer_endpoint = preflight
        _probe_namespace_guard = namespace
        def __getattribute__(self,name):
            if request.get("getters") and name in tracked: record("get:"+name)
            return super().__getattribute__(name)
        def __setattr__(self,name,value): record("set:"+name,value);super().__setattr__(name,value)
        def _init_stream_state(self):
            record("stream")
            if not request.get("stream_override"): base._init_stream_state(self)
            return Token("stream_result")
    runner = object.__new__(Runner)
    previous_stream = ["previous"]
    object.__setattr__(runner,"_stream",previous_stream)
    try:
        wb.os = types.SimpleNamespace(environ=environment,path=types.SimpleNamespace(realpath=helper("realpath",lambda value:repo)))
        wb.resolve_anthropic_key = helper("key",lambda:key)
        wb.claude_bin = helper("claude_bin",lambda:claude_binary)
        wb.cli_version = helper("claude_version",lambda binary:request.get("version","fixture-cli"))
        wb.pricing = types.SimpleNamespace(weights_for=helper("weights",lambda entry:None))
        wb.scaffolds_pkg = types.SimpleNamespace(get=helper("scaffold.get",lambda name:None if request.get("no_scaffold") else scaffold))
        wb.webbench_repo,wb.node_bin = helper("repo",lambda:repo),helper("node",lambda:node)
        wb.webbench_task_timeout = helper("task_timeout",lambda:request.get("task_timeout",240))
        wb.shutil = types.SimpleNamespace(which=helper("which",lambda name:node))
        wb.subprocess = types.SimpleNamespace(run=namespace_run,TimeoutExpired=saved["subprocess"].TimeoutExpired)
        wb._log = lambda message:record("log",message)
        wb.copilot = types.SimpleNamespace(EFFORT_LEVELS=saved["copilot"].EFFORT_LEVELS,PROTECTED_TOOLS=saved["copilot"].PROTECTED_TOOLS,
            normalize_model=helper("normalize",normalize),validate_priceable=helper("priceable",lambda *args:None),effort_applies=helper("effort_applies",lambda model:request.get("effort_applies",model is not None)),
            require_install=helper("copilot_install",lambda **kwargs:copilot_binary),provider_base=helper("provider_base",lambda:"http://copilot"),
            anthropic_optimizer_endpoint=helper("endpoint",lambda:endpoint),cli_version=helper("copilot_version",lambda binary:request.get("version","fixture-cli")))
        wb.codex = types.SimpleNamespace(resolve_codex_routing=helper("routing",lambda:routing),validate_priceable=helper("priceable",lambda *args:None),verify_model=helper("verify",lambda *args:request.get("verification",[True,""])),
            require_install=helper("codex_install",lambda:Token("install_result")),codex_vendor=helper("vendor",lambda:vendor),codex_bin=helper("codex_bin",lambda:codex_binary),cli_version=helper("codex_version",lambda binary:request.get("version","fixture-cli")),
            base_prompt_for=helper("base_prompt",lambda version:request.get("base_prompt")),bundled_system_skills=helper("skills",lambda binary:skills))
        if request.get("real_helpers"):
            sw.os = wb.os
            saved["copilot"].os = saved["codex"].os = wb.os
            wb.resolve_anthropic_key = helper("key",saved["resolve_anthropic_key"])
            wb.pricing.weights_for = helper("weights",saved["pricing"].weights_for)
            for name,label in (("normalize_model","normalize"),("validate_priceable","priceable"),("effort_applies","effort_applies"),("provider_base","provider_base"),("anthropic_optimizer_endpoint","endpoint")):
                setattr(wb.copilot,name,helper(label,getattr(saved["copilot"],name)))
            for name,label in (("resolve_codex_routing","routing"),("validate_priceable","priceable")):
                setattr(wb.codex,name,helper(label,getattr(saved["codex"],name)))
        if request.get("real_host_helpers"):
            wb.webbench_repo = saved["webbench_repo"]
            wb.node_bin = saved["node_bin"]
            wb.webbench_task_timeout = saved["webbench_task_timeout"]
        if request.get("stream_only"):
            base._init_stream_state(runner)
            assert runner._stream is not previous_stream,"stream state reused a previous list"
        else:
            runner.__init__(**arguments)
            if "model" in arguments and arguments["model"] and not request.get("real_helpers") and not (isinstance(arguments["model"],str) and arguments["model"].lower() == "auto"):
                assert runner.model is arguments["model"],"model identity changed"
            assert runner.repo is repo,"repository identity changed"
            assert runner.node is node,"node binary identity changed"
            if agent == "codex": assert runner.codex_vendor is vendor and runner.agent_bin is codex_binary,"Codex installation identity changed"
            if agent == "copilot": assert runner.copilot is copilot_binary,"Copilot binary identity changed"
        result = {"value":None}
    except BaseException as error: result = {"raised":type(error).__name__,"message":str(error)}
    finally:
        result.update(state=encode(vars(runner)),calls=calls)
        for name,value in saved.items(): setattr(wb,name,value)
        for name,value in helper_os.items(): saved[name].os = value
        sw.os = swebench_os
        sys.set_int_max_str_digits(integer_limit)
    print(json.dumps(result,sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
