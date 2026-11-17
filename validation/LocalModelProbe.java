//JAVA 25
//SOURCES ContextProbe.java ProcessInputProbe.java

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.Arrays;
import java.util.Comparator;

/** Native on-device command checks against a local fixture, without invoking the system model. */
class LocalModelProbe {
  static String quote(Object value) { return ProcessInputProbe.quote(value); }
  static void require(boolean ok, String message) { if (!ok) throw new AssertionError(message); }
  static void fixture(String[] args) throws Exception {
    var root = Path.of(System.getenv("PROBE_ROOT"));
    require("1".equals(System.getenv("TE_SIDECALL")), "Missing recursion guard");
    var argv = Arrays.asList(args).subList(1, args.length);
    if (argv.equals(java.util.List.of("available"))) {
      Files.writeString(root.resolve("available"), "probe\n", StandardOpenOption.CREATE, StandardOpenOption.APPEND);
      System.out.print("System model available");
      return;
    }
    require(argv.subList(0, 3).equals(java.util.List.of("respond", "--no-stream", "--greedy")), argv.toString());
    var cwd = Path.of("").toAbsolutePath();
    require(Files.getPosixFilePermissions(cwd).equals(PosixFilePermissions.fromString("rwx------")), "Workspace is not private");
    Files.writeString(root.resolve("cwd"), cwd.toString());
    if (argv.contains("--schema")) {
      var schema = Path.of(argv.get(argv.indexOf("--schema") + 1));
      require(Files.isSameFile(schema.getParent(), cwd) && Files.readString(schema).contains("\"required\":[\"stuck\"]"), "Schema file changed");
    }
    var prompt = new String(System.in.readAllBytes(), StandardCharsets.UTF_8);
    require(!argv.contains(prompt), "Prompt leaked onto argv");
    if (prompt.equals("SECRET")) { Thread.sleep(10_000); return; }
    if (prompt.equals("error")) { System.out.print("\u001b[1mError: context limit\u001b[0m"); return; }
    ContextProbe.expect(prompt, "Native é😀\n&%PATH%");
    System.out.print("\u001b[1m{\"stuck\":false}\u001b[0m\n");
  }
  public static void main(String[] args) throws Exception {
    if (args.length > 0 && args[0].equals("--fixture")) { fixture(args); return; }
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"), "local-model-");
    var cli = root.resolve("fixture-fm");
    var java = Path.of(System.getProperty("java.home"), "bin/java");
    Files.writeString(cli, "#!/bin/sh\nexec '" + java.toString().replace("'", "'\\''") + "' -cp '" + System.getProperty("java.class.path").replace("'", "'\\''") + "' LocalModelProbe --fixture \"$@\"\n");
    Files.setPosixFilePermissions(cli, PosixFilePermissions.fromString("rwx------"));
    var common = "\"cli\":" + quote(cli) + ",\"env\":{\"PROBE_ROOT\":" + quote(root) + "},\"root\":" + quote(root.resolve("state")) + ",\"process_cwd\":" + quote(root);
    try (var client = new ProcessInputProbe.Client(checkout.resolve("target/debug/te-bridge"))) {
      require(client.call("sidecall.fm_available", common + ",\"macos\":false").contains("\"value\":false"), "Local model probed outside macOS");
      require(!Files.exists(root.resolve("available")), "Platform gate started a process");
      var available = client.call("sidecall.fm_available", common + ",\"macos\":true");
      require(available.contains("\"value\":true"), available);
      var key = "\"fm_ok\":";
      var cache = available.substring(available.indexOf(key) + key.length(), available.length() - 2);
      Files.setPosixFilePermissions(cli, PosixFilePermissions.fromString("rw-------"));
      require(client.call("sidecall.fm_available", common + ",\"macos\":true,\"fm_ok\":" + cache).contains("\"value\":true"), "Availability cache was ignored");
      ContextProbe.expect(Files.readString(root.resolve("available")), "probe\n");
      Files.setPosixFilePermissions(cli, PosixFilePermissions.fromString("rwx------"));
      var reply = client.call("sidecall.fm_call", common + ",\"prompt\":" + quote("Native é😀\n&%PATH%") + ",\"schema\":{\"required\":[\"stuck\"]}");
      require(reply.contains("\"value\":[" + quote("{\"stuck\":false}") + ",0]"), reply);
      require(!Files.exists(Path.of(Files.readString(root.resolve("cwd")))), "Schema workspace was retained");
      require(client.call("sidecall.fm_call", common + ",\"prompt\":\"error\"").contains("\"value\":[null,0]"), "Error reply was accepted");
      var timed = client.call("sidecall.fm_call", common + ",\"prompt\":\"SECRET\",\"timeout\":0.5");
      require(timed.contains("\"value\":[null,0]"), timed);
      require(!Files.exists(Path.of(Files.readString(root.resolve("cwd")))), "Timed-out workspace was retained");
      var log = Files.readString(root.resolve("state/errors.log"));
      require(log.contains("TimeoutExpired") && log.contains("context limit") && !log.contains("SECRET"), log);
      System.out.println("Native local-model availability cache, schema, UTF-8 stdin, ANSI cleanup, zero cost and timeout privacy passed");
    } finally {
      try (var paths = Files.walk(root)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path); }
    }
  }
}
