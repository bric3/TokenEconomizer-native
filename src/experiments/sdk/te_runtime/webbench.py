"""Built-in WebBench namespaces and signatures; production policy executes in Rust."""
import glob
import hashlib
import inspect
import json
import os
import queue
import re
import shutil
import signal
import subprocess
import sys
import threading
import time
from pathlib import Path
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from types import ModuleType

from .swebench import bind as swebench_bindings
from .swebench_task import bind_network
from .runner_setup import bind as setup_bindings
from .webbench_calls import bind as call_bindings
from .transport import checked


def bind(bridge,*,shared=None):
    if shared is None: shared=swebench_bindings(bridge)
    module=ModuleType(__name__)
    module.__file__=__file__
    for name in ('EFFORT_LEVELS','PROTECTED_TOOLS','_ANTHROPIC_DIRECT','_log','answer_key_attempts',
                 '_merge_usage','_row_from_result','anthropic_optimizer_call','preflight_optimizer_endpoint',
                 'resolve_anthropic_key','claude_bin','cli_version','guard_accepts','set_optimizer_context',
                 '_cell_of','_UsageRecovery','agent_credential_env','scaffolds_pkg','pricing','NudgeGuard',
                 'copilot','codex','codex_spiral','net_guard'):
        setattr(module,name,getattr(shared,name))
    vars(module).update(glob=glob,hashlib=hashlib,inspect=inspect,json=json,os=os,queue=queue,re=re,
        shutil=shutil,signal=signal,subprocess=subprocess,sys=sys,threading=threading,time=time,
        Path=Path,ThreadingHTTPServer=ThreadingHTTPServer,_LIVE_SESSIONS=set())
    metadata=checked(bridge.request('experiment.webbench.constants',{}))
    classes=metadata.pop('classes')
    module._ATTEMPT_RE=re.compile(metadata.pop('attempt_pattern'))
    for name,value in metadata.items():
        setattr(module,name,tuple(value) if name == '_INIT_FORMAT_LINES' else frozenset(value) if isinstance(value,list) else value)
    def api(signature):
        signature.__qualname__=signature.__name__
        signature.__module__=module.__name__
        setattr(module,signature.__name__,signature)
        return signature
    # Class cells let Rust resolve the original super() target through the SDK.
    @api
    def _cleanup_sessions(*_a): pass
    @api
    def _install_signal_cleanup(): pass
    @api
    def _session_log_text_host(home): pass
    @api
    def webbench_repo(): pass
    @api
    def node_bin(): pass
    @api
    def _load_ignore_globs(repo, project_dir): pass
    @api
    def _ancestors(rel): pass
    @api
    def _ignored(rel, globs): pass
    @api
    def webbench_undici_abort(env=None): pass
    @api
    def webbench_undici_env(env=None, repo=None): pass
    @api
    def _shq(s): pass
    @api
    def webbench_task_timeout(env=None): pass
    @api
    def network_tool_block(): pass
    class _AgentHandler(BaseHTTPRequestHandler):
        def do_POST(self): pass
        def log_message(self, *_a): pass
    class _EvalSession:
        def __init__(self, project, repo, session_dir, *, retry, tasks, node, reply_timeout, log_level="info", max_workers=4): pass
        def _full_task_ids(self): pass
        def _read_stdout(self): pass
        def next_event(self, kind, timeout): pass
        def next_request(self, timeout): pass
        def close(self): pass
    class WebBenchRunner:
        def __init__(self, agent="claude", model=None, repo=None, scaffold=None): pass
        def _preflight_optimizer_endpoint(self): pass
        def _init_stream_state(self): pass
        def optimizer_call(self, prompt, model="agent"): pass
        def begin_stream(self, tasks, *, results_root=None, repo=None, **_info): pass
        def end_stream(self): pass
        def session_for(self, project): pass
        def close_session(self): pass
        def agent_home(self, run_dir_path): pass
        def scaffold_profile_for(self, home): pass
        def _scaffold_context(self, workspace, home, task, run_dir_path): pass
        @staticmethod
        def scaffold_bin_dir(home): pass
        def scaffold_exec_env(self, run_dir_path): pass
        def prepare_agent_home(self, home): pass
        @staticmethod
        def _net_guard_log(home): pass
        def _net_guard_blocks(self, run_dir_path): pass
        def net_guard_settings_path(self, home, run_dir_path): pass
        def net_guard_hook_entry(self, command): pass
        def _install_net_guard(self, home, run_dir_path=None): pass
        def _verify_net_guard(self, home): pass
        def _apply_scaffold(self, workspace, home, task, run_dir_path): pass
        def scaffold_answer_excludes(self): pass
        def _segment(self, workspace, prompt, transcript_fh, exclude_tools, max_turns, guard): pass
        def _materialize(self, workspace, files): pass
        def _collect(self, workspace, request_files, project_dir): pass
        def run_task(self, task, *, arm, injected, exclude_tools, run_dir_path, **run_kwargs): pass
        def _merge_segments(self, a, b): pass
        def _row_from_segment(self, result): pass
        def _segment_usage(self, result): pass
        def _make_guard(self, run_kwargs): pass
        def _write_run_config(self, run_dir_path, task, arm, injected, exclude_tools, max_turns, run_kwargs, sess, project): pass
    @api
    def strip_transport_instructions(description): pass
    @api
    def build_prompt(task, injected, request, project, workspace="."): pass
    class CopilotWebBenchRunner(WebBenchRunner):
        def __init__(self, agent="copilot", model=None, repo=None, scaffold=None): pass
        def _preflight_optimizer_endpoint(self): pass
        def optimizer_call(self, prompt, model="agent"): pass
        def scaffold_profile_for(self, home): pass
        def scaffold_exec_env(self, run_dir_path): pass
        def net_guard_settings_path(self, home, run_dir_path): pass
        def net_guard_hook_entry(self, command): pass
        def prepare_agent_home(self, home): pass
        def scaffold_copilot_home(self, run_dir_path): pass
        def _segment(self, workspace, prompt, transcript_fh, exclude_tools, max_turns, guard): pass
        def _merge_segments(self, a, b): pass
        def _segment_usage(self, result): pass
        def _row_from_segment(self, result): pass
        def run_task(self, task, *, arm, injected, exclude_tools, run_dir_path, **run_kwargs): __class__
    class CodexWebBenchRunner(WebBenchRunner):
        def __init__(self, agent="codex", model=None, repo=None, scaffold=None): pass
        def _probe_namespace_guard(self): pass
        def _hosts_file(self, run_dir_path): pass
        def net_guard_settings_path(self, home, run_dir_path): pass
        def _install_net_guard(self, home, run_dir_path=None): __class__
        def _preflight_optimizer_endpoint(self): pass
        def optimizer_call(self, prompt, model="agent"): pass
        def agent_home(self, run_dir_path): pass
        def scaffold_profile_for(self, home): pass
        def scaffold_exec_env(self, run_dir_path): pass
        def _write_config(self, home, workspace): pass
        def _segment(self, workspace, prompt, transcript_fh, exclude_tools, max_turns, guard): pass
        def _merge_segments(self, a, b): pass
        def _segment_usage(self, result): pass
        def _row_from_segment(self, result): pass
        def _make_guard(self, run_kwargs): pass
        def run_task(self, task, *, arm, injected, exclude_tools, run_dir_path, **run_kwargs): __class__
        def _prune_agent_home(self, run_dir_path): pass
    @api
    def make_runner(agent=None, scaffold=None, **kwargs): pass

    for cls in (_AgentHandler,_EvalSession,WebBenchRunner,CopilotWebBenchRunner,CodexWebBenchRunner):
        cls.__qualname__=cls.__name__
        for name,value in classes.get(cls.__name__,{}).items():
            setattr(cls,name,tuple(value) if isinstance(value,list) else value)
        for descriptor in vars(cls).values():
            function=descriptor.__func__ if isinstance(descriptor,staticmethod) else descriptor
            if inspect.isfunction(function): function.__qualname__=function.__qualname__.removeprefix('bind.<locals>.')
        setattr(module,cls.__name__,cls)
    module.RUNNERS=dict(claude=WebBenchRunner,copilot=CopilotWebBenchRunner,codex=CodexWebBenchRunner)
    call_bindings(bridge,module)
    setup_bindings(bridge,module,web=True)
    bind_network(bridge,module)
    return module
