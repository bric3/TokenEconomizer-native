"""Agent accounting objects and JSON bindings; parsing and cost accounting run in Rust."""
import os
from types import SimpleNamespace

from .transport import checked, data


def bind(request,log):
    def call(operation,**arguments):
        return checked(request("experiment.usage."+operation,data(arguments)))

    def claude_merge(a,b): return call("claude._merge_usage",a=a,b=b)
    def session_output_tokens(text):
        value=call("claude.session_output_tokens",text=text)
        return value[0],set(value[1])
    def row_from_result(result): return call("claude._row_from_result",result=result)

    class UsageRecovery:
        def __init__(self):
            self._by_msg=dict(call("claude.recovery.new")["_by_msg"])

        def _call(self,operation,**arguments):
            result=call("claude.recovery."+operation,self={"_by_msg":list(self._by_msg.items())},**arguments)
            self._by_msg=dict(result["self"]["_by_msg"])
            for message in result["messages"]: log(message)
            return checked(result["value"])

        def observe(self,ev): return self._call("observe",ev=ev)
        def synthesize(self,reason,session_text=None): return self._call("synthesize",reason=reason,session_text=session_text)

    def codex_row(stream_text): return call("codex.row_usage",stream_text=stream_text)
    def codex_merge(a,b): return call("codex.merge_usage",a=a,b=b)
    def sum_openai_usage(log_text): return call("copilot.sum_openai_usage",log_text=log_text)
    def read_log_dir(log_dir): return call("copilot.read_log_dir",log_dir=os.fspath(log_dir),process_cwd=os.getcwd())
    def turns_from_stream(stream_text): return call("copilot.turns_from_stream",stream_text=stream_text)
    def stream_output_tokens(stream_text): return call("copilot.stream_output_tokens",stream_text=stream_text)
    def copilot_row(stream_text,log_text): return call("copilot.row_usage",stream_text=stream_text,log_text=log_text)
    def copilot_merge(a,b): return call("copilot.merge_usage",a=a,b=b)

    return SimpleNamespace(
        claude=SimpleNamespace(_merge_usage=claude_merge,session_output_tokens=session_output_tokens,
            _row_from_result=row_from_result,_UsageRecovery=UsageRecovery),
        codex=SimpleNamespace(row_usage=codex_row,merge_usage=codex_merge),
        copilot=SimpleNamespace(sum_openai_usage=sum_openai_usage,read_log_dir=read_log_dir,
            turns_from_stream=turns_from_stream,stream_output_tokens=stream_output_tokens,
            row_usage=copilot_row,merge_usage=copilot_merge))
