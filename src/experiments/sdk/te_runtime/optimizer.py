"""Optimizer namespaces and host I/O; routing, calls and logging policy run in Rust."""
import json
import shutil
import subprocess
import sys
import tempfile
import time
import urllib

from .providers import bind as provider_bindings, routing
from .transport import checked
from .usage import bind as usage_bindings


def bind(bridge):
    namespaces=routing(bridge,lambda message:None)
    claude,copilot,codex=namespaces.claude,namespaces.copilot,namespaces.codex
    constants=checked(bridge.request("experiment.optimizer.constants",{}))
    for name,module in (("claude",claude),("copilot",copilot),("codex",codex)):
        vars(module).update(constants[name],json=json,shutil=shutil,subprocess=subprocess,
            sys=sys,tempfile=tempfile,time=time,urllib=urllib)
        module.pricing.effective_tokens=lambda rec:checked(bridge.request("pricing.effective_tokens",{"rec":rec}))
    claude._OPT_CTX={}
    claude._OPT_KINDS=tuple(map(tuple,claude._OPT_KINDS))
    for name in ("OPTIMIZER_DENY_TOOLS","OPTIMIZER_EXCLUDE_TOOLS","_FATAL_CONFIG_ERRORS"):
        setattr(copilot,name,tuple(getattr(copilot,name)))
    copilot.PROTECTED_TOOLS=frozenset(copilot.PROTECTED_TOOLS)
    usage=usage_bindings(bridge.request,lambda message:claude._log(message))
    for name in ("read_log_dir","sum_openai_usage"):
        setattr(copilot,name,getattr(usage.copilot,name))
    function=provider_bindings(bridge,shared=claude)
    def operation(module,agent,name=None):
        def declare(signature):
            signature.__qualname__=signature.__name__
            native=function(module,signature,agent,name or signature.__name__)
            setattr(module,signature.__name__,native)
            return native
        return declare
    claude_call=lambda:operation(claude,"claude")
    copilot_call=lambda name=None:operation(copilot,"copilot",name)
    codex_call=lambda name=None:operation(codex,"codex",name)

    @claude_call()
    def optimizer_log_path(): pass
    @claude_call()
    def _cell_of(run_dir_path): pass
    @claude_call()
    def set_optimizer_context(**fields): pass
    @claude_call()
    def optimizer_call_kind(prompt): pass
    @claude_call()
    def _log_optimizer_call(model,prompt,text,usage,stop_reason=None): pass
    @claude_call()
    def _log(msg): pass
    @claude_call()
    def preflight_optimizer_endpoint(*,configured_model,label,remedy,api_key="",bearer="",
                                    base_url=constants["claude"]["_ANTHROPIC_DIRECT"]): pass
    @claude_call()
    def anthropic_optimizer_call(prompt,*,model="agent",configured_model,max_tokens=8192,
                                api_key="",bearer="",base_url=constants["claude"]["_ANTHROPIC_DIRECT"]): pass
    @codex_call()
    def optimizer_call(prompt,*,configured_model,routing,max_tokens=16384,timeout=180): pass
    @codex_call()
    def preflight_optimizer(*,routing,configured_model): pass
    @codex_call()
    def _responses_text(data): pass
    @copilot_call()
    def optimizer_call(prompt,*,binary=None,model=None,timeout=180): pass
    @copilot_call()
    def copilot_bin(): pass
    @copilot_call()
    def copilot_cache(): pass
    @copilot_call()
    def copilot_config(): pass
    @copilot_call()
    def cli_version(binary): pass
    @copilot_call()
    def require_install(*,need_container_seed=False): pass
    @copilot_call()
    def run_env(extra=None): pass
    @copilot_call("invocation_build")
    def build_argv(prompt,*,log_dir,model=None,effort=None,exclude_tools=(),mcp_config=None): pass
    @copilot_call("invocation_fatal")
    def fatal_config_error(stderr_text): pass
    @codex_call("invocation_build")
    def build_argv(*,hook=False): pass
    @codex_call("invocation_resume")
    def build_resume_argv(session_id,*,model=None,effort=None,hook=False): pass
    @codex_call("invocation_env")
    def run_env(extra=None): pass

    return namespaces
