//JAVA 25
//SOURCES ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.Comparator;
import java.util.List;
import java.util.Locale;

/** Measures configuration loads through persistent bridges and fresh-process first requests. */
class ConfigLoadBenchmark {
  record Case(String name, String document) {}
  static long sample(ContextProbe.Client client, String request, String expected, int repeats) throws Exception {
    long start = System.nanoTime();
    for (int i = 0; i < repeats; i++) {
      var actual = client.call("live_config.load_config", request);
      if (!actual.equals(expected)) throw new AssertionError("Configuration output changed");
    }
    return (System.nanoTime() - start) / repeats;
  }
  static long firstRequest(Path executable, String request, String expected) throws Exception {
    long start = System.nanoTime();
    try (var client = new ContextProbe.Client(executable.toAbsolutePath())) {
      ContextProbe.expect(client.call("live_config.load_config", request), expected);
      return System.nanoTime() - start;
    }
  }
  public static void main(String[] args) throws Exception {
    if (args.length != 2) throw new IllegalArgumentException("usage: ConfigLoadBenchmark.java BASELINE_BRIDGE CANDIDATE_BRIDGE");
    var root = Files.createTempDirectory(Path.of("validation/results"), "config-load-benchmark-").toAbsolutePath();
    try (var before = new ContextProbe.Client(Path.of(args[0]).toAbsolutePath()); var after = new ContextProbe.Client(Path.of(args[1]).toAbsolutePath())) {
      var response = before.call("live_config.new", "\"fields\":{}");
      var prefix = "{\"result\":";
      if (!response.startsWith(prefix) || !response.endsWith("}}")) throw new AssertionError(response);
      var defaults = response.substring(prefix.length(), response.length() - 1).replace(",\"warnings\":[]", "");
      if (defaults.contains("\"warnings\"")) throw new AssertionError("Default fixture includes warnings");
      var unknown = new StringBuilder("{");
      for (int i = 0; i < 2000; i++) {
        if (i != 0) unknown.append(',');
        unknown.append('"').append("unknown-").append(i).append("\":1");
      }
      var cases = List.of(new Case("empty", "{}"), new Case("one-field", "{\"mode\":\"headless\"}"),
          new Case("typical", "{\"enabled\":true,\"mode\":\"headless\",\"microskills\":\"envfacts\",\"microskills_max_tokens\":300,\"toolprune\":\"auto\",\"toolprune_sessions\":8,\"repomap\":\"start\",\"loopguard\":\"nudge\"}"),
          new Case("all-default-fields", defaults), new Case("2000-unknown-fields", unknown.append('}').toString()));
      for (var scenario : cases) {
        var directory = Files.createDirectory(root.resolve(scenario.name()));
        Files.writeString(directory.resolve("config.json"), scenario.document());
        var request = "\"root\":" + ContextProbe.quote(directory) + ",\"repo_dir\":null,\"env\":{}";
        var expected = before.call("live_config.load_config", request);
        ContextProbe.expect(after.call("live_config.load_config", request), expected);
        if (!scenario.name().equals("2000-unknown-fields") && !expected.contains("\"warnings\":[]")) throw new AssertionError(expected);
        int repeats = 1;
        while (sample(before, request, expected, repeats) * repeats < 50_000_000L && repeats < 512) repeats *= 2;
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
            scenario.name(), scenario.document().length(), repeats, baseline[4] / 1000.0, candidate[4] / 1000.0, (double) candidate[4] / baseline[4]);
        var coldBaseline = new long[21]; var coldCandidate = new long[21];
        for (int run = 0; run < coldBaseline.length; run++) {
          if (run % 2 == 0) {
            coldBaseline[run] = firstRequest(Path.of(args[0]), request, expected);
            coldCandidate[run] = firstRequest(Path.of(args[1]), request, expected);
          } else {
            coldCandidate[run] = firstRequest(Path.of(args[1]), request, expected);
            coldBaseline[run] = firstRequest(Path.of(args[0]), request, expected);
          }
        }
        Arrays.sort(coldBaseline); Arrays.sort(coldCandidate);
        System.out.printf(Locale.ROOT, "%s fresh process: baseline %.1f us, candidate %.1f us, ratio %.3f%n",
            scenario.name(), coldBaseline[10] / 1000.0, coldCandidate[10] / 1000.0, (double) coldCandidate[10] / coldBaseline[10]);
      }
    } finally {
      try (var paths = Files.walk(root)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.delete(path); }
    }
  }
}
