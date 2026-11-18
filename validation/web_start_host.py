"""Opaque evaluator-startup fixtures and an offline HTTP/process integration binding."""
import builtins
import json
import os
from pathlib import Path
import queue as queues
import subprocess
import sys
import threading
import time
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
    previous = wb._EvalSession.__init__
    install(wb)
    assert wb._EvalSession.__init__ is previous,"evaluator constructor installation is not idempotent"

for line in sys.stdin:
    request = json.loads(line)
    calls,counts,messages,configs,launches = [],{},[],[],[]
    thread_objects,queue_objects,handles,servers,processes = [],[],[],[],[]
    absent = object()
    names = ("os","queue","time","threading","ThreadingHTTPServer","json","subprocess","open","_LIVE_SESSIONS","_log","webbench_undici_env")
    saved = {name:getattr(wb,name,absent) for name in names}
    limit = sys.get_int_max_str_digits()
    real = request.get("real_io",False)
    session = None
    def encode(value):
        if value is session: return {"self":True}
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,Queue): return {"queue":value.name}
        if isinstance(value,Thread): return {"thread":value.name}
        if isinstance(value,Server): return {"server":True}
        if isinstance(value,Process): return {"process":True}
        if isinstance(value,File): return {"file":value.name}
        if isinstance(value,type): return {"class":value.__name__,"module":value.__module__,"session":getattr(value,"session",None) is session}
        if isinstance(value,types.MethodType): return {"method":value.__name__,"owner":"session" if value.__self__ is session else type(value.__self__).__name__}
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
            if kind == "ShadowOSError": raise type("OSError",(Exception,),{})("fixture failure")
            raise getattr(builtins,kind)("fixture failure")
    class Token:
        def __init__(self,name): self.name = name
        def __bool__(self): record("bool:"+self.name);return request.get("booleans",{}).get(self.name,True)
        def __len__(self): record("len:"+self.name);return request.get("length",2)
        def __iter__(self): record("iter:"+self.name);return iter([Task()])
        def __ne__(self,other):
            record("ne:"+self.name,other)
            if request.get("mutate_ids"): other[:] = ["changed"]
            return Token("comparison") if request.get("opaque_comparison") else request.get("different",True)
        def __format__(self,spec):
            record("format:"+self.name,spec)
            if request.get("mutate_log"): wb._log = lambda value:(record("new.log",value),messages.append(value))[0]
            return "formatted-"+self.name
        def __str__(self): record("str:"+self.name);return "string-"+self.name
        def __fspath__(self): record("fspath:"+self.name);return request.get("fspath",{}).get(self.name,self.name)
    class Task:
        def get(self,*args): record("task.get",args);return Token("identifier")
    class Queue:
        def __init__(self,*args,**kwargs):
            self.name = len(queue_objects)+1
            record("queue",[args,list(kwargs),kwargs]);queue_objects.append(self)
            self.actual = queues.Queue(*args,**kwargs)
        def put(self,*args,**kwargs): record("queue.put",self.name);return self.actual.put(*args,**kwargs)
        def get(self,*args,**kwargs): record("queue.get",[self.name,args,kwargs]);return self.actual.get(*args,**kwargs)
    class Thread:
        def __init__(self,**kwargs):
            self.name = "http" if not thread_objects else "reader"
            record("thread",[self.name,list(kwargs),kwargs]);thread_objects.append(self)
            self.actual = threading.Thread(**kwargs) if real else None
        def __getattribute__(self,name):
            if name == "start": record("thread.start_function",object.__getattribute__(self,"name"))
            return object.__getattribute__(self,name)
        def start(self): record("thread.start",self.name);return self.actual.start() if real else Token("thread_result")
    class Server:
        def __init__(self,address,handler):
            record("server",[address,handler]);servers.append(self)
            self.actual = saved["ThreadingHTTPServer"]((address[0],request["port"]),handler) if real else None
        def __getattribute__(self,name):
            if name in ("server_address","serve_forever"): record("server."+name)
            return object.__getattribute__(self,name)
        @property
        def server_address(self): return self.actual.server_address if real else ("127.0.0.1",request.get("port",12345))
        def serve_forever(self,**kwargs): return self.actual.serve_forever(**kwargs) if real else None
    class File:
        def __init__(self,path,mode="r",**kwargs):
            self.name = "config" if str(path).endswith("evaluator_config.json") else "log"
            self.actual = builtins.open(path,mode,**kwargs) if real else None
            handles.append(self)
        def __enter__(self): record("enter",self.name);return self
        def __exit__(self,kind,error,tb):
            info = {"name":self.name,"error":kind.__name__ if kind else None}
            if request.get("exception_state"): info.update(active_error=sys.exception() is error,same_traceback=error is None or error.__traceback__ is tb)
            record("exit",info)
            if self.actual is not None: self.actual.close()
            return Token("exit_result") if request.get("opaque_exit") else request.get("suppress",False)
        def write(self,value): record("file.write",self.name);return self.actual.write(value) if real else len(value)
        def flush(self): return self.actual.flush() if real else None
        def __iter__(self): return iter(self.actual) if real else iter([])
    class Process:
        def __init__(self,argv,**kwargs):
            record("popen",[argv,list(kwargs),kwargs]);launches.append([argv,list(kwargs),kwargs])
            self.actual = subprocess.Popen(argv,**kwargs) if real else None
            processes.append(self)
            self.own_group = os.getpgid(self.actual.pid) == self.actual.pid if real else False
        @property
        def pid(self): record("pid");return 8888
        @property
        def stdout(self): return self.actual.stdout if real else iter([])
        @property
        def returncode(self): return self.actual.returncode if real else 0
        def wait(self,*args,**kwargs): return self.actual.wait(*args,**kwargs) if real else 0
    class Registry(set):
        def add(self,value): record("register",value);return super().add(value)
    registry = Registry()
    def strftime(*args): record("strftime",args);return Token("timestamp") if request.get("opaque_timestamp") else "20261118-120000"
    def makedirs(*args,**kwargs): record("makedirs",[args,list(kwargs),kwargs]);return os.makedirs(*args,**kwargs) if real else Token("mkdir_result")
    def join(*args): record("join",args);return os.path.join(*args)
    def exists(path): record("exists",path);return os.path.exists(path) if real else request.get("exists",True)
    def open_file(*args,**kwargs): record("open",[args,list(kwargs),kwargs]);return File(*args,**kwargs)
    def dump(value,handle,**kwargs):
        record("dump",[list(value),value,handle,list(kwargs),kwargs]);configs.append(value)
        assert value["retry"] is arguments["retry"],"retry identity changed"
        return json.dump(value,handle,**kwargs) if real else json.dumps(value,**kwargs)
    def undici(env,**kwargs): record("undici",[env,list(kwargs),kwargs]);return saved["webbench_undici_env"](env,**kwargs) if request.get("real_helpers") else request.get("undici",{})
    class Paths:
        def __getattribute__(self,name):
            if name in ("join","exists"): record("path."+name)
            return object.__getattribute__(self,name)
    paths = Paths()
    paths.join,paths.exists = join,exists
    arguments = {"project":"project","repo":"/fixture/repo","session_dir":"/fixture/session","retry":2,"tasks":[{"instance_id":"init"},{"instance_id":"task-1"}],"node":"/fixture/node","reply_timeout":10,**request.get("arguments",{})}
    for key in request.get("objects",[]): arguments[key] = Token(key)
    tracked = {"project","repo","dir","retry","reply_timeout","requests","events","finished","exit_code","hash","log_path","httpd","port","_http_thread","_full_task_ids","config_path","_log_fh","proc","_reader","_read_stdout"}
    class Session(wb._EvalSession):
        def __setattr__(self,name,value): record("write:"+name,value);super().__setattr__(name,request.get("rewrites",{}).get(name,value))
        def __getattribute__(self,name):
            if name in tracked:
                record("read:"+name)
                if request.get("mutate_on_get") == name:
                    paths.join = lambda *args:(record("new.join",args),os.path.join(*args))[1]
                    wb.os.makedirs = lambda *args,**kwargs:record("new.makedirs",[args,kwargs])
                    wb.threading.Thread = lambda **kwargs:(record("new.thread"),Thread(**kwargs))[1]
                    wb.subprocess.Popen = lambda *args,**kwargs:(record("new.popen"),Process(*args,**kwargs))[1]
            return super().__getattribute__(name)
        def _full_task_ids(self): record("full");return super()._full_task_ids() if request.get("real_helpers") else Token("full") if request.get("opaque_full") else request.get("full",["init","task-1"])
        def _read_stdout(self):
            if request.get("real_helpers"): return super()._read_stdout()
    session = object.__new__(Session)
    try:
        if "int_max_digits" in request: sys.set_int_max_str_digits(request["int_max_digits"])
        wb.os = types.SimpleNamespace(path=paths,makedirs=makedirs,environ=request.get("env",{}))
        wb.queue = types.SimpleNamespace(Queue=Queue,Empty=queues.Empty)
        wb.time = types.SimpleNamespace(strftime=strftime,time=time.time)
        wb.threading = types.SimpleNamespace(Thread=Thread)
        wb.ThreadingHTTPServer,wb.open = Server,open_file
        wb.json = types.SimpleNamespace(dump=dump,loads=json.loads,dumps=json.dumps)
        wb.subprocess = types.SimpleNamespace(Popen=Process,PIPE=subprocess.PIPE,STDOUT=subprocess.STDOUT)
        wb._LIVE_SESSIONS,wb.webbench_undici_env = registry,undici
        wb._log = lambda value:(record("log",value),messages.append(value))[0]
        value = session.__init__(**arguments)
        if real:
            agent_request,reply = session.next_request(5)
            if agent_request is None: raise AssertionError("offline evaluator sent no request")
            reply.put({"files":{"answer.txt":"fixture answer"},"inputTokens":1,"outputTokens":2})
            object.__getattribute__(session,"_reader").actual.join(5)
            if object.__getattribute__(session,"_reader").actual.is_alive(): raise AssertionError("stdout reader did not finish")
            observed = [session.next_event("started",1),session.next_event("verdict",1),session.next_event("verdict",1)]
        result = {"value":encode(value)}
        if real: result.update(request=agent_request,events=observed)
    except BaseException as error:
        result = {"raised":type(error).__name__,"message":str(error)}
        if request.get("exception_state"): result["context"] = type(error.__context__).__name__ if error.__context__ is not None else None
    finally:
        for child in processes:
            if child.actual is not None:
                if child.actual.poll() is None: child.actual.kill()
                child.actual.wait()
        for server in servers:
            if server.actual is not None: server.actual.shutdown();server.actual.server_close()
        for thread in thread_objects:
            if thread.actual is not None and thread.actual.ident is not None: thread.actual.join(5)
        for handle in handles:
            if handle.actual is not None: handle.actual.close()
        for child in processes:
            if child.actual is not None and child.actual.stdout is not None: child.actual.stdout.close()
        for key,value in saved.items():
            if value is absent:
                if hasattr(wb,key): delattr(wb,key)
            else: setattr(wb,key,value)
        sys.set_int_max_str_digits(limit)
    result.update(state=encode(vars(session)),configs=encode(configs),launches=encode(launches),registered=session in registry,messages=sorted(messages) if real else messages)
    if not real: result["calls"] = calls
    else:
        log_path = Path(vars(session)["log_path"])
        result["log"] = log_path.read_text(encoding="utf-8") if log_path.is_file() else None
        result["process_groups"] = [child.own_group for child in processes]
    print(json.dumps(result,sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
