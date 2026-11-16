//JAVA 25
//JAVA_OPTIONS --enable-native-access=ALL-UNNAMED

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.io.PrintWriter;
import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.Linker;
import java.lang.foreign.ValueLayout;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.Arrays;
import java.util.Comparator;
import java.util.concurrent.TimeUnit;
import java.util.regex.Pattern;

/** Native context state and detached process checks; fixtures make no agent or model calls. */
class ContextProbe {
  static class Client implements AutoCloseable {
    final Process process;
    final PrintWriter input;
    final BufferedReader output;
    final String namespace;
    Client(Path executable) throws Exception {
      this(executable, "context");
    }
    Client(Path executable, String namespace) throws Exception {
      this.namespace = namespace;
      process = new ProcessBuilder(executable.toString()).redirectError(ProcessBuilder.Redirect.INHERIT).start();
      input = new PrintWriter(new OutputStreamWriter(process.getOutputStream(), StandardCharsets.UTF_8));
      output = new BufferedReader(new InputStreamReader(process.getInputStream(), StandardCharsets.UTF_8));
    }
    String once(String op, String args) throws Exception {
      input.println("{\"op\":\"" + op + "\",\"args\":{" + args + "}}"); input.flush();
      var line = output.readLine();
      if (line == null) throw new AssertionError("Native context process exited");
      return line;
    }
    String call(String op, String args) throws Exception {
      var line = once(op, args);
      long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(10);
      while (line.contains("\"method\":\"pending\"")) {
        if (System.nanoTime() >= deadline) throw new AssertionError("Context job remained pending");
        line = once(namespace + ".resume", "\"job\":" + job(line));
      }
      return line;
    }
    String resume(String pending, String method, String reply) throws Exception {
      if (!pending.contains("\"method\":\"" + method + "\"")) throw new AssertionError(pending);
      return call(namespace + ".resume", "\"job\":" + job(pending) + ",\"reply\":" + reply);
    }
    public void close() throws Exception {
      input.close();
      if (!process.waitFor(2, TimeUnit.SECONDS)) process.destroyForcibly().waitFor();
      output.close();
    }
  }
  static String job(String line) {
    var match = Pattern.compile("\"job\":(\\d+)").matcher(line);
    if (!match.find()) throw new AssertionError(line);
    return match.group(1);
  }
  static String quote(String value) { return "\"" + value.replace("\\", "\\\\").replace("\"", "\\\"") + "\""; }
  static String quote(Path value) { return quote(value.toString()); }
  static String value(String result) {
    var prefix = "{\"result\":{\"value\":";
    if (!result.startsWith(prefix)) throw new AssertionError(result);
    return result.substring(prefix.length(), result.length() - 2);
  }
  static void expect(String actual, String expected) { if (!actual.equals(expected)) throw new AssertionError(actual + " != " + expected); }

