"""Streamed-rollout controller objects; folding, decisions and sampling run in Rust."""
import json
from types import SimpleNamespace

from .transport import checked


def state_data(state):
    return {name:getattr(state,name) for name in state.__slots__}


def guard_data(guard):
    return dict({key:value for key,value in vars(guard).items() if key not in ("judge","nudge_writer","st","pend")},
        st=state_data(guard.st),
        pend={json.dumps(key,ensure_ascii=False,separators=(",",":")):value for key,value in guard.pend.items()})


def bind(request):
    constants=checked(request("nudge_guard.constants",{}))
    fields=checked(request("state.new",{}))

    class SpiralState:
        __slots__=tuple(fields)

        def __init__(self): self._restore(checked(request("state.new",{})))
        def _restore(self,value):
            for name,item in value.items(): setattr(self,name,item)
        def _read(self,name,**arguments):
            return checked(request("state."+name,dict(self=state_data(self),**arguments)))
        def update(self,tool_name,args,result_text,failed=None):
            self._restore(self._read("update",tool_name=tool_name,args=args,result_text=result_text,failed=failed))
        def recent_digest(self,n=24,envelope=None): return self._read("recent_digest",n=n,envelope=envelope)
        def snapshot(self): return self._read("snapshot")
        edit_churn=property(lambda self:self._read("edit_churn"))
        distinct_edit_files=property(lambda self:self._read("distinct_edit_files"))
        stagnation=property(lambda self:self._read("stagnation"))

    class NudgeGuard:
        def __init__(self,envelope=None,judge=None,thr=None,nudge_budget=1,verbose=False,nudge_writer=None,resume_header=None):
            self.judge,self.nudge_writer=judge,nudge_writer
            self._restore(checked(request("nudge_guard.new",dict(envelope=envelope,thr=thr,nudge_budget=nudge_budget,
                verbose=verbose,resume_header=resume_header))))

        def _restore(self,value):
            for name,item in value.items():
                if name == "st":
                    state=SpiralState.__new__(SpiralState)
                    state._restore(item)
                    item=state
                elif name == "pend": item={json.loads(key):entry for key,entry in item.items()}
                setattr(self,name,item)

        def _call(self,name,**arguments):
            arguments.update(self=guard_data(self),judge=bool(self.judge) if name == "monitor" else self.judge is not None,
                writer=self.nudge_writer is not None)
            replies=[]
            while True:
                result=checked(request("nudge_guard."+name,dict(arguments,replies=replies)))
                if "callback" not in result:
                    self._restore(result["self"])
                    for message in result["messages"]: print(message)
                    return checked(result["value"])
                try:
                    callback=self.judge if result["callback"] == "judge" else self.nudge_writer
                    replies.append({"value":callback(result["digest"])})
                except Exception as error: replies.append({"error":str(error)})

        def monitor(self,line): return self._call("monitor",line=line)
        def _sampled_judge(self,digest): return self._call("_sampled_judge",digest=digest)
        def _nudge_body(self): return self._call("_nudge_body")
        def continuation(self,base_prompt): return self._call("continuation",base_prompt=base_prompt)
        def _stop(self,action,reason): return self._call("_stop",action=action,reason=reason)

    return SimpleNamespace(NudgeGuard=NudgeGuard,SpiralState=SpiralState,
        RESUME_HEADER=constants["RESUME_HEADER"],DEFAULT_THRESHOLDS=constants["DEFAULT_THRESHOLDS"])
