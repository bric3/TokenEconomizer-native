//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.FileTime;
import java.util.List;

/** Compares tilde expansion and real history discovery with the pinned source. */
class PathExpansionProbe {
  static int checks, failures;
  static String q(Object value) { return ParserProbe.q(value); }
  static String compare(AdapterProbe.Client nativeCode, AdapterProbe.Client source, String fields) throws Exception {
    var request = "{" + fields + "}";
    var expected = source.call(request);
    var actual = nativeCode.call(request);
    checks++;
    if (!actual.equals(expected)) {
      if (failures < 20) System.out.println(request + "\nNative: " + actual + "\nSource: " + expected);
      failures++;
    }
    return expected;
  }
  public static void main(String[] args) throws Exception {
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"), "path-expansion-");
    boolean passed = false;
    try (var nativeCode = new AdapterProbe.Client(checkout, true, "path_expansion_host.py");
         var source = new AdapterProbe.Client(checkout, false, "path_expansion_host.py")) {
      var username = System.getProperty("user.name");
      var home = Path.of(System.getProperty("user.home"));
      var names = List.of("", username, "root", "__te_unknown_user_51725__", "é😀", "\\", "with spaces", "a".repeat(4096), "\u0000", username + "\u0000");
      for (var base : List.of("", "/", "//", root.toString(), root + "/", root + "//"))
        for (var name : names)
          for (var tail : List.of("", "/", "//", "/a//b/../", "\\a", "/é😀", "/\u0000"))
            compare(nativeCode, source, "\"path\":" + q("~" + name + tail) + ",\"env\":{\"HOME\":" + q(base) + "}");
      for (var path : List.of("", "relative", "/absolute/~", "./~root", "plain\u0000", "a~b"))
        compare(nativeCode, source, "\"path\":" + q(path));

      var config = Files.createDirectories(root.resolve("config"));
      var projects = Files.createDirectories(config.resolve("projects"));
      var folder = Files.createDirectories(projects.resolve("-fixture"));
      var log = folder.resolve("path-audit-session.jsonl");
      Files.writeString(log, "{}\n");
      Files.setLastModifiedTime(log, FileTime.fromMillis(1_700_000_000_000L));
      Files.createSymbolicLink(root.resolve("linked"), config);
      var relative = home.relativize(config).toString();
      var extra = List.of(config.toString(), projects.toString(),
          "~/config", "~" + username + "/" + relative, "~/linked", "missing", "~__te_unknown_user_51725__", "~/config/../config");
      var common = "\"operation\":\"history\",\"cwd\":" + q(root)
          + ",\"env\":{\"HOME\":" + q(root) + "},\"history_env\":{\"CLAUDE_CONFIG_DIR\":" + q(root.resolve("absent")) + "}";
      for (var path : extra) for (boolean objects : List.of(false, true)) {
        var expected = compare(nativeCode, source, common + ",\"extra\":[" + q(path) + "],\"path_objects\":" + objects);
        if ((path.startsWith("~/") || path.startsWith("~" + username + "/") || Path.of(path).isAbsolute())
            && !expected.contains("path-audit-session")) throw new AssertionError("History fixture did not discover its log: " + path);
      }
      compare(nativeCode, source, common + ",\"extra\":" + ParserProbe.array(extra));
      compare(nativeCode, source, common + ",\"extra\":null");
      for (var value : List.of("17", "true", "null", "[]", "{}"))
        compare(nativeCode, source, common + ",\"extra\":[" + value + "]");
      compare(nativeCode, source, common + ",\"extra\":[\"~\\u0000\"]");
      if (failures != 0) throw new AssertionError(failures + " mismatches in " + checks + " path comparisons");
      passed = true;
    } finally {
      if (passed) ContainerProbe.clear(root); else System.err.println("Probe artifacts: " + root);
    }
    System.out.println("Path expansion and Claude history match the source in " + checks + " comparisons");
  }
}
