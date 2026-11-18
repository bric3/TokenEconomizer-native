"""Session, agent, queue and file fixtures for Web-Bench task-loop comparisons."""
import builtins
import json
import os
from pathlib import Path
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
    previous = wb.WebBenchRunner.run_task
    install(wb)
    assert previous is wb.WebBenchRunner.run_task,"task installation is not idempotent"

for line in sys.stdin:
    request = json.loads(line)
    calls,counts,contexts,replies,guards,written = [],{},[],[],[],{}
    saved = {name:getattr(wb,name) for name in ("os","time","json","pricing","_log","set_optimizer_context","_cell_of","build_prompt","answer_key_attempts")}
    missing = object()
    saved_open = getattr(wb,"open",missing)
    references = {}
    transcript,run_kwargs_ref,request_files_ref,collected_ref = None,None,None,None
    segment_index,request_index,clock_index = 0,0,0
    class DetailError(Exception):
        def __str__(self): record("error.str");return "fixture task failure é😀"
    class Token:
        def __init__(self,name): self.name = name
        def __bool__(self): record("bool:"+self.name);return request.get("booleans",{}).get(self.name,True)
        def __format__(self,spec): record("format:"+self.name,spec);return "opaque "+self.name
        def __repr__(self): record("repr:"+self.name);return "opaque_"+self.name
        def __fspath__(self): record("fspath:"+self.name);return request.get("run_dir","/run/arm/project/task")
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,Guard): return {"guard":object.__getattribute__(value,"index")}
        if isinstance(value,Session): return {"session":True}
        if isinstance(value,Manager): return {"file":value.path}
        if isinstance(value,dict): return {key:encode(item) for key,item in dict.items(value)}
        if isinstance(value,(tuple,list)): return [encode(item) for item in value]
        return value
    def record(name,args=None):
        calls.append([name,encode(args)])
        counts[name] = counts.get(name,0)+1
        fault = next((fault for fault in [request,*request.get("failures",[])] if fault.get("fail") == name and counts[name] == fault.get("fail_at",1)),None)
        if fault is not None:
            kind = fault.get("error","RuntimeError")
            if kind == "DetailError": raise DetailError()
            if kind.startswith("Shadow"): raise type(kind.removeprefix("Shadow"),(Exception,),{})("fixture failure")
            raise getattr(builtins,kind)("fixture failure")
    class Manager:
        def __init__(self,path,args,kwargs): self.path,self.args,self.kwargs,self.closed,self.actual = path,args,kwargs,False,None
        def __enter__(self):
            global transcript
            record("enter",self.path)
            if request.get("real_files"):
                self.actual = builtins.open(self.path,*self.args,**self.kwargs)
            if str(self.path).endswith("transcript.jsonl"): transcript = self
            if "w" in self.args: written[self.path] = ""
            return self
        def __exit__(self,kind,error,tb):
            record("exit",[self.path,kind.__name__ if kind else None,sys.exception() is error,error is None or error.__traceback__ is tb])
            self.closed = True
            if self.actual is not None: self.actual.close()
            key = "suppress_transcript" if str(self.path).endswith("transcript.jsonl") else "suppress_request"
            return Token("exit_result") if request.get("opaque_exit") else request.get(key,False)
        def write(self,value):
            record("write",[self.path,value])
            assert not self.closed,"write after transcript close"
            written[self.path] += value
            if self.actual is not None: return self.actual.write(value)
            return len(value)
        def flush(self):
            record("flush",self.path)
            if self.actual is not None: self.actual.flush()
        def read(self): record("read",self.path);return self.actual.read()
        def __iter__(self): record("file.iter",self.path);return iter(self.actual)
    def open_file(path,*args,**kwargs):
        record("open",[path,args,list(kwargs),kwargs])
        context = Manager(path,args,kwargs)
        contexts.append(context)
        return context
    def path_function(name):
        def call(*args): record(name,args);return getattr(os.path,name)(*args)
        return call
    class Paths:
        def __getattribute__(self,name):
            if name in ("join","basename","isdir","dirname","relpath","getsize"): record("path."+name)
            return object.__getattribute__(self,name)
    paths = Paths()
    for name in ("join","basename","isdir","dirname","relpath","getsize"): setattr(paths,name,path_function(name))
    def makedirs(path,**kwargs):
        record("makedirs",[path,list(kwargs),kwargs])
        if request.get("real_files"): os.makedirs(path,**kwargs)
    def clock():
        global clock_index
        record("clock")
        values = request.get("clock",[100.0,101.25])
        value = values[min(clock_index,len(values)-1)];clock_index += 1
        return value
    def context(**kwargs): record("optimizer_context",[list(kwargs),kwargs])
    def cell(path): record("cell",path);return "arm/project"
    def log(value): record("log",value)
    def prompt(*args,**kwargs): record("build_prompt",[args,list(kwargs),kwargs]);return Token("prompt") if request.get("opaque_prompt") else "implement é😀"
    def answer_key(path): record("answer_key_attempts",path);return request.get("answer_key_attempts",3)
    def effective(row): record("effective_tokens",row);return request.get("effective",123.0)
    def dump(value,handle,**kwargs):
        record("json.dump",[value,handle,list(kwargs),kwargs])
        json.dump(value,handle,**kwargs)
    class Request(dict):
        def __getattribute__(self,name):
            if name == "get": record("req.get_function")
            return dict.__getattribute__(self,name)
        def get(self,*args): record("req.get",args);return dict.get(self,*args)
    class Reply:
        def __getattribute__(self,name):
            if name == "put": record("reply.put_function")
            return object.__getattribute__(self,name)
        def put(self,value):
            record("reply.put",value)
            assert value["files"] is collected_ref,"collected files lost their identity"
            assert transcript is not None and not transcript.closed,"transcript closed before evaluator reply"
            replies.append(value)
    class Guard:
        def __init__(self,index):
            self.index,self.segment,self.remaining = index,0,request.get("resumes",0)
            data = {"aborted":False,"abort_suppressed":False,"nudges_done":1,"events":[{"attempt":index}],"judge_tokens":2.5,"nudge_writer_tokens":3.25}
            data.update(request.get("guard_fields",{}))
            fields = request.get("per_guard",[])
            if index < len(fields): data.update(fields[index])
            for key,value in data.items(): setattr(self,key,value)
            for key in request.get("missing_guard",[]): delattr(self,key)
        def __getattribute__(self,name):
            if name in ("continuation","segment","aborted","abort_suppressed","nudges_done","events","judge_tokens","nudge_writer_tokens"): record("guard."+name,object.__getattribute__(self,"index"))
            return object.__getattribute__(self,name)
        def __bool__(self): record("guard.bool");return not request.get("falsey_guard",False)
        def continuation(self,prompt):
            record("continuation",prompt)
            assert not transcript.closed,"transcript closed during continuation"
            if self.remaining <= 0: return Token("continuation") if request.get("opaque_stop") else None
            self.remaining -= 1;self.segment += 1
            return {"mode":"resume","prompt":"nudge"} if request.get("mapping_resume") else "nudge"
    task = {"instance_id":"task-1","repo":"@web-bench/project","problem_statement":"implement",**request.get("task",{})}
    if request.get("missing_task_id"): task.pop("instance_id",None)
    if request.get("opaque_id"): task["instance_id"] = Token("task_id")
    events = request.get("events",[{"task":task.get("instance_id")},{"task":task.get("instance_id"),"ok":True}])
    event_iter = iter(events)
    class Session:
        finished = request.get("finished",False)
        def __getattribute__(self,name):
            if name in ("next_event","next_request","finished"): record("session."+name)
            return object.__getattribute__(self,name)
        def next_event(self,*args):
            record("next_event",args)
            assert transcript is not None and not transcript.closed,"transcript closed while waiting for event"
            return next(event_iter,None)
        def next_request(self,*args):
            global request_index,request_files_ref
            record("next_request",args)
            reqs = request.get("requests",[{"type":"task","task":"implement","files":{"src/a.js":"old"}}])
            req = reqs[min(request_index,len(reqs)-1)] if reqs else None
            request_index += 1
            if req is None: return None,None
            if isinstance(req,dict):
                value = Request(req);request_files_ref = dict.get(value,"files")
                return value,Reply()
            return req,Reply()
    session = Session()
    methods = ("session_for","_row_from_segment","_write_run_config","_install_net_guard","agent_home","_make_guard","_materialize","_apply_scaffold","_segment","_merge_segments","_collect","_segment_usage","_net_guard_blocks")
    fields = ("model","agent","effort","cli_version","repo","_stream_broken","retry","start_timeout","verdict_timeout","scaffold","keep_workspaces")
    class Scaffold:
        name = "fixture-scaffold"
        def __getattribute__(self,name):
            if name == "name": record("scaffold.name")
            return object.__getattribute__(self,name)
    class Runner(wb.WebBenchRunner):
        def __getattribute__(self,name):
            if name in fields or name in methods:
                record("runner."+name)
                if name == "model" and request.get("mutate_context"): wb.set_optimizer_context = lambda **kwargs:record("new.context",kwargs)
            return object.__getattribute__(self,name)
        def __setattr__(self,name,value):
            if name in ("_current_task","_stream_broken"): record("set:"+name,value)
            object.__setattr__(self,name,value)
        def session_for(self,*args): record("session_for",args);return session
        def _row_from_segment(self,result):
            record("row_from_segment",result)
            assert transcript is None or transcript.closed,"transcript remains open while constructing row"
            row = saved_row(result)
            row.update(request.get("row_fields",{}))
            return row
        def _write_run_config(self,*args):
            global run_kwargs_ref
            record("write_run_config",args)
            assert args[1] is task and args[4] is excluded and args[7] is session,"configuration lost an argument reference"
            run_kwargs_ref = args[6]
        def agent_home(self,path): record("agent_home",path);return os.path.join(os.fspath(path),".home")
        def _install_net_guard(self,*args): record("install_net_guard",args);return Token("armed") if request.get("opaque_armed") else request.get("armed",True)
        def _make_guard(self,kwargs):
            record("make_guard",kwargs)
            assert kwargs is run_kwargs_ref,"run kwargs lost their identity"
            if not request.get("guard"): return None
            guard = Guard(len(guards));guards.append(guard);return guard
        def _materialize(self,workspace,files):
            record("materialize",[workspace,files])
            if request_files_ref: assert files is request_files_ref,"request files lost their identity"
            if request.get("real_files"): return super()._materialize(workspace,files)
        def _apply_scaffold(self,*args):
            record("apply_scaffold",args)
            notes = request.get("notes",[{"scaffold_installed":True}])
            return notes[min(counts["apply_scaffold"]-1,len(notes)-1)]
        def _segment(self,workspace,prompt,handle,tools,turns,guard):
            global segment_index
            record("segment",[workspace,prompt,handle,tools,turns,guard])
            assert handle is transcript and tools is excluded and not handle.closed,"segment arguments lost their references"
            handle.write("segment "+str(segment_index)+"\n");handle.flush()
            values = request.get("segments",[{"usage":{"input_tokens":10,"cache_read_input_tokens":20,"cache_creation_input_tokens":30,"output_tokens":4},"num_turns":1}])
            result = values[min(segment_index,len(values)-1)];segment_index += 1
            if request.get("real_files"): Path(workspace,"src/a.js").write_text("changed é😀\n",encoding="utf-8")
            return result
        def _merge_segments(self,a,b): record("merge_segments",[a,b]);return saved_merge(a,b)
        def _collect(self,*args):
            global collected_ref
            record("collect",args)
            if request.get("real_files"): value = super()._collect(*args)
            else: value = request.get("collection",[{"src/a.js":"changed"},["deleted"],["generated"]])
            if isinstance(value,(list,tuple)) and len(value)==3: collected_ref = value[0]
            return value
        def _segment_usage(self,result): record("segment_usage",result);return saved_usage(result)
        def _net_guard_blocks(self,*args): record("net_guard_blocks",args);return request.get("blocks",2)
        def scaffold_answer_excludes(self): record("scaffold_excludes");return []
    saved_row,saved_merge,saved_usage = wb.WebBenchRunner._row_from_segment,wb._merge_usage,wb.WebBenchRunner._segment_usage
    # Bind the unmodified accounting hook without constructing or probing a runner.
    saved_row = types.MethodType(saved_row,object.__new__(wb.WebBenchRunner))
    saved_usage = types.MethodType(saved_usage,object.__new__(wb.WebBenchRunner))
    runner = object.__new__(Runner)
    initial = {"model":"claude-opus-4-8","agent":"claude","effort":"high","cli_version":"fixture-cli","repo":request.get("repo","/repo"),"_stream_broken":False,"retry":2,"start_timeout":12,"verdict_timeout":34,"keep_workspaces":False,"scaffold":Scaffold() if request.get("scaffold") else None,**request.get("fields",{})}
    for key,value in initial.items(): object.__setattr__(runner,key,value)
    for key in request.get("missing_fields",[]): object.__delattr__(runner,key)
    excluded = Token("excluded") if request.get("opaque_excluded") else request.get("excluded",["WebSearch"])
    run_dir = Token("run_dir") if request.get("opaque_path") else request.get("run_dir","/run/arm/project/task")
    try:
        wb.os = types.SimpleNamespace(path=paths,makedirs=makedirs,environ=request.get("env",{}),walk=os.walk,sep=os.sep)
        wb.time,wb.json,wb.open = types.SimpleNamespace(time=clock),types.SimpleNamespace(dump=dump),open_file
        wb.pricing = types.SimpleNamespace(effective_tokens=effective)
        wb._log,wb.set_optimizer_context,wb._cell_of = log,context,cell
        wb.build_prompt,wb.answer_key_attempts = prompt,answer_key
        saved_remove = wb.shutil.rmtree
        def remove(path,**kwargs):
            record("rmtree",[path,list(kwargs),kwargs])
            if request.get("real_files"): saved_remove(path,**kwargs)
        wb.shutil.rmtree = remove
        value = runner.run_task(task,arm=request.get("arm","baseline"),injected=Token("injected") if request.get("opaque_injected") else request.get("injected"),exclude_tools=excluded,run_dir_path=run_dir,**request.get("kwargs",{}))
        assert value[1] is run_dir,"run directory lost its identity"
        result = {"value":encode(value)}
    except BaseException as error:
        result = {"raised":type(error).__name__,"message":str(error)}
        if request.get("exception_chain"):
            chain=[];current=error.__context__
            while current is not None and len(chain)<5: chain.append(type(current).__name__);current=current.__context__
            result["context"] = chain
    finally:
        closed = [[manager.path,manager.closed] for manager in contexts]
        for manager in contexts:
            if manager.actual is not None: manager.actual.close()
        wb.shutil.rmtree = saved_remove
        for key,value in saved.items(): setattr(wb,key,value)
        if saved_open is missing: vars(wb).pop("open",None)
        else: wb.open = saved_open
    result.update(calls=calls,replies=encode(replies),written=written,closed=closed,state={key:encode(vars(runner).get(key)) for key in ("_current_task","_stream_broken")})
    print(json.dumps(result,sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
