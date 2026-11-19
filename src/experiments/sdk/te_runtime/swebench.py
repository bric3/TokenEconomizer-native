"""Built-in SWE-bench namespaces and signatures; production policy executes in Rust."""
import inspect
import signal

from .codex_install import bind as install_bindings
from .codex_spiral import bind as spiral_bindings
from .nudge_guard import bind as guard_bindings
from .optimizer import bind as optimizer_bindings
from .providers import bind as provider_bindings
from .runner_setup import bind as setup_bindings
from .scaffolds import bind as scaffold_bindings
from .swebench_runner import bind as runner_bindings
from .swebench_task import bind as task_bindings
from .transport import checked
from .usage import bind as usage_bindings
from types import SimpleNamespace


def bind(bridge,*,container_helpers=lambda:None):
    namespaces=optimizer_bindings(bridge)
    module,copilot,codex=namespaces.claude,namespaces.copilot,namespaces.codex
    module.__name__=__name__
    module.__file__=__file__
    module.codex,module.copilot=codex,copilot
    codex.signal=signal
    module.inspect,module.signal=inspect,signal
    module._LIVE_CONTAINERS=set()
    module.scaffolds_pkg=scaffold_bindings(bridge).registry
    module.NudgeGuard=guard_bindings(bridge.request).NudgeGuard
    module.codex_spiral=spiral_bindings(bridge.request)
    module.net_guard=SimpleNamespace(**checked(bridge.request("experiment.net_guard.constants",{})),
        hosts_block=lambda:checked(bridge.request("experiment.net_guard.hosts_block",{})))
    install_bindings(bridge,codex)
    accounting=usage_bindings(bridge.request,lambda message:module._log(message))
    for owner,usage in ((module,accounting.claude),(copilot,accounting.copilot),(codex,accounting.codex)):
        vars(owner).update(vars(usage))
    metadata=checked(bridge.request("experiment.harness.constants",{}))
    for name,owner in (("claude",module),("copilot",copilot),("codex",codex)):
        for key,value in metadata[name].items():
            setattr(owner,key,tuple(value) if key == "DEFAULT_SYSTEM_SKILLS" else frozenset(value) if isinstance(value,list) else value)

    def declaration(owner):
        def declare(signature):
            signature.__qualname__=signature.__qualname__.removeprefix("bind.<locals>.")
            signature.__module__=owner.__name__
            setattr(owner,signature.__name__,signature)
            return signature
        return declare
    api=declaration(module)
    @api
    def answer_key_attempts(run_dir_path): pass
    @api
    def guard_accepts(param): pass
    @api
    def default_grading_path(): pass
    @api
    def swebench_repo(): pass
    @api
    def build_prompt(task,injected): pass
    @api
    def _run(cmd,timeout=120,check=False): pass
    @api
    def env_activation_enabled(): pass
    @api
    def network_tool_block(): pass
    @api
    def _cpu_limit_args(): pass
    @api
    def choose_project_env(env_bins): pass
    @api
    def activation_env(env_bin,container_path): pass
    @api
    def image_for(instance_id): pass
    @api
    def _import_swebench(): pass
    @api
    def claude_bin(): pass
    @api
    def cli_version(binary): pass
    @api
    def make_runner(agent=None,scaffold=None,**kwargs): pass
    @api
    def _cleanup_containers(*_a): pass
    @api
    def _install_signal_cleanup(): pass

    class _Grader:
        def __init__(self,grading_path=None): pass
        def instance(self,instance_id): pass
        def eval_script(self,instance_id): pass
        def score(self,instance_id,model_patch,log_path): pass

    class ClaudeSwebenchRunner:
        NET_GUARD_PROBE=metadata["swebench"]["NET_GUARD_PROBE"]
        def __init__(self,agent="claude",model=None,grading_path=None,scaffold=None): pass
        def optimizer_call(self,prompt,model="agent"): pass
        def _preflight_optimizer_endpoint(self): pass
        def _segment(self,name,prompt,transcript_fh,exclude_tools,max_turns,guard,mcp_config=None): pass
        def _session_log_text(self,name): pass
        def _scaffold_context(self,name,task,run_dir_path): pass
        @property
        def NET_GUARD_SCRIPT(self): pass
        @property
        def NET_GUARD_LOG(self): pass
        def _answer_key_attempts(self,run_dir_path): pass
        @staticmethod
        def _net_guard_merge_program(entry): pass
        def _write_hook_entry(self,name,python,path): pass
        def _register_net_guard(self,name,python): pass
        def _write_run_config(self,run_dir_path,task,arm,injected,exclude_tools,max_turns,run_kwargs,guard): pass
        def _apply_scaffold(self,name,task,run_dir_path): pass
        def _reap_exited(self,name): pass
        def _stop(self,name): pass
        def _exec(self,name,script,timeout=300,workdir="/testbed"): pass
        def _env_activation_args(self,name): pass
        def _activate_project_env(self,name): pass
        def _env_activation_info(self,name): pass
        def _record_env_activation(self,run_dir_path,name,row): pass
        def _start(self,name,instance_id): pass
        def run_task(self,task,*,arm,injected,exclude_tools,run_dir_path,**run_kwargs): pass
        def _build_guard(self,run_kwargs): pass
        def _guard_row(self,guard): pass
        def _guard_continuation(self,guard,prompt): pass
        def _install_net_guard(self,name): pass
        def _install_answer_key_hosts(self,name): pass
        def _verify_net_guard(self,name,python): pass
        def _net_guard_blocks(self,name): pass
        def _graded_diff(self,name): pass
        def _patch_excludes(self): pass
        def _grade(self,name,instance_id,patch,run_dir_path): pass
        def _eval_exec_failed(self,instance_id,host_log,out,rc,stderr,phase): pass
        def _container_name(self,instance_id,arm,run_dir_path): pass
        def _row_from_segment(self,result): pass
        def _merge_segments(self,a,b): pass

    class CopilotSwebenchRunner(ClaudeSwebenchRunner):
        def __init__(self,agent="copilot",model=None,grading_path=None,scaffold=None): pass
        def optimizer_call(self,prompt,model="agent"): pass
        def _preflight_optimizer_endpoint(self): pass
        def _segment(self,name,prompt,transcript_fh,exclude_tools,max_turns,guard,mcp_config=None): pass
        def _merge_segments(self,a,b): pass
        def _register_net_guard(self,name,python): pass
        def _start(self,name,instance_id): pass
        def run_task(self,task,*,arm,injected,exclude_tools,run_dir_path,**run_kwargs): pass
        def _row_from_segment(self,result): pass

    class CodexSwebenchRunner(ClaudeSwebenchRunner):
        _guard_python=""
        def __init__(self,agent="codex",model=None,grading_path=None,scaffold=None): pass
        def optimizer_call(self,prompt,model="agent"): pass
        def _segment(self,name,prompt,transcript_fh,exclude_tools,max_turns,guard,mcp_config=None): pass
        def _merge_segments(self,a,b): pass
        def _register_net_guard(self,name,python): pass
        def _start(self,name,instance_id): pass
        def run_task(self,task,*,arm,injected,exclude_tools,run_dir_path,**run_kwargs): pass
        def _row_from_segment(self,result): pass
        def _write_config(self,name): pass
        def _apply_scaffold(self,name,task,run_dir_path): pass
        def _build_guard(self,run_kwargs): pass
        def _guard_row(self,guard): pass
        def _guard_continuation(self,guard,prompt): pass

    for cls in (_Grader,ClaudeSwebenchRunner,CopilotSwebenchRunner,CodexSwebenchRunner):
        cls.__qualname__=cls.__name__
        for descriptor in vars(cls).values():
            function=descriptor.fget if isinstance(descriptor,property) else descriptor.__func__ if isinstance(descriptor,staticmethod) else descriptor
            if inspect.isfunction(function): function.__qualname__=function.__qualname__.removeprefix("bind.<locals>.")
        setattr(module,cls.__name__,cls)
    function=provider_bindings(bridge)
    for agent,cls in (("claude",ClaudeSwebenchRunner),("copilot",CopilotSwebenchRunner),("codex",CodexSwebenchRunner)):
        fields=dict(metadata["swebench"]["profiles"][agent])
        for key in ("nav_tools","patch_excludes"): fields[key]=tuple(fields[key])
        def mcp_args(cfg): pass
        mcp_args=function(module,mcp_args,"webbench","scaffold_mcp_claude" if agent == "claude" else "scaffold_mcp_empty")
        cls.scaffold_profile=module.scaffolds_pkg.AgentProfile(**fields,mcp_args=mcp_args)
    module.RUNNERS=dict(claude=ClaudeSwebenchRunner,copilot=CopilotSwebenchRunner,codex=CodexSwebenchRunner)

    @declaration(codex)
    def network_block_toggles(): pass
    @declaration(codex)
    def config_toggles(capabilities): pass
    @declaration(codex)
    def config_toml(*,routing,model,effort,project="/testbed",capabilities=(),system_skills=None,
                    instructions=None,instructions_path=None,mcp_servers=None): pass
    @declaration(codex)
    def skills_config_entries(system_skills=None): pass
    @declaration(codex)
    def lean_instructions(base_text,capabilities): pass
    @declaration(codex)
    def _unwrap_shell(cmd): pass
    @declaration(codex)
    def rtk_compliance(transcript_path): pass
    @declaration(codex)
    def mcp_tool_calls(transcript_path,server=None): pass
    @declaration(codex)
    def container_mounts(vendor): pass
    @declaration(copilot)
    def container_mounts(binary): pass
    @declaration(copilot)
    def container_seed_setup(): pass

    task_bindings(bridge,module,container_helpers=container_helpers)
    setup_bindings(bridge,module)
    runner_bindings(bridge,module,container_helpers=container_helpers)
    return module
