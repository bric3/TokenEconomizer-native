//JAVA 25

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.HexFormat;
import java.util.regex.Pattern;

/** Copies only the pinned string constants; default mode verifies the native assets. */
class ExtractProposerPrompts {
  private static final Pattern LITERAL = Pattern.compile("\"(?:\\\\.|[^\"\\\\])*\"|'(?:\\\\.|[^'\\\\])*'");

  public static void main(String[] args) throws Exception {
    var source = Files.readString(Path.of("reference/upstream/tokeneconomizer/microskills/distiller.py"));
    var head = constant(source, "_PROPOSER_HEAD", "");
    var tail = constant(source, "PROPOSER_SYSTEM", "_PROPOSER_HEAD%MAX_CANDS").replace("%d", "4");
    var genericHead = constant(source, "_GENERIC_HEAD", "");
    var paper = head + tail;
    var generic = (genericHead + tail).replace("repro_written, importerror, badarg, ran_tests",
        "ran_checks, ran_tests, command_failed, missing_command, missing_dependency, bad_usage, edited");
    var hash = HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(paper.getBytes(StandardCharsets.UTF_8)));
    if (!hash.startsWith("fad9adfb8fda5875")) throw new AssertionError("Paper prompt hash changed: " + hash);
    var directory = Path.of("src/microskills/distiller/prompts");
    boolean write = args.length == 1 && args[0].equals("--write");
    for (var name : new String[]{"paper", "generic"}) {
      var path = directory.resolve(name + ".txt");
      var text = name.equals("paper") ? paper : generic;
      if (write) { Files.createDirectories(directory); Files.writeString(path, text); }
      if (!Files.readString(path).equals(text)) throw new AssertionError("Native prompt differs: " + path);
    }
    System.out.println("Verified native proposer prompts; paper SHA-256: " + hash);
  }

  private static String constant(String source, String name, String expectedRest) {
    var start = source.indexOf(name + " = (");
    if (start < 0) throw new AssertionError("Missing constant: " + name);
    start += (name + " = (").length();
    var end = source.indexOf("\n)", start);
    var body = source.substring(start, end);
    var matcher = LITERAL.matcher(body);
    var text = new StringBuilder();
    var rest = new StringBuilder();
    int previous = 0;
    while (matcher.find()) {
      rest.append(body, previous, matcher.start());
      var literal = matcher.group();
      text.append(decode(literal.substring(1, literal.length() - 1)));
      previous = matcher.end();
    }
    rest.append(body, previous, body.length());
    if (!rest.toString().replaceAll("[\\s+()]", "").equals(expectedRest)) {
      throw new AssertionError("Unexpected expression in " + name + ": " + rest);
    }
    return text.toString();
  }

  private static String decode(String source) {
    var out = new StringBuilder();
    for (int i = 0; i < source.length(); i++) {
      char c = source.charAt(i);
      if (c != '\\') { out.append(c); continue; }
      if (++i == source.length()) throw new AssertionError("Trailing escape");
      out.append(switch (source.charAt(i)) {
        case 'n' -> '\n'; case 'r' -> '\r'; case 't' -> '\t';
        case '\\' -> '\\'; case '\'' -> '\''; case '"' -> '"';
        default -> throw new AssertionError("Unsupported escape: " + source.charAt(i));
      });
    }
    return out.toString();
  }
}
