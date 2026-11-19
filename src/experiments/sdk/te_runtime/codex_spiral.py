"""Codex controller objects and JSON bindings; state transitions execute in Rust."""
from collections import Counter, deque
import dataclasses
from types import SimpleNamespace

from .transport import checked


def state_data(state):
    return dict(vars(state), edited_paths=dict(state.edited_paths),
        failure_sigs_since_green=dict(state.failure_sigs_since_green),
        recent_commands=list(state.recent_commands), recent_blocked=list(state.recent_blocked),
        recent_window=state.recent_commands.maxlen,
        outstanding_failure_scopes=list(state.outstanding_failure_scopes))


def restore_state(state, data):
    for key, value in data.items():
        if key == "recent_window": continue
        if key in ("edited_paths", "failure_sigs_since_green"): value=Counter(value)
        elif key in ("recent_commands", "recent_blocked"): value=deque(value,maxlen=data["recent_window"])
        elif key == "outstanding_failure_scopes": value=set(value)
        setattr(state,key,value)
    return state


def bind(request):
    def call(name, **arguments): return checked(request("codex_spiral."+name,arguments))
    constants=call("constants")
    defaults=constants["DEFAULT_POLICY"]
    CodexSpiralPolicy=dataclasses.make_dataclass("CodexSpiralPolicy",
        [(key,type(value),dataclasses.field(default=value)) for key,value in defaults.items()],
        frozen=True,namespace={"__module__":__name__})
    def at_threshold(self,turns):
        return CodexSpiralPolicy(**call("policy.at_threshold",self=dataclasses.asdict(self),turns=turns))
    CodexSpiralPolicy.at_threshold=at_threshold
    default_policy=CodexSpiralPolicy(**defaults)

    @dataclasses.dataclass(frozen=True)
    class CodexEvent:
        kind: str
        command: str = ""
        output: str = ""
        exit_code: int | None = None
        paths: tuple[str,...] = ()
        thread_id: str = ""

    def event(value): return CodexEvent(**dict(value,paths=tuple(value["paths"])))

    class CodexSpiralState:
        def __init__(self,recent_window=default_policy.recent_window):
            restore_state(self,call("state.new",recent_window=recent_window))

        def _call(self,name,**arguments):
            result=call("state."+name,self=state_data(self),**arguments)
            restore_state(self,result["self"])
            return result["value"]

        def update(self,event): return self._call("update",event=dataclasses.asdict(event))
        def decision(self,policy=default_policy): return self._call("decision",policy=dataclasses.asdict(policy))
        def snapshot(self): return self._call("snapshot")
        distinct_edit_files=property(lambda self:self._call("distinct_edit_files"))
        edit_churn=property(lambda self:self._call("edit_churn"))
        green_after_edit=property(lambda self:self._call("green_after_edit"))
        currently_green=property(lambda self:self._call("currently_green"))
        repeated_failure_count=property(lambda self:self._call("repeated_failure_count"))

    class CodexResumeGuard:
        def __init__(self,policy=default_policy,*,verbose=False):
            self._restore(call("guard.new",policy=dataclasses.asdict(policy),verbose=verbose))

        def _restore(self,value):
            self.policy=CodexSpiralPolicy(**value["policy"])
            if not hasattr(self,"state"): self.state=CodexSpiralState.__new__(CodexSpiralState)
            restore_state(self.state,value["state"])
            for name in ("verbose","nudges_done","pending_reason","events"): setattr(self,name,value[name])
            self.fired_reasons=set(value["fired_reasons"])

        def _call(self,name,**arguments):
            data=dict(vars(self),state=state_data(self.state),policy=dataclasses.asdict(self.policy),fired_reasons=list(self.fired_reasons))
            result=call("guard."+name,self=data,**arguments)
            self._restore(result["self"])
            for message in result.get("messages",[]): print(message)
            return result["value"]

        def monitor(self,line): return self._call("monitor",line=line)
        def continuation(self): return self._call("continuation")
        def telemetry(self): return self._call("telemetry")
        segment=property(lambda self:self._call("segment"))

    tracker=constants["DEFAULT_TRACKER"]
    class CodexEnvelopeTracker:
        def __init__(self,*,cold_start=tracker["cold_start"],floor=tracker["floor"],
                min_samples=tracker["min_samples"],percentile=tracker["percentile"],margin=tracker["margin"]):
            vars(self).update(call("tracker.new",fields=dict(cold_start=cold_start,floor=floor,
                min_samples=min_samples,percentile=percentile,margin=margin)))

        def _call(self,name,**arguments):
            result=call("tracker."+name,self=vars(self),**arguments)
            vars(self).update(result["self"])
            return result["value"]

        def update(self,tool_turns,resolved): return self._call("update",tool_turns=tool_turns,resolved=resolved)
        def snapshot(self): return self._call("snapshot")
        threshold=property(lambda self:self._call("threshold"))

    def shipped_policy(threshold): return CodexSpiralPolicy(**call("shipped_policy",threshold=threshold))
    def defensive_policy(threshold): return CodexSpiralPolicy(**call("defensive_policy",threshold=threshold))
    def profile_policy(profile,threshold): return CodexSpiralPolicy(**call("profile_policy",profile=profile,threshold=threshold))
    def profile_version(profile): return call("profile_version",profile=profile)
    def _normalize(text,limit=180): return call("_normalize",text=text,limit=limit)
    def _command_fingerprint(command): return call("_command_fingerprint",command=command)
    def green_is_trustworthy(command): return call("green_is_trustworthy",command=command)
    def verification_scopes(command):
        value=call("verification_scopes",command=command)
        return set(value) if value is not None else None
    def is_test_command(command): return call("is_test_command",command=command)
    def is_verification_command(command): return call("is_verification_command",command=command)
    def is_research_command(command): return call("is_research_command",command=command)
    def failure_signatures(output): return set(call("failure_signatures",output=output))
    def patch_paths(command): return tuple(call("patch_paths",command=command))
    def shell_patch_applied(command,output,exit_code): return call("shell_patch_applied",command=command,output=output,exit_code=exit_code)
    def parse_events(line): return [event(value) for value in call("parse_events",line=line)]

    def iter_events(lines,*,first_thread_only=False):
        cursor=None
        for line in lines:
            value=call("iterator",self=cursor,line=line,first_thread_only=first_thread_only)
            cursor=value["self"]
            yield from (event(item) for item in value["events"])
            if cursor["finished"]: return

    def replay_events(events,policy=default_policy):
        value=call("replay_events",events=[dataclasses.asdict(event) for event in events],policy=dataclasses.asdict(policy))
        return value["decision"],restore_state(CodexSpiralState.__new__(CodexSpiralState),value["state"])
    def _percentile(values,pct): return call("_percentile",values=values,pct=pct)
    def nudge_text(reason): return call("nudge_text",reason=reason)

    return SimpleNamespace(CodexEvent=CodexEvent,CodexSpiralPolicy=CodexSpiralPolicy,
        CodexSpiralState=CodexSpiralState,CodexResumeGuard=CodexResumeGuard,CodexEnvelopeTracker=CodexEnvelopeTracker,
        DEFAULT_POLICY=default_policy,SHIPPED_POLICY=CodexSpiralPolicy(**constants["SHIPPED_POLICY"]),
        DEFENSIVE_POLICY=CodexSpiralPolicy(**constants["DEFENSIVE_POLICY"]),
        PROFILE_VERSION=constants["PROFILE_VERSION"],DEFENSIVE_PROFILE_VERSION=constants["DEFENSIVE_PROFILE_VERSION"],
        CODEX_DEFAULT_PROFILE=constants["CODEX_DEFAULT_PROFILE"],shipped_policy=shipped_policy,
        defensive_policy=defensive_policy,profile_policy=profile_policy,profile_version=profile_version,
        _normalize=_normalize,_command_fingerprint=_command_fingerprint,green_is_trustworthy=green_is_trustworthy,
        verification_scopes=verification_scopes,is_test_command=is_test_command,is_verification_command=is_verification_command,
        is_research_command=is_research_command,failure_signatures=failure_signatures,patch_paths=patch_paths,
        shell_patch_applied=shell_patch_applied,parse_events=parse_events,iter_events=iter_events,
        replay_events=replay_events,_percentile=_percentile,nudge_text=nudge_text)
