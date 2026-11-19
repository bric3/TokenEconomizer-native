//JAVA 25
//SOURCES ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;

/** Measures complete JSON file reads and responses through persistent release bridges. */
class NumberCodecBenchmark {
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
      for (int size : new int[]{8, 128, 8192}) {
        var document = new StringBuilder("{\"numbers\":[");
        for (int i = 0; i < size; i++) {
          if (i != 0) document.append(',');
          document.append(switch (i % 4) {
            case 0 -> "9007199254740993";
            case 1 -> "-1700000000000000000";
            case 2 -> "1.2345678901234567";
            default -> "0.00000125";
          });
        }
        Files.writeString(file, document.append("]}").toString());
        var request = "\"path\":" + ContextProbe.quote(file.toAbsolutePath());
        var expected = before.call("live_config._read", request);
        ContextProbe.expect(after.call("live_config._read", request), expected);
        int repeats = size == 8192 ? 12 : 300;
        sample(before, request, expected, repeats);
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
        System.out.printf(java.util.Locale.ROOT, "%d numbers, %d bytes: baseline %.1f us, candidate %.1f us, ratio %.3f%n",
            size, Files.size(file), baseline[4] / 1000.0, candidate[4] / 1000.0, (double) candidate[4] / baseline[4]);
      }
    } finally { Files.delete(file); }
  }
}
