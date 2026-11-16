//JAVA 25

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import java.util.regex.Pattern;

/** Checks native queue ordering and process isolation without a reference hook process. */
class QueueProbe {
  static class Client implements AutoCloseable {
    final Process process;
    final PrintWriter input;
    final BufferedReader output;
    final ExecutorService reader = Executors.newSingleThreadExecutor(Thread.ofVirtual().factory());
    Client(Path executable) throws Exception {
      process = new ProcessBuilder(executable.toString()).redirectError(ProcessBuilder.Redirect.INHERIT).start();
      input = new PrintWriter(new OutputStreamWriter(process.getOutputStream(), StandardCharsets.UTF_8));
      output = new BufferedReader(new InputStreamReader(process.getInputStream(), StandardCharsets.UTF_8));
    }
    String once(String op, String args) throws Exception {
      input.println("{\"op\":\"hookqueue." + op + "\",\"args\":{" + args + "}}");
      input.flush();
      var result = reader.submit(output::readLine).get(15, TimeUnit.SECONDS);
      if (result == null) throw new AssertionError("Native queue process exited");
      return result;
    }
    String call(String op, String args) throws Exception {
      long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(15);
      var result = once(op, args);
      while (result.contains("\"method\":\"pending\"")) {
        if (System.nanoTime() >= deadline) throw new AssertionError("Native queue job did not complete");
        var id = Pattern.compile("\"job\":(\\d+)").matcher(result);
        if (!id.find()) throw new AssertionError(result);
        result = once("resume", "\"job\":" + id.group(1));
      }
      return result;
    }
    String resume(String pending, String method, String reply) throws Exception {
      var id = Pattern.compile("\"job\":(\\d+)").matcher(pending);
      if (!pending.contains("\"callback\":\"hookqueue\"") || !pending.contains("\"method\":\"" + method + "\"") || !id.find())
        throw new AssertionError("Expected " + method + ": " + pending);
      return call("resume", "\"job\":" + id.group(1) + ",\"reply\":" + reply);
    }
    public void close() throws Exception {
      input.close();
      if (!process.waitFor(2, TimeUnit.SECONDS)) process.destroyForcibly().waitFor();
      output.close();
      reader.shutdownNow();
    }
  }

  static String quoted(Path path) { return "\"" + path.toString().replace("\\", "\\\\").replace("\"", "\\\"") + "\""; }
  static String args(Path root) {
    return "\"root\":" + quoted(root) + ",\"host\":\"claude\",\"session_id\":\"S\",\"event\":\"PostToolUse\",\"repomap\":\"off\",\"wait\":5,\"payload\":{\"hook_event_name\":\"PostToolUse\"}";
  }
  static void expect(String actual, String expected) {
    if (!actual.equals(expected)) throw new AssertionError("Expected " + expected + ", received " + actual);
  }

  static void drain(Path root, Path executable) throws Exception {
    var queue = Files.createDirectories(root.resolve("drain"));
    Files.writeString(queue.resolve("000.json"), "broken");
    for (int i = 1; i <= 4; i++) Files.writeString(queue.resolve("%03d.json".formatted(i)), "{\"n\":" + i + ",\"hook_event_name\":\"PostToolUse\"}");
    var base = "\"queue\":" + quoted(queue) + ",\"session_id\":\"S\"";
    try (var client = new Client(executable)) {
      expect(client.call("_safe", "\"text\":null"), "{\"result\":{\"value\":\"unknown\"}}");
      expect(client.call("_safe", "\"text\":\"A/B:é😀\""), "{\"result\":{\"value\":\"A_B___\"}}");
      expect(client.call("_safe", "\"text\":\"" + "A".repeat(250) + "\""), "{\"result\":{\"value\":\"" + "A".repeat(200) + "\"}}");
      var pending = client.call("drain", base + ",\"max_events\":3");
      for (int n = 1; n <= 3; n++) {
        if (!pending.contains("\"n\":" + n) || Files.exists(queue.resolve("%03d.json".formatted(n))))
          throw new AssertionError("The event was replayed out of order or before removal: " + pending);
        pending = client.resume(pending, "prepare_replay", "{\"value\":{\"session_id\":\"" + (n == 2 ? "other" : "S") + "\",\"handler\":true}}");
        if (n == 1) pending = client.resume(pending, "replay_handler", "{\"value\":null}");
        if (n == 3) {
          pending = client.resume(pending, "replay_handler", "{\"error\":\"replayed\"}");
          pending = client.resume(pending, "record_error", "{\"value\":null}");
        }
      }
      expect(pending, "{\"result\":{\"value\":null}}");
      if (Files.exists(queue.resolve("000.json")) || !Files.exists(queue.resolve("004.json"))) throw new AssertionError("Drain limit or malformed-entry cleanup failed");
      expect(client.call("drain", base + ",\"budget\":0"), "{\"result\":{\"value\":null}}");
      if (!Files.exists(queue.resolve("004.json"))) throw new AssertionError("An expired drain processed another event");
      pending = client.call("drain", base);
      pending = client.resume(pending, "prepare_replay", "{\"value\":{\"session_id\":\"S\",\"handler\":true}}");
      expect(client.resume(pending, "replay_handler", "{\"value\":null}"), "{\"result\":{\"value\":null}}");
      expect(client.call("drain", base), "{\"result\":{\"value\":null}}");
    }
  }

