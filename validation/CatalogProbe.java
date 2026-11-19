//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

/** Offline comparison of the real source capture flow, including cleanup and snapshot writes. */
class CatalogProbe {
  static int checks;
  static String q(Object value) { return ParserProbe.q(value); }
  static void compare(AdapterProbe.Client nativeCode, AdapterProbe.Client source, Path out, String fields) throws Exception {
    var request = "{\"out\":" + q(out) + (fields.isEmpty()?"":","+fields) + "}";
    var expected = source.call(request);
    var actual = nativeCode.call(request);
    if (!actual.equals(expected)) throw new AssertionError(request + "\nNative: " + actual + "\nSource: " + expected);
    checks++;
  }
  public static void main(String[] args) throws Exception {
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"), "catalog-");
    boolean passed = false;
    try (var nativeCode = new AdapterProbe.Client(checkout,true,"catalog_host.py");
         var source = new AdapterProbe.Client(checkout,false,"catalog_host.py")) {
      var out = root.resolve("snapshot/tools.json");
      for (var text : List.of("null","false","17","[]","{}",q(""),q("bad\n{\"type\":\"system\",\"tools\":[\"Write\"]}\n"),q("{}\n{\"type\":\"system\",\"tools\":null}\n{\"type\":\"system\",\"tools\":[]}"),q("[]"),q("null"),q("false"),q("17"),q("\"text\"")))
        compare(nativeCode,source,out,"\"operation\":\"system_tools\",\"text\":"+text);
      compare(nativeCode,source,out,"");
      compare(nativeCode,source,out,"\"operation\":\"system_tools\",\"text\":"+q("{\"type\":\"system\",\"tools\":[\"Write\"],\"ignored\":NaN}"));
      compare(nativeCode,source,out,"\"operation\":\"system_tools\",\"text\":"+q("NaN"));
      for (var limit : List.of("9".repeat(100),"-"+"9".repeat(100)))
        compare(nativeCode,source,out,"\"options\":{\"desc_chars\":"+q(limit)+"}");
      for (var environment : List.of("{}","{\"ANTHROPIC_API_KEY\":\"\"}","{\"ANTHROPIC_API_KEY\":\"fixture-key\",\"TE_CATALOG_PORT\":\"0\",\"TE_MAX_TURNS\":\"2\",\"TE_CATALOG_DESC_CHARS\":\"5\",\"ANTHROPIC_MODEL\":\"model\",\"TE_EFFORT_LEVEL\":\"low\"}","{\"ANTHROPIC_API_KEY\":\"fixture-key\",\"TE_CATALOG_PORT\":\"bad\"}"))
        compare(nativeCode,source,out,"\"env\":"+environment);
      for (var limit : List.of(-100,-1,0,1,5,300)) for (var prior : List.of("{}","{\"Old\":\"retained\",\"Write\":\"longer than the capture\"}","bad"))
        compare(nativeCode,source,out,"\"options\":{\"desc_chars\":"+limit+"},\"prior\":"+q(prior));
      for (var readiness : List.of("[\"7\",\"7\\n\",\"0\"]","[\"7\"]","[\"6\"]","[\" 7 \",\"7\\u001f\",\"0\"]"))
        compare(nativeCode,source,out,"\"readiness\":"+readiness);
      for (var captured : List.of("{}","bad","[]","null","{\"Write\":\"é😀 descriptions\",\"Read\":\"read\"}"))
        compare(nativeCode,source,out,"\"captures\":["+q(captured)+"]");
      var streams = List.of("{\"type\":\"system\",\"tools\":[\"Write\",\"Task\"]}","{\"type\":\"system\",\"tools\":[\"Write\",\"Read\"]}");
      for (var captures : List.of(List.of("{\"Write\":\"write\"}","{\"Write\":\"write\",\"Read\":\"read\"}"),List.of("{}","{\"Write\":\"write\"}")))
        compare(nativeCode,source,out,"\"streams\":"+ParserProbe.array(streams)+",\"captures\":"+ParserProbe.array(captures));
      compare(nativeCode,source,out,"\"docker_code\":23,\"docker_stderr\":"+q("failure é😀".repeat(100)));
      for (var failure : List.of("run","popen","agent","claude_bin","image_for","log","sleep","live_add","live_discard"))
        for (var error : List.of("RuntimeError","OSError","KeyboardInterrupt"))
          for (int at : List.of(1,2,3))
            compare(nativeCode,source,out,"\"fail\":"+q(failure)+",\"error\":"+q(error)+",\"fail_at\":"+at+",\"readiness\":[\"7\",\"0\"]");
      passed = true;
    } finally { if (passed) ContainerProbe.clear(root); else System.err.println("Probe artifacts: " + root); }
    System.out.println("Catalogue capture matches the source in " + checks + " offline comparisons");
  }
}