  static void child(String[] args) throws Throwable {
    long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(10);
    while (!Files.exists(Path.of("release"))) {
      if (System.nanoTime() >= deadline) throw new AssertionError("Parent did not release the detached child");
      Thread.sleep(10);
    }
    var linker = Linker.nativeLinker();
    var getsid = linker.downcallHandle(linker.defaultLookup().find("getsid").orElseThrow(), FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.JAVA_INT));
    int pid = Math.toIntExact(ProcessHandle.current().pid());
    int sid = (int) getsid.invokeExact(pid);
    if (pid != sid) throw new AssertionError("The detached child did not start a new session");
    var expected = Arrays.asList("--child", "-m", "tokeneconomizer.plugin.cli", "fold", "space word", "a'b", "$HOME", "é😀", "--host", "claude", "--cwd", System.getenv("EXPECTED_CWD"));
    if (!Arrays.asList(args).equals(expected) || !Files.isSameFile(Path.of("."), Path.of(System.getenv("EXPECTED_CWD")))) throw new AssertionError("Child arguments or cwd changed");
    if (!"1".equals(System.getenv("PYTHONUTF8")) || !System.getenv("PYTHONPATH").equals(System.getenv("EXPECTED_PYTHONPATH"))) throw new AssertionError("Child environment changed");
    System.out.write("stdout é😀\n".getBytes(StandardCharsets.UTF_8));
    System.err.write("stderr é😀\n".getBytes(StandardCharsets.UTF_8));
    Files.writeString(Path.of("done"), "ok");
  }

  static void context(Path root, Path executable) throws Exception {
    var cwd = Files.createDirectories(root.resolve("context's directory"));
    var state = root.resolve("state");
    var core = root.resolve("core's path");
    var wrapper = root.resolve("launch-java");
    Files.writeString(wrapper, "#!/bin/sh\nexec '" + Path.of(System.getProperty("java.home"), "bin/java").toString().replace("'", "'\\''")
        + "' --enable-native-access=ALL-UNNAMED -cp '" + System.getProperty("java.class.path").replace("'", "'\\''") + "' ContextProbe --child \"$@\"\n");
    Files.setPosixFilePermissions(wrapper, PosixFilePermissions.fromString("rwx------"));
    var env = "{\"TOKENECONOMIZER_HOME\":" + quote(state) + ",\"GIT_CEILING_DIRECTORIES\":" + quote(root)
        + ",\"TE_MODE\":\"headless\",\"PYTHONPATH\":\"prior path\",\"EXPECTED_CWD\":" + quote(cwd)
        + ",\"EXPECTED_PYTHONPATH\":" + quote(core + ":prior path") + ",\"LANG\":\"en_US.UTF-8\"}";
    try (var client = new Client(executable)) {
      var build = "\"host\":\"claude\",\"keeps_history\":true,\"cwd\":" + quote(cwd) + ",\"env\":" + env
          + ",\"home\":" + quote(root) + ",\"session_id\":\"S\",\"settle\":false";
      var ctx = value(client.call("context.build", build));
      if (Files.exists(state) || ctx.contains("\"unsettled\":null")) throw new AssertionError("A deferred context wrote state or lost its settlement");
      expect(client.call("context.is_headless", "\"self\":" + ctx), "{\"result\":{\"value\":true}}");
      var settled = client.call("context.settle", "\"self\":" + ctx);
      ctx = settled.substring(settled.indexOf("\"self\":") + 7, settled.length() - 2);
      if (!ctx.contains("\"unsettled\":null")) throw new AssertionError("Context did not settle");
      var pinned = value(client.call("context.build", build.replace(quote(cwd), quote(root.resolve("other")))));
      if (!pinned.contains("\"repo\":{\"root\":" + quote(cwd)) || !pinned.contains("\"unsettled\":null")) throw new AssertionError("A pinned session changed repository: " + pinned);
      var before = Files.getLastModifiedTime(state.resolve("index.json"));
      client.call("context.settle", "\"self\":" + ctx);
      if (!before.equals(Files.getLastModifiedTime(state.resolve("index.json")))) throw new AssertionError("An already settled context wrote again");
      var mechanisms = value(client.call("context.mech_dir", "\"self\":" + ctx));
      if (!Files.isDirectory(Path.of(mechanisms.substring(1, mechanisms.length() - 1)))) throw new AssertionError("Mechanism directory was not created");
      expect(client.call("context.spawn", "\"self\":" + ctx + ",\"argv\":[\"fold\",\"space word\",\"a'b\",\"$HOME\",\"é😀\"],\"log_name\":\"child.log\",\"executable\":" + quote(wrapper) + ",\"core\":" + quote(core) + ",\"runner\":\"unused\""), "{\"result\":{\"value\":true}}");
    }
    Files.writeString(cwd.resolve("release"), "go");
    long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(15);
    while (!Files.exists(cwd.resolve("done"))) {
      if (System.nanoTime() >= deadline) throw new AssertionError("Detached child did not finish");
      Thread.sleep(10);
    }
    try (var files = Files.walk(state)) {
      var log = files.filter(p -> p.getFileName().toString().equals("child.log")).findFirst().orElseThrow();
      expect(Files.readString(log), "stdout é😀\nstderr é😀\n");
    }
  }

  static void wmi(Path root, Path executable) throws Exception {
    var log = root.resolve("worker.log");
    var spec = "{\"cmd\":[\"te\",\"a'b\"],\"cwd\":" + quote(root) + ",\"env\":{\"K\":\"V\"},\"log\":" + quote(log) + "}";
    Files.writeString(root.resolve("pythonw.exe"), "fixture");
    var args = "\"spec\":" + spec + ",\"executable\":" + quote(root.resolve("python.exe")) + ",\"runner\":" + quote(root.resolve("winspawn.py")) + ",\"transport\":{\"run\":true}";
    try (var client = new Client(executable)) {
      var pending = client.call("winspawn._spawn_wmi", args);
      if (!pending.contains("Invoke-CimMethod") || !pending.contains("pythonw.exe") || !pending.contains("context''s")) throw new AssertionError("WMI command quoting changed: " + pending);
      expect(client.resume(pending, "powershell", "{\"value\":1}"), "{\"result\":{\"value\":false}}");
      try (var files = Files.list(root)) { if (files.anyMatch(p -> p.getFileName().toString().startsWith("te-spawn-"))) throw new AssertionError("Failed WMI launch retained a spec"); }
      pending = client.call("winspawn._spawn_wmi", args);
      expect(client.resume(pending, "powershell", "{\"value\":0}"), "{\"result\":{\"value\":true}}");
      Path path;
      try (var files = Files.list(root)) { path = files.filter(p -> p.getFileName().toString().startsWith("te-spawn-")).findFirst().orElseThrow(); }
      if (!Files.getPosixFilePermissions(path).equals(PosixFilePermissions.fromString("rw-------"))) throw new AssertionError("Spawn spec permissions were left loose");
      pending = client.call("winspawn._run", "\"spec_path\":" + quote(path) + ",\"transport\":{\"call\":true}");
      if (Files.exists(path) || !pending.contains("134217728") || !pending.contains("\"K\":\"V\"")) throw new AssertionError("The WMI runner did not consume its spec or preserve its environment: " + pending);
      expect(client.resume(pending, "call", "{\"value\":7}"), "{\"result\":{\"value\":7}}");
    }
  }

  public static void main(String[] args) throws Throwable {
    if (args.length > 0 && args[0].equals("--child")) { child(args); return; }
    var checkout = Path.of(args.length == 0 ? "." : args[0]).toAbsolutePath().normalize();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"), "context's-probe-");
    try {
      context(root, checkout.resolve("target/debug/te-bridge"));
      wmi(Files.createDirectory(root.resolve("WMI context's folder")), checkout.resolve("target/debug/te-bridge"));
      System.out.println("Native context, deferred settlement, pinning, detached session and WMI spec checks passed");
    } finally {
      try (var files = Files.walk(root)) { for (var path : files.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path); }
    }
  }
}
