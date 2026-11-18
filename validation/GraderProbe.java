//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;
import java.util.ArrayList;

/** Compares the official-grader wrapper using local SDK stand-ins. */
class GraderProbe {
  static int cases;
  static int jsonErrors;
  static List<String> diagnosticDifferences = new ArrayList<>();
  static Path artifacts;
  static String q(Object value) { return ParserProbe.q(value); }
  static void clear(Path root) throws Exception {
    if (Files.exists(root)) try (var paths = Files.walk(root)) {
      for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.delete(path);
    }
  }
  static void compare(AdapterProbe.Client nativeCode, AdapterProbe.Client source, String fields) throws Exception {
    compare(nativeCode,source,fields,null);
  }
  static void compare(AdapterProbe.Client nativeCode, AdapterProbe.Client source, String fields, String jsonDocument) throws Exception {
    var root = artifacts.resolve("case-" + cases);
    var request = "{\"kind\":\"official\",\"root\":" + q(root) + (fields.isEmpty() ? "" : "," + fields.replace("@ROOT@",q(root))) + "}";
    var expected = source.call(request); clear(root);
    var actual = nativeCode.call(request);
    if(jsonDocument != null) {
      for(var result:List.of(expected,actual)) if(!result.contains("\"raised\":\"JSONDecodeError\"") || !result.contains("\"doc\":"+q(jsonDocument))) throw new AssertionError("JSON failure lost its type or document: "+result);
      jsonErrors++;
      if(!actual.equals(expected))diagnosticDifferences.add("{\"request\":"+request+",\"source\":"+expected+",\"native\":"+actual+"}");
      cases++;return;
    }
    if (cases == 0 && (!expected.contains("\"value\":") || expected.contains("\"raised\":"))) throw new AssertionError("Source fixture did not complete: " + expected);
    if (!actual.equals(expected)) {
      Files.writeString(artifacts.resolve("request.json"),request);
      Files.writeString(artifacts.resolve("expected.json"),expected);
      Files.writeString(artifacts.resolve("actual.json"),actual);
      int at=0; while(at<expected.length() && at<actual.length() && expected.charAt(at)==actual.charAt(at))at++;
      throw new AssertionError("Case " + cases + " differs at " + at + "\nNative: " + actual.substring(Math.max(0,at-100),Math.min(actual.length(),at+240)) + "\nSource: " + expected.substring(Math.max(0,at-100),Math.min(expected.length(),at+240)));
    }
    cases++;
  }
  public static void main(String[] args) throws Exception {
    artifacts=Files.createTempDirectory("te-grader-probe-"); boolean passed=false;
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"task_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"task_host.py")) {
      compare(nativeCode,source,"");
      for(var action:List.of("instance","eval_script","score","grade","task"))compare(nativeCode,source,"\"action\":"+q(action));
      for(var row:List.of("{}","null","[]","\"\"","[\"ab\"]","[[\"repo\",\"acme/widget\"],[\"problem_statement\",null]]","{\"instance_id\":\"wrong\",\"problem_statement\":null}","{\"FAIL_TO_PASS\":\"null\",\"PASS_TO_PASS\":\"{}\"}","{\"FAIL_TO_PASS\":\"[\\\"é😀\\\"]\",\"PASS_TO_PASS\":null}","{\"FAIL_TO_PASS\":[],\"PASS_TO_PASS\":0}","true","5","1.5","\"x\"","[1]","[[\"x\"]]","[[\"x\",1,2]]"))
        compare(nativeCode,source,"\"table\":{\"acme__widget-1\":"+row+"}");
      for(var table:List.of("{}","[]","null","false","5","\"x\""))compare(nativeCode,source,"\"table\":"+table);
      for(var resolved:List.of("true","false","null","0","\"\"","\"yes\"","[]","[1]","{}","{\"ok\":true}"))
        compare(nativeCode,source,"\"action\":\"score\",\"report\":{\"acme__widget-1\":{\"resolved\":"+resolved+",\"keep\":7},\"other\":{\"resolved\":false}}");
      for(var report:List.of("{}","null","[]","false","\"x\"","{\"other\":{\"resolved\":true}}","{\"acme__widget-1\":null}","{\"acme__widget-1\":[]}"))
        compare(nativeCode,source,"\"action\":\"score\",\"report\":"+report);
      for(var method:List.of("import_grader","make_test_spec","eval_script_value","get_eval_report"))
        for(var kind:List.of("RuntimeError","ValueError","KeyboardInterrupt","TimeoutExpired","ShadowRuntimeError"))
          compare(nativeCode,source,"\"action\":"+q(method.equals("eval_script_value")?"eval_script":"score")+",\"fail\":"+q(method)+",\"error_kind\":"+q(kind));
      for(var action:List.of("eval_script","score"))compare(nativeCode,source,"\"action\":"+q(action)+",\"instance_override\":{\"instance_id\":\"override\",\"problem_statement\":\"injected\"}");
      compare(nativeCode,source,"\"path_object\":true");
      compare(nativeCode,source,"\"grading_path\":\"grading.json\",\"local_cwd\":true");
      compare(nativeCode,source,"\"grading_path\":null,\"env\":{\"TE_SWEBENCH_GRADING\":\"grading.json\"},\"local_cwd\":true");
      compare(nativeCode,source,"\"grading_path\":\"\",\"env\":{\"DATA\":@ROOT@},\"table_filename\":\"swebench/swebench_grading.json\"");
      for(var env:List.of("{}","{\"DATA\":\"\"}","{\"DATA\":\"a//\"}","{\"DATA\":\"a\",\"TE_SWEBENCH_GRADING\":\"grading.json\"}","{\"DATA\":\"a\",\"TE_SWEBENCH_GRADING\":\"\"}"))
        compare(nativeCode,source,"\"kind\":\"grading_setting\",\"env\":"+env);
      for(var env:List.of("{}","{\"TE_SWEBENCH_REPO\":\"\"}","{\"TE_SWEBENCH_REPO\":\".\"}","{\"TE_SWEBENCH_REPO\":\"missing/../sdk\"}"))
        compare(nativeCode,source,"\"kind\":\"grading_setting\",\"method\":\"swebench_repo\",\"env\":"+env+",\"local_cwd\":true");
      for(var bytes:List.of("[255]","[195]","[195,32]","[97,237,160,128]"))compare(nativeCode,source,"\"table_bytes\":"+bytes);
      compare(nativeCode,source,"\"table_text\":"+q("{\r\n\"acme__widget-1\": {\r\"repo\": \"é😀\"\r}}"));
      compare(nativeCode,source,"\"skip_table\":true");
      compare(nativeCode,source,"\"skip_table\":true,\"grading_path\":\"grading.json\",\"local_cwd\":true");
      compare(nativeCode,source,"\"table_is_directory\":true");
      compare(nativeCode,source,"\"skip_table\":true,\"table_filename\":\"grading é😀 'file'.json\"");
      compare(nativeCode,source,"\"grading_path\":"+q("bad\u0000path"));
      compare(nativeCode,source,"\"table_text\":\"{\"","{");
      compare(nativeCode,source,"\"table\":{\"acme__widget-1\":{\"FAIL_TO_PASS\":\"[1,\"}}","[1,");
      compare(nativeCode,source,"\"table\":{\"acme__widget-1\":{\"PASS_TO_PASS\":\"x\"}}","x");
      var diagnosticLog=Path.of("validation/results/grader-json-diagnostics.json");
      Files.createDirectories(diagnosticLog.getParent());
      Files.writeString(diagnosticLog,"["+String.join(",",diagnosticDifferences)+"]\n");
      System.out.println("Official-grader wrapper matches the source in "+(cases-jsonErrors)+" cases; table loading, normalisation, SDK arguments, report selection and nested task execution passed with local SDK stand-ins");
      System.out.println(jsonErrors+" JSON exception type/document checks passed; "+diagnosticDifferences.size()+" diagnostic differences remain in "+diagnosticLog);
      passed=true;
    } finally { if(passed)clear(artifacts);else System.err.println("Probe artifacts: "+artifacts); }
  }
}
