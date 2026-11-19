"""Signature-only fixture interfaces for production harness bindings; no reference imports."""
def answer_key_attempts(run_dir_path): raise AssertionError('unbound fixture API: .answer_key_attempts')
def guard_accepts(param): raise AssertionError('unbound fixture API: .guard_accepts')
def default_grading_path(): raise AssertionError('unbound fixture API: .default_grading_path')
def swebench_repo(): raise AssertionError('unbound fixture API: .swebench_repo')
def build_prompt(task, injected): raise AssertionError('unbound fixture API: .build_prompt')
def _run(cmd, timeout=120, check=False): raise AssertionError('unbound fixture API: ._run')
def env_activation_enabled(): raise AssertionError('unbound fixture API: .env_activation_enabled')
def network_tool_block(): raise AssertionError('unbound fixture API: .network_tool_block')
def _cpu_limit_args(): raise AssertionError('unbound fixture API: ._cpu_limit_args')
def choose_project_env(env_bins): raise AssertionError('unbound fixture API: .choose_project_env')
def activation_env(env_bin, container_path): raise AssertionError('unbound fixture API: .activation_env')
def image_for(instance_id): raise AssertionError('unbound fixture API: .image_for')
def _import_swebench(): raise AssertionError('unbound fixture API: ._import_swebench')
def claude_bin(): raise AssertionError('unbound fixture API: .claude_bin')
def cli_version(binary): raise AssertionError('unbound fixture API: .cli_version')

class _Grader:
    def __init__(self, grading_path=None): raise AssertionError('unbound fixture API: _Grader.__init__')
    def instance(self, instance_id): raise AssertionError('unbound fixture API: _Grader.instance')
    def eval_script(self, instance_id): raise AssertionError('unbound fixture API: _Grader.eval_script')
    def score(self, instance_id, model_patch, log_path): raise AssertionError('unbound fixture API: _Grader.score')

class ClaudeSwebenchRunner:
    NET_GUARD_PROBE="echo https://github.com/probe"
    @property
    def NET_GUARD_SCRIPT(self): raise AssertionError("unbound guard script")
    @property
    def NET_GUARD_LOG(self): raise AssertionError("unbound guard log")
    def _answer_key_attempts(self,run_dir_path): raise AssertionError("unbound attempts")
    @staticmethod
    def _net_guard_merge_program(entry): raise AssertionError("unbound merge program")
    def _write_hook_entry(self,name,python,path): raise AssertionError("unbound hook entry")
    def _register_net_guard(self,name,python): raise AssertionError("unbound guard registration")
    def __init__(self, agent="claude", model=None, grading_path=None, scaffold=None): raise AssertionError('unbound fixture API: ClaudeSwebenchRunner.__init__')
    def _write_run_config(self, run_dir_path, task, arm, injected, exclude_tools,
                              max_turns, run_kwargs, guard): raise AssertionError('unbound fixture API: ClaudeSwebenchRunner._write_run_config')
    def _apply_scaffold(self, name, task, run_dir_path): raise AssertionError('unbound fixture API: ClaudeSwebenchRunner._apply_scaffold')
    def _reap_exited(self, name): raise AssertionError('unbound fixture API: ClaudeSwebenchRunner._reap_exited')
    def _stop(self, name): raise AssertionError('unbound fixture API: ClaudeSwebenchRunner._stop')
    def _exec(self, name, script, timeout=300, workdir="/testbed"): raise AssertionError('unbound fixture API: ClaudeSwebenchRunner._exec')
    def _env_activation_args(self, name): raise AssertionError('unbound fixture API: ClaudeSwebenchRunner._env_activation_args')
    def _activate_project_env(self, name): raise AssertionError('unbound fixture API: ClaudeSwebenchRunner._activate_project_env')
    def _env_activation_info(self, name): raise AssertionError('unbound fixture API: ClaudeSwebenchRunner._env_activation_info')
    def _record_env_activation(self, run_dir_path, name, row): raise AssertionError('unbound fixture API: ClaudeSwebenchRunner._record_env_activation')
    def _start(self, name, instance_id): raise AssertionError('unbound fixture API: ClaudeSwebenchRunner._start')
    def run_task(self, task, *, arm, injected, exclude_tools, run_dir_path, **run_kwargs): raise AssertionError('unbound fixture API: ClaudeSwebenchRunner.run_task')
    def _build_guard(self, run_kwargs): raise AssertionError('unbound fixture API: ClaudeSwebenchRunner._build_guard')
    def _guard_row(self, guard): raise AssertionError('unbound fixture API: ClaudeSwebenchRunner._guard_row')
    def _guard_continuation(self, guard, prompt): raise AssertionError('unbound fixture API: ClaudeSwebenchRunner._guard_continuation')
    def _install_net_guard(self, name): raise AssertionError('unbound fixture API: ClaudeSwebenchRunner._install_net_guard')
    def _install_answer_key_hosts(self, name): raise AssertionError('unbound fixture API: ClaudeSwebenchRunner._install_answer_key_hosts')
    def _verify_net_guard(self, name, python): raise AssertionError('unbound fixture API: ClaudeSwebenchRunner._verify_net_guard')
    def _net_guard_blocks(self, name): raise AssertionError('unbound fixture API: ClaudeSwebenchRunner._net_guard_blocks')
    def _graded_diff(self, name): raise AssertionError('unbound fixture API: ClaudeSwebenchRunner._graded_diff')
    def _patch_excludes(self): raise AssertionError('unbound fixture API: ClaudeSwebenchRunner._patch_excludes')
    def _grade(self, name, instance_id, patch, run_dir_path): raise AssertionError('unbound fixture API: ClaudeSwebenchRunner._grade')
    def _eval_exec_failed(self, instance_id, host_log, out, rc, stderr, phase): raise AssertionError('unbound fixture API: ClaudeSwebenchRunner._eval_exec_failed')
    def _container_name(self, instance_id, arm, run_dir_path): raise AssertionError('unbound fixture API: ClaudeSwebenchRunner._container_name')
    def _row_from_segment(self, result): raise AssertionError('unbound fixture API: ClaudeSwebenchRunner._row_from_segment')
    def _merge_segments(self,a,b): raise AssertionError("unbound fixture API: _merge_segments")

