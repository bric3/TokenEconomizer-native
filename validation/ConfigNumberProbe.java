//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java
//SOURCES ../scripts/UnicodeRegexTables.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Locale;
import java.util.regex.MatchResult;
import java.util.regex.Pattern;

/** Compares configuration coercion with Python's decimal digits, separators and whitespace. */
class ConfigNumberProbe {
  static final List<String> INTEGER_SETTINGS = List.of("microskills_max_facts", "microskills_max_tokens", "microskills_propose_every",
      "toolprune_evidence", "toolprune_min_sessions", "toolprune_sessions", "toolprune_quiet_minutes",
      "loopguard_nudges", "repomap_tokens", "repomap_timeout", "history_max_sessions", "history_days");
  record Case(String key, String valueJson) {
    String json() { return "{\"key\":" + ParserProbe.q(key) + ",\"value\":" + valueJson + "}"; }
  }
  static final List<Case> CASES = new ArrayList<>();
  static void number(String text) {
    for (var key : List.of("history_max_sessions", "toolprune_quiet_minutes", "local_judge_confidence", "loopguard.min_turns", "loopguard.stagnation"))
      CASES.add(new Case(key, ParserProbe.q(text)));
  }
  static int ioChecks;
  static String compare(AdapterProbe.Client nativeCode, AdapterProbe.Client source, String request) throws Exception {
    var expected = source.call(request);
    var actual = nativeCode.call(request);
    if (!actual.equals(expected)) throw new AssertionError(request+"\nNative: "+actual+"\nSource: "+expected);
    ioChecks++;
    return expected;
  }
  static void persistence(AdapterProbe.Client nativeCode, AdapterProbe.Client source, Path root) throws Exception {
    var registry = compare(nativeCode, source, "[{\"action\":\"environment\"}]");
    var names = Pattern.compile("TE_[A-Z_]+").matcher(registry).results().map(MatchResult::group).toList();
    if (names.size() != 54) throw new AssertionError("Environment registry changed: " + registry);
    for (var name : names) for (var value : List.of("null", "\"\"", "\"on\"", "\"2\"", "\"é😀\"", "{}"))
      compare(nativeCode, source, "[{\"action\":\"load\",\"root\":"+ParserProbe.q(root)+",\"env\":{\""+name+"\":"+value+"}}]");
    compare(nativeCode, source, "[{\"action\":\"load\",\"root\":"+ParserProbe.q(root)
        +",\"env\":{\"TE_INJECT\":\"off\",\"TE_MICROSKILLS\":\"on\",\"TE_UNTOOL\":\"off\",\"TE_TOOLPRUNE\":\"auto\",\"TE_SPIRAL\":\"off\",\"TE_LOOPGUARD\":\"nudge\"}}]");
    var file = root.resolve("config.json");
    var repo = Files.createDirectory(root.resolve("repo"));
    var seed = "{\"paid_model\":\"keep\"}\n";
    for (var key : INTEGER_SETTINGS) for (var value : List.of("9007199254740993", "9007199254740995", "1234567890123456789", "9223372036854775806", "9223372036854775807")) {
      var write = "[{\"action\":\"write\",\"path\":"+ParserProbe.q(file)+",\"updates\":{\""+key+"\":"+value+"}}]";
      Files.writeString(file,seed);
      var expected = source.call(write);
      var saved = Files.readAllBytes(file);
      if (!expected.contains("\""+key+"\":"+value)) throw new AssertionError("Source did not preserve the integer: "+expected);
      Files.writeString(file,seed);
      var actual = nativeCode.call(write);
      if (!actual.equals(expected) || !Arrays.equals(saved,Files.readAllBytes(file))) throw new AssertionError("Saved integer changed: "+write+"\nNative: "+actual+"\nSource: "+expected);
      ioChecks++;
      compare(nativeCode,source,"[{\"action\":\"load\",\"root\":"+ParserProbe.q(root)+"}]");
      Files.writeString(file,"{\""+key+"\":1}");
      Files.writeString(repo.resolve("config.json"),"{\""+key+"\":"+value+"}");
      compare(nativeCode,source,"[{\"action\":\"load\",\"root\":"+ParserProbe.q(root)+",\"repo\":"+ParserProbe.q(repo)+"}]");
      compare(nativeCode,source,"[{\"action\":\"load\",\"root\":"+ParserProbe.q(root)+",\"repo\":"+ParserProbe.q(repo)
          +",\"env\":{\"TE_"+key.toUpperCase(Locale.ROOT)+"\":"+ParserProbe.q(value)+"}}]");
    }
    for (var value : List.of("null", "[]", "{}", "true")) {
      Files.writeString(file,seed);
      var result = compare(nativeCode,source,"[{\"action\":\"write\",\"path\":"+ParserProbe.q(file)+",\"updates\":{\"history_max_sessions\":"+value+"}}]");
      if (!result.contains("\"raised\":\"ValueError\"") || !Files.readString(file).equals(seed)) throw new AssertionError("A rejected update changed the file: "+result);
    }
  }
  public static void main(String[] args) throws Exception {
    var digits = new ArrayList<Integer>();
    for (var line : UnicodeRegexTables.input(17, "UnicodeData.txt")) {
      var fields = line.split(";", -1);
      if (fields[2].equals("Nd")) {
        int point = Integer.parseInt(fields[0], 16);
        digits.add(point);
        number(new String(Character.toChars(point)));
        if (fields[6].equals("1")) {
          var one = new String(Character.toChars(point));
          var zero = new String(Character.toChars(point - 1));
          for (var text : List.of("+"+one, "-"+one, one+"_"+zero, one+"."+zero, "."+one,
              one+"e+"+one, zero+"."+one, one+"_0", "1_"+one, "_"+one, one+"_", one+"__0", one+"._0")) number(text);
        }
      }
    }
    for (var point : List.of(0x9, 0xa, 0xb, 0xc, 0xd, 0x1c, 0x1d, 0x1e, 0x1f, 0x20, 0x85, 0xa0,
        0x1680, 0x2000, 0x2007, 0x200b, 0x2028, 0x2029, 0x202f, 0x205f, 0x3000, 0xfeff)) {
      var space = new String(Character.toChars(point));
      for (var text : List.of(space+"1", "1"+space, space+"١"+space, "1"+space+"0")) number(text);
    }
    for (var text : List.of("", " ", "+", "-", "1__0", "+_1", "1_.0", "1._0", "1e_0", "1_e0", "1e+_0",
        "NaN", "+nan", "-NaN", "Inf", "iNfInItY", "+INFINITY", "-inf", "NAN(0)", "0x1", "1d0", "1.e1", ".0", ".", "1\u0000",
        "²", "①", "Ⅻ", "−1", "1.0", "1e1", "1e-1", "0.9", "1_0.0_0e+0_1")) number(text);
    for (var key : INTEGER_SETTINGS) {
      for (var value : List.of("null", "true", "false", "[]", "{}", "0", "1", "-1", "0.0", "1.0", "1.25", "-1.0",
          "4503599627370495", "4503599627370496", "9007199254740991", "9007199254740992", "9007199254740993",
          "9007199254740995", "1234567890123456789", "9223372036854775806", "9223372036854775807",
          "-9007199254740993", "-9223372036854775808", "9007199254740993.0", "9.223372036854775e18"))
        CASES.add(new Case(key, value));
      for (var value : List.of("9007199254740993", "9007199254740995", "1234567890123456789",
          "9223372036854775806", "9223372036854775807", "-9223372036854775808"))
        CASES.add(new Case(key, ParserProbe.q(value)));
    }
    int failures = 0;
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"),"config-number-");
    boolean passed = false;
    try (var nativeCode = new AdapterProbe.Client(checkout, true, "config_number_host.py");
         var source = new AdapterProbe.Client(checkout, false, "config_number_host.py")) {
      for (int index = 0; index < CASES.size(); index += 50) {
        var batch = CASES.subList(index, Math.min(index + 50, CASES.size()));
        var request = "[" + String.join(",", batch.stream().map(Case::json).toList()) + "]";
        var expected = source.call(request);
        var actual = nativeCode.call(request);
        if (!actual.equals(expected)) {
          for (var sample : batch) {
            var item = "[" + sample.json() + "]";
            var wanted = source.call(item);
            var got = nativeCode.call(item);
            if (!got.equals(wanted)) {
              if (failures < 12) System.out.println(sample.json()+"\nNative: "+got+"\nSource: "+wanted);
              failures++;
            }
          }
        }
      }
      if (failures == 0) persistence(nativeCode,source,root);
      passed = failures == 0;
    } finally {
      if (passed) ContainerProbe.clear(root); else System.err.println("Probe artifacts: "+root);
    }
    if (failures != 0) throw new AssertionError(failures + " mismatches in " + CASES.size() + " numeric configuration cases");
    System.out.println("Configuration coercion matches the source in " + CASES.size() + " cases covering all " + digits.size() + " Unicode 17 decimal digits, separators, exponents, whitespace, signed 64-bit integer boundaries and rejected types; "+ioChecks+" saved-file, global/repository and environment checks passed");
  }
}
