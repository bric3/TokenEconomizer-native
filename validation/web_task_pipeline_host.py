"""Actual HTTP, process and filesystem bindings for an offline Web-Bench task."""
import json
import os
from pathlib import Path
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
from te_harness import swebench
if "--native" in sys.argv:
    from native_experiment_usage import install as install_usage
    install_usage()
    from native_experiment_task import install as install_task
    install_task()
    from native_experiment_providers import install
    install()
from te_harness import webbench as wb
if "--native" in sys.argv:
    install(wb)
    from native_experiment_net_guard import install as install_guard
    install_guard()
    from native_plugin import native_cli_process
    wb.subprocess.run = native_cli_process

binary,repository,destination = sys.argv[1:4]
run = Path(destination)
operator_config = run.parent/"operator-config"
operator_config.mkdir()
(operator_config/"settings.json").write_text('{"fixture":"operator"}',encoding="utf-8")
os.environ["COPILOT_HOME"] = str(operator_config)
agent = os.environ.get("TE_FIXTURE_AGENT","claude")
task = {"instance_id":"init","repo":"@web-bench/project","problem_statement":"implement"}
wb._log = lambda value:None
session = wb._EvalSession("project",repository,str(run/"evaluator"),retry=2,tasks=[task],node=binary,reply_timeout=5)
base = {"claude":wb.WebBenchRunner,"copilot":wb.CopilotWebBenchRunner,"codex":wb.CodexWebBenchRunner}[agent]
class Scaffold:
    name="fixture"
    patch_excludes=("SCAFFOLD.md","*.fixture")
    def setup(self,context):
        expected=["--mcp-config","/fixture/mcp.json"] if agent == "claude" else []
        assert context.profile.mcp_args("/fixture/mcp.json") == expected
        staged=operator_config/"fixture-tool"
        staged.write_text("fixture scaffold binary",encoding="utf-8")
        installed=Path(context.profile.bin_dir,"fixture-helper")
        result=context.copy_in(str(staged),str(installed))
        assert result.returncode == 0 and installed.read_text(encoding="utf-8") == "fixture scaffold binary"
        assert installed.stat().st_ino == staged.stat().st_ino
        result=context.exec_('printf \'%s\' "$SCAFFOLD_VALUE" > SCAFFOLD.md; cat SCAFFOLD.md',env={"SCAFFOLD_VALUE":"installed"})
        assert result.returncode == 0 and result.stdout == "installed"
        result=context.host_run(["/bin/echo","fixture-host"])
        assert result.returncode == 0 and result.stdout == "fixture-host\n"
        return {"fixture_scaffold_installed":True}
class Runner(base):
    def session_for(self,project):
        assert project == "project"
        return session
runner = object.__new__(Runner)
for key,value in {"model":"gpt-5.4","agent":agent,"effort":"high","cli_version":"fixture-cli","agent_bin":os.environ["TE_FIXTURE_AGENT_BIN"],"claude":os.environ["TE_FIXTURE_AGENT_BIN"],"copilot":os.environ["TE_FIXTURE_AGENT_BIN"],"api_key":"fixture-key","_net_guard_armed":False,"routing_base":"https://fixture.invalid","routing":{"base_url":"https://fixture.invalid"},"base_prompt":"# Using skills\nFixture skills\n# Task\nImplement fixture task\n","system_skills":["fixture-skill"],"optimizer_model":"fixture-optimizer","protected_tools":frozenset({"Bash"}),"repo":repository,"_stream_broken":False,"retry":2,"start_timeout":5,"verdict_timeout":5,"task_timeout":240,"max_workers":1,"node":binary,"keep_workspaces":True,"scaffold":Scaffold(),"scaffold_name":"fixture","_lean_instructions_tokens":8,"answer_key_ns":False}.items(): setattr(runner,key,value)
try:
    row,out = runner.run_task(task,arm="baseline",injected=None,exclude_tools=[],run_dir_path=str(run),codex_config_capabilities=["skills"])
    assert out == str(run) and row["resolved"] is True and row["resolved_p1"] is True and row["resolved_p2"] is True
    assert row["webbench_attempts"] == 1 and row["webbench_answered_files"] == [["src/a.js"]]
    assert row["net_guard_installed"] is True and row["net_guard_blocks"] == 0
    assert row["fixture_scaffold_installed"] is True and row["scaffold"] == "fixture"
    assert Path(run,"ws-attempt1/SCAFFOLD.md").read_text(encoding="utf-8") == "installed"
    assert row["input_tokens"] == 10 and row["cache_read_input_tokens"] == 20 and row["cache_creation_input_tokens"] == 30 and row["output_tokens"] == 4
    transcript = Path(run,"transcript.jsonl").read_text(encoding="utf-8")
    assert transcript.startswith('{"type":') and len(transcript.splitlines()) == 2
    assert Path(run,"ws-attempt1/src/a.js").read_text(encoding="utf-8") == "fixture answer"
    request = json.loads(Path(run,"agent_request_1.json").read_text(encoding="utf-8"))
    assert request == {"type":"task","task":"init","error":None,"files":["src/a.js"]}
    config = json.loads(Path(run,"run_config.json").read_text(encoding="utf-8"))
    assert config["agent"] == agent and config["webbench"]["evaluator_config"] == session.config_path
    assert config["spiral"]["guard_per_attempt"] is True and config["tools"]["protected"] == ["Bash"]
    if agent == "copilot":
        assert "_debug_log" not in row and '"prompt_tokens":60' in Path(run,"copilot_debug.log").read_text(encoding="utf-8")
        settings=json.loads(Path(run,".copilot-home/settings.json").read_text(encoding="utf-8"))
        assert settings["fixture"] == "operator" and settings["hooks"]["preToolUse"][0]["matcher"] == "bash"
    if agent == "codex":
        assert row["turns_basis"] == "codex_steps" and row["codex_config_capabilities"] == ["skills"]
        home=Path(run,".codex-home")
        instructions=home.joinpath("instructions.md").read_text(encoding="utf-8")
        assert row["codex_lean_instructions_tokens"] == len(instructions)//4 > 0 and row["answer_key_hosts_blocked"] is False
        settings=tomllib.loads(home.joinpath("config.toml").read_text(encoding="utf-8"))
        assert settings["model_instructions_file"] == str(home/"instructions.md")
        assert settings["projects"][str(run/"ws-attempt1")]["trust_level"] == "trusted"
        assert "# Using skills" not in instructions and "Implement fixture task" in instructions
    assert operator_config.joinpath("settings.json").read_text(encoding="utf-8") == '{"fixture":"operator"}'
    session._reader.join(5)
    assert not session._reader.is_alive()
    assert session.proc.wait(timeout=5) == 0
