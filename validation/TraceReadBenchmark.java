//JAVA 25
//SOURCES ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.Comparator;
import java.util.List;
import java.util.Locale;

/** Measures complete prompt counts through the shared JSONL reader in release bridges. */
class TraceReadBenchmark {
  record Case(String name, int records, int textLength, boolean malformed) {}
  static long sample(ContextProbe.Client client, String request, String expected, int repeats) throws Exception {
    long start = System.nanoTime();
    for (int i = 0; i < repeats; i++) {
      ContextProbe.expect(client.call("trace.claude.claude_prompt_count", request), expected);
    }
    return (System.nanoTime() - start) / repeats;
  }
  public static void main(String[] args) throws Exception {
    if (args.length != 2) throw new IllegalArgumentException("usage: TraceReadBenchmark.java BASELINE_BRIDGE CANDIDATE_BRIDGE");
    var root = Files.createTempDirectory(Path.of("validation/results"), "trace-read-benchmark-").toAbsolutePath();
    try (var before = new ContextProbe.Client(Path.of(args[0]).toAbsolutePath()); var after = new ContextProbe.Client(Path.of(args[1]).toAbsolutePath())) {
      for (var scenario : List.of(new Case("empty", 0, 0, false), new Case("small", 8, 128, false),
          new Case("typical", 512, 512, false), new Case("large", 8192, 1024, false),
          new Case("wide-lines", 4, 1 << 20, false), new Case("malformed", 8192, 256, true))) {
        var file = root.resolve(scenario.name() + ".jsonl");
        var payload = "message é😀 " + "x".repeat(scenario.textLength());
        try (var writer = Files.newBufferedWriter(file)) {
          for (int i = 0; i < scenario.records(); i++) {
            if (scenario.malformed()) writer.write("invalid " + payload);
            else writer.write("{\"type\":\"" + (i % 4 == 0 ? "user" : "assistant") + "\",\"id\":" + i
                + ",\"message\":{\"content\":" + ContextProbe.quote(payload) + "}}");
            writer.write(switch (i % 3) { case 0 -> "\n"; case 1 -> "\r\n"; default -> "\r"; });
          }
        }
        var request = "\"transcript_path\":" + ContextProbe.quote(file);
        var expected = "{\"result\":" + (scenario.malformed() ? 0 : (scenario.records() + 3) / 4) + "}";
        ContextProbe.expect(before.call("trace.claude.claude_prompt_count", request), expected);
        ContextProbe.expect(after.call("trace.claude.claude_prompt_count", request), expected);
        int repeats = 1;
        while (sample(before, request, expected, repeats) * repeats < 50_000_000L && repeats < 8192) repeats *= 2;
        sample(after, request, expected, repeats);
        var baseline = new long[9]; var candidate = new long[9];
        for (int run = 0; run < baseline.length; run++) {
          if (run % 2 == 0) {
            baseline[run] = sample(before, request, expected, repeats);
            candidate[run] = sample(after, request, expected, repeats);
          } else {
            candidate[run] = sample(after, request, expected, repeats);
            baseline[run] = sample(before, request, expected, repeats);
          }
        }
        Arrays.sort(baseline); Arrays.sort(candidate);
        System.out.printf(Locale.ROOT, "%s, %d bytes, %d repeats: baseline %.1f us, candidate %.1f us, ratio %.3f%n",
            scenario.name(), Files.size(file), repeats, baseline[4] / 1000.0, candidate[4] / 1000.0, (double) candidate[4] / baseline[4]);
      }
    } finally {
      try (var paths = Files.walk(root)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.delete(path); }
    }
  }
}
