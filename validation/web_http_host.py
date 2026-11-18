"""Opaque HTTP objects, queues and local sockets for evaluator request handling."""
import builtins
import http.client
import io
import json
from pathlib import Path
import queue
import sys
import threading
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
    previous = wb._AgentHandler.do_POST
    install(wb)
    assert wb._AgentHandler.do_POST is previous,"HTTP handler installation is not idempotent"

for line in sys.stdin:
    request = json.loads(line)
    calls,counts,parsed,packets,responses,writes,messages = [],{},[],[],[],[],[]
    absent = object()
    saved = {name:getattr(wb,name,absent) for name in ("json","queue","_log","int")}
    integer_limit = sys.get_int_max_str_digits()
    server,thread,connection = None,None,None
    handler,reply_queue = None,None
    document = request.get("document",'{"task":"init","files":{"é":"😀"}}')
    raw = bytes(request["raw"]) if "raw" in request else document.encode("utf-8")
    def encode(value):
        if isinstance(value,Token): return {"ref":value.name}
        if isinstance(value,Reply): return {"reply_queue":True}
        if isinstance(value,bytes): return {"bytes":list(value)}
        if isinstance(value,dict): return {key:encode(item) for key,item in dict.items(value)}
        if isinstance(value,(tuple,list)): return [encode(item) for item in value]
        return value
    class DetailError(Exception):
        def __str__(self): record("error.str");return request.get("error_text","fixture failure é<&>😀")
        def __format__(self,spec):
            record("error.format",spec)
            if request.get("fail_format"): raise RuntimeError("fixture format failure")
            if request.get("mutate_error_formatter"): object.__setattr__(handler,"send_error",lambda *args:record("new.send_error",args))
            return str(self)
    class SilentEmpty(queue.Empty):
        def __str__(self): record("empty.str");return "empty fixture"
    def record(name,args=None):
        item = [name,encode(args)]
        if request.get("exception_state"):
            active = sys.exception()
            item.append(type(active).__name__ if active is not None else None)
        calls.append(item)
        counts[name] = counts.get(name,0)+1
        if request.get("fail") == name and counts[name] == request.get("fail_at",1):
            kind = request.get("error","RuntimeError")
            if kind == "DetailError": raise DetailError()
            if kind == "Empty": raise queue.Empty()
            if kind == "SilentEmpty": raise SilentEmpty()
            if kind == "ShadowEmpty": raise type("Empty",(Exception,),{})("fixture failure")
            if kind == "ShadowBrokenPipeError": raise type("BrokenPipeError",(Exception,),{})("fixture failure")
            if kind == "ShadowException": raise type("Exception",(BaseException,),{})("fixture failure")
            raise getattr(builtins,kind)("fixture failure")
    class Token:
        def __init__(self,name): self.name = name
        def __bool__(self):
            record("bool:"+self.name)
            if request.get("mutate_loads"): wb.json.loads = lambda value:(record("new.loads",value),{"changed":True})[1]
            return request.get("booleans",{}).get(self.name,True)
        def __int__(self): record("int:"+self.name);return request.get("integer",len(raw))
        def __len__(self):
            record("len:"+self.name)
            if request.get("mutate_length"): object.__setattr__(handler,"send_header",lambda *args:record("new.send_header",args))
            return request.get("body_length",7)
        def __format__(self,spec):
            record("format:"+self.name,spec)
            if request.get("mutate_log"): wb._log = lambda value:record("new.log",value)
            return "formatted-"+self.name
        def decode(self,*args): record("decode:"+self.name,args);return Token("document") if request.get("opaque_document") else request.get("decoded",'{"decoded":true}')
        def encode(self,*args): record("encode:"+self.name,args);return Token("body") if request.get("opaque_body") else request.get("encoded","é😀").encode(*args)
    class Headers:
        def get(self,*args):
            record("headers.get",args)
            if request.get("mutate_integer"): wb.int = lambda value:(record("new.int",value),0)[1]
            return Token("length") if request.get("opaque_length") else request.get("length",str(len(raw)))
    class Input:
        def read(self,n):
            record("read",n)
            if request.get("mutate_loads"): wb.json.loads = lambda value:(record("new.loads",value),{"changed":True})[1]
            return Token("raw") if request.get("opaque_raw") else io.BytesIO(raw).read(n)
    class Output:
        def write(self,value): record("write",value);writes.append(value);return Token("write_result")
    class Requests:
        def put(self,value):
            record("requests.put",value)
            assert value[0] is parsed[-1] and value[1] is reply_queue,"request or reply queue lost identity"
            packets.append(value)
            return Token("put_result")
    class Reply:
        def __init__(self,**kwargs):
            global reply_queue
            record("queue",[list(kwargs),kwargs]);reply_queue = self
        def __getattribute__(self,name):
            if name == "get": record("reply.get_function")
            return object.__getattribute__(self,name)
        def get(self,**kwargs):
            record("reply.get",[list(kwargs),kwargs])
            if request.get("empty"): raise queue.Empty()
            return Token("payload") if request.get("opaque_payload") else request.get("payload",{"files":{"answer.txt":"é😀"},"inputTokens":2,"outputTokens":1})
    timeouts = iter(request.get("timeouts",[]))
    class Session:
        @property
        def requests(self): record("session.requests");return Requests()
        @property
        def reply_timeout(self):
            record("session.reply_timeout")
            if request.get("mutate_log"): wb._log = lambda value:record("new.log",value)
            if "timeouts" in request: return next(timeouts)
            return Token("timeout") if request.get("opaque_timeout") else request.get("timeout",10)
    class Handler(wb._AgentHandler):
        def __getattribute__(self,name):
            if name in ("headers","rfile","session","wfile","send_error","send_response","send_header","end_headers"): record("member:"+name)
            return super().__getattribute__(name)
        def send_error(self,*args): record("send_error",args);responses.append(["error",args]);return Token("error_result")
        def send_response(self,*args): record("send_response",args);responses.append(["response",args]);return Token("response_result")
        def send_header(self,*args): record("send_header",args);responses.append(["header",args]);return Token("header_result")
        def end_headers(self): record("end_headers");responses.append(["end"]);return Token("end_result")
    def loads(value): record("loads",value);decoded = Token("request") if request.get("opaque_request") else json.loads(value);parsed.append(decoded);return decoded
    def dumps(value): record("dumps",value);return Token("encoded") if request.get("opaque_encoded") else json.dumps(value)
    def logger(value):
        record("log",value)
        if request.get("log_error"): raise getattr(builtins,request["log_error"])("fixture log failure")
        messages.append(value)
        return Token("log_result")
    try:
        if "int_max_digits" in request: sys.set_int_max_str_digits(request["int_max_digits"])
        wb.json,wb.queue = types.SimpleNamespace(loads=loads,dumps=dumps),types.SimpleNamespace(Queue=Reply,Empty=queue.Empty)
        wb._log = logger
        if request.get("socket"):
            socket_handler = type("SocketHandler",(wb._AgentHandler,),{"session":Session()})
            server = wb.ThreadingHTTPServer(("127.0.0.1",0),socket_handler)
            thread = threading.Thread(target=server.serve_forever,kwargs={"poll_interval":0.01},daemon=True);thread.start()
            connection = http.client.HTTPConnection("127.0.0.1",server.server_address[1],timeout=5)
            headers = {"content-length":request["length"]} if "length" in request else {}
            connection.request("POST","/message",body=raw,headers=headers)
            response = connection.getresponse()
            body = response.read()
            result = {"status":response.status,"headers":{name:response.getheader(name) for name in ("content-type","content-length")},"body":list(body)}
        else:
            handler = object.__new__(Handler)
            handler.headers,handler.rfile,handler.wfile,handler.session = Headers(),Input(),Output(),Session()
            value = handler.log_message(*request.get("arguments",[])) if request.get("quiet") else handler.do_POST()
            result = {"value":encode(value)}
    except BaseException as error:
        result = {"raised":type(error).__name__,"message":str(error)}
        if request.get("exception_state"): result["context"] = type(error.__context__).__name__ if error.__context__ is not None else None
    finally:
        if connection is not None: connection.close()
        if server is not None: server.shutdown();server.server_close()
        if thread is not None: thread.join(5)
        for key,value in saved.items():
            if value is absent:
                if hasattr(wb,key): delattr(wb,key)
            else: setattr(wb,key,value)
        sys.set_int_max_str_digits(integer_limit)
    result.update(calls=calls,requests=encode(packets),responses=encode(responses),writes=encode(writes),messages=messages)
    if request.get("string_points"):
        result["message_points"] = [[ord(char) for char in item] for item in messages]
        result["error_points"] = [[ord(char) for char in item[1][-1]] for item in responses if item[0] == "error"]
    print(json.dumps(result,sort_keys=True,separators=(",",":"),ensure_ascii=True),flush=True)
