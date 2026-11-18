//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;

/** Checks runner settings and the exact provenance file against the pinned source. */
class SettingsProbe {
  static int cases;
  static Path artifacts;
  static String q(Object value) { return ParserProbe.q(value); }
  static void clear(Path root) throws Exception {
    if(Files.exists(root))try(var paths=Files.walk(root)) {
      for(var path:paths.sorted(Comparator.reverseOrder()).toList())Files.delete(path);
    }
  }
  static void compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var root=artifacts.resolve("case-"+cases);
    var request="{\"kind\":\"run_config\",\"root\":"+q(root)+(fields.isEmpty()?"":","+fields)+"}";
    var expected=source.call(request);clear(root);
    var actual=nativeCode.call(request);
    if(cases==0 && (expected.contains("\"raised\":") || !expected.contains("\"run_config.json\":[")))throw new AssertionError("Source fixture did not write a config: "+expected);
    if(!actual.equals(expected)) {
      Files.writeString(artifacts.resolve("request.json"),request);
      Files.writeString(artifacts.resolve("expected.json"),expected);
      Files.writeString(artifacts.resolve("actual.json"),actual);
      int at=0;while(at<actual.length()&&at<expected.length()&&actual.charAt(at)==expected.charAt(at))at++;
      throw new AssertionError("Case "+cases+" differs at "+at+"\nNative: "+actual.substring(Math.max(0,at-100),Math.min(actual.length(),at+260))+"\nSource: "+expected.substring(Math.max(0,at-100),Math.min(expected.length(),at+260)));
    }
    cases++;
  }
  static String setting(String method,String arguments,String extra) {
    return "\"kind\":\"runner_setting\",\"method\":"+q(method)+",\"arguments\":{"+arguments+"}"+(extra.isEmpty()?"":","+extra);
  }
  public static void main(String[] args) throws Exception {
    artifacts=Files.createTempDirectory("te-settings-probe-");boolean passed=false;
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"task_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"task_host.py")) {
      compare(nativeCode,source,"");
      for(var raw:List.of("", "0", "-1", "0.9", "1", "2.9", "NaN", "+nan", "-NaN", "inf", "-Infinity", "+inF", "1e1000", "1e-1000", "1e20", "1e100", "1.7976931348623157e308", "0x10", "12_3", "1__2", "1._2", "1e0_2", "١٢.٩", "１２.５", " 8 \u001f", "1 2", "not a number"))
        compare(nativeCode,source,setting("_cpu_limit_args","","\"env\":{\"TE_SWEBENCH_MAX_CPUS\":"+q(raw)+"}"));
      for(var raw:List.of("", "0", " 0 ", "false", "no", "1", "true", "yes", "True", "\u001fyes\n")) {
        compare(nativeCode,source,setting("env_activation_enabled","","\"env\":{\"TE_ENV_ACTIVATION\":"+q(raw)+"}"));
        for(var domain:List.of("swebench","webbench"))compare(nativeCode,source,setting("network_tool_block","","\"domain\":"+q(domain)+",\"env\":{\"TE_ALLOW_NETWORK_TOOLS\":"+q(raw)+"}"));
      }
      for(var bins:List.of("[]","null","false","0","\"\"","\"a\"","[null,\"\",false,0]","[\"/a/bin\"]","[\"/a/bin\",\"/b/bin\"]","[\"/a/bin\",\"/x/testbed/bin\"]","[\"/x/testbed/bin/\",\"/b/bin\"]","[\"/x/testbed/./bin\",\"/b/bin\"]","[1]","[{}]"))
        compare(nativeCode,source,setting("choose_project_env","\"env_bins\":"+bins,""));
      for(var bin:List.of("\"/x/envs/é😀/bin\"","\"relative/bin/\"","\"//\"","\"bin\"","\"\"","null","1"))for(var inherited:List.of("\"/usr/bin:/bin\"","\"\"","null","false","1"))
        compare(nativeCode,source,setting("activation_env","\"env_bin\":"+bin+",\"container_path\":"+inherited,""));
      for(var id:List.of("\"acme__widget-1\"","\"a____b/é😀\"","\"\"","1","null","[]"))compare(nativeCode,source,setting("image_for","\"instance_id\":"+id,""));
      for(boolean scaffold:List.of(false,true))for(boolean guard:List.of(false,true))for(var excluded:List.of("null","[]","[\"Bash\",\"Unused\",\"WebSearch\",\"Unused\"]"))
        compare(nativeCode,source,"\"scaffold\":"+scaffold+",\"guard\":"+guard+",\"excluded\":"+excluded+",\"injected\":\"manual é😀\\n\",\"kwargs\":{\"spiral_guard\":true,\"spiral_judge\":false,\"spiral_nudge_writer\":0,\"abort_envelope\":{\"budget\":10},\"spiral_thr\":{\"beta\":0.25}},\"guard_attributes\":{\"nudge_budget\":2,\"never_abort\":true}");
      for(var injected:List.of("false","0","1","[]","[1]","{}","{\"x\":1}"))compare(nativeCode,source,"\"injected\":"+injected);
      for(var excluded:List.of("false","0","1","\"abc\"","{\"Bash\":1,\"Unused\":2}"))compare(nativeCode,source,"\"excluded\":"+excluded);
      compare(nativeCode,source,"\"scaffold\":true,\"falsey_scaffold\":true,\"guard\":true,\"falsey_guard\":true,\"object_kwargs\":[\"spiral_guard\",\"spiral_judge\",\"spiral_nudge_writer\",\"abort_envelope\",\"spiral_thr\"]");
      compare(nativeCode,source,"\"scaffold\":true,\"scaffold_excludes\":[\"z\",\"a\",\"z\",\"é😀\"],\"profile_excludes\":[\"z\",\"CLAUDE.md\"]");
      compare(nativeCode,source,"\"protected\":[\"Bash\",\"WebSearch\"],\"excluded\":[\"WebSearch\",\"Bash\"],\"env\":{\"TE_ALLOW_NETWORK_TOOLS\":\"true\",\"TE_ENV_ACTIVATION\":\"0\",\"TE_SWEBENCH_MAX_CPUS\":\"0\"},\"optimizer_transport\":\"cli\"");
      for(var field:List.of("agent","model","effort","cli_version","agent_bin","routing_base","optimizer_model","optimizer_transport","protected_tools","scaffold","scaffold_profile","task_timeout","eval_timeout","grader"))
        for(var error:List.of("RuntimeError","AttributeError","KeyboardInterrupt"))compare(nativeCode,source,"\"scaffold\":true,\"excluded\":[\"Unused\"],\"fail\":"+q("attr:"+field)+",\"error_kind\":"+q(error));
      compare(nativeCode,source,"\"missing_run_dir\":true");
      compare(nativeCode,source,"\"config_is_directory\":true");
      compare(nativeCode,source,"\"run_dir\":"+q("bad\u0000path"));
      compare(nativeCode,source,"\"local_cwd\":true,\"run_dir_relative\":true");
      for(var task:List.of("{}","null","[]","\"x\""))compare(nativeCode,source,"\"task\":"+task);
      compare(nativeCode,source,"\"scaffold\":true,\"fail\":\"_patch_excludes\"");
      for(var cpu:List.of("0","2.9","inf"))compare(nativeCode,source,"\"env\":{\"TE_SWEBENCH_MAX_CPUS\":"+q(cpu)+"}");
      for(var kwargs:List.of("{}","{\"max_turns\":3,\"spiral_guard\":true}"))compare(nativeCode,source,"\"kind\":\"configured_task\",\"guard\":true,\"scaffold\":true,\"injected\":\"manual é😀\",\"kwargs\":"+kwargs);
      System.out.println("Runner settings and provenance match the source in "+cases+" cases; CPU limits, activation, masks, fingerprints, scaffold exclusions, callback order and file bytes passed without Docker or model calls");
      passed=true;
    } finally {if(passed)clear(artifacts);else System.err.println("Probe artifacts: "+artifacts);}
  }
}
