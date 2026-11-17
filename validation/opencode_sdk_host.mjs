/** SDK, stream and filesystem I/O fixture. The loaded plugin owns every session decision. */
import { createInterface } from "node:readline";
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const plugin = (await import(pathToFileURL(process.argv[2]).href)).default;
const version = Number(process.argv[3]);
const baseline = { ...process.env };
const handlers = new Map();
let actions = [], rejectToast = false, cleanup;
let observation;
const queue = [];
let wake, active = false;
const subscribe = ({ signal }) => {
  active = true;
  signal.addEventListener("abort", () => { active = false; wake?.(); });
  return (async function* () {
    while (active) {
      if (!queue.length) await new Promise((resolve) => { wake = resolve; });
      const item = queue.shift();
      if (item) { yield item.event; item.resolve(); }
    }
  })();
};
const observed = () => observation ? { observed: JSON.parse(JSON.stringify(observation)) } : {};
console.error = (text) => actions.push({ method: "stderr", text, ...observed() });
const client = { tui: { showToast: async (packet) => {
  if (rejectToast) throw new Error("toast unavailable");
  actions.push({ method: "toast", packet, ...observed() });
} } };
if (version === 1) {
  const api = await plugin.server({ client, directory: baseline.TE_OC_WORK });
  for (const [name, callback] of Object.entries(api)) handlers.set(name, callback);
} else {
  const unavailable = JSON.parse(baseline.TE_OC_UNAVAILABLE ?? "[]");
  cleanup = await plugin.setup({ location: { directory: baseline.TE_OC_WORK },
    session: { hook: async (name, callback) => { if (unavailable.includes(name)) throw new Error(name); handlers.set(name, callback); } },
    tool: { hook: async (name, callback) => { if (unavailable.includes(name)) throw new Error(name); handlers.set(name, callback); } },
    event: { subscribe },
  });
}
const canonical = (value) => JSON.stringify(value, (_key, v) => v && typeof v === "object" && !Array.isArray(v)
  ? Object.fromEntries(Object.entries(v).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0)) : v);
async function invoke(request) {
  process.env = { ...baseline, ...request.env };
  rejectToast = request.rejectToast ?? false;
  observation = undefined;
  if (request.method === "exit") { process.emit("exit", 0); return; }
  if (request.method === "kill") {
    const file = baseline.TE_OC_LOG + ".backend";
    if (existsSync(file)) {
      process.kill(Number(readFileSync(file, "utf8")), "SIGKILL");
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
    return;
  }
  if (request.chdir) { process.chdir(request.chdir); return; }
  if (request.writeExport) {
    const path = join(baseline.TOKENECONOMIZER_HOME, "opencode-sessions", request.writeExport.id + ".json");
    mkdirSync(join(baseline.TOKENECONOMIZER_HOME, "opencode-sessions"), { recursive: true });
    writeFileSync(path, request.writeExport.text); return;
  }
  if (request.method === "cleanup") { cleanup?.(); return; }
  if (request.method === "stream") {
    if (!active) return "closed";
    await new Promise((resolve) => { queue.push({ event: request.body, resolve }); wake?.(); });
    return;
  }
  const callback = handlers.get(request.method);
  if (!callback) return "unavailable";
  const input = request.input ?? {}, output = request.output ?? {}, body = request.body ?? {};
  if (request.observe) observation = version === 1 ? output : body;
  if (version === 1 && request.method === "event") await callback({ event: body });
  else if (version === 1) await callback(input, output);
  else await callback(body);
  if (request.release) writeFileSync(baseline.TE_OC_LOG + ".barrier-" + request.release, "done");
  return version === 1 ? output : body;
}
function exports() {
  const dir = join(baseline.TOKENECONOMIZER_HOME, "opencode-sessions");
  if (!existsSync(dir)) return {};
  return Object.fromEntries(readdirSync(dir).map((name) => {
    const text = readFileSync(join(dir, name), "utf8");
    let value;
    try { value = JSON.parse(text); } catch { value = { raw: text }; }
    return [name, { value, mode: statSync(join(dir, name)).mode & 0o777 }];
  }));
}
for await (const line of createInterface({ input: process.stdin })) {
  const request = JSON.parse(line);
  actions = [];
  writeFileSync(baseline.TE_OC_LOG, "");
  try {
    const value = request.batch ? await Promise.all(request.batch.map(invoke)) : await invoke(request);
    const calls = readFileSync(baseline.TE_OC_LOG, "utf8").trim().split("\n").filter(Boolean).map(JSON.parse)
      .sort((a, b) => canonical(a).localeCompare(canonical(b)));
    process.stdout.write(canonical({ value, actions, calls, exports: exports() }) + "\n");
  } catch (error) { process.stdout.write(canonical({ error: String(error) }) + "\n"); }
}
