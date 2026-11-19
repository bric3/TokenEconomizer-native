//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java WebContractProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;
import java.util.Random;

/** Checks repo-map policy and executes archive/process/file integration without aider. */
class RepoMapScaffoldProbe {
  static String q(Object value){return ParserProbe.q(value);}
  public static void main(String[] args)throws Exception {
    WebContractProbe.artifacts=Files.createTempDirectory(Path.of("validation/results"),"repomap-scaffold-").toAbsolutePath();boolean passed=false;
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"scaffold_repomap_host.py",args.length==1&&args[0].equals("--sdk")?new String[]{"--sdk"}:new String[0]);var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"scaffold_repomap_host.py")) {
      for(var operation:List.of("tokens","idents","preflight","fetch","generate","setup")) {
        var base="\"operation\":"+q(operation);
        WebContractProbe.compare(nativeCode,source,base);
        for(var option:List.of("\"env\":{\"TE_REPOMAP_TOKENS\":\"512\"}","\"env\":{\"TE_REPOMAP_TOKENS\":\"١_٢\"}","\"env\":{\"TE_REPOMAP_TOKENS\":\"bad\"}","\"opaque_budget\":true","\"text\":null","\"text\":false","\"text\":42","\"text\":[]","\"text\":[1]","\"bytes_text\":true","\"objects\":[\"text\"]","\"objects\":[\"text\"],\"booleans\":{\"text\":false}","\"empty_blob\":true","\"rc\":1","\"rc\":1,\"stderr\":null","\"stdout\":null","\"objects\":[\"rc\"],\"opaque_comparison\":true","\"objects\":[\"stdout\",\"stderr\"]","\"objects\":[\"src_dir\",\"GEN\"]","\"objects\":[\"idents\"]","\"objects\":[\"member_name\"]","\"objects\":[\"regular\"]","\"members\":[]","\"stub_fetch\":true,\"objects\":[\"src\"]","\"stub_fetch\":true,\"src\":null","\"stub_generate\":true,\"map\":\"\"","\"stub_generate\":true,\"objects\":[\"map\",\"err\",\"written\"]","\"stub_generate\":true,\"generated\":[1]","\"stub_generate\":true,\"generated\":[1,2,3]"))WebContractProbe.compare(nativeCode,source,base+","+option);
        var phases=switch(operation) {
          case "tokens"->List.of("env.get","int:budget");
          case "idents"->List.of("bool:text");
          case "preflight"->List.of("import","sys.executable","error.str");
          case "fetch"->List.of("os.path","path.join","context.get:run_dir","join","os.makedirs","makedirs","context.get:read_bytes","read","scaffold.get:TAR_SCRIPT","tarfile.open","io.BytesIO","bytesio","tar.open","enter","members","member.isfile","member.name","extract","exit");
          case "generate"->List.of("subprocess.run","sys.executable","env.get","run","returncode","stdout","stderr");
          default->List.of("context.get:task","env.get","scaffold.get:fetch_sources","read","scaffold.get:generate","run","returncode","stdout","subprocess.TimeoutExpired","os.path","path.isdir","isdir","shutil.rmtree","rmtree","context.get:write_instructions","write","context.get:profile","profile.instructions_file","context.get:log","log","strip:stdout");
        };
        for(var phase:phases)for(var error:List.of("ScaffoldError","TimeoutExpired","RuntimeError","KeyboardInterrupt","DetailError","ImportError")) {
          var options=switch(phase){case "int:budget"->",\"opaque_budget\":true";case "bool:text"->",\"objects\":[\"text\"]";case "stderr"->",\"rc\":1";case "sys.executable","error.str"->operation.equals("preflight")?",\"fail\":\"import\",\"error\":\"DetailImportError\",\"failures\":[{\"fail\":"+q(phase)+",\"error\":"+q(error)+"}]":"";case "subprocess.TimeoutExpired"->",\"failures\":[{\"fail\":\"run\",\"error\":\"RuntimeError\"}]";case "context.get:log","log"->",\"stub_generate\":true,\"map\":\"\"";case "strip:stdout"->",\"objects\":[\"stdout\"]";default->"";};
          var fault=operation.equals("preflight")&&List.of("sys.executable","error.str").contains(phase)?"":",\"fail\":"+q(phase)+",\"error\":"+q(error);
          WebContractProbe.compare(nativeCode,source,base+options+fault);
        }
      }
      for(var text:List.of("","`a.b` `ab` `foo.bar.baz` `A..z`","CFTimeIndex foo_bar a_b _a FooBar _FooBar FooBar_","éfoo_baré ²FooBar² ◌́FooBar◌́","get_axis_num get_axis_num FooBar"))WebContractProbe.compare(nativeCode,source,"\"operation\":\"idents\",\"text\":"+q(text));
      var random=new Random(23819);var alphabet="abcXYZ012_.` é́²𑶠😀\n";
      var points=alphabet.codePoints().toArray();
      for(int run=0;run<180;run++){var text=new StringBuilder();for(int i=0;i<100;i++)text.appendCodePoint(points[random.nextInt(points.length)]);WebContractProbe.compare(nativeCode,source,"\"operation\":\"idents\",\"text\":"+q(text));}
      for(var boundary:List.of(0x301,0xb2,0x2163,0x200c,0x11db0,0x11de0,0xd800,0xdc00))WebContractProbe.compare(nativeCode,source,"\"operation\":\"idents\",\"text_points\":["+boundary+",70,111,111,66,97,114,32,102,111,111,95,98,97,114,"+boundary+"]");
      for(var phase:List.of("extract","member.name","members"))WebContractProbe.compare(nativeCode,source,"\"operation\":\"fetch\",\"suppress\":true,\"fail\":"+q(phase)+",\"error\":\"DetailError\"");
      for(var operation:List.of("fetch","setup")) {
        var run=Files.createDirectories(WebContractProbe.artifacts.resolve(operation));
        WebContractProbe.compare(nativeCode,source,"\"operation\":"+q(operation)+",\"real_files\":true,\"real_process\":true,\"run_dir\":"+q(run)+",\"java_bin\":"+q(Path.of(System.getProperty("java.home"),"bin/java")));
        if(operation.equals("fetch")) {
          if(!Files.isRegularFile(run.resolve("_repomap_src/keep.py"))||Files.exists(run.resolve("_repomap_src/link.py"))||Files.exists(run.resolve("_repomap_src/..hidden.py")))throw new AssertionError("archive filtering changed");
        }else if(Files.exists(run.resolve("_repomap_src"))||!Files.readString(run.resolve("workspace/CLAUDE.md")).contains("nested/a b.py"))throw new AssertionError("map output or cleanup changed");
      }
      int failure=0;
      for(var options:List.of("\"fail\":\"read\",\"error\":\"ScaffoldError\"","\"empty_blob\":true","\"bad_archive\":true","\"fail\":\"run\",\"error\":\"TimeoutExpired\"","\"fail\":\"run\",\"error\":\"RuntimeError\"","\"fail\":\"run\",\"error\":\"RuntimeError\",\"failures\":[{\"fail\":\"rmtree\",\"error\":\"DetailError\"}]","\"fail\":\"run\",\"error\":\"RuntimeError\",\"failures\":[{\"fail\":\"subprocess.TimeoutExpired\",\"error\":\"DetailError\"}]")) {
        var run=Files.createDirectories(WebContractProbe.artifacts.resolve("failure-"+failure++));
        WebContractProbe.compare(nativeCode,source,"\"operation\":\"setup\",\"real_files\":true,\"real_process\":true,\"run_dir\":"+q(run)+",\"java_bin\":"+q(Path.of(System.getProperty("java.home"),"bin/java"))+","+options);
        var retained=options.contains("read")||options.contains("empty_blob")||options.contains("bad_archive")||options.contains("rmtree");
        if(Files.exists(run.resolve("_repomap_src"))!=retained)throw new AssertionError("failure cleanup changed: "+options);
      }
      WebContractProbe.compare(nativeCode,source,"\"operation\":\"idents\",\"text\":"+q(java.util.stream.IntStream.range(0,100).mapToObj(i->"`prefix.field"+i+"`").collect(java.util.stream.Collectors.joining(" "))));
      passed=true;
    }finally{if(passed)try(var paths=Files.walk(WebContractProbe.artifacts)){for(var path:paths.sorted(Comparator.reverseOrder()).toList())Files.delete(path);}}
    System.out.println("Repo-map scaffold matches the source in "+WebContractProbe.cases+" comparisons; identifiers, archive filtering, generator arguments, cleanup and error scopes passed with real archive/process/file checks");
  }
}
