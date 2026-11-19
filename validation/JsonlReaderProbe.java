//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java

import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

/** Compares byte fixtures through the original and native JSONL iterators. */
class JsonlReaderProbe {
  public static void main(String[] args) throws Exception {
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"), "jsonl-reader-");
    int checks = 0; boolean passed = false;
    try (var nativeCode = new AdapterProbe.Client(checkout, true, "jsonl_host.py"); var source = new AdapterProbe.Client(checkout, false, "jsonl_host.py")) {
      for (int padding : List.of(0, 1, 8178, 8179, 8180, 8181, 8189, 8190, 8191, 8192, 8193, 16383))
        for (var separator : List.of("\n", "\r\n", "\r"))
          for (var space : List.of("", " ", "\u001c", "\u001d", "\u001e", "\u001f", "\u0085", "\u00a0", "\u2003")) {
            var bytes = new ByteArrayOutputStream();
            bytes.write(" ".repeat(padding).getBytes(StandardCharsets.UTF_8));
            bytes.write((separator + space + "{\"text\":\"é😀\"}" + space + separator + "[]" + separator
                + "invalid" + separator + "{\"text\":\"bad ").getBytes(StandardCharsets.UTF_8));
            bytes.write(255);
            bytes.write(("\"}" + separator + "{\"n\":18446744073709551616}" + separator + "{\"half\":").getBytes(StandardCharsets.UTF_8));
            var file = root.resolve("fixture.jsonl"); Files.write(file, bytes.toByteArray());
            for (var method : List.of("lines", "records")) {
              var request = "{\"method\":" + ParserProbe.q(method) + ",\"path\":" + ParserProbe.q(file) + "}";
              var actual = nativeCode.call(request); var expected = source.call(request);
              if (!actual.equals(expected)) throw new AssertionError("padding=" + padding + ", separator=" + ParserProbe.q(separator) + ", space=" + ParserProbe.q(space)
                  + "\nNative: " + actual + "\nSource: " + expected);
              checks++;
            }
          }
      passed = true;
    } finally { if (passed) ContainerProbe.clear(root); else System.err.println("Probe artifacts: " + root); }
    System.out.println("JSONL lines and records match the source in " + checks + " comparisons; chunk boundaries, Unicode whitespace, replacement decoding, skipped rows and incomplete tails passed");
  }
}
