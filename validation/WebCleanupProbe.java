//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java

import java.nio.file.Path;
import java.util.List;

/** Compares registry snapshots, signal registration and signal-handler coordination. */
class WebCleanupProbe {
  static int cases;
  static String q(Object value){return ParserProbe.q(value);}
  static void compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,String fields) throws Exception {
    var request="{"+fields+"}";var expected=source.call(request);var actual=nativeCode.call(request);
    if(!actual.equals(expected))throw new AssertionError("Case "+cases+": "+request+"\nNative: "+actual+"\nSource: "+expected);
    cases++;
  }
  public static void main(String[] args) throws Exception {
    boolean swe=args.length>0&&args[0].equals("swebench");
    boolean sdk=List.of(args).contains("--sdk");
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"web_cleanup_host.py",sdk?new String[]{"--sdk"}:new String[0]);var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"web_cleanup_host.py")) {
      for(var mode:List.of("cleanup","install","handler")) {
        var base="\"mode\":"+q(mode)+",\"domain\":"+q(swe?"swebench":"webbench");
        compare(nativeCode,source,base);
        for(var option:List.of("\"sessions\":0","\"sessions\":1","\"mutate_registry\":true","\"replace_list\":true","\"args\":[1,null,{}]","\"repeat\":true","\"mutate_signals\":true","\"replace_cleanup\":true","\"opaque_pid\":true,\"opaque_signum\":true","\"mutate_kill\":true","\"mutate_reset\":true"))compare(nativeCode,source,base+","+option);
        if(swe)for(var option:List.of("\"opaque_names\":true","\"mutate_run\":true","\"entries\":[null,false,0,{},[],\"é😀\"]","\"entries\":[\"duplicate\",\"duplicate\"]"))compare(nativeCode,source,base+","+option);
        var phases=mode.equals("cleanup")?(swe?List.of("iter","len","list","get.run","run","get.discard","discard"):List.of("iter","len","list","get.close:0","close:0","get.close:1","close:1")):mode.equals("install")?List.of("signal.SIGTERM","signal.SIGINT","signal.SIGHUP","signal.signal","register"):(swe?List.of("cleanup","iter","len","get.run","run","get.discard","discard","signal.signal","signal.SIG_DFL","register","getpid","kill"):List.of("cleanup","iter","len","get.close:0","close:0","signal.signal","signal.SIG_DFL","register","getpid","kill"));
        for(var phase:phases)for(var error:List.of("RuntimeError","OSError","ValueError","KeyboardInterrupt","DetailError","ShadowOSError","ShadowValueError")) {
          var extra=phase.equals("list")?",\"replace_list\":true":phase.equals("cleanup")?",\"replace_cleanup\":true":"";
          compare(nativeCode,source,base+",\"fail\":"+q(phase)+",\"error\":"+q(error)+extra);
        }
        if(!mode.equals("cleanup"))for(int attempt:List.of(2,3))for(var phase:List.of("signal.signal","register"))for(var error:List.of("ValueError","OSError","KeyboardInterrupt"))compare(nativeCode,source,base+",\"fail\":"+q(phase)+",\"fail_at\":"+attempt+",\"error\":"+q(error));
        if(swe&&!mode.equals("install"))for(int attempt:List.of(2,3))for(var phase:List.of("get.run","run","get.discard","discard"))for(var error:List.of("OSError","KeyboardInterrupt"))compare(nativeCode,source,base+",\"fail\":"+q(phase)+",\"fail_at\":"+attempt+",\"error\":"+q(error));
      }
    }
    System.out.println((swe?"SWE-bench":"Web-Bench")+" registry and signal policy matches the source in "+cases+" comparisons; snapshots, callback identity, registration order, eager captures and exception boundaries passed");
  }
}
