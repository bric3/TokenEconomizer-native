/** Pi SDK I/O for TokenEconomizer native. Session and response policy run in Rust. */
import type { ExtensionAPI, ExtensionContext } from "@earendil-works/pi-coding-agent";
import { spawn, spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = (() => {
  try { return dirname(fileURLToPath(import.meta.url)); }
  catch { return __dirname; }
})();
const SCRIPTS = join(HERE, "..", "scripts");
export function launch(script: string, args: string[], platform: string = process.platform): [string, string[]] {
  return [join(SCRIPTS, script + (platform === "win32" ? ".exe" : "")), args];
}

// Project SDK entries without copying conversation messages or image bodies into IPC.
function branch(entries: any[]) {
  return entries.map((entry) => ({
    type: entry?.type,
    message: entry?.message && { role: entry.message.role, sections: entry.message.sections },
    systemMessage: entry?.systemMessage && { sections: entry.systemMessage.sections },
  }));
}
export function carriedSection(entries: any[]): string {
  const [program, argv] = launch("te", ["--pi-carried-section"]);
  const out = spawnSync(program, argv, { input: JSON.stringify(branch(entries)), encoding: "utf8", windowsHide: true });
  if (out.error || out.status !== 0) throw out.error ?? new Error(out.stderr);
  return JSON.parse(out.stdout);
}

function connection() {
  let child: ReturnType<typeof spawn> | undefined;
  // shortcut: backend failure requires a reload, restore state before declaring production parity.
  let failed = false;
  let next = 0;
  let buffer = "";
  const pending = new Map<number, (value: any) => void>();
  const references = (active: boolean) => {
    for (const handle of [child, child?.stdin, child?.stdout] as any[]) {
      if (active) handle?.ref?.(); else handle?.unref?.();
    }
  };
  const stop = () => {
    failed = true;
    child?.kill("SIGKILL");
    for (const resolve of pending.values()) resolve({});
    pending.clear();
    references(false);
  };
  return (event: string, context: any, body: any): Promise<any> => new Promise((resolve) => {
    if (failed) { resolve({}); return; }
    try {
      if (!child) {
        const [program, argv] = launch("te", ["--pi-bridge"]);
        child = spawn(program, argv, { stdio: ["pipe", "pipe", "ignore"], cwd: SCRIPTS, windowsHide: true });
        child.on("error", stop);
        child.on("close", stop);
        child.stdin!.on("error", stop);
        child.stdout!.setEncoding("utf8");
        child.stdout!.on("data", (chunk: string) => {
          buffer += chunk;
          let end;
          while ((end = buffer.indexOf("\n")) >= 0) {
            const line = buffer.slice(0, end); buffer = buffer.slice(end + 1);
            try {
              const packet = JSON.parse(line);
              const waiter = pending.get(packet.id);
              if (!waiter) { stop(); return; }
              pending.delete(packet.id);
              waiter(packet.value ?? {});
              if (!pending.size) references(false);
            } catch { stop(); return; }
          }
        });
      }
      const id = ++next;
      let process_cwd: string | undefined;
      try { process_cwd = process.cwd(); } catch { /* A removed cwd makes hook launches fail open. */ }
      const packet = JSON.stringify({ id, event, context: { ...context, env: process.env, process_cwd }, body });
      pending.set(id, resolve);
      references(true);
      child.stdin!.write(packet + "\n");
    } catch { stop(); resolve({}); }
  });
}

export default function tokeneconomizer(pi: ExtensionAPI) {
  const request = connection();
  const context = (ctx: ExtensionContext) => ({ session_id: ctx.sessionManager.getSessionId(),
    transcript_path: ctx.sessionManager.getSessionFile() ?? "", cwd: ctx.cwd, mode: ctx.mode });
  const apply = (response: any, ctx: ExtensionContext, event: any) => {
    for (const action of response?.actions ?? []) {
      if (action.method === "notify") {
        if (ctx.hasUI) ctx.ui.notify(action.text, "info"); else console.error(action.text);
      } else if (action.method === "steer") pi.sendMessage(action.message, action.options);
      else if (action.method === "sections") event.systemPromptOptions.sections = {
        ...(event.systemPromptOptions.sections ?? {}), ...action.sections };
    }
  };
  pi.on("session_start", async (event, ctx) => apply(await request("session_start", context(ctx),
    { reason: event.reason, entries: branch((ctx.sessionManager.getBranch?.() ?? []) as any[]) }), ctx, event));
  pi.on("before_agent_start", async (event, ctx) => apply(await request("before_agent_start", context(ctx),
    { prompt: event.prompt }), ctx, event));
  pi.on("tool_result", async (event, ctx) => apply(await request("tool_result", context(ctx), {
    toolName: event.toolName, toolCallId: event.toolCallId, input: event.input, isError: event.isError,
    structuredContent: event.structuredContent && { exit_code: (event.structuredContent as any).exit_code },
    content: (event.content ?? []).map((part: any) => ({ type: part?.type, text: part?.text })),
  }), ctx, event));
  pi.on("agent_end", async (event, ctx) => apply(await request("agent_end", context(ctx), {}), ctx, event));
  pi.on("session_shutdown", async (event, ctx) => apply(await request("session_shutdown", context(ctx),
    { reason: event.reason }), ctx, event));
  pi.registerCommand("te", {
    description: "TokenEconomizer: status | microskills | config | repos (what it learned here, what it did)",
    handler: async (args, ctx) => apply(await request("command", context(ctx), { args }), ctx, {}),
  });
}
