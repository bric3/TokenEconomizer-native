/** opencode SDK I/O. Session, redirect and export policy runs in Rust. */
import type { Plugin } from "@opencode-ai/plugin";
import { spawn, spawnSync } from "node:child_process";
import { randomBytes } from "node:crypto";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = (() => { try { return dirname(fileURLToPath(import.meta.url)); } catch { return __dirname; } })();
const SCRIPTS = join(HERE, "..", "scripts");
const HIDDEN = { windowsHide: true } as const;
function launch(script: string, args: string[], platform: string = process.platform): [string, string[]] {
  return [join(SCRIPTS, script + (platform === "win32" ? ".exe" : "")), args];
}
function context() {
  let process_cwd: string | undefined;
  try { process_cwd = process.cwd(); } catch { /* A removed working directory makes child launches fail open. */ }
  return { env: process.env, argv: process.argv, home: homedir(), exec_path: process.execPath, process_cwd, windows_hide: HIDDEN.windowsHide };
}

async function connection(version: number, client?: any) {
  const token = randomBytes(32).toString("hex");
  let child: ReturnType<typeof spawn> | undefined;
  let port: number | undefined;
  let bootstrap: any;
  let generation = 0;
  let restored = false;
  let next = 0;
  let buffer = "";
  const checkpoint = new Map<string, any>();
  const pending = new Map<number, { resolve: (value: any) => void; reject: (error: Error) => void; mutate?: (action: any) => void }>();
  const references = (active: boolean) => {
    for (const handle of [child, child?.stdin, child?.stdout] as any[]) {
      if (active) handle?.ref?.(); else handle?.unref?.();
    }
  };
  let ready: (value: void) => void;
  let startup: Promise<void>;
  let timer: ReturnType<typeof setTimeout>;
  const stop = (running = child) => {
    if (!running || running !== child) return;
    clearTimeout(timer);
    references(false); child = undefined; port = undefined; buffer = "";
    running.kill("SIGKILL");
    for (const waiter of pending.values()) waiter.resolve({});
    pending.clear(); ready();
  };
  const write = (packet: any) => { child!.stdin!.write(JSON.stringify(packet) + "\n"); };
  const start = () => {
    startup = new Promise<void>((resolve) => { ready = resolve; });
    restored = generation++ === 0;
    try {
      const [program, argv] = launch("te", ["--opencode-bridge", String(version)]);
      // A Unix backend drains accepted I/O after its SDK worker closes the pipes.
      child = spawn(program, argv, { cwd: SCRIPTS, detached: process.platform !== "win32", ...HIDDEN, stdio: ["pipe", "pipe", "ignore"],
        env: { ...process.env, TE_NATIVE_CONTROL_TOKEN: token } });
      const running = child;
      child.on("error", () => stop(running)); child.on("close", () => stop(running)); child.stdin!.on("error", () => stop(running));
      child.stdout!.setEncoding("utf8");
      child.stdout!.on("data", (chunk: string) => {
        if (child !== running) return;
        buffer += chunk;
        let end;
        while ((end = buffer.indexOf("\n")) >= 0) {
          const line = buffer.slice(0, end); buffer = buffer.slice(end + 1);
          try {
            const packet = JSON.parse(line);
            if (packet.checkpoint) {
              if (!Array.isArray(packet.checkpoint) || packet.checkpoint.some((row: any) =>
                !Array.isArray(row) || row.length < 1 || row.length > 2 || typeof row[0] !== "string")) { stop(running); return; }
              for (const row of packet.checkpoint) {
                if (row.length === 1) checkpoint.delete(row[0]); else checkpoint.set(row[0], row[1]);
              }
              continue;
            }
            if (port === undefined) {
              if (!Number.isInteger(packet.ready) || packet.ready < 1 || packet.ready > 65535) { stop(running); return; }
              port = packet.ready; clearTimeout(timer); ready(); references(false); continue;
            }
            const waiter = pending.get(packet.id);
            if (!waiter) { stop(running); return; }
            if (packet.callback) {
              (async () => {
                try {
                  let value;
                  if (packet.callback === "context") value = context();
                  else if (packet.callback === "apply") {
                    if (!waiter.mutate) throw new Error("SDK mutation callback is missing");
                    waiter.mutate(packet.args[0]);
                  }
                  else if (packet.callback === "stderr") console.error(packet.args[0]);
                  else if (packet.callback === "toast") await client.tui.showToast({ body: { message: packet.args[0], variant: "info" } });
                  else throw new Error("Unknown SDK callback: " + packet.callback);
                  if (child === running) write({ id: packet.id, reply: { value } });
                } catch (error) { if (child === running) write({ id: packet.id, reply: { error: String(error) } }); }
              })();
            } else {
              pending.delete(packet.id);
              if (packet.error) waiter.reject(new Error(packet.error)); else waiter.resolve(packet.value ?? {});
              if (!pending.size) references(false);
            }
          } catch { stop(running); return; }
        }
      });
      timer = setTimeout(() => stop(running), 60_000);
    } catch { if (child) stop(); else ready(); }
  };
  return {
    request: async (method: string, body: any, mutate?: (action: any) => void): Promise<any> => {
      const caller = context();
      if (method === "init") bootstrap = body;
      if (!child) start();
      const running = child;
      await startup;
      if (!running || child !== running) return {};
      return new Promise((resolve, reject) => {
        const id = ++next; pending.set(id, { resolve, reject, mutate }); references(true);
        try {
          const restore = restored ? undefined : [...checkpoint];
          restored = true;
          write({ id, method, body, context: caller, restore, bootstrap: restore ? bootstrap : undefined });
        } catch { stop(running); }
      });
    },
    exit: () => {
      if (bootstrap === undefined) return;
      const [program, argv] = launch("te", ["--opencode-exit"]);
      try { spawnSync(program, argv, { ...HIDDEN, input: JSON.stringify({ port, token, context: context(), version, restore: [...checkpoint], bootstrap }), stdio: ["pipe", "ignore", "ignore"] }); }
      catch { /* fail open */ }
    },
  };
}

