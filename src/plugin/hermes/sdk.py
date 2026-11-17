"""Hermes SDK binding for TokenEconomizer native. Gateway policy runs in Rust."""
import atexit
import json
import logging
import os
from pathlib import Path
import queue
import subprocess
import threading
import weakref

logger = logging.getLogger(__name__)
EXECUTABLE = Path(__file__).resolve().parent.parent / "scripts" / ("te.exe" if os.name == "nt" else "te")
SECTION_ID = "tokeneconomizer.facts"
_bridges = weakref.WeakSet()


class Bridge:
    def __init__(self):
        self._connection = None
        self._write = threading.Lock()
        self._next = 0
        _bridges.add(self)

    @staticmethod
    def _read(process, pending, guard):
        output = process.stdout
        try:
            for line in output:
                packet = json.loads(line)
                if not isinstance(packet, dict) or packet.get("id") is None:
                    raise ValueError("invalid native response frame")
                with guard:
                    waiter = pending.get(packet.get("id"))
                if waiter is not None:
                    waiter.put(packet)
        except (OSError, TypeError, ValueError):
            pass
        finally:
            output.close()
            try:
                if process.poll() is None:
                    process.terminate()
                process.wait(timeout=1)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
            except OSError:
                pass
            with guard:
                waiting = list(pending.values())
                pending.clear()
            for waiter in waiting:
                waiter.put({"error": "native bridge closed"})

    def _send(self, process, packet):
        with self._write:
            process.stdin.write(json.dumps(packet) + "\n")
            process.stdin.flush()

    def _request(self, method, args, default=None):
        connection, request_id, finished = None, None, False
        try:
            with self._write:
                if self._connection is None or self._connection[0].poll() is not None:
                    process = subprocess.Popen([str(EXECUTABLE), "--hermes-bridge"],
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                        text=True, encoding="utf-8", errors="replace", bufsize=1,
                        cwd=str(EXECUTABLE.parent),
                        **({"creationflags": subprocess.CREATE_NO_WINDOW} if os.name == "nt" else {}))
                    pending, guard = {}, threading.Lock()
                    self._connection = process, pending, guard
                    threading.Thread(target=self._read, args=(process, pending, guard), daemon=True).start()
                connection = self._connection
                process, pending, guard = connection
                self._next += 1
                request_id = self._next
                waiter = queue.Queue()
                with guard:
                    pending[request_id] = waiter
                try:
                    cwd = os.getcwd()
                except OSError:
                    cwd = None
                process.stdin.write(json.dumps({"id": request_id, "method": method, "args": args,
                    "env": dict(os.environ), "home": os.path.expanduser("~"), "cwd": cwd}) + "\n")
                process.stdin.flush()
            while True:
                packet = waiter.get()
                if "error" in packet:
                    finished = True
                    return default
                if "callback" not in packet:
                    finished = True
                    return packet.get("value")
                try:
                    if packet["callback"] == "home":
                        # Hermes binds its profile to the calling SDK thread.
                        try:
                            from hermes_constants import get_hermes_home
                            value = str(get_hermes_home())
                        except Exception:
                            value = None
                    elif packet["callback"] == "notice":
                        value = logger.info("%s", packet["args"][0])
                    else:
                        raise ValueError("unknown native callback")
                    reply = {"value": value}
                except Exception as error:
                    reply = {"error": str(error)}
                self._send(process, {"id": request_id, "reply": reply})
        except (OSError, TypeError, ValueError, subprocess.SubprocessError):
            return default
        finally:
            if connection is not None and request_id is not None:
                with connection[2]:
                    connection[1].pop(request_id, None)
                if not finished:
                    try:
                        self._send(connection[0], {"id": request_id, "cancel": True})
                    except (OSError, ValueError):
                        pass

    def section(self, info):
        return self._request("section", {"info": info}, "")

    def pre_llm_call(self, session_id="", user_message="", turn_id="", **_):
        return self._request("pre_llm_call", dict(session_id=session_id, user_message=user_message, turn_id=turn_id))

    def transform_tool_result(self, tool_name="", args=None, result=None, session_id="", turn_id="",
                              tool_call_id="", status=None, error_message=None, **_):
        return self._request("transform_tool_result", dict(tool_name=tool_name, args=args, result=result,
            session_id=session_id, turn_id=turn_id, tool_call_id=tool_call_id, status=status, error_message=error_message))

    def on_session_end(self, session_id="", turn_id="", completed=False, interrupted=False, failed=False, **_):
        return self._request("on_session_end", dict(session_id=session_id, turn_id=turn_id,
            completed=completed, interrupted=interrupted, failed=failed))

    def on_session_finalize(self, session_id=None, reason="", **_):
        return self._request("on_session_finalize", dict(session_id=session_id, reason=reason))

    def close(self):
        with self._write:
            connection, self._connection = self._connection, None
            if connection is None:
                return
            process = connection[0]
            try:
                process.stdin.close()
            except OSError:
                pass
        try:
            process.wait(timeout=2)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()

    def __del__(self):
        self.close()


@atexit.register
def _close_bridges():
    for bridge in list(_bridges):
        bridge.close()


def register(ctx):
    bridge = Bridge()
    ctx.register_system_prompt_section(SECTION_ID, bridge.section, position="after_memory", max_chars=4000)
    ctx.register_hook("pre_llm_call", bridge.pre_llm_call)
    ctx.register_hook("transform_tool_result", bridge.transform_tool_result)
    ctx.register_hook("on_session_end", bridge.on_session_end)
    ctx.register_hook("on_session_finalize", bridge.on_session_finalize)
