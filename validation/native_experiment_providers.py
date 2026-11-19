"""Bind original SDK namespaces to the production object and I/O runtime."""
import functools
import inspect
import sys

from native_plugin import ROOT
from sdk_transport import ValidationBridge

sys.path.insert(0,str(ROOT / "src/experiments/sdk"))
from te_runtime.providers import bind
from te_runtime.swebench_runner import bind as runner_bindings
from te_runtime.webbench_calls import bind as webbench_bindings


def install(runner_module=None,scaffold_module=None,driver_module=None):
    if scaffold_module is None and driver_module is None:
        from te_harness import swebench, copilot, codex
    replacements = {}
    def adapter_factories():
        from te_bench.adapters import SwebenchAdapter, WebBenchAdapter, CredDataAdapter, ReposVulAdapter
        return dict(SwebenchAdapter=SwebenchAdapter,WebBenchAdapter=WebBenchAdapter,CredDataAdapter=CredDataAdapter,ReposVulAdapter=ReposVulAdapter)
    runtime=bind(ValidationBridge(),shared=sys.modules.get("te_harness.swebench"),adapters=adapter_factories,
        container_helpers=str(ROOT/"target/container-helpers"))
    def function(module,original,agent,name,captures=None):
        native=runtime(module,original,agent,name,captures)
        signature=inspect.signature(original)
        @functools.wraps(native)
        def checked_arguments(*args,**kwargs):
            try: signature.bind(*args,**kwargs)
            except TypeError: return original(*args,**kwargs)
            return native(*args,**kwargs)
        checked_arguments._te_provider_operation=agent,name
        return checked_arguments
    if driver_module is not None:
        for name in (("env_path","_parse","load_env","require_keys") if hasattr(driver_module,"require_keys") else ("load_runner","load_adapter","workload_ids","_same_group","load_tasks","lift_overhead","main")):
            original = getattr(driver_module,name)
            if getattr(original,"_te_provider_operation",None) == ("driver",name): continue
            replacements[original] = function(driver_module,original,"driver",name)
            setattr(driver_module,name,replacements[original])
        if hasattr(driver_module,"StubRunner"):
            for name,operation in (("__init__","new"),("optimizer_call","optimizer"),("_write_transcript","transcript"),("run_task","task")):
                original=getattr(driver_module.StubRunner,name)
                if getattr(original,"_te_provider_operation",None) == ("driver","stub_"+operation): continue
                replacements[original]=function(driver_module,original,"driver","stub_"+operation)
                setattr(driver_module.StubRunner,name,replacements[original])
        return replacements
    def scaffold_calls(owner):
        if owner.__name__ == "te_bench.baselines.repomap":
            for name,operation in (("map_tokens","tokens"),("extract_idents","idents")):
                original=getattr(owner,name)
                if getattr(original,"_te_provider_operation",None) == ("scaffold","repomap_"+operation): continue
                replacements[original]=function(owner,original,"scaffold","repomap_"+operation)
                setattr(owner,name,replacements[original])
            cls=owner.RepoMapScaffold
            for name,operation in (("preflight","preflight"),("fetch_sources","fetch"),("generate","generate"),("setup","setup")):
                original=getattr(cls,name)
                if getattr(original,"_te_provider_operation",None) == ("scaffold","repomap_"+operation): continue
                replacements[original]=function(owner,original,"scaffold","repomap_"+operation)
                setattr(cls,name,replacements[original])
            return
        if owner.__name__ == "te_bench.baselines":
            for name in ("get","for_arm"):
                original=getattr(owner,name)
                if getattr(original,"_te_provider_operation",None) == ("scaffold","registry_"+name): continue
                replacements[original]=function(owner,original,"scaffold","registry_"+name)
                setattr(owner,name,replacements[original])
            return
        if owner.__name__ == "te_bench.baselines.rtk":
            original=owner.cache_dir
            if getattr(original,"_te_provider_operation",None) != ("scaffold","rtk_cache"):
                replacements[original]=function(owner,original,"scaffold","rtk_cache")
                owner.cache_dir=replacements[original]
            cls=owner.RtkScaffold
            for name,operation in (("_hook_binary","hook_binary"),("_claude_hook","claude_hook"),("_copilot_hook_config","copilot_config"),("bin_path","bin_path"),("preflight","preflight"),("stage_host_binary","stage"),("setup","setup"),("_codex_instructions","codex"),("_install_copilot_hook","copilot"),("_probe_hook_rewrites","probe")):
                descriptor=vars(cls)[name]
                original=descriptor.__func__ if isinstance(descriptor,(staticmethod,classmethod)) else descriptor
                if getattr(original,"_te_provider_operation",None) == ("scaffold","rtk_"+operation): continue
                replacements[original]=function(owner,original,"scaffold","rtk_"+operation)
                setattr(cls,name,type(descriptor)(replacements[original]) if isinstance(descriptor,(staticmethod,classmethod)) else replacements[original])
            return
        if owner.__name__ == "te_bench.baselines.serena":
            cls=owner.SerenaScaffold
            for name,operation in (("setup","serena_setup"),("mcp_server","serena_server"),("nudge","serena_nudge")):
                original=getattr(cls,name)
                if getattr(original,"_te_provider_operation",None) == ("scaffold",operation): continue
                replacements[original]=function(owner,original,"scaffold",operation)
                setattr(cls,name,staticmethod(replacements[original]) if name != "setup" else replacements[original])
            return
        for cls_name,methods in (("AgentProfile",(("__init__","profile_new"),("mcp_args","profile_mcp"))),
                                 ("ScaffoldContext",(("__init__","context_new"),("read_bytes","context_read"),("log","context_log"),("write_instructions","instructions"),("write_mcp_config","mcp_config"),("merge_json_file","merge_json"))),
                                 ("Scaffold",(("mounts","mounts"),("preflight","preflight"),("setup","setup")))):
            cls = getattr(owner,cls_name)
            for name,operation in methods:
                original = getattr(cls,name)
                if getattr(original,"_te_provider_operation",None) == ("scaffold",operation): continue
                replacements[original] = function(owner,original,"scaffold",operation)
                setattr(cls,name,replacements[original])
    if scaffold_module is not None:
        scaffold_calls(scaffold_module)
        return replacements
    if runner_module is not None:
        replacements.update(webbench_bindings(ValidationBridge(),runner_module))
        return replacements
    replacements.update(runner_bindings(ValidationBridge(),swebench,
        container_helpers=str(ROOT/"target/container-helpers")))
    for module,agent,names in ((swebench,"claude",("resolve_anthropic_key","agent_credential_env","preflight_optimizer_endpoint","anthropic_optimizer_call","optimizer_log_path","_cell_of","set_optimizer_context","optimizer_call_kind","_log_optimizer_call","_log")),
                              (copilot,"copilot",("auth_env","byok_active","provider_base","anthropic_optimizer_endpoint","provider_env","provider_secret_names","normalize_model","effort_applies","validate_priceable","optimizer_call","copilot_bin","copilot_cache","copilot_config","cli_version","require_install","run_env")),
                              (codex,"codex",("resolve_codex_routing","validate_priceable","fetch_catalog","verify_model","preflight_optimizer","optimizer_call","_responses_text"))):
        for name in names:
            original = getattr(module,name)
            replacements[original] = function(module,original,agent,name)
            setattr(module,name,replacements[original])
    for module,agent,methods in ((copilot,"copilot",(("build_argv","build"),("fatal_config_error","fatal"))),
                                 (codex,"codex",(("build_argv","build"),("build_resume_argv","resume"),("run_env","env")))):
        for name,suffix in methods:
            original = getattr(module,name)
            operation = "invocation_"+suffix
            if getattr(original,"_te_provider_operation",None) == (agent,operation): continue
            replacements[original] = function(module,original,agent,operation)
            setattr(module,name,replacements[original])
    webbench = sys.modules.get("te_harness.webbench")
    if webbench is not None:
        for name in ("resolve_anthropic_key","agent_credential_env","preflight_optimizer_endpoint","anthropic_optimizer_call","set_optimizer_context","_cell_of","_log","claude_bin","cli_version"):
            original = getattr(webbench,name,None)
            if original in replacements: setattr(webbench,name,replacements[original])
    if webbench is not None: replacements.update(webbench_bindings(ValidationBridge(),webbench))
    from te_bench import baselines
    from te_bench.baselines import base,serena,rtk,repomap
    scaffold_calls(baselines)
    scaffold_calls(base)
    scaffold_calls(serena)
    scaffold_calls(rtk)
    scaffold_calls(repomap)
    return replacements
