"""Process, log, regex and opaque-object fixtures for evaluator stdout processing."""
import builtins
import json
from pathlib import Path
import re
import subprocess
import sys
import types

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT/"reference/upstream"),str(ROOT/"reference/upstream/experiments")]
from te_harness import swebench
if "--native" in sys.argv:
    from native_experiment_providers import install
    install()
from te_harness import webbench as wb
if "--native" in sys.argv:
    install(wb)
    previous = wb._EvalSession._read_stdout
    install(wb)
    assert wb._EvalSession._read_stdout is previous,"stdout installation is not idempotent"

for line in sys.stdin:
    request = json.loads(line)
    calls,counts,events,logs,written = [],{},{},{},[]
    absent = object()
    saved = {name:getattr(wb,name,absent) for name in ("_ATTEMPT_RE","_MARK_OK","_log","int")}
    limit = sys.get_int_max_str_digits()
    process,handle = None,None
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,dict): return {key:encode(item) for key,item in dict.items(value)}
        if isinstance(value,(tuple,list)): return [encode(item) for item in value]
        return value
    class DetailError(Exception):
        def __str__(self): calls.append(["error.str",None]);return "fixture failure é😀"
    def record(name,args=None):
        calls.append([name,encode(args)])
        counts[name] = counts.get(name,0)+1
        if request.get("fail") == name and counts[name] == request.get("fail_at",1):
            kind = request.get("error","RuntimeError")
            if kind == "DetailError": raise DetailError()
            raise getattr(builtins,kind)("fixture failure")
    class Token:
        def __init__(self,name): self.name = name
        def __bool__(self): record("bool:"+self.name);return request.get("booleans",{}).get(self.name,self.name != "equals_started")
        def __int__(self): record("int:"+self.name);return request.get("integer",2)
        def __eq__(self,other):
            record("eq:"+self.name,other)
            if request.get("opaque_comparison"): return Token("equals_started" if other == "Started" else "equals_ok")
            return other == request.get("equals","✅")
        def __format__(self,spec):
            record("format:"+self.name,spec)
            if request.get("mutate_log"): wb._log = lambda value:record("new.log",value)
            return "formatted-"+self.name
        def strip(self):
            record("strip:"+self.name)
            if request.get("mutate_pattern"): wb._ATTEMPT_RE = Pattern("new")
            return Token("stripped") if request.get("opaque_stripped") else request.get("stripped","[p] task-1 2nd Attempt ✅\n")
        def rstrip(self):
            record("rstrip:"+self.name)
            if request.get("mutate_put"): queue.put = lambda value:record("new.put",value)
            return Token("rstrip")
    class Match:
        def __getattribute__(self,name):
            if name == "group": record("match.group_function")
            return object.__getattribute__(self,name)
        def __bool__(self): record("match.bool");return request.get("match_truthy",True)
        def group(self,name):
            record("group",name)
            if name == request.get("mutate_integer_at"): wb.int = lambda value:(record("new.int",value),Token("new_integer"))[1]
            if name == "mark" and request.get("mutate_mark"): wb._MARK_OK = request.get("new_mark","Started")
            return Token(name) if name in request.get("group_objects",[]) else request.get("groups",{}).get(name,{"mark":"✅","task":"task-3","ord":"2"}[name])
    class Pattern:
        def __init__(self,name="original"): self.name = name
        def __getattribute__(self,name):
            if name == "match": record("pattern.match_function",object.__getattribute__(self,"name"))
            return object.__getattribute__(self,name)
        def match(self,value): record("match",value);return None if request.get("no_match") else Match()
    class Stream:
        def __init__(self,values): self.values = iter(values)
        def __iter__(self): record("iter");return self
        def __next__(self): record("next");return next(self.values)
    class Process:
        def __getattribute__(self,name):
            if name in ("stdout","wait","returncode"): record("proc."+name)
            return object.__getattribute__(self,name)
        @property
        def stdout(self):
            if process is not None: return process.stdout
            return stream
        @property
        def returncode(self): return process.returncode if process is not None else Token("code") if request.get("opaque_code") else request.get("code",0)
        def wait(self): record("wait");return process.wait() if process is not None else Token("wait_result")
    class File:
        def __init__(self,name): self.name = name;logs[name] = []
        def __getattribute__(self,name):
            if name in ("write","flush"): record("file."+name,object.__getattribute__(self,"name"))
            return object.__getattribute__(self,name)
        def write(self,value):
            record("write",[self.name,value]);logs[self.name].append(value);written.append(value)
            if handle is not None: handle.write(value)
            if request.get("mutate_file"): object.__setattr__(session,"_log_fh",replacement)
            return Token("write_result")
        def flush(self): record("flush",self.name);return handle.flush() if handle is not None else Token("flush_result")
    class Queue:
        def __getattribute__(self,name):
            if name == "put": record("queue.put_function")
            return object.__getattribute__(self,name)
        def put(self,value):
            record("put",[list(value),value]);events[len(events)] = value
            if "line" in value and type(written[-1]) is str and written[-1].rstrip() is written[-1]:
                assert value["line"] is written[-1],"unchanged line lost identity"
            return Token("put_result")
    queue = Queue()
    raw_lines = list(request.get("lines",["[project] task-1 1st Attempt Started\n","[project] task-1 1st Attempt ✅\n"]))
    lines = [Token("line") for value in raw_lines] if request.get("opaque_lines") else raw_lines
    stream = Stream(lines)
    original,replacement = File("original"),File("replacement")
    fields = {"proc":Process(),"_log_fh":original,"events":queue,"project":"project","finished":False,"exit_code":None,**request.get("fields",{})}
    for key in request.get("field_objects",[]): fields[key] = Token(key)
    for key in request.get("missing",[]): fields.pop(key,None)
    class Session(wb._EvalSession):
        def __getattribute__(self,name):
            if name in fields or name in request.get("missing",[]): record("read:"+name)
            return super().__getattribute__(name)
        def __setattr__(self,name,value): record("write:"+name,value);super().__setattr__(name,value)
    session = object.__new__(Session)
    for key,value in fields.items(): object.__setattr__(session,key,value)
    try:
        if "int_max_digits" in request: sys.set_int_max_str_digits(request["int_max_digits"])
        if request.get("custom_pattern"): wb._ATTEMPT_RE = Pattern()
        if "pattern" in request: wb._ATTEMPT_RE = re.compile(request["pattern"],request.get("flags",0))
        if "ok_mark" in request: wb._MARK_OK = request["ok_mark"]
        if request.get("opaque_mark"): wb._MARK_OK = Token("ok_mark")
        wb._log = lambda value:record("log",value)
        if request.get("real_process"):
            process = subprocess.Popen([request["binary"]],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,encoding="utf-8",env={"FIXTURE_OUTPUT":"".join(raw_lines),"FIXTURE_EXIT":str(request.get("code",0))})
            handle = open(request["log_path"],"w",encoding="utf-8")
        value = session._read_stdout()
        result = {"value":encode(value)}
    except BaseException as error: result = {"raised":type(error).__name__,"message":str(error)}
    finally:
        if process is not None:
            if process.poll() is None: process.kill()
            process.wait();process.stdout.close();process.stderr.close()
        if handle is not None: handle.close()
        for key,value in saved.items():
            if value is absent:
                if hasattr(wb,key): delattr(wb,key)
            else: setattr(wb,key,value)
        sys.set_int_max_str_digits(limit)
    result.update(calls=calls,events=encode(list(events.values())),logs=encode(logs),state={key:encode(vars(session)[key]) for key in ("finished","exit_code") if key in vars(session)})
    print(json.dumps(result,sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
