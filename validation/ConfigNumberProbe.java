//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java
//SOURCES ../scripts/UnicodeRegexTables.java

import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/** Compares configuration coercion with Python's decimal digits, separators and whitespace. */
class ConfigNumberProbe {
  record Case(String key, String value) {
    String json() { return "{\"key\":" + ParserProbe.q(key) + ",\"value\":" + ParserProbe.q(value) + "}"; }
  }
  static final List<Case> CASES = new ArrayList<>();
  static void number(String text) {
    for (var key : List.of("history_max_sessions", "toolprune_quiet_minutes", "local_judge_confidence", "loopguard.min_turns", "loopguard.stagnation"))
      CASES.add(new Case(key, text));
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
    int failures = 0;
    var checkout = Path.of("").toAbsolutePath();
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
    }
    if (failures != 0) throw new AssertionError(failures + " mismatches in " + CASES.size() + " numeric configuration cases");
    System.out.println("Configuration coercion matches the source in " + CASES.size() + " cases covering all " + digits.size() + " Unicode 17 decimal digits, separators, exponents and whitespace");
  }
}
