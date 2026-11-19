//JAVA 25
//SOURCES ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.List;
import java.util.Locale;

/** Measures complete JSON file reads and responses through persistent release bridges. */
class NumberCodecBenchmark {
  record Case(String name, int count, List<String> tokens) {}
  static long sample(ContextProbe.Client client, String request, String expected, int repeats) throws Exception {
    long start = System.nanoTime();
    for (int i = 0; i < repeats; i++) {
      if (!client.call("live_config._read", request).equals(expected)) throw new AssertionError("Read result changed");
    }
    return (System.nanoTime() - start) / repeats;
  }
  public static void main(String[] args) throws Exception {
    if (args.length != 2) throw new IllegalArgumentException("usage: NumberCodecBenchmark.java BASELINE_BRIDGE CANDIDATE_BRIDGE");
    var file = Files.createTempFile(Path.of("validation/results"), "number-codec-benchmark-", ".json");
    try (var before = new ContextProbe.Client(Path.of(args[0])); var after = new ContextProbe.Client(Path.of(args[1]))) {
      var ordinary = List.of("9007199254740993", "-1700000000000000000", "1.2345678901234567", "0.00000125");
      for (var scenario : List.of(new Case("small", 8, ordinary), new Case("typical", 128, ordinary), new Case("large", 8192, ordinary),
          new Case("wide-integers", 2048, List.of("18446744073709551616", "170141183460469231731687303715884105728", "9".repeat(1000), "-" + "9".repeat(1000))),
          new Case("nonfinite", 8192, List.of("NaN", "Infinity", "-Infinity")),
          new Case("extreme-finite", 8192, List.of("5e-324", "1.7976931348623157e308", "-0.0", "0.0")))) {
        var document = new StringBuilder("{\"numbers\":[");
        for (int i = 0; i < scenario.count(); i++) {
          if (i != 0) document.append(',');
          document.append(scenario.tokens().get(i % scenario.tokens().size()));
        }
        Files.writeString(file, document.append("]}").toString());
        var request = "\"path\":" + ContextProbe.quote(file.toAbsolutePath());
        var expected = before.call("live_config._read", request);
        ContextProbe.expect(after.call("live_config._read", request), expected);
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
        System.out.printf(Locale.ROOT, "%s, %d numbers, %d bytes, %d repeats: baseline %.1f us, candidate %.1f us, ratio %.3f%n",
            scenario.name(), scenario.count(), Files.size(file), repeats, baseline[4] / 1000.0, candidate[4] / 1000.0, (double) candidate[4] / baseline[4]);
      }
    } finally { Files.delete(file); }
  }
}