// Project fields read by Rust without copying image bodies or unrelated SDK metadata.
function content(parts: any) {
  return Array.isArray(parts) ? parts.map((p) => typeof p === "string" ? p : p == null ? p : { type: p.type, text: p.text }) : parts;
}
function fields(value: any): any {
  if (value == null) return value;
  return { sessionID: value.sessionID, assistantMessageID: value.assistantMessageID, id: value.id, name: value.name,
    info: value.info && { id: value.info.id, parentID: value.info.parentID }, input: value.input,
    model: value.model && { id: value.model.id, modelID: value.model.modelID }, cost: value.cost, tokens: value.tokens,
    content: content(value.content), metadata: value.metadata, error: value.error && { message: value.error.message } };
}
function eventBody(event: any) {
  return { ...fields(event), type: event?.type, properties: fields(event?.properties), data: fields(event?.data) };
}
function apply(response: any, output: any, event?: any) {
  for (const action of response?.actions ?? []) {
    if (action.method === "append_system") output.system.push(action.value);
    else if (action.method === "set_output") output.output = action.value;
    else if (action.method === "set_content") event.result.content = action.value;
    else if (action.method === "append_content") event.result.content = [...event.result.content, action.value];
  }
}

const server: Plugin = async ({ client, directory }) => {
  const peer = await connection(1, client);
  await peer.request("init", { directory });
  process.once("exit", peer.exit);
  return {
    event: async ({ event }: { event: any }) => { await peer.request("event", eventBody(event)); },
    "chat.message": async (input: any, output: any) => { await peer.request("prompt", { sessionID: input?.sessionID, output: { parts: content(output?.parts) } }); },
    "experimental.chat.system.transform": async (input: any, output: any) => apply(await peer.request("context", { sessionID: input?.sessionID }), output),
    "tool.execute.after": async (input: any, output: any) => { await peer.request("tool", {
      sessionID: input?.sessionID, tool: input?.tool, callID: input?.callID, args: input?.args,
      output: { output: output?.output, metadata: output?.metadata && { exit: output.metadata.exit } },
    }, (action) => apply({ actions: [action] }, output)); },
  };
};
const setup = async (ctx: any) => {
  const peer = await connection(2);
  await peer.request("init", { directory: ctx.location?.directory, path: ctx.location?.path, process_cwd: context().process_cwd });
  process.once("exit", peer.exit);
  try { await ctx.session.hook("prompt", async (e: any) => { await peer.request("prompt", {
    sessionID: e.sessionID, messageID: e.messageID, prompt: e.prompt && { text: e.prompt.text },
  }); }); } catch { /* unavailable on this SDK release */ }
  try { await ctx.session.hook("context", async (e: any) => apply(await peer.request("context", {
    sessionID: e.sessionID, system_array: Array.isArray(e.system),
  }), e)); } catch { /* unavailable on this SDK release */ }
  try { await ctx.tool.hook("execute.after", async (e: any) => { await peer.request("tool", {
    sessionID: e.sessionID, status: e.status, tool: e.tool, id: e.id, input: e.input,
    error: e.error, errorMessage: e.error?.message,
    result: e.result && { content: content(e.result.content), output: e.result.output, metadata: e.result.metadata && { exit: e.result.metadata.exit } },
  }, (action) => apply({ actions: [action] }, {}, e)); }); } catch { /* unavailable on this SDK release */ }
  const controller = new AbortController();
  (async () => {
    try { for await (const event of ctx.event.subscribe({ signal: controller.signal })) await peer.request("event", eventBody(event)); }
    catch { /* subscription ended */ }
  })();
  return () => controller.abort();
};
export default { id: "tokeneconomizer", server, setup };
