"""Registry mappings, constructor and exception bindings for scaffold selection."""
import builtins
import json
from pathlib import Path
import sys

ROOT=Path(__file__).resolve().parents[1]
if "--sdk" in sys.argv:
    from standalone_scaffolds import runtime
    baselines=runtime.registry
else:
    sys.path[:0]=[str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
    from te_bench import baselines
    if "--native" in sys.argv:
        from native_experiment_providers import install
        install(scaffold_module=baselines)
        previous=baselines.get,baselines.for_arm
        install(scaffold_module=baselines)
        assert previous == (baselines.get,baselines.for_arm)

assert "te_harness.swebench" not in sys.modules and "te_harness.webbench" not in sys.modules

for line in sys.stdin:
    request=json.loads(line);calls,counts=[],{}
    saved=baselines.SCAFFOLDS,baselines.ARM_SCAFFOLDS,baselines.ScaffoldError,baselines.get
    class Token:
        def __init__(self,name): self.name=name
        def __bool__(self): record("bool:"+self.name);return request.get("truthy",True)
        def __hash__(self): record("hash:"+self.name);return hash("fixture")
        def __eq__(self,other): record("eq:"+self.name,other);return other == "fixture"
        def __repr__(self): record("repr:"+self.name);return "opaque é😀"
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,dict): return {key:encode(item) for key,item in value.items()}
        if isinstance(value,(list,tuple)): return [encode(item) for item in value]
        return value
    class DetailError(Exception):
        def __str__(self): record("error.str");return "registry fixture failure é😀"
    def record(name,args=None):
        calls.append([name,encode(args),type(sys.exception()).__name__ if sys.exception() is not None else None]);counts[name]=counts.get(name,0)+1
        if request.get("fail") == name and counts[name] == request.get("fail_at",1):
            kind=request.get("error","RuntimeError")
            if kind == "DetailError": raise DetailError()
            raise getattr(builtins,kind)("fixture failure")
    class Mapping(dict):
        def __getitem__(self,key): record(self.tag+".item",key);return dict.__getitem__(self,key)
        def __iter__(self): record(self.tag+".iter");return dict.__iter__(self)
        def get(self,key): record(self.tag+".get",key);return dict.get(self,key)
    def mapping(tag,items): value=Mapping(items);value.tag=tag;return value
    def constructor():
        record("constructor")
        if request.get("replace_registry"): baselines.SCAFFOLDS=mapping("replacement",{"new":constructor});raise KeyError("constructor key")
        return Token("scaffold")
    class ScaffoldError(saved[2]):
        def __init__(self,message): record("error.new",message);super().__init__(message)
    try:
        baselines.SCAFFOLDS=mapping("registry",{key:constructor for key in request.get("keys",["serena","rtk","repomap","fixture"])})
        baselines.ARM_SCAFFOLDS=mapping("arms",request.get("arms",{"fixture-arm":"fixture","none":None,"":"fixture"}))
        baselines.ScaffoldError=ScaffoldError
        if request.get("noncallable"): baselines.SCAFFOLDS["fixture"]=42
        if request.get("stub_get"):
            def get(name): record("get",name);return Token("get result")
            baselines.get=get
        name=Token("name") if request.get("opaque_name") else request.get("name","fixture")
        arm=Token("arm") if request.get("opaque_arm") else request.get("arm","fixture-arm")
        value=baselines.for_arm(arm) if request.get("operation") == "for_arm" else baselines.get(name)
        output={"value":value}
    except BaseException as error:
        output={"raised":type(error).__name__,"message":str(error),"context":type(error.__context__).__name__ if error.__context__ else None,"cause":type(error.__cause__).__name__ if error.__cause__ else None,"suppressed":error.__suppress_context__}
    finally: baselines.SCAFFOLDS,baselines.ARM_SCAFFOLDS,baselines.ScaffoldError,baselines.get=saved
    output.update(calls=calls)
    print(json.dumps(encode(output),sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
