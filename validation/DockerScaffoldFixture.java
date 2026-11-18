import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.LinkedHashMap;
import java.util.Map;

/** Implements the Docker exec argument shape for owned process I/O checks. */
class DockerScaffoldFixture {
  static Path target(String path)throws Exception {
    var owned=Path.of(System.getenv("TE_DOCKER_FIXTURE_DIRECTORY"));
    if(!owned.startsWith(Path.of(System.getProperty("java.class.path"))))throw new AssertionError("fixture directory escaped");
    var result=owned.resolve("container").resolve(path.replaceFirst("^/","")).normalize();
    if(!result.startsWith(owned.resolve("container")))throw new AssertionError("copy escaped the fixture");
    return result;
  }
  public static void main(String[] args)throws Exception {
    if(args.length==3&&args[0].equals("cp")) {
      if(!args[2].startsWith("fixture-container:"))throw new AssertionError("copy container changed");
      var destination=target(args[2].substring("fixture-container:".length()));
      Files.createDirectories(destination.getParent());Files.copy(Path.of(args[1]),destination,StandardCopyOption.REPLACE_EXISTING);return;
    }
    if(args.length<5||!args[0].equals("exec"))throw new AssertionError("Docker exec arguments changed");
    int at=1;Map<String,String> environment=new LinkedHashMap<>();
    if(args[at].equals("-w")){if(!args[++at].equals("/testbed"))throw new AssertionError("working directory changed");at++;}
    while(args[at].equals("-e")){var pair=args[++at].split("=",2);environment.put(pair[0],pair[1]);at++;}
    if(!args[at++].equals("fixture-container")||!args[at++].equals("bash")||!args[at++].equals("-lc"))throw new AssertionError("container or shell changed");
    var script=args[at++];if(at!=args.length)throw new AssertionError("extra Docker arguments");
    if(script.startsWith("cd /testbed && git ls-files")){System.out.write(Files.readAllBytes(Path.of(System.getenv("TE_DOCKER_FIXTURE_DIRECTORY"),"source.tar")));return;}
    if(script.startsWith("mkdir -p ")){Files.createDirectories(target(script.substring("mkdir -p ".length())));return;}
    switch(script){
      case "fixture-binary"->System.out.write(new byte[]{0,(byte)255,(byte)128,10});
      case "fixture-error"->{System.err.write(("x".repeat(210)+"é😀").getBytes(StandardCharsets.UTF_8));System.exit(7);}
      case "fixture-env"->{if(!environment.equals(Map.of("FOO","fixture value","BAR","42")))throw new AssertionError("environment arguments changed");System.out.println("environment passed");}
      default->throw new AssertionError("unsupported fixture script: "+script);
    }
  }
}
