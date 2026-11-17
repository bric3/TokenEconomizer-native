//JAVA 25
//SOURCES ContextProbe.java ProcessInputProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.Arrays;
import java.util.Comparator;
import java.util.List;

/** Native measurement and verification with local CLI fixtures and captured request bodies. */
class ToolcheckProbe {
  static String quote(Object value) { return ProcessInputProbe.quote(value); }
  static void require(boolean ok, String message) { if (!ok) throw new AssertionError(message); }
  static String ascii(String value) { return value.replace("é", "\\u" + "00e9").replace("😀", "\\u" + "d83d\\u" + "de00"); }
  static void fixture(String[] args) throws Exception {
    var argv = Arrays.asList(args).subList(1, args.length);
    if (argv.equals(List.of("--help"))) { System.out.print("--no-session-persistence"); return; }
    if (argv.equals(List.of("--version"))) { System.out.print(" \u001c9.1\u0085ignored\u2028"); return; }
    require("1".equals(System.getenv("TE_SIDECALL")), "Missing recursion guard");
    if (argv.equals(List.of("features", "list"))) {
      require(Files.isSameFile(Path.of(""), Path.of(System.getenv("PROBE_ROOT"))), "Repository cwd changed");
      System.out.print("multi_agent stable false\nignored true\ngoals stable true\nview_image invalid false\n");
      System.exit(7); // The source contract reads feature state even on a non-zero exit.
    }
    var cwd = Path.of("").toAbsolutePath();
    require(Files.getPosixFilePermissions(cwd).equals(PosixFilePermissions.fromString("rwx------")), "Workspace is not private");
    Files.writeString(Path.of(System.getenv("PROBE_ROOT"), "workspaces"), cwd + "\n", StandardOpenOption.CREATE, StandardOpenOption.APPEND);
    if (argv.subList(0, 2).equals(List.of("debug", "prompt-input"))) {
      require(argv.getLast().equals("OK"), argv.toString());
      if (argv.contains("-c")) require(argv.containsAll(List.of("include_collaboration_mode_instructions=false", "agents.enabled=false", "features.multi_agent=false")), argv.toString());
      System.out.print("é😀".repeat(argv.contains("-c") ? 53 : 50));
      return;
    }
    require(argv.subList(0, 9).equals(List.of("-p", "Reply with exactly: OK", "--output-format", "json", "--model", "haiku", "--max-turns", "1", "--no-session-persistence")), argv.toString());
    boolean lean = argv.contains("--settings");
    if (lean) {
      require(argv.size() == 11, "The deny list reached argv");
      var path = Path.of(argv.getLast());
      require(Files.readString(path).contains(quote("mcp__srv__é😀")) || Files.readString(path).contains(quote(ascii("mcp__srv__é😀")).replace("\\\\u", "\\u")), "Settings lost the deny list");
      Files.writeString(Path.of(System.getenv("PROBE_ROOT"), "settings"), path.toString());
    }
    System.out.print(lean ? "{\"usage\":{\"input_tokens\":5},\"total_cost_usd\":0.00015}" : "{\"usage\":{\"input_tokens\":\"9007199254740993\",\"cache_read_input_tokens\":2,\"cache_creation_input_tokens\":3},\"total_cost_usd\":0.00015}");
  }
  public static void main(String[] args) throws Exception {
    if (args.length > 0 && args[0].equals("--fixture")) { fixture(args); return; }
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"), "toolcheck-");
    var wrapper = root.resolve("fixture");
    Files.writeString(wrapper, "#!/bin/sh\nexec '" + Path.of(System.getProperty("java.home"), "bin/java").toString().replace("'", "'\\''") + "' -cp '" + System.getProperty("java.class.path").replace("'", "'\\''") + "' ToolcheckProbe --fixture \"$@\"\n");
    Files.setPosixFilePermissions(wrapper, PosixFilePermissions.fromString("rwx------"));
    var common = "\"cli\":" + quote(wrapper) + ",\"env\":{\"PROBE_ROOT\":" + quote(root) + "},\"process_cwd\":" + quote(root) + ",\"home\":" + quote(root);
    try (var client = new ContextProbe.Client(checkout.resolve("target/debug/te-bridge"), "toolcheck")) {
      var claude = client.call("toolcheck.measure", common + ",\"host\":\"claude\",\"tools\":[" + quote("mcp__srv__é😀") + "]");
      require(claude.contains("\"prompt_tokens_full\":9007199254740998") && claude.contains("\"saved_per_request\":9007199254740993") && claude.contains("\"probe_cost_usd\":0.0003"), claude);
      require(!Files.exists(Path.of(Files.readString(root.resolve("settings")))), "Settings workspace was retained");
      var codex = client.call("toolcheck.measure", common + ",\"host\":\"codex\",\"tools\":[\"collaboration\"]");
      require(codex.contains("\"prompt_input_chars_full\":100") && codex.contains("\"prompt_input_chars_lean\":106") && codex.contains("\"saved_tokens_per_request_est\":-2"), codex);
      var pending = client.call("toolcheck.verify_codex", common + ",\"repo_root\":\".\",\"capabilities\":[\"collaboration\",\"goals\",\"image_media\"]");
      var verified = client.resume(pending, "which", "{\"value\":" + quote(wrapper) + "}");
      require(verified.contains("\"value\":{\"multi_agent\":false,\"goals\":true,\"view_image\":false}"), verified);
      require(client.call("toolcheck._copilot_version", common).contains("\"value\":\"9.1\""), "Version parsing changed");
      var names = client.call("toolcheck._request_tools", common + ",\"body\":{\"tools\":[{\"function\":{\"name\":\"nested\"},\"name\":\"outer\"},{\"function\":{},\"name\":\"fallback\"},{\"type\":false},{\"type\":\"\"},{}]}");
      require(names.contains("\"value\":[\"nested\",\"fallback\",false,\"\",null]"), names);

      var tools = "[{\"function\": {\"name\": \"acme-é😀\"}}, {\"name\": \"\"}, {\"type\": \"web_search\"}]";
      var messages = "[{\"role\": \"user\", \"content\": \"é😀\"}]";
      var body = "{\"tools\":" + tools + ",\"messages\":" + messages + "}";
      pending = client.call("toolcheck.measure", common + ",\"host\":\"copilot\",\"tools\":[\"acme\"],\"transport\":{\"copilot_capture\":true}");
      require(pending.contains("copilot_capture"), pending);
      pending = client.resume(pending, "probe", "{\"value\":" + body + "}");
      require(pending.contains("--disable-mcp-server") && pending.contains("acme"), pending);
      var measured = client.resume(pending, "probe", "{\"value\":{\"tools\":[],\"messages\":" + messages + "}}");
      require(measured.contains("\"request_chars_full\":" + (ascii(tools).length() + ascii(messages).length())) && measured.contains("\"saved_chars_per_request\":" + (ascii(tools).length() - 2)) && measured.contains("\"tools_full\":3"), measured);
      pending = client.call("toolcheck.verify_copilot", common + ",\"repo_root\":" + quote(root) + ",\"servers\":[\"acme\",\"other\"],\"transport\":{\"copilot_capture\":true}");
      var cp = client.resume(pending, "probe", "{\"value\":" + body + "}");
      require(cp.contains("\"value\":{\"acme\":1,\"other\":0}"), cp);
      for (var path : Files.readAllLines(root.resolve("workspaces"))) require(!Files.exists(Path.of(path)), "Probe retained its private workspace");
      System.out.println("Native Claude token sums, settings cleanup, Codex character counts and feature state, Copilot request sizes and tool-name precedence passed");
    } finally {
      try (var paths = Files.walk(root)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path); }
    }
  }
}
