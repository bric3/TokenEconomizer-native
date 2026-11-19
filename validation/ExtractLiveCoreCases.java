//JAVA 25

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.HexFormat;
import java.util.regex.Pattern;

/** Copies the pinned parameter tables; Python's AST library only decodes literal data. */
class ExtractLiveCoreCases {
  public static void main(String[] args) throws Exception {
    if (args.length > 1 || args.length == 1 && !args[0].equals("--write")) throw new IllegalArgumentException("usage: ExtractLiveCoreCases.java [--write]");
    var source = Files.readString(Path.of("reference/upstream/tests/test_live_core.py"));
    var tap = Pattern.compile("(?m)^TAP_FAILING = (\"[^\\r\\n]*\")$").matcher(source);
    if (!tap.find()) throw new AssertionError("Missing TAP fixture");
    var parameters = Pattern.compile("(?ms)^@pytest\\.mark\\.parametrize\\(\"[^\"]+\",\\s*(\\[.*?\\])\\)\\r?\\ndef test_(\\w+)\\(").matcher(source);
    var hash = HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(source.getBytes(StandardCharsets.UTF_8)));
    var tables = new ArrayList<String>();var names = new HashSet<String>();
    tables.add("\"_source_sha256\":\"" + hash + "\"");
    while (parameters.find()) {
      var name = parameters.group(2);
      if (!names.add(name)) throw new AssertionError("Duplicate parameter table: " + name);
      var literal = parameters.group(1).replace("TAP_FAILING", tap.group(1));
      var process = new ProcessBuilder(".venv/bin/python", "-c", "import ast,json,sys; print(json.dumps(ast.literal_eval(sys.stdin.read()), ensure_ascii=False, indent=2))")
          .redirectError(ProcessBuilder.Redirect.INHERIT).start();
      try (var input = process.outputWriter(StandardCharsets.UTF_8)) {input.write(literal);}
      var json = new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8).strip();
      if (process.waitFor() != 0) throw new AssertionError("Literal decoding failed: " + name);
      tables.add("\"" + name + "\":" + json);
    }
    if (names.size() != 18) throw new AssertionError("Expected all 18 parameter tables, found " + names.size());
    var text = "{\n" + String.join(",\n",tables) + "\n}\n";
    var path = Path.of("tests/fixtures/live_core_shell.json");
    if (args.length == 1) Files.writeString(path,text);
    if (!Files.readString(path).equals(text)) throw new AssertionError("Native parameter tables differ from the pinned source");
    System.out.println("Verified all " + names.size() + " live-core parameter tables; source SHA-256: " + hash);
  }
}
