//JAVA 25
//SOURCES ContextProbe.java ProcessInputProbe.java

import java.net.HttpURLConnection;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.Arrays;
import java.util.Comparator;
import java.util.List;

/** Native streaming discovery, process cleanup and local HTTP capture, without real models. */
class DiscoveryProbe {
  static String quote(Object value) { return ProcessInputProbe.quote(value); }
  static void require(boolean ok, String message) { if (!ok) throw new AssertionError(message); }
  static String http(URI uri, String body, String contentType) throws Exception {
    var connection = (HttpURLConnection) uri.toURL().openConnection();
    connection.setConnectTimeout(2000);
    connection.setReadTimeout(2000);
    if (body != null) {
      connection.setRequestMethod("POST");
      connection.setDoOutput(true);
      var bytes = body.getBytes(StandardCharsets.UTF_8);
      connection.setFixedLengthStreamingMode(bytes.length);
      try (var output = connection.getOutputStream()) { output.write(bytes); }
    }
    try {
      require(connection.getResponseCode() == 200, "HTTP capture returned " + connection.getResponseCode());
      require(contentType.equals(connection.getContentType()), "Response content type changed");
      try (var input = connection.getInputStream()) { return new String(input.readAllBytes(), StandardCharsets.UTF_8); }
    } finally { connection.disconnect(); }
  }
  static String rawStatus(URI uri, int count, String header) throws Exception {
    try (var socket = new Socket(uri.getHost(),uri.getPort())) {
      socket.setSoTimeout(2000);
      Thread.sleep(30);
      var request = new StringBuilder("GET /v1/models HTTP/1.0\r\n");
      for (int i = 0; i < count; i++) request.append(i == 0 ? header : "X-Header-" + i + ": v").append("\r\n");
      request.append("\r\n");
      socket.getOutputStream().write(request.toString().getBytes(StandardCharsets.ISO_8859_1));
      return new BufferedReader(new InputStreamReader(socket.getInputStream(),StandardCharsets.ISO_8859_1)).readLine();
    }
  }
  static void fixture(String[] args) throws Exception {
    var root = Path.of(System.getenv("PROBE_ROOT"));
    var host = args[1];
    var argv = Arrays.asList(args).subList(2, args.length);
    if (argv.equals(List.of("--help"))) { System.out.print("--no-session-persistence"); return; }
    require("1".equals(System.getenv("TE_SIDECALL")), "Missing recursion guard");
    var cwd = Path.of("").toAbsolutePath();
    require(Files.getPosixFilePermissions(cwd).equals(PosixFilePermissions.fromString("rwx------")), "Workspace is not private");
    Files.writeString(root.resolve("cwd"), cwd.toString());
    Files.writeString(root.resolve("pid"), Long.toString(ProcessHandle.current().pid()));
    if (host.equals("quiet")) { Thread.sleep(10_000); return; }
    if (host.equals("claude")) {
      require(argv.containsAll(List.of("-p", "Reply with OK", "--output-format", "stream-json", "--verbose", "--model", "haiku", "--max-turns", "1", "--no-session-persistence")), argv.toString());
      Files.deleteIfExists(root.resolve("descendant"));
      new ProcessBuilder(Path.of(System.getProperty("java.home"), "bin/java").toString(), "-cp", System.getProperty("java.class.path"), "DiscoveryProbe", "--descendant", root.toString())
          .redirectOutput(ProcessBuilder.Redirect.DISCARD).redirectError(ProcessBuilder.Redirect.DISCARD).start();
      long deadline = System.nanoTime() + 3_000_000_000L;
      while (!Files.exists(root.resolve("descendant"))) {
        if (System.nanoTime() > deadline) throw new AssertionError("Descendant did not start");
        Thread.sleep(10);
      }
      System.out.print("null\r[]\r\n42\nnot json\n{\"type\":\"system\",\"subtype\":\"init\",\"tools\":[\"Z\",\"A\",\"é😀\"],\"claude_code_version\":\"2.1\"}\n");
      System.out.flush();
      Thread.sleep(5000);
      Files.writeString(root.resolve("model-called"), "unexpected");
      return;
    }
    require(argv.containsAll(List.of("-p", "Reply with OK", "--no-auto-update", "--no-color")), argv.toString());
    require("claude-test".equals(System.getenv("COPILOT_MODEL")), "Provider prefix was retained");
    require("te-local-probe".equals(System.getenv("COPILOT_PROVIDER_API_KEY")) && System.getenv("COPILOT_PROVIDER_EXTRA") == null, "Provider settings reached the probe");
    require("openai".equals(System.getenv("COPILOT_PROVIDER_TYPE")) && "completions".equals(System.getenv("COPILOT_PROVIDER_WIRE_API")), "Capture wire protocol changed");
    require("127.0.0.1,localhost".equals(System.getenv("NO_PROXY")) && "127.0.0.1,localhost".equals(System.getenv("no_proxy")), "Proxy bypass changed");
    var base = URI.create(System.getenv("COPILOT_PROVIDER_BASE_URL"));
    require(base.getHost().equals("127.0.0.1") && base.getPath().equals("/v1"), "Capture left localhost");
    Files.writeString(root.resolve("endpoint"), base.toString());
    // An idle connection must leave another connection free to complete its request.
    try (var idle = new Socket(base.getHost(), base.getPort())) {
      var status = rawStatus(base,100,"Content-Length: unused");
      require(status.startsWith("HTTP/1.0 200"), "The 100-header boundary changed: " + status);
      require(rawStatus(base,101,"X-Header: value").startsWith("HTTP/1.0 431"), "Too many headers were accepted");
      require(rawStatus(base,1,"X-Header: " + "x".repeat(65_536)).startsWith("HTTP/1.0 431"), "Oversized header was accepted");
      require(http(URI.create(base + "/models"), null, "application/json").contains("gpt-5.4"), "Model discovery changed");
      boolean stream = argv.contains("--stream-fixture");
      var body = argv.contains("--malformed-fixture") ? "not json" : "{\"first\":true,\"messages\":[{\"role\":\"user\",\"content\":\"é😀\"}],\"tools\":[{\"function\":{\"name\":\"acme-search\"}}],\"stream\":" + stream + "}";
      var response = http(URI.create(base + "/chat/completions"), body, stream ? "text/event-stream" : "application/json");
      require(response.contains("\"content\": \"OK\"") && response.contains("\"total_tokens\": 2"), response);
      if (stream) require(response.endsWith("data: [DONE]\n\n") && response.contains("chat.completion.chunk"), response);
      http(URI.create(base + "/chat/completions"), "{\"second\":true}", "application/json");
    }
    if (argv.contains("--sleep-fixture")) Thread.sleep(10_000);
  }
  static void stopped(Path root) throws Exception {
    require(!Files.exists(Path.of(Files.readString(root.resolve("cwd")))), "Probe workspace was retained");
    long pid = Long.parseLong(Files.readString(root.resolve("pid")));
    require(ProcessHandle.of(pid).map(ProcessHandle::isAlive).orElse(false) == false, "Probe process survived");
  }
  public static void main(String[] args) throws Exception {
    if (args.length > 0 && args[0].equals("--fixture")) {
      try { fixture(args); }
      catch (Exception | AssertionError error) {
        Files.writeString(Path.of(System.getenv("PROBE_ROOT"), "fixture-error"), error + "\n" + Arrays.toString(error.getStackTrace()));
        throw error;
      }
      return;
    }
    if (args.length > 0 && args[0].equals("--descendant")) {
      var path = Path.of(args[1], "descendant");
      for (int i = 0; ; i++) { Files.writeString(path, Integer.toString(i)); Thread.sleep(25); }
    }
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"), "discovery-");
    try (var client = new ContextProbe.Client(checkout.resolve("target/debug/te-bridge"), "toolcheck")) {
      require(client.call("toolcheck._kill_tree","\"pid\":0").contains("invalid probe PID"), "An invalid process group was accepted");
      for (var host : List.of("claude", "quiet", "copilot")) {
        var wrapper = root.resolve(host);
        Files.writeString(wrapper, "#!/bin/sh\nexec '" + Path.of(System.getProperty("java.home"), "bin/java").toString().replace("'", "'\\''") + "' -cp '" + System.getProperty("java.class.path").replace("'", "'\\''") + "' DiscoveryProbe --fixture " + host + " \"$@\"\n");
        Files.setPosixFilePermissions(wrapper, PosixFilePermissions.fromString("rwx------"));
      }
      var common = "\"env\":{\"PROBE_ROOT\":" + quote(root) + ",\"COPILOT_PROVIDER_MODEL_ID\":\"provider/claude-test\",\"COPILOT_PROVIDER_API_KEY\":\"SECRET\",\"COPILOT_PROVIDER_EXTRA\":\"SECRET\"},\"process_cwd\":" + quote(root) + ",\"home\":" + quote(root);
      long start = System.nanoTime();
      var init = client.call("toolcheck._init_tools", common + ",\"cli\":" + quote(root.resolve("claude")) + ",\"extra_args\":[],\"timeout\":30");
      require(init.contains("\"value\":[[\"Z\",\"A\",\"é😀\"],\"2.1\"]") && System.nanoTime() - start < 4_000_000_000L, init);
      stopped(root);
      var heartbeat = Files.readString(root.resolve("descendant"));
      Thread.sleep(250);
      require(heartbeat.equals(Files.readString(root.resolve("descendant"))) && !Files.exists(root.resolve("model-called")), "Discovery continued beyond init");
      start = System.nanoTime();
      var quiet = client.call("toolcheck._init_tools", common + ",\"cli\":" + quote(root.resolve("quiet")) + ",\"extra_args\":[],\"timeout\":0.5");
      require(quiet.contains("\"value\":[null,null]") && System.nanoTime() - start < 3_000_000_000L, quiet);
      stopped(root);
      var state = root.resolve("state");
      var catalog = client.call("toolcheck.refresh_catalog", common + ",\"cli\":" + quote(root.resolve("claude")) + ",\"state_root\":" + quote(state) + ",\"host\":\"claude\"");
      var saved = Files.readString(state.resolve("hosts/claude/catalog.json"));
      require(catalog.contains("[\"A\",\"Z\",\"é😀\"]") && saved.contains("\"claude_code_version\": \"2.1\"") && saved.contains("\"probed_at\":"), catalog + saved);
      stopped(root);
      for (var extra : List.of("", "--stream-fixture", "--malformed-fixture", "--sleep-fixture")) {
        var capture = client.call("toolcheck.copilot_capture", common + ",\"cli\":" + quote(root.resolve("copilot")) + ",\"extra_args\":[" + (extra.isEmpty() ? "" : quote(extra)) + "],\"timeout\":" + (extra.equals("--sleep-fixture") ? "1" : "5"));
        require(capture.contains(extra.equals("--malformed-fixture") ? "\"value\":{}" : "\"first\":true") && !capture.contains("\"second\":true") && !capture.contains("SECRET"), capture + (Files.exists(root.resolve("fixture-error")) ? Files.readString(root.resolve("fixture-error")) : ""));
        stopped(root);
        var endpoint = URI.create(Files.readString(root.resolve("endpoint")));
        try (var socket = new Socket()) {
          try { socket.connect(new InetSocketAddress(endpoint.getHost(), endpoint.getPort()), 250); throw new AssertionError("Capture listener survived"); }
          catch (java.net.ConnectException expected) { }
        }
      }
      var cp = client.call("toolcheck.refresh_catalog", common + ",\"cli\":" + quote(root.resolve("copilot")) + ",\"state_root\":" + quote(state) + ",\"host\":\"copilot\",\"transport\":{\"_copilot_version\":true}");
      require(cp.contains("\"callback\":"), cp + (Files.exists(root.resolve("fixture-error")) ? Files.readString(root.resolve("fixture-error")) : ""));
      cp = client.resume(cp,"probe","{\"value\":\"1.2\"}");
      saved = Files.readString(state.resolve("hosts/copilot/catalog.json"));
      require(cp.contains("acme-search") && saved.contains("\"copilot_version\": \"1.2\""), cp + saved);
      System.out.println("Native streaming init, quiet timeout, descendant cleanup, private workspaces, local JSON/SSE capture, first request retention and catalog refresh passed");
    } finally {
      try (var paths = Files.walk(root)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path); }
    }
  }
}
