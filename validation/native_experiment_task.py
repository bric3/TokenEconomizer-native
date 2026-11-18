"""I/O callbacks for the native SWE-bench task lifecycle."""
import dataclasses
import functools
import os

from native_plugin import invoke
from native_experiment_spiral import checked


def data(value):
    if isinstance(value, bytes): return {"bytes":list(value)}
    if dataclasses.is_dataclass(value): return dataclasses.asdict(value)
    if isinstance(value, os.PathLike): return os.fspath(value)
    if isinstance(value, dict): return {key: data(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)): return [data(item) for item in value]
    if callable(value): return True
    return value


def install():
    from te_harness import swebench as module
    cls = module.ClaudeSwebenchRunner
    def drive(self, result, run_kwargs):
        errors = []
        guard = None
        transcript = None
        while "callback" in result:
            method, args = result["method"], result["args"]
            try:
                if method == "clock": value = module.time.time()
                elif method == "log": value = module._log(*args)
                elif method == "optimizer_context":
                    module._OPT_CTX.clear()
                    module._OPT_CTX.update(args[0])
                    value = None
                elif method == "_build_guard":
                    guard = self._build_guard(run_kwargs)
                    value = guard is not None
                elif method == "guard_segment": value = guard.segment
                elif method == "_guard_continuation": value = self._guard_continuation(guard,*args)
                elif method == "_guard_row": value = self._guard_row(guard)
                elif method == "_write_run_config": value = self._write_run_config(*args,run_kwargs,guard)
                elif method == "_record_env_activation":
                    row = args[2]
                    self._record_env_activation(args[0],args[1],row)
                    value = row
                elif method == "transcript_open":
                    transcript = open(args[0], "w", encoding="utf-8")
                    value = None
                elif method == "transcript_close":
                    transcript.close()
                    transcript = None
                    value = None
                elif method == "_segment":
                    name, prompt, path, excluded, turns, mcp = args
                    value = self._segment(name,prompt,transcript,excluded,turns,guard,mcp_config=mcp)
                elif method == "_exec":
                    process = self._exec(args[0],args[1],**args[2])
                    value = {key:getattr(process,key) for key in ("returncode","stdout","stderr")}
                elif method == "eval_script": value = self.grader.eval_script(*args)
                elif method == "score": value = self.grader.score(*args)
                elif method == "scaffold_info": value = {"name":self.scaffold.name} if self.scaffold is not None else None
                else: value = getattr(self,method)(*args)
                reply = {"value":data(value)}
            except BaseException as error:
                errors.append(error)
                reply = {"error":{"kind":type(error).__name__,"message":str(error),"bases":[base.__module__ + "." + base.__qualname__ for base in type(error).__mro__],"stdout":data(getattr(error,"stdout",None))}}
            result = invoke("experiment.swebench.resume", {"job":result["job"],"reply":reply})
        if isinstance(result,dict) and "raised" in result:
            for error in reversed(errors):
                if type(error).__name__ == result["raised"] and str(error) == result["message"]:
                    raise error
            checked(result)
        return result
    original = cls.run_task
    @functools.wraps(original)
    def run_task(self, task, *, arm, injected, exclude_tools, run_dir_path, **run_kwargs):
        result = invoke("experiment.swebench.run_task", {"task":data(task), "arm":arm, "injected":injected,
                        "exclude_tools":exclude_tools, "run_dir":os.fspath(run_dir_path), "kwargs":data(run_kwargs),
                        "metadata":{key:getattr(self,key) for key in ("agent","model","effort","cli_version")},
                        "environment":{key:os.environ[key] for key in ("TE_MAX_TURNS",) if key in os.environ}, "process_cwd":os.getcwd()})
        return drive(self,result,run_kwargs), run_dir_path
    cls.run_task = run_task
    original_grade = cls._grade
    @functools.wraps(original_grade)
    def grade(self, name, instance_id, patch, run_dir_path):
        result = invoke("experiment.swebench.grade", {"name":name,"instance_id":instance_id,"patch":patch,
                        "run_dir":os.fspath(run_dir_path),"eval_timeout":self.eval_timeout,"process_cwd":os.getcwd()})
        return tuple(drive(self,result,{}))
    cls._grade = grade
    original_failed = cls._eval_exec_failed
    @functools.wraps(original_failed)
    def failed(self, instance_id, host_log, out, rc, stderr, phase):
        result = invoke("experiment.swebench.eval_exec_failed", {"instance_id":instance_id,"host_log":os.fspath(host_log),
                        "out":out,"rc":rc,"stderr":stderr,"phase":phase,"process_cwd":os.getcwd()})
        return drive(self,result,{})
    cls._eval_exec_failed = failed
    original_name = cls._container_name
    @functools.wraps(original_name)
    def container_name(self, instance_id, arm, run_dir_path):
        return checked(invoke("experiment.swebench.container_name", {"instance_id":instance_id,"arm":arm,"agent":self.agent,
                       "model":self.model,"run_dir_path":os.fspath(run_dir_path),"process_cwd":os.getcwd()}))
    cls._container_name = container_name
    original_prompt = module.build_prompt
    @functools.wraps(original_prompt)
    def build_prompt(task,injected):
        return checked(invoke("experiment.swebench.build_prompt", {"task":task,"injected":injected}))
    module.build_prompt = build_prompt
    for klass, agent in ((cls,"claude"), (module.CopilotSwebenchRunner,"copilot"), (module.CodexSwebenchRunner,"codex")):
        original_row = klass._row_from_segment
        def row_method(original, domain):
            @functools.wraps(original)
            def native(self,result):
                return checked(invoke("experiment.swebench.row_from_segment", {"agent":domain,"result":result}))
            return native
        klass._row_from_segment = row_method(original_row,agent)
