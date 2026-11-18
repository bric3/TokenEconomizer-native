//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.concurrent.TimeUnit;
import java.util.regex.Pattern;

/** Compares the complete registry and verifies fresh configuration and relocation. */
class ArmRegistryProbe {
  public static void main(String[] args)throws Exception {
    int checks=0;
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"arms_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"arms_host.py")) {
      var expected=source.call("{}");var actual=nativeCode.call("{}");
      if(!actual.equals(expected))throw new AssertionError("Native registry differs\nNative: "+actual+"\nSource: "+expected);checks++;
      var names=Pattern.compile("\"([^\"]+)\":\\{\"config\":").matcher(expected);int count=0;
      while(names.find()) {
        var request="{\"operation\":\"mutate\",\"name\":"+ParserProbe.q(names.group(1))+"}";
        if(!nativeCode.call(request).equals(source.call(request)))throw new AssertionError("configuration mutation leaked from "+names.group(1));
        checks++;count++;
      }
      if(count!=27)throw new AssertionError("registry omitted arms: "+count);checks++;
    }
    var root=Files.createTempDirectory("te-arm-registry-");boolean passed=false;
    try {
      var binary=Files.copy(Path.of("target/debug/te-bridge"),root.resolve("engine"),StandardCopyOption.COPY_ATTRIBUTES);
      var builder=new ProcessBuilder(binary.toString()).directory(root.toFile());builder.environment().clear();
      var process=builder.start();
      process.getOutputStream().write("{\"op\":\"experiment.arms.config\",\"args\":{\"name\":\"codex-trimech\"}}\n{\"op\":\"experiment.arms.config\",\"args\":{\"name\":\"unknown\"}}\n".getBytes(java.nio.charset.StandardCharsets.UTF_8));process.getOutputStream().close();
      if(!process.waitFor(10,TimeUnit.SECONDS)){process.destroyForcibly().waitFor();throw new AssertionError("relocated registry timed out");}
      var output=new String(process.getInputStream().readAllBytes(),java.nio.charset.StandardCharsets.UTF_8);
      if(process.exitValue()!=0||!output.contains("\"profile\":\"envfacts+distiller\"")||!output.contains("\"codex_spiral_profile\":\"defensive\"")||!output.endsWith("{\"result\":null}\n"))throw new AssertionError("relocated registry failed: "+output);checks++;
      passed=true;
    }finally{if(passed)ContainerProbe.clear(root);else System.err.println("Probe artifacts: "+root);}
    System.out.println("Arm registry passed "+checks+" checks; all 27 configurations, canonical order, scaffold and effort metadata, configuration independence and a relocated engine passed against the pinned source");
  }
}
