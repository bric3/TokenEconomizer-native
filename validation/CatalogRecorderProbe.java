//JAVA 25
//SOURCES ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java AdapterProbe.java ExtractProposerPrompts.java

import com.sun.net.httpserver.HttpServer;
import java.net.InetSocketAddress;
import java.net.ServerSocket;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.Executors;

/** Real loopback HTTP comparison of source and native recorder policy, with no model calls. */
class CatalogRecorderProbe {
  static int checks;
  static int port() throws Exception { try (var socket = new ServerSocket(0)) { return socket.getLocalPort(); } }
  static class Recorder implements AutoCloseable {
    final Process process;
    final int port;
    final Path snapshot;
    Recorder(Path root, boolean nativeCode, String upstream) throws Exception {
      Files.createDirectories(root);
      port = port();
      snapshot = root.resolve(nativeCode ? "native.json" : "source.json");
      var script = nativeCode ? "src/experiments/swebench/catalog/recorder_io.py" : "validation/catalog_recorder_source.py";
      var builder = new ProcessBuilder(Path.of(".venv/bin/python").toAbsolutePath().toString(), script,
          Integer.toString(port), snapshot.toString(), upstream + "///");
      builder.environment().put("REAL_ANTHROPIC_KEY", "fixture-real-key");
      builder.environment().put("TE_CATALOG_HELPER", Path.of("target/debug/te-net-guard").toAbsolutePath().toString());
      builder.redirectError(root.resolve(nativeCode ? "native.log" : "source.log").toFile());
      process = builder.start();
      long deadline = System.nanoTime() + Duration.ofSeconds(10).toNanos();
      while (true) {
        try (var socket = new java.net.Socket()) { socket.connect(new InetSocketAddress("127.0.0.1",port),100); break; }
        catch (java.io.IOException error) {
          if (!process.isAlive() || System.nanoTime() > deadline) throw new AssertionError("Recorder did not listen: " + script);
          Thread.sleep(10);
        }
      }
    }
    public void close() throws Exception {
      var children = process.descendants().toList();
      process.destroyForcibly();
      for (var child : children) child.destroyForcibly();
      process.waitFor();
    }
    String snapshot() throws Exception { return Files.isDirectory(snapshot) ? "<directory>" : Files.exists(snapshot) ? Files.readString(snapshot) : "<absent>"; }
  }
  static void compare(Recorder source, Recorder nativeCode, HttpClient client, List<String> requests, String route, byte[] body, boolean version) throws Exception {
    HttpResponse<byte[]> expected = null;
    for (var recorder : List.of(source,nativeCode)) {
      var request = HttpRequest.newBuilder(URI.create("http://127.0.0.1:"+recorder.port+route)).timeout(Duration.ofSeconds(10))
          .header("Authorization","redacted fixture").header("x-api-key","dummy fixture").header("X-Custom","keep fixture");
      if (version) request.header("anthropic-version","fixture-version");
      if (body != null) request.POST(HttpRequest.BodyPublishers.ofByteArray(body));
      var actual = client.send(request.build(),HttpResponse.BodyHandlers.ofByteArray());
      if (expected == null) expected = actual;
      else {
        var actualHeaders = new java.util.TreeMap<>(actual.headers().map());
        var expectedHeaders = new java.util.TreeMap<>(expected.headers().map());
        for (var headers : List.of(actualHeaders,expectedHeaders)) { headers.remove("date"); headers.remove("server"); }
        if (actual.statusCode()!=expected.statusCode() || !java.util.Arrays.equals(actual.body(),expected.body()) || !actualHeaders.equals(expectedHeaders))
          throw new AssertionError("Proxy response differs for " + route);
        checks++;
      }
    }
    if (requests != null) {
      var last = requests.size();
      if (!requests.get(last-1).equals(requests.get(last-2))) throw new AssertionError("Upstream request differs: " + requests.subList(last-2,last));
      checks++;
    }
    if (!nativeCode.snapshot().equals(source.snapshot())) throw new AssertionError("Snapshot differs\nNative: " + nativeCode.snapshot()+"\nSource: "+source.snapshot());
    checks++;
  }
  public static void main(String[] args) throws Exception {
    var root = Files.createTempDirectory(Path.of("validation/results").toAbsolutePath(), "catalog-recorder-");
    var requests = new CopyOnWriteArrayList<String>();
    var upstream = HttpServer.create(new InetSocketAddress("127.0.0.1",0),0);
    var executor = Executors.newVirtualThreadPerTaskExecutor();
    upstream.setExecutor(executor);
    upstream.createContext("/", exchange -> {
      try (exchange) {
        var body = exchange.getRequestBody().readAllBytes();
        var headers = new java.util.TreeMap<String,List<String>>();
        exchange.getRequestHeaders().forEach((name,values) -> { if (!name.equalsIgnoreCase("Host")) headers.put(name.toLowerCase(java.util.Locale.ROOT),values); });
        requests.add(exchange.getRequestMethod() + " " + exchange.getRequestURI() + " " + headers + " " + java.util.HexFormat.of().formatHex(body));
        exchange.getResponseHeaders().add("X-Fixture","kept");
        exchange.getResponseHeaders().add("Content-Type","application/json");
        var response = (exchange.getRequestURI().getPath().equals("/error") ? "{\"error\":\"fixture\"}" : "stream é😀\n".repeat(2000)).getBytes(StandardCharsets.UTF_8);
        exchange.sendResponseHeaders(exchange.getRequestURI().getPath().equals("/error") ? 429 : 200,response.length);
        exchange.getResponseBody().write(response);
      }
    });
    upstream.start();
    boolean passed = false;
    try (var source = new Recorder(root,false,"http://127.0.0.1:"+upstream.getAddress().getPort());
         var nativeCode = new Recorder(root,true,"http://127.0.0.1:"+upstream.getAddress().getPort());
         var client = HttpClient.newBuilder().connectTimeout(Duration.ofSeconds(3)).build()) {
      var documents = new ArrayList<>(List.of("{\"tools\":[{\"name\":\"Write\",\"description\":\"  write é😀 \\u001f\"},{\"name\":\"Empty\"}]}",
          "{\"tools\":[{\"name\":\"Write\",\"description\":\"x\"},{\"name\":\"Read\",\"description\":\"read\"}]}",
          "{\"tools\":[{\"name\":\"Read\",\"description\":\"longer\"},{\"name\":\"Read\",\"description\":\"r\"}]}",
          "{\"tools\":[null,17,\"text\",{}, {\"name\":null}, {\"name\":\"Write\",\"description\":false}]}",
          "{\"tools\":[{\"name\":\"Skip\",\"description\":\"skip entire offer\"},{\"name\":\"Bad\",\"description\":true}]}",
          "{\"tools\":[{\"name\":\"Surrogate\\ud800\",\"description\":\"value\\udfff\"}]}",
          "{\"metadata\":NaN,\"tools\":[{\"name\":\"Finite\",\"description\":\"kept\"}]}",
          "null","[]","true","17","\"text\"","bad","{\"tools\":null}","{\"tools\":true}","{\"tools\":{}}","{\"tools\":\"text\"}"));
      for (var document : documents) for (var route : List.of("/v1/messages?fixture=yes","/error")) {
        compare(source,nativeCode,client,requests,route,document.getBytes(StandardCharsets.UTF_8),true);
      }
      for (var charset : List.of("UTF-8","UTF-16LE","UTF-16BE","UTF-32LE","UTF-32BE"))
        compare(source,nativeCode,client,requests,"/v1/messages",("{\"tools\":[{\"name\":\""+charset+"\",\"description\":\"encoding é😀\"}]}").getBytes(java.nio.charset.Charset.forName(charset)),false);
      compare(source,nativeCode,client,requests,"/v1/models",null,false);
      compare(source,nativeCode,client,requests,"/error",null,false);
      for (var recorder : List.of(source,nativeCode)) { Files.delete(recorder.snapshot); Files.createDirectory(recorder.snapshot); }
      compare(source,nativeCode,client,requests,"/v1/messages","{\"tools\":[{\"name\":\"AfterFailure\",\"description\":\"retained after a failed write\"}]}".getBytes(StandardCharsets.UTF_8),false);
      for (var recorder : List.of(source,nativeCode)) Files.delete(recorder.snapshot);
      compare(source,nativeCode,client,requests,"/v1/messages","{\"tools\":[{\"name\":\"AfterFailure\",\"description\":\"short\"}]}".getBytes(StandardCharsets.UTF_8),false);
      compare(source,nativeCode,client,requests,"/v1/messages","{\"tools\":[{\"name\":\"Recovered\",\"description\":\"writes resume\"}]}".getBytes(StandardCharsets.UTF_8),false);
      for (var recorder : List.of(source,nativeCode)) {
        var pending = new ArrayList<java.util.concurrent.CompletableFuture<HttpResponse<byte[]>>>();
        for (int i=0;i<16;i++) pending.add(client.sendAsync(HttpRequest.newBuilder(URI.create("http://127.0.0.1:"+recorder.port+"/v1/messages"))
            .POST(HttpRequest.BodyPublishers.ofString("{\"tools\":[{\"name\":\"Concurrent"+i+"\",\"description\":\"kept\"}]}"))
            .build(),HttpResponse.BodyHandlers.ofByteArray()));
        for (var response : pending) if (response.get(15,java.util.concurrent.TimeUnit.SECONDS).statusCode()!=200) throw new AssertionError("Concurrent capture failed");
        for (int i=0;i<16;i++) { if (!recorder.snapshot().contains("\"Concurrent"+i+"\": \"kept\"")) throw new AssertionError("Lost a concurrent capture"); checks++; }
      }
      try (var failedSource = new Recorder(root.resolve("failed-source"),false,"http://127.0.0.1:"+port());
           var failedNative = new Recorder(root.resolve("failed-native"),true,"http://127.0.0.1:"+port())) {
        compare(failedSource,failedNative,client,null,"/v1/models",null,false);
      }
      passed = true;
    } finally {
      upstream.stop(0); executor.close();
      if (passed) ContainerProbe.clear(root); else System.err.println("Probe artifacts: " + root);
    }
    System.out.println("Catalogue recorder passed " + checks + " source comparisons over real loopback HTTP");
  }
}
