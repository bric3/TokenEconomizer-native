//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java

import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/** Checks redirect insertion and byte preservation for Python JSON constants and malformed tokens. */
class HermesRedirectProbe {
  static final List<String> RESULTS = new ArrayList<>();
  static void shapes(String value) {
    RESULTS.add(value);
    RESULTS.add("{\"value\":"+value+"}");
    RESULTS.add("["+value+"]");
    RESULTS.add("{\"nested\":[{\"value\":"+value+"}]}");
  }
  public static void main(String[] args) throws Exception {
    for (var value : List.of("NaN", "Infinity", "-Infinity", "null", "true", "false", "0", "-0", "1e9999", "-1e9999",
        "1e-9999", "123456789012345678901234567890", "9".repeat(512), "\"NaN Infinity -Infinity\"", "\"\\ud800\"",
        "\"\\ud800\\udc00\"", "\"\\udbff\\udfff\"", "\"\\u0000\"")) shapes(value);
    for (var token : List.of("NaN", "Infinity", "-Infinity"))
      for (var before : List.of("", "-", "+", "0", "1", "a", "true", "\\", " "))
        for (var after : List.of("", "0", "1", ".0", "e1", "NaN", "Infinity", "true", ",", "]", "}", ":"))
          shapes(before+token+after);
    for (var raw : List.of("", "plain output", "{}", " \r\n{\t}\r\n", " { \"value\" : NaN, \"keep\" : -Infinity } \r\n",
        "{\"tokeneconomizer_redirect\":1,\"value\":NaN}", "{\"value\":NaN,\"value\":Infinity}",
        "{\"NaN\":\"Infinity\",\"value\":-Infinity}", "{\"\\ud800\":NaN}", "{\"value\":\"\\NaN\"}",
        "{\"value\":\"\\Infinity\"}", "{\"value\":\"\\\\NaN\"}", "{\"value\":\"\\\"NaN\",\"other\":Infinity}",
        "{\"value\":\"NaN\\\"Infinity\"}", "{\"value\":NaN,}", "{\"value\":NaN} extra", "{\"value\":nan}",
        "{\"value\":inf}", "{\"value\":+Infinity}", "{\"value\":-NaN}", "\ufeff{\"value\":NaN}")) RESULTS.add(raw);
    for (int depth : List.of(1, 16, 64, 100)) RESULTS.add("{\"value\":"+"[".repeat(depth)+"NaN"+"]".repeat(depth)+"}");
    var cases = new ArrayList<String>();
    for (var result : RESULTS) for (var text : List.of("redirect", "", "é😀\n\\\"NaN Infinity"))
      cases.add("{\"result\":"+ParserProbe.q(result)+",\"text\":"+ParserProbe.q(text)+"}");
    for (var result : List.of("null", "true", "17", "[]", "{}")) cases.add("{\"result\":"+result+",\"text\":\"redirect\"}");
    int failures = 0;
    var checkout = Path.of("").toAbsolutePath();
    try (var nativeCode = new AdapterProbe.Client(checkout,true,"hermes_redirect_host.py");
         var source = new AdapterProbe.Client(checkout,false,"hermes_redirect_host.py")) {
      for (int index=0; index<cases.size(); index+=50) {
        var batch=cases.subList(index,Math.min(index+50,cases.size()));
        var request="["+String.join(",",batch)+"]";
        var expected=source.call(request);
        var actual=nativeCode.call(request);
        if (!actual.equals(expected)) for (var item : batch) {
          expected=source.call("["+item+"]"); actual=nativeCode.call("["+item+"]");
          if (!actual.equals(expected)) {
            if(failures<12)System.out.println(item+"\nNative: "+actual+"\nSource: "+expected);
            failures++;
          }
        }
      }
    }
    if (failures!=0) throw new AssertionError(failures+" mismatches in "+cases.size()+" Hermes redirect cases");
    System.out.println("Hermes redirects match source bytes in "+cases.size()+" cases covering non-finite constants, malformed tokens, escaped strings, Unicode and nested objects");
  }
}
