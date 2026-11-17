/** SDK fixture I/O. The loaded extension owns all session and response policy. */
import { createInterface } from "node:readline";
import { readFileSync, writeFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const binding = await import(pathToFileURL(process.argv[2]).href);
const handlers = new Map();
let command;
let actions = [];
binding.default({
  on: (event, callback) => handlers.set(event, callback),
  registerCommand: (_name, spec) => { command = spec.handler; },
  sendMessage: (message, options) => actions.push({ method: "steer", message, options }),
});
console.error = (text) => actions.push({ method: "stderr", text });
const baseline = { ...process.env };
const canonical = (value) => JSON.stringify(value, (_key, v) =>
  v && typeof v === "object" && !Array.isArray(v)
    ? Object.fromEntries(Object.entries(v).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0)) : v);

async function invoke(request) {
  if (request.chdir) { process.chdir(request.chdir); return; }
  process.env = { ...baseline, ...request.env };
  const ctx = {
    cwd: request.cwd ?? baseline.HOME, mode: request.mode ?? "interactive", hasUI: request.hasUI ?? true,
    ui: { notify: (text, level) => actions.push({ method: "notify", text, level }) },
    sessionManager: { getSessionId: () => request.sid ?? "session", getSessionFile: () => request.file,
      getBranch: () => request.entries ?? [] },
  };
  const event = request.body ?? {};
  if (request.event === "command") await command(request.args, ctx);
  else await handlers.get(request.event)(event, ctx);
  return event.systemPromptOptions;
}
for await (const line of createInterface({ input: process.stdin })) {
  const request = JSON.parse(line);
  actions = [];
  writeFileSync(baseline.TE_PI_LOG, "");
  try {
    const options = request.batch ? await Promise.all(request.batch.map(invoke)) : await invoke(request);
    const calls = readFileSync(baseline.TE_PI_LOG, "utf8").trim().split("\n").filter(Boolean).map(JSON.parse)
      .sort((a, b) => canonical(a).localeCompare(canonical(b)));
    process.stdout.write(canonical({ actions, options, calls }) + "\n");
  } catch (error) {
    process.stdout.write(canonical({ error: String(error) }) + "\n");
  }
}