finally:
    session.close()
    if session.proc.poll() is None: session.proc.kill();session.proc.wait()
    if session.proc.stdout is not None: session.proc.stdout.close()
    session._reader.join(5);session._http_thread.join(5)
assert session.finished and session._log_fh.closed
if "--native" in sys.argv:
    import native_plugin
    for name in ("run_task","write_run_config","build_prompt","strip_transport","eval_start","http_post","materialize","collect","load_ignore_globs","ancestors","next_event","next_request","_read_stdout","eval_close"):
        assert native_plugin.CALLS["experiment.swebench.provider.webbench."+name] > 0,"pipeline bypassed native "+name
    for name in (["copilot_task"] if agent == "copilot" else ["codex_task","prune_home"] if agent == "codex" else []):
        assert native_plugin.CALLS["experiment.swebench.provider.webbench."+name] > 0,"pipeline bypassed native "+name
    assert native_plugin.CALLS["experiment.swebench.provider.webbench."+("guard_codex" if agent == "codex" else "guard_claude")] > 0
    assert native_plugin.CALLS["experiment.swebench.provider.webbench.segment_"+agent] == 1
    if agent != "claude": assert native_plugin.CALLS["experiment.swebench.provider."+agent+".invocation_build"] == 1
    if agent == "copilot": assert native_plugin.CALLS["experiment.swebench.provider.copilot.invocation_fatal"] == 1
    if agent == "codex": assert native_plugin.CALLS["experiment.swebench.provider.codex.invocation_env"] == 1
    for name in ("network_install","network_verify","network_blocks"):
        assert native_plugin.CALLS["experiment.swebench.provider.webbench."+name] > 0,"pipeline bypassed native "+name
    assert native_plugin.CALLS["experiment.net_guard.process"] == 1
    for name in ("scaffold_apply","scaffold_context","scaffold_exec","scaffold_copy","scaffold_place","scaffold_host","scaffold_excludes","scaffold_profile_"+agent,"scaffold_mcp_claude" if agent == "claude" else "scaffold_mcp_empty"):
        assert native_plugin.CALLS["experiment.swebench.provider.webbench."+name] > 0,"pipeline bypassed native "+name
    for name in ("agent_home_codex" if agent == "codex" else "agent_home_claude","agent_bin_dir","agent_row_"+agent,"agent_usage_"+agent):
        assert native_plugin.CALLS["experiment.swebench.provider.webbench."+name] > 0,"pipeline bypassed native "+name
    if agent == "copilot":
        assert native_plugin.CALLS["experiment.swebench.provider.webbench.agent_copilot_home"] > 0
        assert native_plugin.CALLS["experiment.swebench.provider.copilot.copilot_config"] > 0
    if agent == "codex":
        assert native_plugin.CALLS["experiment.swebench.provider.webbench.agent_config_codex"] == 1
        assert native_plugin.CALLS["experiment.swebench.codex.config_toml"] == 1
        assert native_plugin.CALLS["experiment.swebench.codex.lean_instructions"] == 1
print("Offline Web-Bench task passed: HTTP request, task policy, workspace edit, usage reply, verdict, transcript and resource cleanup",flush=True)
