"""Own a JSON-lines engine connection and send each request once."""
import contextlib
import json
import os
from pathlib import Path
import signal
import subprocess
import threading


class Bridge:
    def __init__(self, executable):
        self.executable=Path(executable)
        self._process=None
        self._lock=threading.Lock()
        self._signals=[]

    def _guard(self,number,handler):
        if not callable(handler): return
        def defer(number,frame): self._signals.append((handler,number,frame))
        defer._te_bridge=self
        defer._te_user_handler=handler
        signal.signal(number,defer)

    @contextlib.contextmanager
    def frames(self):
        if threading.current_thread() is not threading.main_thread():
            yield
            return
        previous={number:signal.getsignal(number) for number in (signal.SIGINT,signal.SIGTERM)}
        for number,handler in previous.items():
            if getattr(handler,"_te_bridge",None) is not self:
                self._guard(number,getattr(handler,"_te_user_handler",handler))
        try: yield
        finally:
            for number,handler in previous.items():
                current=signal.getsignal(number)
                if getattr(current,"_te_bridge",None) is self:
                    owner=getattr(handler,"_te_bridge",None)
                    if owner is not None: owner._guard(number,current._te_user_handler)
                    else: signal.signal(number,current._te_user_handler)

    @contextlib.contextmanager
    def callback_scope(self):
        main=threading.current_thread() is threading.main_thread()
        if main:
            for number in (signal.SIGINT,signal.SIGTERM):
                handler=signal.getsignal(number)
                if hasattr(handler,"_te_user_handler"): signal.signal(number,handler._te_user_handler)
        try: yield
        finally:
            if main:
                for number in (signal.SIGINT,signal.SIGTERM):
                    handler=signal.getsignal(number)
                    self._guard(number,getattr(handler,"_te_user_handler",handler))

    def request(self, operation, arguments):
        with self._lock,self.frames():
            if self._process is None:
                self._process=subprocess.Popen([str(self.executable)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,
                    text=True,encoding="utf-8",**({"creationflags":subprocess.CREATE_NEW_PROCESS_GROUP} if os.name == "nt" else {"start_new_session":True}))
            process=self._process
            process.stdin.write(json.dumps({"op":operation,"args":arguments})+"\n")
            process.stdin.flush()
            line=process.stdout.readline()
            if not line: raise RuntimeError(f"native bridge exited: {process.poll()}")
            response=json.loads(line)
            if not isinstance(response,dict): raise ValueError("invalid native response frame")
            if "error" in response: raise ValueError(response["error"])
            if "result" not in response: raise ValueError("native response has no result")
            return response["result"]

    def deliver_signals(self):
        pending,self._signals=self._signals,[]
        for handler,number,frame in pending: handler(number,frame)

    def close(self):
        with self._lock:
            process,self._process=self._process,None
            if process is None: return
            try: process.stdin.close()
            except BrokenPipeError: pass
            try: process.wait(timeout=5)
            except subprocess.TimeoutExpired: process.kill();process.wait()
            finally: process.stdout.close()

    def __enter__(self): return self
    def __exit__(self,*error): self.close()
