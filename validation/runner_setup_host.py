"""SDK references and dependency stand-ins for native runner constructors."""
import builtins
import json
import os
from pathlib import Path
import sys
import types

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
from te_harness import swebench as sw
if "--native" in sys.argv:
    from native_experiment_task import install
    install()
    from native_runner_setup import install
    install()
    from native_experiment_providers import install
    install()

for line in sys.stdin:
    request = json.loads(line)
    calls = []
    integer_limit = sys.get_int_max_str_digits()
    if "int_max_digits" in request: sys.set_int_max_str_digits(request["int_max_digits"])
    saved = {name:getattr(sw,name) for name in ("os","scaffolds_pkg","pricing","codex","copilot","resolve_anthropic_key","claude_bin","cli_version","_Grader","_log")}
    helper_os = {name:saved[name].os for name in ("copilot","codex")}
    references = {}
    class Token:
        def __init__(self,name): self.name = name
    class Model(str): pass
    class Name(str): pass
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,Scaffold): return {"ref":"scaffold"}
        if isinstance(value,dict): return {key:encode(item) for key,item in value.items()}
        if isinstance(value,(tuple,list)): return [encode(item) for item in value]
        if isinstance(value,(set,frozenset)): return sorted(value)
        return value
    def record(name,args=None):
        calls.append([name,encode(args)])
        if request.get("fail") == name:
            error = request.get("error","RuntimeError")
            if error == "ShadowValueError": raise type("ValueError",(Exception,),{})("fixture failure")
            raise getattr(builtins,error)("fixture failure")
    class Environment(dict):
        def get(self,*args): record("env.get",list(args));return super().get(*args)
    class Scaffold:
        name = "fixture"
        def __bool__(self): record("scaffold.bool");return not request.get("falsey_scaffold")
        def preflight(self,p):
            assert p is profile,"profile lost identity"
            record("scaffold.preflight",p)
    scaffold,profile,key,grader = Scaffold(),Token("profile"),Token("key"),Token("grader")
    claude_binary,copilot_binary = Token("claude_binary"),Token("copilot_binary")
    grading_path = Token("grading_path")
    routing = request.get("routing",{"base_url":"http://fixture/v1","api_key":key})
    endpoint = request.get("endpoint")
    if isinstance(endpoint,list): endpoint = tuple(endpoint)
    skills = request.get("skills",["zeta","alpha"])
    if request.get("skills_tuple"): skills = tuple(skills)
    arguments = dict(request.get("arguments",{}))
    if isinstance(arguments.get("model"),str): arguments["model"] = Model(arguments["model"])
    if isinstance(arguments.get("scaffold"),str): arguments["scaffold"] = Name(arguments["scaffold"])
    arguments["grading_path"] = grading_path
    def helper(name,implementation):
        def called(*args,**kwargs):
            record(name,[args,list(kwargs),kwargs])
            if name == "validate_config": assert args[0] is references["config"],"configuration result lost identity"
            if name == "lean" and request.get("mutate_lean"):
                runner.model = "changed model"
                runner.routing = {"base_url":"changed endpoint","api_key":"changed"}
                runner.system_skills = ("changed skill",)
                runner.effort = "low"
                sw.codex.config_toml = helper("replaced_config",lambda **kwargs:Token("replaced config"))
            returned = implementation(*args,**kwargs)
            references[name] = returned
            return returned
        return called
    def get_scaffold(name):
        if "scaffold" in arguments and arguments["scaffold"]: assert name is arguments["scaffold"],"scaffold name lost identity"
        return None if request.get("no_scaffold") else scaffold
    def make_grader(path): assert path is grading_path,"grading path lost identity";return grader
    def normalize(model):
        return None if model is None or isinstance(model,str) and model.lower() == "auto" else model
    def preflight(self): record("preflight")
    agent = request.get("agent","claude")
    base_class = {"claude":sw.ClaudeSwebenchRunner,"copilot":sw.CopilotSwebenchRunner,"codex":sw.CodexSwebenchRunner}[agent]
    class Runner(base_class):
        scaffold_profile = profile
        _preflight_optimizer_endpoint = preflight
        def __setattr__(self,name,value):
            record("set:"+name,value)
            super().__setattr__(name,value)
    runner = object.__new__(Runner)
    try:
        sw.os = types.SimpleNamespace(environ=Environment(request.get("env",{})))
        sw._log = lambda message:record("log",message)
        sw.scaffolds_pkg = types.SimpleNamespace(get=helper("scaffold.get",get_scaffold))
        sw.pricing = types.SimpleNamespace(weights_for=helper("weights",lambda entry:None))
        sw.resolve_anthropic_key = helper("key",lambda:key)
        sw.claude_bin = helper("claude_bin",lambda:claude_binary)
        sw.cli_version = helper("claude_version",lambda binary:"fixture-cli")
        sw._Grader = helper("grader",make_grader)
        sw.copilot = types.SimpleNamespace(EFFORT_LEVELS=saved["copilot"].EFFORT_LEVELS,PROTECTED_TOOLS=saved["copilot"].PROTECTED_TOOLS,
            normalize_model=helper("normalize",normalize),validate_priceable=helper("priceable",lambda *args:None),
            effort_applies=helper("effort_applies",lambda model:request.get("effort_applies",bool(model))),
            require_install=helper("copilot_install",lambda **kwargs:copilot_binary),provider_base=helper("provider_base",lambda:"http://copilot"),
            anthropic_optimizer_endpoint=helper("endpoint",lambda:endpoint),cli_version=helper("copilot_version",lambda binary:"fixture-cli"))
        sw.codex = types.SimpleNamespace(**{name:getattr(saved["codex"],name) for name in ("EFFORT_LEVELS","PROTECTED_TOOLS","C_BIN","CONFIG_CAPABILITIES","DEFAULT_SYSTEM_SKILLS")},
            resolve_codex_routing=helper("routing",lambda:routing),validate_priceable=helper("priceable",lambda *args:None),
            require_install=helper("codex_install",lambda:request.get("install_pair",["shim","vendor"])),cli_version=helper("codex_version",lambda binary:request.get("version","fixture-cli")),
            bundled_system_skills=helper("skills",lambda **kwargs:skills),base_prompt_for=helper("base_prompt",lambda version:request.get("base_prompt")),
            lean_instructions=helper("lean",lambda *args:Token("instructions")),config_toml=helper("config",lambda **kwargs:Token("config")),
            validate_config=helper("validate_config",lambda *args,**kwargs:request.get("bad_config","")),
            verify_model=helper("verify",lambda *args:request.get("verification",[True,""])),preflight_optimizer=helper("codex_preflight",lambda **kwargs:None))
        if request.get("real_helpers"):
            saved["copilot"].os = saved["codex"].os = sw.os
            sw.resolve_anthropic_key = helper("key",saved["resolve_anthropic_key"])
            sw.pricing.weights_for = helper("weights",saved["pricing"].weights_for)
            for name,label in (("normalize_model","normalize"),("validate_priceable","priceable"),("effort_applies","effort_applies"),("provider_base","provider_base"),("anthropic_optimizer_endpoint","endpoint")):
                setattr(sw.copilot,name,helper(label,getattr(saved["copilot"],name)))
            for name,label in (("resolve_codex_routing","routing"),("validate_priceable","priceable")):
                setattr(sw.codex,name,helper(label,getattr(saved["codex"],name)))
        if request.get("real_config"):
            def config_helper(name):
                def invoke_config(*args,**kwargs):
                    previous = saved["codex"].os.environ
                    saved["codex"].os.environ = dict(previous)
                    try: return getattr(saved["codex"],name)(*args,**kwargs)
                    finally: saved["codex"].os.environ = previous
                return invoke_config
            sw.codex.lean_instructions = helper("lean",config_helper("lean_instructions"))
            sw.codex.config_toml = helper("config",config_helper("config_toml"))
        runner.__init__(**arguments)
        if runner.scaffold is not None: assert runner.scaffold is scaffold,"scaffold lost identity"
        assert runner.grader is grader,"grader lost identity"
        if agent == "codex":
            assert type(runner.system_skills) is tuple,"skills were not frozen"
            assert type(runner._capabilities) is tuple,"capabilities were not a tuple"
            if not request.get("mutate_lean"): assert runner.routing is references["routing"],"routing lost identity"
        if agent == "copilot" and not request.get("mutate_lean"):
            assert runner.model is references["normalize"],"normalised model lost identity"
        elif "model" in arguments and arguments["model"] and not request.get("mutate_lean"):
            assert runner.model is arguments["model"],"model lost identity"
        response = {"value":None}
    except BaseException as error: response = {"raised":type(error).__name__,"message":str(error)}
    finally:
        response.update(state=encode(vars(runner)),calls=calls)
        for name,value in saved.items(): setattr(sw,name,value)
        for name,value in helper_os.items(): saved[name].os = value
        sys.set_int_max_str_digits(integer_limit)
    print(json.dumps(response,sort_keys=True,separators=(",",":"),ensure_ascii=False),flush=True)
