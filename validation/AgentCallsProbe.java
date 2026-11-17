//JAVA 25
//SOURCES ContextProbe.java ProcessInputProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.Arrays;
import java.util.Comparator;
import java.util.List;
import java.util.regex.Pattern;

/** Runs native agent side calls against local CLI fixtures without models or credentials. */
class AgentCallsProbe {
  static final String PROMPT = "Native é😀\n&%PATH%";
  static String quote(Object value) { return ProcessInputProbe.quote(value); }
  static void require(boolean ok, String message) { if (!ok) throw new AssertionError(message); }
  static void child(String[] args) throws Exception {
    String host = args[1];
    var argv = Arrays.asList(args).subList(2, args.length);
    var root = Path.of(System.getenv("PROBE_ROOT"));
    if (host.equals("opencode") && argv.contains("--help")) {
      Files.writeString(root.resolve("helps"), "help\n", StandardOpenOption.CREATE, StandardOpenOption.APPEND);
      System.out.print("--pure --standalone");
      return;
    }
    var cwd = Path.of("").toAbsolutePath();
    require(Files.getPosixFilePermissions(cwd).equals(PosixFilePermissions.fromString("rwx------")), "Workspace is not private");
    require("1".equals(System.getenv("TE_SIDECALL")), "Hook recursion was left enabled");
    Files.writeString(root.resolve(host + "-cwd"), cwd.toString());
    if (host.equals("opencode") && argv.getFirst().equals("session")) {
      require(argv.equals(List.of("session", "delete", "native-session")), argv.toString());
      Files.writeString(root.resolve("deleted"), "deleted\n", StandardOpenOption.CREATE, StandardOpenOption.APPEND);
      System.exit(1); // A cleanup failure must preserve the priced reply.
    }
    require(argv.get(argv.indexOf(host.equals("codex") ? "-m" : "--model") + 1).equals("small"), "The model changed");
    require(argv.contains(PROMPT) || host.equals("droid") && argv.contains("legacy"), "The prompt was split or expanded");
    switch (host) {
      case "codex" -> {
        require(argv.subList(0, 8).equals(List.of("exec", "--json", "--disable", "hooks", "--ephemeral", "-s", "read-only", "--skip-git-repo-check")), argv.toString());
        require(argv.get(argv.indexOf("-c") + 1).equals("model_reasoning_effort=\"minimal\""), "Missing reasoning effort");
        System.out.println("junk\n[]\n{\"type\":\"item.completed\",\"item\":\"invalid\"}");
        System.out.println("{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"codex reply\"}}");
        System.out.println("{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":\"9007199254740993\",\"cached_input_tokens\":\"9007199254740992\",\"output_tokens\":0}}");
      }
      case "pi" -> {
        require(argv.containsAll(List.of("--no-tools", "--no-extensions", "--offline", "--no-session")), argv.toString());
        require(argv.get(argv.size() - 2).equals("--"), "The prompt option boundary disappeared");
        var extensions = java.util.stream.IntStream.range(0, argv.size() - 1).filter(i -> argv.get(i).equals("--extension")).mapToObj(i -> argv.get(i + 1)).toList();
        require(extensions.equals(List.of(root.resolve("home/ext.ts").toString(), "builtin:web", root.resolve("relative.ts").toString(), root.toString())), extensions.toString());
        Files.writeString(root.resolve("pi-attempts"), (argv.contains("--no-mcp") ? "new\n" : "old\n"), StandardOpenOption.CREATE, StandardOpenOption.APPEND);
        if (argv.contains("--no-mcp")) { System.err.print("Unknown option: --no-mcp"); System.exit(2); }
        System.out.print("{\"type\":\"message_end\",\"message\":{\"role\":\"assistant\",\"model\":\"small\",\"content\":[{\"type\":\"text\",\"text\":\"pi reply\"}],\"usage\":{\"input\":10,\"output\":2,\"cacheRead\":3,\"cacheWrite\":1,\"cost\":{\"total\":999}}}}");
      }
      case "cursor" -> {
        require(argv.subList(0, 6).equals(List.of("--print", "--output-format", "json", "--mode", "ask", "--trust")), argv.toString());
        System.out.print("{\"result\":\"cursor reply\"}");
      }
      case "grok" -> {
        require(argv.get(argv.indexOf("--tools") + 1).equals("read_file"), "The read-only allowlist changed");
        require(argv.get(argv.indexOf("--disallowed-tools") + 1).equals("use_tool,search_tool"), "MCP dispatchers were left enabled");
        require(argv.containsAll(List.of("Bash", "Edit", "Write", "WebFetch", "WebSearch", "Task")), "Missing denied actions");
        System.out.print("{\"text\":\"grok reply\",\"usage\":{\"input_tokens\":10,\"output_tokens\":2,\"cache_read_input_tokens\":3,\"cache_creation_input_tokens\":1},\"modelUsage\":{\"small\":{}}}");
      }
      case "opencode" -> {
        require(argv.containsAll(List.of("--pure", "--standalone", "--format", "json")), argv.toString());
        require(System.getenv("OPENCODE_CONFIG_CONTENT").contains("\"bash\": \"deny\""), "Tools that act were left enabled");
        System.out.println("{\"type\":\"text\",\"sessionID\":\"native-session\",\"part\":{\"text\":\"opencode reply\"}}");
        System.out.println("{\"type\":\"step_finish\",\"part\":{\"cost\":999,\"tokens\":{\"input\":10,\"output\":2,\"reasoning\":1,\"cache\":{\"read\":3,\"write\":1}}}}");
      }
      case "copilot" -> {
        require(argv.containsAll(List.of("--no-custom-instructions", "--disable-builtin-mcps", "--no-auto-update")), argv.toString());
        var disabled = java.util.stream.IntStream.range(0, argv.size() - 1).filter(i -> argv.get(i).equals("--disable-mcp-server")).mapToObj(i -> argv.get(i + 1)).toList();
        require(disabled.equals(List.of("acme-db", "z-server")), disabled.toString());
        require(java.util.Collections.frequency(argv, "--deny-tool") == 24 && java.util.Collections.frequency(argv, "--excluded-tools") == 24, "Copilot's tools were left enabled");
        var log = root.resolve("cp/session-state/native-copilot/events.jsonl");
        Files.createDirectories(log.getParent());
        Files.writeString(log, "{\"type\":\"session.start\",\"data\":{\"selectedModel\":\"small\"}}\n");
        new ProcessBuilder(Path.of(System.getProperty("java.home"), "bin/java").toString(), "-cp", System.getProperty("java.class.path"), "AgentCallsProbe", "--shutdown", root.toString())
            .directory(root.toFile()).redirectOutput(ProcessBuilder.Redirect.DISCARD).redirectError(ProcessBuilder.Redirect.DISCARD).start();
        System.out.println("{\"type\":\"assistant.message\",\"data\":[1]}");
        System.out.println("{\"type\":\"assistant.message\",\"data\":{\"content\":\"copilot reply\"}}");
        System.out.println("{\"type\":\"result\",\"sessionId\":\"native-copilot\"}");
      }
      case "droid" -> {
        require(argv.containsAll(List.of("exec", "--disable-builtin-skills", "--append-system-prompt")), argv.toString());
        require(argv.get(argv.indexOf("--only-tools") + 1).equals("Read"), "Droid's allowlist changed");
        if (argv.getLast().equals("legacy")) { System.out.print("  legacy reply  \n"); return; }
        var settings = root.resolve("droid-sessions/ws/native-droid.settings.json");
        Files.createDirectories(settings.getParent());
        Files.writeString(settings, "{\"model\":\"small\",\"tokenUsage\":{\"inputTokens\":10,\"outputTokens\":2,\"cacheReadTokens\":3,\"cacheCreationTokens\":1}}");
        System.out.println("[]\nnot JSON\n{\"session_id\":\"native-droid\",\"data\":{\"content\":\"droid reply\"}}");
      }
      default -> throw new AssertionError(host);
    }
  }
  static String cache(String result, String key) {
    int start = result.indexOf(quote(key) + ":");
    require(start >= 0, result);
    return result.substring(start + key.length() + 3, result.length() - 2);
  }
  static double charge(String result, String text) {
    var match = Pattern.compile("\\\"value\\\":\\[\\\"" + text + "\\\",([^\\]]+)\\]").matcher(result);
    require(match.find(), result);
    return Double.parseDouble(match.group(1));
  }
  public static void main(String[] args) throws Exception {
    if (args.length > 0 && args[0].equals("--shutdown")) {
      Thread.sleep(350);
      Files.writeString(Path.of(args[1]).resolve("cp/session-state/native-copilot/events.jsonl"), "{\"type\":\"session.shutdown\",\"data\":{\"modelMetrics\":{\"small\":{\"requests\":{\"count\":1},\"usage\":{\"inputTokens\":14,\"cacheReadTokens\":3,\"cacheWriteTokens\":1,\"outputTokens\":2}}}}}\n", StandardOpenOption.APPEND);
      return;
    }
    if (args.length > 0 && args[0].equals("--fixture")) { child(args); return; }
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"), "agent-calls-");
    var java = Path.of(System.getProperty("java.home"), "bin/java");
    try (var client = new ProcessInputProbe.Client(checkout.resolve("target/debug/te-bridge"))) {
      var common = "\"model\":\"small\",\"effort\":\"minimal\",\"prompt\":" + quote(PROMPT) + ",\"root\":" + quote(root.resolve("state"))
          + ",\"env\":{\"PROBE_ROOT\":" + quote(root) + ",\"COPILOT_HOME\":" + quote(root.resolve("cp")) + ",\"DROID_SESSIONS_DIR\":" + quote(root.resolve("droid-sessions")) + "},\"process_cwd\":" + quote(root) + ",\"home\":" + quote(root.resolve("home"));
      Files.createDirectories(root.resolve("cp"));
      Files.writeString(root.resolve("cp/mcp-config.json"), "{\"mcpServers\":{\"z-server\":{},\"github-mcp-server\":{},\"acme-db\":{}}}");
      for (var host : List.of("codex", "pi", "cursor", "grok", "opencode", "copilot", "droid")) {
        var wrapper = root.resolve(host);
        Files.writeString(wrapper, "#!/bin/sh\nexec '" + java.toString().replace("'", "'\\''") + "' -cp '" + System.getProperty("java.class.path").replace("'", "'\\''") + "' AgentCallsProbe --fixture " + host + " \"$@\"\n");
        Files.setPosixFilePermissions(wrapper, PosixFilePermissions.fromString("rwx------"));
        var extra = host.equals("pi") ? ",\"extensions\":[\"~/ext.ts\",\"builtin:web\",\"relative.ts\",\"\"]" : "";
        var call = common + ",\"cli\":" + quote(wrapper) + extra;
        var result = client.call("sidecall." + host + "_call", call);
        require(!result.contains("\"raised\"") && result.contains(host + " reply"), host + ": " + result + (Files.exists(root.resolve("state/errors.log")) ? "\n" + Files.readString(root.resolve("state/errors.log")) : ""));
        switch (host) {
          case "codex" -> require(charge(result, "codex reply") == 1.0 + 9_007_199_254_740_992d * 0.1, result);
          case "pi", "grok", "copilot", "droid" -> require(charge(result, host + " reply") == 23.55, result);
          case "opencode" -> {
            require(charge(result, "opencode reply") == 29.55, result);
            var second = client.call("sidecall.opencode_call", call + ",\"oc_flags\":" + cache(result, "oc_flags"));
            require(charge(second, "opencode reply") == 29.55, second);
            ContextProbe.expect(Files.readString(root.resolve("helps")), "help\n");
            require(Files.readAllLines(root.resolve("deleted")).size() == 2, "A completed session was not deleted");
          }
          case "cursor" -> require(result.contains("\"value\":[\"cursor reply\",null]"), result);
        }
        require(!Files.exists(Path.of(Files.readString(root.resolve(host + "-cwd")))), "Workspace was retained for " + host);
        if (host.equals("droid")) {
          var legacy = client.call("sidecall.droid_call", call.replace(quote(PROMPT), quote("legacy")));
          require(legacy.contains("\"value\":[\"legacy reply\",null]"), legacy);
          require(!Files.exists(Path.of(Files.readString(root.resolve("droid-cwd")))), "Legacy Droid call retained its workspace");
        }
      }
      ContextProbe.expect(Files.readString(root.resolve("pi-attempts")), "new\nold\n");
      var expansion = client.call("sidecall.pi_extension_args", "\"home\":\"/\",\"process_cwd\":" + quote(root) + ",\"extensions\":[\"~\",\"~te-native-unknown-user/file\",\"~" + System.getProperty("user.name") + "/x\"]");
      require(expansion.contains("\"/\"") && expansion.contains(root.resolve("~te-native-unknown-user/file").toString()) && expansion.contains(Path.of(System.getProperty("user.home")).resolve("x").toString()), expansion);
      System.out.println("Native seven-host side calls, delayed Copilot usage, Droid settings, pricing, CLI fallbacks, home expansion and cleanup passed");
    } finally {
      try (var paths = Files.walk(root)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path); }
    }
  }
}
