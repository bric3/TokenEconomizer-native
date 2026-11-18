"""Object, process, clock and file bindings for the segment comparison corpus."""
import builtins
import glob
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT/"reference/upstream"), str(ROOT/"reference/upstream/experiments")]
from te_harness import swebench
if "--native" in sys.argv:
    from native_experiment_providers import install
    install()
from te_harness import webbench as wb
if "--native" in sys.argv:
    install(wb)
    previous = wb.WebBenchRunner._segment, wb.CopilotWebBenchRunner._segment, wb.CodexWebBenchRunner._segment, wb._session_log_text_host
    install(wb)
    assert previous == (wb.WebBenchRunner._segment, wb.CopilotWebBenchRunner._segment, wb.CodexWebBenchRunner._segment, wb._session_log_text_host)

for line in sys.stdin:
    request = json.loads(line)
    calls, counts, managers = [], {}, []
    absent = object()
    saved = {name: getattr(wb, name, absent) for name in ("os", "time", "signal", "subprocess", "json", "glob", "open", "copilot", "codex", "net_guard", "network_tool_block", "agent_credential_env", "_UsageRecovery", "_session_log_text_host", "_log", "_shq", "Exception", "OSError", "ProcessLookupError", "PermissionError")}

    class Token:
        def __init__(self, name): self.name = name
        def __bool__(self): record("bool:"+self.name);return request.get("booleans", {}).get(self.name, True)
        def __str__(self): record("str:"+self.name);return "opaque "+self.name
        def __repr__(self): record("repr:"+self.name);return "opaque "+self.name
        def __format__(self, spec): record("format:"+self.name, spec);return "opaque "+self.name
        def __fspath__(self): record("fspath:"+self.name);return request.get("path", "/run/work space")
        def __iter__(self): record("iter:"+self.name);return iter(request.get("iterables", {}).get(self.name, ["Bash", "WebFetch"]))
        def __contains__(self, value): record("contains:"+self.name, value);return False
        def __ne__(self, value): record("ne:"+self.name, value);return request.get("not_equal", False)
        def __eq__(self, value): record("eq:"+self.name, value);return False

    def encode(value):
        if isinstance(value, Token): return {"ref": value.name}
        if isinstance(value, (list, tuple)): return [encode(item) for item in value]
        if isinstance(value, dict): return {key: encode(item) for key, item in dict.items(value)}
        if isinstance(value, type): return value.__name__
        return value

    class DetailError(Exception):
        def __str__(self): record("error.str", type(sys.exception()).__name__ if sys.exception() else None);return "segment fixture é😀"

    def record(name, args=None):
        calls.append([name, encode(args)])
        counts[name] = counts.get(name, 0)+1
        if (request.get("fail") == name and counts[name] == request.get("fail_at", 1)) or name in request.get("failures", {}):
            kind = request.get("failures", {}).get(name, request.get("error", "RuntimeError"))
            if kind == "DetailError": raise DetailError()
            if kind == "TimeoutExpired": raise subprocess.TimeoutExpired("agent", 60)
            if kind.startswith("Shadow"): raise type(kind.removeprefix("Shadow"), (Exception,), {})("fixture failure")
            raise getattr(builtins, kind)("fixture failure")

    class Namespace:
        def __init__(self, tag, **values): self.__dict__.update(values);self.tag = tag
        def __getattribute__(self, name):
            if name not in ("tag", "__dict__", "__class__"): record(object.__getattribute__(self, "tag")+"."+name)
            return object.__getattribute__(self, name)

    def callback(name, value=None, implementation=None):
        def call(*args, **kwargs):
            record(name, [args, list(kwargs), kwargs])
            return implementation(*args, **kwargs) if implementation else value
        return call

    class Mapping(dict):
        def __getattribute__(self, name):
            if name in ("get", "update"): record(object.__getattribute__(self, "tag")+"."+name+"_function")
            return dict.__getattribute__(self, name)
        def get(self, *args): record(self.tag+".get", args);return dict.get(self, *args)
        def __getitem__(self, key): record(self.tag+".item", key);return dict.__getitem__(self, key)
        def __bool__(self): record(self.tag+".bool");return dict.__len__(self) != 0
        def __setitem__(self, key, value): record(self.tag+".put", [key, value]);return dict.__setitem__(self, key, value)

    def mapping(tag, value):
        result = Mapping(value);result.tag = tag;return result

    class Pipe:
        def __init__(self, name): self.name, self.written, self.closed = name, [], False
        def write(self, value): record(self.name+".write", value);self.written.append(value)
        def flush(self): record(self.name+".flush")
        def close(self): record(self.name+".close");self.closed = True
        def read(self): record(self.name+".read");return request.get("stderr", "")
        def __iter__(self): record(self.name+".iter");return iter(request.get("lines", ["  ", " bad json \n", '{"type":"result","num_turns":2}\n']))

    class Process:
        def __init__(self): self.stdin, self.stdout, self.stderr, self.pid = Pipe("stdin"), Pipe("stdout"), Pipe("stderr"), 1234
        def __getattribute__(self, name):
            if name in ("stdin", "stdout", "stderr", "pid", "kill", "wait"): record("proc."+name)
            return object.__getattribute__(self, name)
        def kill(self): record("kill")
        def wait(self, **kwargs): record("wait", [list(kwargs), kwargs]);return 0

    process, transcript = Process(), Pipe("transcript")
    launched = []
    def popen(*args, **kwargs): record("popen", [args, list(kwargs), kwargs]);launched.append([args, kwargs]);return process

    class Guard:
        def __getattribute__(self, name):
            if name in ("segment", "monitor"): record("guard."+name)
            return object.__getattribute__(self, name)
        segment = 3
        def __bool__(self): record("guard.bool");return False
        def monitor(self, value): record("monitor", value);return counts["monitor"] >= request.get("stop_after", 999)

    class Recovery:
        def __init__(self): record("recovery")
        def __getattribute__(self, name):
            if name in ("observe", "synthesize"): record("recovery."+name)
            return object.__getattribute__(self, name)
        def observe(self, event): record("observe", event)
        def synthesize(self, reason, logs): record("synthesize", [reason, logs]);return {"recovered": reason, "logs": logs}

    class Manager:
        def __init__(self, path): self.path, self.closed = path, False
        def __enter__(self): record("enter", self.path);return self
        def __exit__(self, kind, error, tb):
            record("exit", [self.path, kind.__name__ if kind else None, sys.exception() is error, error is None or error.__traceback__ is tb])
            self.closed = True
            return request.get("suppress", False)
        def read(self): record("read", self.path);return request.get("contents", {}).get(self.path, self.path+" contents")

    def open_file(path, **kwargs):
        record("open", [path, list(kwargs), kwargs])
        if request.get("real_files"): return builtins.open(path, **kwargs)
        manager = Manager(path);managers.append(manager);return manager

    agent = request.get("agent", "claude")
    cls = {"claude": wb.WebBenchRunner, "copilot": wb.CopilotWebBenchRunner, "codex": wb.CodexWebBenchRunner}[agent]
    tracked = ("claude", "copilot", "agent_bin", "model", "effort", "protected_tools", "api_key", "task_timeout", "_net_guard_armed", "answer_key_ns", "agent_home", "_net_guard_log", "scaffold_bin_dir", "scaffold_copilot_home", "_write_config", "_hosts_file")
    class Runner(cls):
        def __getattribute__(self, name):
            if name in tracked: record("get:"+name)
            return super().__getattribute__(name)
        agent_home = staticmethod(callback("agent_home", "/run/agent-home"))
        _net_guard_log = staticmethod(callback("guard_log", "/run/guard.jsonl"))
        scaffold_bin_dir = staticmethod(callback("bin_dir", "/run/bin"))
        scaffold_copilot_home = staticmethod(callback("copilot_home", "/run/copilot-home"))
        _write_config = staticmethod(callback("config"))
        _hosts_file = staticmethod(callback("hosts_file", "/run/hosts 'quoted'"))

    runner = object.__new__(Runner)
    for name, value in {"claude": "/bin/claude", "copilot": "/bin/copilot", "agent_bin": "/bin/codex", "model": "fixture-model", "effort": "high", "protected_tools": ["Bash"], "api_key": "fixture-key", "task_timeout": 10, "_net_guard_armed": True, "answer_key_ns": False, **request.get("fields", {})}.items(): object.__setattr__(runner, name, value)
    for name in request.get("missing", []): object.__delattr__(runner, name)
    inputs = {"workspace": request.get("workspace", "/run/work space"), "prompt": request.get("prompt", "fix é😀\n"), "exclude_tools": request.get("exclude_tools", ["Bash", "Read", "WebFetch", "Read"]), "max_turns": request.get("max_turns", 8), "home": request.get("home", "/run/home")}
    if isinstance(inputs["prompt"], dict): inputs["prompt"] = mapping("resume", inputs["prompt"])
    usage = mapping("usage", request.get("usage", {"num_turns": 2, "input_tokens": 11}))
    raw_argv = request.get("argv", ["agent", "exec", "-C", "/testbed", "-"])
    for name in request.get("objects", []):
        if name in inputs: inputs[name] = Token(name)
        elif name == "argv": raw_argv[2] = Token(name)
        else: object.__setattr__(runner, name, Token(name))
    def tick(): record("clock");return request.get("times", [0, 1, 2, 3])[min(counts["clock"]-1, len(request.get("times", [0, 1, 2, 3]))-1)]
    def log(value): record("log", [value, type(sys.exception()).__name__ if sys.exception() else None])
    try:
        paths = Namespace("path", dirname=callback("dirname", implementation=os.path.dirname), basename=callback("basename", implementation=os.path.basename), join=callback("join", implementation=os.path.join))
        wb.os = Namespace("os", path=paths, environ=request.get("env", {"HOME": "/operator", "PATH": "/usr/bin", "KEEP": "yes"}), pathsep=os.pathsep, makedirs=callback("mkdir"), killpg=callback("killpg"), getpgid=callback("getpgid", 1234))
        wb.time = Namespace("time", time=tick, sleep=callback("sleep"))
        wb.signal = Namespace("signal", SIGTERM=15, SIGKILL=9)
        wb.subprocess = Namespace("subprocess", Popen=popen, PIPE=-1, DEVNULL=-3, TimeoutExpired=getattr(builtins, request["timeout_type"]) if "timeout_type" in request else subprocess.TimeoutExpired)
        wb.json = Namespace("json", loads=callback("loads", implementation=json.loads))
        wb.glob = Namespace("glob", glob=callback("glob_call", request.get("paths", ["b.jsonl", "a.jsonl"]), glob.glob if request.get("real_files") else None))
        wb.open, wb._log, wb._UsageRecovery = open_file, log, Recovery
        wb.net_guard = Namespace("net_guard", LOG_ENV="FIXTURE_GUARD_LOG")
        wb.network_tool_block = callback("network", request.get("network", ["WebFetch", "WebSearch", "WebSearch"]))
        wb.agent_credential_env = callback("credentials")
        options = dict(build_argv=callback("build_argv", raw_argv), run_env=callback("run_env", request.get("run_env", {"RUN_ENV": "yes"})), row_usage=callback("row_usage", usage))
        wb.copilot = Namespace("copilot", **options, fatal_config_error=callback("fatal_config", request.get("fatal", "")), read_log_dir=callback("read_log_dir", request.get("log_text", "debug logs")))
        wb.codex = Namespace("codex", **options, KEY_ENV="FIXTURE_CODEX_KEY", build_resume_argv=callback("resume_argv", raw_argv))
        wb._shq = callback("quote", implementation=saved["_shq"])
        for name, kind in request.get("exception_types", {}).items(): setattr(wb, name, getattr(builtins, kind) if isinstance(kind, str) else kind)
        if request.get("operation") == "session_log": value = wb._session_log_text_host(inputs["home"])
        else:
            wb._session_log_text_host = callback("session_log", request.get("log_text", "session logs"))
            value = runner._segment(inputs["workspace"], inputs["prompt"], transcript, inputs["exclude_tools"], inputs["max_turns"], Guard() if request.get("guard", True) else None)
            if agent != "claude": assert value is usage, "usage identity changed"
        result = {"value": encode(value)}
    except BaseException as error:
        result = {"raised": type(error).__name__, "message": str(error)}
        if request.get("exception_chain"):
            chain, current = [], error.__context__
            while current is not None and len(chain)<5: chain.append(type(current).__name__);current = current.__context__
            result["context"] = chain
    finally:
        for name, value in saved.items():
            if value is absent: vars(wb).pop(name, None)
            else: setattr(wb, name, value)
    result.update(calls=calls, transcript=transcript.written, stdin=object.__getattribute__(process, "stdin").written, stdin_closed=object.__getattribute__(process, "stdin").closed, launched=len(launched), closed=[manager.closed for manager in managers], usage=encode(usage))
    print(json.dumps(encode(result), sort_keys=True, separators=(",", ":"), ensure_ascii=True), flush=True)
