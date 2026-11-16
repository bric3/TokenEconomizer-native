//JAVA 25
//SOURCES ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.Comparator;
import java.util.regex.Pattern;

/** Native folding locks, callback failure cleanup and imported-session boundaries. */
class FoldProbe {
  static class Client extends ContextProbe.Client {
    Client(Path executable) throws Exception { super(executable); }
    @Override String call(String op, String args) throws Exception {
      var line = once(op, args);
      long deadline = System.nanoTime() + Duration.ofSeconds(10).toNanos();
      while (line.contains("\"method\":\"pending\"")) {
        if (System.nanoTime() >= deadline) throw new AssertionError("Fold job remained pending");
        line = once("fold.resume", "\"job\":" + ContextProbe.job(line));
      }
      return line;
    }
    @Override String resume(String pending, String method, String reply) throws Exception {
      if (!pending.contains("\"method\":\"" + method + "\"")) throw new AssertionError(pending);
      return call("fold.resume", "\"job\":" + ContextProbe.job(pending) + ",\"reply\":" + reply);
    }
  }
  static String quote(Object value) { return ContextProbe.quote(value.toString()); }
  static void expect(String actual, String expected) { ContextProbe.expect(actual, expected); }
  static String value(String value) { return "{\"value\":" + value + "}"; }
  static String workdir(String callback) {
    var match = Pattern.compile("\"mechanism_trace\",\\[\"[^\"]*\",\"([^\"]+)\"\\]").matcher(callback);
    if (!match.find()) throw new AssertionError(callback);
    return match.group(1);
  }
  static String finish(Client client, String pending, Path transcript, boolean imported) throws Exception {
    String work = null;
    while (pending.contains("\"callback\":\"fold\"")) {
      if (!pending.contains("\"method\":\"host\"")) throw new AssertionError("Unexpected production callback: " + pending);
      String reply;
      if (pending.contains("\"complete\",")) {
        if (imported) throw new AssertionError("Import completed a live session");
        reply = quote(transcript);
      } else if (pending.contains("\"mechanism_trace\",")) {
        work = workdir(pending);
        if (!Files.isDirectory(Path.of(work))) throw new AssertionError("Missing private workspace");
        reply = quote(transcript);
      } else if (pending.contains("\"prompt_count\",")) {
        reply = "2";
      } else if (pending.contains("\"settle\",")) {
        if (imported) throw new AssertionError("Import settled a live session");
        reply = "null";
      } else if (pending.contains("\"session_usage\",")) {
        reply = "{\"model\":\"unknown\",\"requests\":1,\"input_tokens\":100,\"output_tokens\":10}";
      } else throw new AssertionError(pending);
      pending = client.resume(pending, "host", value(reply));
    }
    if (work == null || Files.exists(Path.of(work))) throw new AssertionError("Fold workspace was not removed");
    return pending;
  }
  public static void main(String[] args) throws Exception {
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"), "fold-probe-");
    try {
      var state = root.resolve("state");
      var transcript = root.resolve("session.jsonl");
      Files.writeString(transcript, "");
      var ctx = "{\"root\":" + quote(root) + ",\"base\":" + quote(state)
          + ",\"repo\":{\"root\":" + quote(root) + ",\"key\":\"probe\",\"label\":\"probe\",\"remote\":\"\",\"how\":\"path\"},\"host\":\"claude\",\"env\":{},\"home\":" + quote(root)
          + ",\"config\":{\"microskills\":\"envfacts\",\"vocabulary\":\"auto\",\"microskills_propose_every\":5,\"microskills_max_facts\":20,\"microskills_max_tokens\":500,\"toolprune\":\"off\",\"shadow\":false}}";
      var common = "\"ctx\":" + ctx + ",\"lock_wait\":0.02,\"transcript_path\":" + quote(transcript);
      try (var holder = new Client(checkout.resolve("target/debug/te-bridge"));
           var waiter = new Client(checkout.resolve("target/debug/te-bridge"))) {
        var pending = holder.call("fold.fold_session", common + ",\"session_id\":\"s\"");
        if (!pending.contains("\"complete\",")) throw new AssertionError(pending);
        expect(waiter.call("fold.fold_session", common + ",\"session_id\":\"s\""), "{\"result\":{\"value\":null}}");
        if (Files.exists(state.resolve("ledger.jsonl"))) throw new AssertionError("Contended fold wrote its record");
        var record = finish(holder, pending, transcript, false);
        if (!record.contains("\"effective_tokens\":160.0") || !record.contains("\"prompts\":2")) throw new AssertionError(record);
        expect(waiter.call("fold.fold_session", common + ",\"session_id\":\"s\""), "{\"result\":{\"value\":null}}");
        if (Files.readAllLines(state.resolve("ledger.jsonl")).size() != 1) throw new AssertionError("Session folded twice");

        var deferred = waiter.call("fold.fold_session", common + ",\"session_id\":\"later\"");
        expect(waiter.resume(deferred, "host", "{\"error_kind\":\"FoldLater\",\"error\":\"still active\"}"), "{\"result\":{\"value\":null}}");
        if (Files.readString(state.resolve("folded.json")).contains("later")) throw new AssertionError("Active session was marked folded");

        Files.createDirectories(state.resolve("sessions"));
        Files.writeString(state.resolve("sessions/retry.json"), "{\"judge_tokens\":\"2.5\",\"writer_tokens\":\"1.25\",\"nudges\":\"2\"}");
        var failed = waiter.call("fold.fold_session", common + ",\"session_id\":\"retry\"");
        failed = waiter.resume(failed, "host", value(quote(transcript)));
        var work = Path.of(workdir(failed));
        failed = waiter.resume(failed, "host", "{\"error_kind\":\"RuntimeError\",\"error\":\"trace failed\"}");
        expect(failed, "{\"result\":{\"raised\":\"RuntimeError\",\"message\":\"trace failed\"}}");
        if (Files.exists(work) || Files.readString(state.resolve("folded.json")).contains("retry")) throw new AssertionError("Failed fold retained its workspace or committed");
        var retried = finish(holder, holder.call("fold.fold_session", common + ",\"session_id\":\"retry\""), transcript, false);
        if (!retried.contains("\"session_id\":\"retry\"") || !retried.contains("\"spiral_judge_tokens\":2.5")
            || !retried.contains("\"spiral_nudge_writer_tokens\":1.25") || !retried.contains("\"nudges\":2")) throw new AssertionError(retried);

        var imported = finish(holder, holder.call("fold.fold_locked", common + ",\"session_id\":\"old\",\"imported\":true,\"span\":[1,2]"), transcript, true);
        if (!imported.contains("\"imported\":true") || !imported.contains("\"untool_applied\":[]")) throw new AssertionError(imported);
        if (Files.readAllLines(state.resolve("ledger.jsonl")).size() != 3) throw new AssertionError("Lost a completed fold");
        if (!Files.readString(state.resolve("injection.json")).contains("\"imported_sessions\": 1")) throw new AssertionError("Import did not learn through the native mechanism");

        var injection = Files.readString(state.resolve("injection.json"));
        var invalid = injection.replace("\"sessions\": 3", "\"sessions\": \"invalid\"");
        if (injection.equals(invalid)) throw new AssertionError("Missing injection session counter");
        Files.writeString(state.resolve("injection.json"), invalid);
        var malformed = holder.call("fold.fold_session", common + ",\"session_id\":\"malformed\"");
        malformed = holder.resume(malformed, "host", value(quote(transcript)));
        var malformedWork = Path.of(workdir(malformed));
        malformed = holder.resume(malformed, "host", value(quote(transcript)));
        malformed = holder.resume(malformed, "host", value("2"));
        if (!malformed.contains("\"raised\":\"ValueError\"") || Files.exists(malformedWork)
            || Files.readAllLines(state.resolve("ledger.jsonl")).size() != 3
            || !Files.readString(state.resolve("injection.json")).equals(invalid)) throw new AssertionError("Invalid counter was accepted or committed: " + malformed);

        Files.writeString(state.resolve("sessions/a.json"), "{\"started\":\"10\",\"ended\":true}");
        Files.writeString(state.resolve("sessions/z.json"), "{\"started\":\"2\",\"ended\":true}");
        var ordered = holder.call("fold.pending_sessions", "\"ctx\":" + ctx);
        while (ordered.contains("\"callback\":\"fold\"")) {
          if (!ordered.contains("\"last_active\",")) throw new AssertionError(ordered);
          ordered = holder.resume(ordered, "host", "{\"error_kind\":\"OSError\",\"error\":\"missing transcript\"}");
        }
        expect(ordered, "{\"result\":{\"value\":[\"z\",\"a\"]}}");
      }
      System.out.println("Native fold exclusion, once-only learning, deferred sessions, failure cleanup and import boundaries passed");
    } finally {
      try (var paths = Files.walk(root)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path); }
    }
  }
}
