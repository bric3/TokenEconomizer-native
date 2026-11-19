//JAVA 25

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashMap;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.regex.Pattern;

/** Matches collected original cases to Rust test names; assertions and parameter values need review. */
class TestConversionInventory {
  static final Pattern NODE = Pattern.compile("^((?:experiments/)?tests/[^:]+\\.py)::(?:\\w+::)*(test_\\w+)(?:\\[.*\\])?$");
  static final Pattern SOURCE = Pattern.compile("Converted from reference/upstream/(\\S+\\.py)");
  static final Pattern TEST = Pattern.compile("(?m)^#\\[test]\\s*fn (\\w+)\\(");

  public static void main(String[] args) throws Exception {
    if (args.length != 0) throw new IllegalArgumentException("usage: TestConversionInventory.java");
    var results = Files.createDirectories(Path.of("validation/results"));
    var collection = results.resolve("test-conversion-collected.txt");
    var process = new ProcessBuilder(".venv/bin/python", "-m", "pytest", "-o", "addopts=", "--collect-only", "-q",
        "reference/upstream/tests", "reference/upstream/experiments/tests")
        .redirectErrorStream(true).redirectOutput(collection.toFile()).start();
    if (process.waitFor() != 0) throw new AssertionError("Collection failed; see " + collection);
    var cases = new TreeMap<String, Map<String, Integer>>();
    for (var line : Files.readAllLines(collection)) {
      var match = NODE.matcher(line);
      if (match.matches()) cases.computeIfAbsent(match.group(1), ignored -> new TreeMap<>()).merge(match.group(2).substring(5), 1, Integer::sum);
      else if (line.contains(".py::")) throw new AssertionError("Unrecognised test ID: " + line);
    }
    int total = cases.values().stream().flatMap(module -> module.values().stream()).mapToInt(Integer::intValue).sum();
    if (total != 1591) throw new AssertionError("Expected the pinned 1591-case inventory, found " + total);
    var converted = new HashMap<String, Set<String>>();
    try (var paths = Files.walk(Path.of("tests"))) {
      for (var path : paths.filter(path -> path.toString().endsWith(".rs")).sorted().toList()) {
        var text = Files.readString(path);
        var source = SOURCE.matcher(text);
        if (!source.find()) continue;
        var module = source.group(1);
        if (!cases.containsKey(module)) throw new AssertionError("Unknown original module in " + path + ": " + module);
        var names = converted.computeIfAbsent(module, ignored -> new HashSet<>());
        for (var test : TEST.matcher(text).results().toList()) {
          var name = test.group(1);
          if (!cases.get(module).containsKey(name)) throw new AssertionError("Unknown original test in " + path + ": " + name);
          if (!names.add(name)) throw new AssertionError("Duplicate converted test: " + module + "::" + name);
        }
      }
    }
    int mapped = 0, complete = 0;
    for (var entry : cases.entrySet()) {
      var names = converted.getOrDefault(entry.getKey(), Set.of());
      int count = entry.getValue().entrySet().stream().filter(test -> names.contains(test.getKey())).mapToInt(Map.Entry::getValue).sum();
      int original = entry.getValue().values().stream().mapToInt(Integer::intValue).sum();
      mapped += count;
      if (count == original) complete++;
      System.out.printf("%s: %d/%d functions, %d/%d reference cases mapped%n", entry.getKey(), names.size(), entry.getValue().size(), count, original);
      for (var name : entry.getValue().keySet()) if (!names.contains(name)) System.out.println("  pending: " + name);
    }
    System.out.printf("%d/%d modules have all original test names; %d/%d reference cases map to Rust tests.%n", complete, cases.size(), mapped, total);
    System.out.println("Name mapping does not verify assertions, parameter values, fixture behaviour or execution.");
  }
}