  static void contention(Path root, Path executable) throws Exception {
    var queue = root.resolve("locks/claude/S.queue");
    try (var holder = new Client(executable); var other = new Client(executable)) {
      var pending = holder.call("run", args(root));
      expect(other.call("run", args(root).replace("\"wait\":5", "\"wait\":0.01")), "{\"result\":{\"value\":{}}}");
      try (var files = Files.list(queue)) {
        if (files.filter(p -> p.toString().endsWith(".json")).count() != 1) throw new AssertionError("The contending hook was not queued");
      }
      var prompt = other.call("run", args(root).replace("\"wait\":5", "\"wait\":0.01")
          .replace("\"event\":\"PostToolUse\"", "\"event\":\"UserPromptSubmit\"").replace("\"repomap\":\"off\"", "\"repomap\":\"on\""));
      expect(other.resume(prompt, "handler", "{\"value\":{\"context\":true}}"), "{\"result\":{\"value\":{\"context\":true}}}");
      pending = holder.resume(pending, "handler", "{\"error\":\"current failed\"}");
      pending = holder.resume(pending, "prepare_replay", "{\"value\":{\"session_id\":\"S\",\"handler\":true}}");
      var failed = holder.resume(pending, "replay_handler", "{\"value\":null}");
      if (!failed.contains("current failed") || !failed.contains("\"raised\":\"RuntimeError\"")) throw new AssertionError(failed);
      pending = other.call("run", args(root));
      expect(other.resume(pending, "handler", "{\"value\":null}"), "{\"result\":{\"value\":{}}}");
      try (var files = Files.list(queue)) { if (files.findAny().isPresent()) throw new AssertionError("The queue was not drained after a failed handler"); }
    }
  }

  static void parallel(Path root, Path executable) throws Exception {
    Files.createDirectories(root);
    var counter = root.resolve("counter");
    Files.writeString(counter, "0");
    var clients = new ArrayList<Client>();
    var start = new CountDownLatch(1);
    try (var executor = Executors.newVirtualThreadPerTaskExecutor()) {
      var results = new ArrayList<Future<?>>();
      for (int i = 0; i < 9; i++) {
        var client = new Client(executable);
        clients.add(client);
        results.add(executor.submit(() -> {
          start.await();
          var pending = client.call("run", args(root));
          int count = Integer.parseInt(Files.readString(counter));
          Thread.sleep(10);
          Files.writeString(counter, Integer.toString(count + 1));
          expect(client.resume(pending, "handler", "{\"value\":{}}"), "{\"result\":{\"value\":{}}}");
          return null;
        }));
      }
      start.countDown();
      for (var result : results) result.get(15, TimeUnit.SECONDS);
      if (!Files.readString(counter).equals("9")) throw new AssertionError("Parallel native hooks lost an update");
    } finally { for (var client : clients) client.close(); }
  }

  static void sharedBridge(Path root, Path executable) throws Exception {
    try (var client = new Client(executable)) {
      var holder = client.call("run", args(root));
      var waiter = client.once("run", args(root));
      var id = Pattern.compile("\"job\":(\\d+)").matcher(waiter);
      if (!waiter.contains("\"method\":\"pending\"") || !id.find()) throw new AssertionError("The waiting hook blocked the bridge: " + waiter);
      expect(client.resume(holder, "handler", "{\"value\":{}}"), "{\"result\":{\"value\":{}}}");
      var pending = client.call("resume", "\"job\":" + id.group(1));
      expect(client.resume(pending, "handler", "{\"value\":{}}"), "{\"result\":{\"value\":{}}}");
    }
  }

  public static void main(String[] args) throws Exception {
    var checkout = Path.of(args.length == 0 ? "." : args[0]).toAbsolutePath().normalize();
    var executable = checkout.resolve("target/debug/te-bridge");
    var root = Files.createTempDirectory(checkout.resolve("validation/results"), "queue-");
    try {
      drain(root, executable);
      contention(root.resolve("contention"), executable);
      parallel(root.resolve("parallel"), executable);
      sharedBridge(root.resolve("shared-bridge"), executable);
      System.out.println("Native queue ordering, failure recovery, contention and nine-process serialization checks passed");
    } finally {
      try (var files = Files.walk(root)) { for (var path : files.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path); }
    }
  }
}
