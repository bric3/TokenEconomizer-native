//JAVA 25
//SOURCES ContextProbe.java ProcessInputProbe.java AgentCallsProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.Comparator;
import java.util.List;

/** Native runner policy and repeated calls; local fixtures never contact a model. */
class RunnerProbe {
  static String quote(Object value) { return ProcessInputProbe.quote(value); }
  static void require(boolean ok, String message) { if (!ok) throw new AssertionError(message); }
  static String state(String result) {
    int start = result.indexOf("\"runner\":");
    require(start >= 0, result);
    return result.substring(start + 9, result.length() - 2);
  }
  static String run(ProcessInputProbe.Client client, String common, String state, String extra) throws Exception {
    return client.call("sidecall.runner_run", common + ",\"runner\":" + state + ",\"prompt\":" + quote(AgentCallsProbe.PROMPT) + ",\"model\":\"requested\"" + extra);
  }
  public static void main(String[] args) throws Exception {
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"), "runner-");
    var config = "{\"paid_model\":\"small\",\"codex_paid_model\":\"specific\",\"codex_paid_effort\":\"minimal\",\"local_model\":\"fm\",\"local_roles\":[\"judge\"],\"pi_cli\":\"~/pi\",\"pi_extensions\":[\"~/ext.ts\",\"builtin:web\",\"relative.ts\",\"\"]}";
    var common = "\"root\":" + quote(root.resolve("state")) + ",\"home\":" + quote(root.resolve("home")) + ",\"process_cwd\":" + quote(root) + ",\"env\":{\"PROBE_ROOT\":" + quote(root) + "},\"macos\":true";
    try (var client = new ProcessInputProbe.Client(checkout.resolve("target/debug/te-bridge"))) {
      for (var host : List.of("claude", "codex", "copilot", "pi", "opencode", "cursor", "grok", "hermes", "antigravity", "droid", "unknown")) {
        var selected = client.call("sidecall.runner_for", common + ",\"host_name\":" + quote(host) + ",\"config\":" + config);
        if (List.of("antigravity", "droid").contains(host)) {
          require(selected.contains("\"value\":false"), selected);
        } else {
          var method = host.equals("unknown") ? "claude_call" : host + "_call";
          require(selected.contains("\"method\":" + quote(method)), selected);
          require(selected.contains(host.equals("codex") ? "\"model\":\"specific\",\"effort\":\"minimal\"" : "\"model\":\"small\""), selected);
        }
      }
      require(client.call("sidecall.runner_for", common + ",\"host_name\":\"droid\",\"config\":{\"droid_sidecall\":true}").contains("droid_call"), "Configured Droid opt-in was ignored");
      var localArgs = common + ",\"host_name\":\"codex\",\"role\":\"judge\",\"config\":" + config + ",\"transport\":{\"fm_available\":true}";
      var selected = client.resume(client.call("sidecall.runner_for", localArgs), "fm_available", "{\"value\":true}");
      require(selected.contains("\"title\":\"Verdict\"") && selected.contains("\"answered_locally\":false"), selected);
      var flags = ",\"transport\":{\"fm_call\":true,\"codex_call\":true}";
      var pending = run(client, common, state(selected), flags);
      require(pending.contains("\"schema\":{\"title\":\"Verdict\""), pending);
      var answered = client.resume(pending, "model_call", "{\"value\":[\"local reply\",0]}");
      require(answered.contains("\"value\":[\"local reply\",0.0]") && answered.contains("\"answered_locally\":true"), answered);
      pending = client.resume(run(client, common, state(answered), flags), "model_call", "{\"value\":[null,0]}");
      require(pending.contains("\"codex_call\"") && pending.contains("\"model\":\"specific\"") && pending.contains("\"effort\":\"minimal\""), pending);
      var fallback = client.resume(pending, "model_call", "{\"value\":[\"host reply\",7]}");
      require(fallback.contains("\"value\":[\"host reply\",7.0]") && fallback.contains("\"answered_locally\":false"), fallback);
      var empty = client.resume(run(client, common, state(fallback), flags), "model_call", "{\"value\":[\"\",0]}");
      require(empty.contains("\"value\":[\"\",0.0]") && empty.contains("\"answered_locally\":true"), empty);
      pending = client.resume(run(client, common, state(empty), flags), "model_call", "{\"value\":[null,0]}");
      var failed = client.resume(pending, "model_call", "{\"failure\":{\"type\":\"timeout\",\"timeout\":180}}");
      require(failed.contains("\"answered_locally\":false") && failed.contains("\"raised\""), failed);
      var log = root.resolve("state/errors.log");
      require(Files.readString(log).endsWith("sidecall codex: no reply (timed out after 180s)\n") && !Files.readString(log).contains(AgentCallsProbe.PROMPT), "Failure note exposed the prompt");

      var localOnly = client.resume(client.call("sidecall.local_runner", common + ",\"role\":\"judge\",\"config\":" + config + ",\"transport\":{\"fm_available\":true}"), "fm_available", "{\"value\":true}");
      pending = client.resume(run(client, common, state(localOnly), ",\"has_fallback\":true,\"transport\":{\"fm_call\":true}"), "model_call", "{\"value\":[null,0]}");
      require(pending.contains("\"args\":[" + quote(AgentCallsProbe.PROMPT) + ",\"requested\"]"), pending);
      require(client.resume(pending, "fallback", "{\"value\":[\"external\",3]}").contains("\"value\":[\"external\",3.0]"), "External fallback reply changed");
      require(client.resume(run(client, common, state(localOnly), ",\"transport\":{\"fm_call\":true}"), "model_call", "{\"value\":[null,0]}").contains("\"value\":[\"\",0.0]"), "Missing fallback did not return an empty reply");

      Files.createDirectories(root.resolve("home"));
      var wrapper = root.resolve("home/pi");
      Files.writeString(wrapper, "#!/bin/sh\nexec '" + Path.of(System.getProperty("java.home"), "bin/java").toString().replace("'", "'\\''") + "' -cp '" + System.getProperty("java.class.path").replace("'", "'\\''") + "' AgentCallsProbe --fixture pi \"$@\"\n");
      Files.setPosixFilePermissions(wrapper, PosixFilePermissions.fromString("rwx------"));
      var pi = client.call("sidecall.runner_for", common + ",\"host_name\":\"pi\",\"config\":" + config);
      var nativeReply = run(client, common, state(pi), "");
      require(AgentCallsProbe.charge(nativeReply, "pi reply") == 23.55, nativeReply);
      require(!Files.exists(Path.of(Files.readString(root.resolve("pi-cwd")))), "Runner retained its private workspace");
      Files.delete(wrapper);
      var missing = run(client, common, state(pi), "");
      require(missing.contains("\"name\":\"FileNotFoundError\"") && missing.contains("\"raised\""), missing);
      require(Files.readAllLines(log).size() == 2 && Files.readString(log).contains("sidecall pi: failed (FileNotFoundError:"), "Native I/O failure was not noted once");
      System.out.println("Native runner selection, judge schema, repeated local fallback, empty answers, external fallback, Pi execution and failure privacy passed");
    } finally {
      try (var paths = Files.walk(root)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path); }
    }
  }
}
