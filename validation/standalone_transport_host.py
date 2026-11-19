"""Exercise SDK process framing, signal delivery and connection lifetime."""
from concurrent.futures import ThreadPoolExecutor
import json
from pathlib import Path
import signal
import sys
import time

sys.path.insert(0,str(Path(__file__).resolve().parents[1]/"src/experiments/sdk"))
from te_runtime.transport import Bridge

mode=sys.argv[2]
number=signal.SIGTERM if mode == "term" else signal.SIGINT
if mode == "term":
    def stopped(number,frame): raise SystemExit(17)
    signal.signal(number,stopped)
previous=signal.getsignal(number)
output={}
with Bridge(sys.argv[1]) as bridge:
    try:
        if mode == "threads":
            def request(index):
                value=bridge.request("op-"+str(index),{})
                assert json.loads(value["line"])["op"] == "op-"+str(index),"response crossed requests"
                return value["sequence"]
            with ThreadPoolExecutor(max_workers=4) as executor: output["sequences"]=sorted(executor.map(request,range(32)))
        elif mode in ("gap","callback"):
            with bridge.frames():
                output["result"]=bridge.request("once",{})
                if mode == "gap":
                    root=Path(sys.argv[3]);(root/"ready").write_text("ready")
                    while not (root/"continue").exists(): time.sleep(0.005)
                with bridge.callback_scope():
                    bridge.deliver_signals()
                    if mode == "callback": signal.raise_signal(signal.SIGINT)
                    output["continued"]=True
        else: output["result"]=bridge.request("once",{})
        bridge.deliver_signals()
    except BaseException as error: output.update(raised=type(error).__name__,message=str(error))
    output["restored"]=signal.getsignal(number) is previous
print(json.dumps(output,separators=(",",":")),flush=True)
