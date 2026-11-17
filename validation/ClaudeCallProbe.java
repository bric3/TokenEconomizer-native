//JAVA 25
//SOURCES ContextProbe.java ProcessInputProbe.java

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.Arrays;
import java.util.Comparator;

/** Runs the native Claude side call through a local CLI fixture, without a model. */
class ClaudeCallProbe {
  static String quote(Object value) { return ProcessInputProbe.quote(value); }
  static void child(String[] args) throws Exception {
    var root = Path.of(System.getenv("PROBE_ROOT"));
    var argv = Arrays.asList(args);
    if (argv.contains("--help")) {
      Files.writeString(root.resolve("helps"), "help\n", StandardOpenOption.CREATE, StandardOpenOption.APPEND);
      System.out.print("--no-session-persistence --strict-mcp-config --effort");
      return;
    }
    for (var flag : new String[]{"--no-session-persistence", "--strict-mcp-config", "--mcp-config", "--tools"}) {
      if (!argv.contains(flag)) throw new AssertionError("Missing " + flag);
    }
    if (!argv.get(argv.indexOf("--model") + 1).equals("small") || !argv.get(argv.indexOf("--effort") + 1).equals("max")
        || !argv.get(argv.indexOf("--tools") + 1).isEmpty() || !"1".equals(System.getenv("TE_SIDECALL"))) throw new AssertionError("Wrong side-call settings");
    var config = Path.of(argv.get(argv.indexOf("--mcp-config") + 1));
    ContextProbe.expect(Files.readString(config), "{\"mcpServers\": {}}");
    var cwd = Path.of("").toAbsolutePath();
    if (!Files.isSameFile(cwd, config.getParent()) || !Files.getPosixFilePermissions(cwd).equals(PosixFilePermissions.fromString("rwx------"))) throw new AssertionError("The prompt workspace was not private");
    Files.writeString(root.resolve("cwd"), cwd.toString());
    var prompt = new String(System.in.readAllBytes(), StandardCharsets.UTF_8);
    if (prompt.equals("fail")) throw new IllegalStateException("fixture failed");
    if (prompt.equals("signed out")) {
      System.out.print("[{\"type\":\"result\",\"is_error\":true,\"result\":\"Please run /login\"}]");
      return;
    }
    ContextProbe.expect(prompt, "Native é😀\n&%PATH%");
    System.out.print("[{\"type\":\"assistant\"},{\"type\":\"result\",\"result\":\"fixture reply\",\"usage\":{\"input_tokens\":12,\"output_tokens\":3},\"modelUsage\":{\"claude-haiku-4-5\":{}}}]");
  }
  public static void main(String[] args) throws Exception {
    if (args.length > 0 && args[0].equals("--fixture")) { child(args); return; }
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"), "claude-call-");
    var wrapper = root.resolve("fixture-cli");
    var java = Path.of(System.getProperty("java.home"), "bin/java");
    Files.writeString(wrapper, "#!/bin/sh\nexec '" + java.toString().replace("'", "'\\''") + "' -cp '" + System.getProperty("java.class.path").replace("'", "'\\''") + "' ClaudeCallProbe --fixture \"$@\"\n");
    Files.setPosixFilePermissions(wrapper, PosixFilePermissions.fromString("rwx------"));
    var common = "\"cli\":" + quote(wrapper) + ",\"model\":\"small\",\"effort\":\"max\",\"root\":" + quote(root.resolve("state"))
        + ",\"env\":{\"PROBE_ROOT\":" + quote(root) + "},\"process_cwd\":" + quote(root) + ",\"home\":" + quote(root);
    try (var client = new ProcessInputProbe.Client(checkout.resolve("target/debug/te-bridge"))) {
      var result = client.call("sidecall.claude_call", common + ",\"prompt\":" + quote("Native é😀\n&%PATH%"));
      if (!result.contains("\"value\":[\"fixture reply\",27.0]")) throw new AssertionError(result);
      if (Files.exists(Path.of(Files.readString(root.resolve("cwd"))))) throw new AssertionError("A completed call retained its workspace");
      int helpStart = result.indexOf("\"help\":");
      if (helpStart < 0) throw new AssertionError(result);
      var cache = result.substring(helpStart + 7, result.length() - 2);
      var signedOut = client.call("sidecall.claude_call", common + ",\"help\":" + cache + ",\"prompt\":\"signed out\"");
      if (!signedOut.contains("\"value\":[\"\",null]") || !Files.readString(root.resolve("state/hosts/claude/sidecall.json")).contains("Please run /login")) throw new AssertionError(signedOut);
      ContextProbe.expect(Files.readString(root.resolve("helps")), "help\n");
      client.call("sidecall.claude_call", common + ",\"help\":" + cache + ",\"prompt\":\"fail\"");
      if (Files.exists(Path.of(Files.readString(root.resolve("cwd"))))) throw new AssertionError("A failed call retained its workspace");
      System.out.println("Native Claude stdin, MCP isolation, help caching, pricing, pause state and workspace cleanup passed");
    } finally {
      try (var paths = Files.walk(root)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path); }
    }
  }
}