class CopilotSwebenchRunner(ClaudeSwebenchRunner):
    def _register_net_guard(self,name,python): raise AssertionError("unbound copilot registration")
    def __init__(self, agent="copilot", model=None, grading_path=None, scaffold=None): raise AssertionError('unbound fixture API: CopilotSwebenchRunner.__init__')
    def _start(self, name, instance_id): raise AssertionError('unbound fixture API: CopilotSwebenchRunner._start')
    def run_task(self, task, *, arm, injected, exclude_tools, run_dir_path, **run_kwargs): raise AssertionError('unbound fixture API: CopilotSwebenchRunner.run_task')
    def _row_from_segment(self, result): raise AssertionError('unbound fixture API: CopilotSwebenchRunner._row_from_segment')

class CodexSwebenchRunner(ClaudeSwebenchRunner):
    _guard_python=""
    def _register_net_guard(self,name,python): raise AssertionError("unbound codex registration")
    def __init__(self, agent="codex", model=None, grading_path=None, scaffold=None): raise AssertionError('unbound fixture API: CodexSwebenchRunner.__init__')
    def _start(self, name, instance_id): raise AssertionError('unbound fixture API: CodexSwebenchRunner._start')
    def run_task(self, task, *, arm, injected, exclude_tools, run_dir_path, **run_kwargs): raise AssertionError('unbound fixture API: CodexSwebenchRunner.run_task')
    def _row_from_segment(self, result): raise AssertionError('unbound fixture API: CodexSwebenchRunner._row_from_segment')
    def _write_config(self, name): raise AssertionError('unbound fixture API: CodexSwebenchRunner._write_config')
    def _apply_scaffold(self, name, task, run_dir_path): raise AssertionError('unbound fixture API: CodexSwebenchRunner._apply_scaffold')
    def _build_guard(self, run_kwargs): raise AssertionError('unbound fixture API: CodexSwebenchRunner._build_guard')
    def _guard_row(self, guard): raise AssertionError('unbound fixture API: CodexSwebenchRunner._guard_row')
    def _guard_continuation(self, guard, prompt): raise AssertionError('unbound fixture API: CodexSwebenchRunner._guard_continuation')

class CodexInterface:
    @staticmethod
    def network_block_toggles(): raise AssertionError('unbound fixture API: .network_block_toggles')
    @staticmethod
    def config_toggles(capabilities): raise AssertionError('unbound fixture API: .config_toggles')
    @staticmethod
    def config_toml(*, routing, model, effort, project="/testbed", capabilities=(),
                    system_skills=None, instructions=None, instructions_path=None,
                    mcp_servers=None): raise AssertionError('unbound fixture API: .config_toml')
    @staticmethod
    def skills_config_entries(system_skills=None): raise AssertionError('unbound fixture API: .skills_config_entries')
    @staticmethod
    def lean_instructions(base_text, capabilities): raise AssertionError('unbound fixture API: .lean_instructions')
    @staticmethod
    def _unwrap_shell(cmd): raise AssertionError('unbound fixture API: ._unwrap_shell')
    @staticmethod
    def rtk_compliance(transcript_path): raise AssertionError('unbound fixture API: .rtk_compliance')
    @staticmethod
    def mcp_tool_calls(transcript_path, server=None): raise AssertionError('unbound fixture API: .mcp_tool_calls')
    @staticmethod
    def container_mounts(vendor): raise AssertionError('unbound fixture API: .container_mounts')

class CopilotInterface:
    @staticmethod
    def copilot_cache(): raise AssertionError('unbound fixture API: .copilot_cache')
    @staticmethod
    def copilot_config(): raise AssertionError('unbound fixture API: .copilot_config')
    @staticmethod
    def container_mounts(binary): raise AssertionError('unbound fixture API: .container_mounts')
    @staticmethod
    def container_seed_setup(): raise AssertionError('unbound fixture API: .container_seed_setup')

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
