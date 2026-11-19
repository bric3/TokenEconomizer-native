"""Signature-only WebBench fixtures; SWE-bench uses the production class assembly."""
def network_tool_block(): raise AssertionError('unbound network fixture')

class WebBenchRunner:
    def __init__(self,agent="claude",model=None,repo=None,scaffold=None): raise AssertionError("unbound web constructor")
    def _init_stream_state(self): raise AssertionError("unbound stream state")

class CopilotWebBenchRunner(WebBenchRunner):
    def __init__(self,agent="copilot",model=None,repo=None,scaffold=None): raise AssertionError("unbound copilot web constructor")

class CodexWebBenchRunner(WebBenchRunner):
    def __init__(self,agent="codex",model=None,repo=None,scaffold=None): raise AssertionError("unbound codex web constructor")
    def _probe_namespace_guard(self): raise AssertionError("unbound namespace probe")

def webbench_repo(): raise AssertionError("unbound web repo")
def node_bin(): raise AssertionError("unbound web node")
def webbench_task_timeout(env=None): raise AssertionError("unbound web timeout")
def webbench_undici_abort(env=None): raise AssertionError("unbound web abort timeout")
