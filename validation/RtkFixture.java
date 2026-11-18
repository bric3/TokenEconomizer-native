import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;

/** Offline RTK process fixture for owned scaffold pipeline directories. */
class RtkFixture {
  public static void main(String[] args)throws Exception {
    if(Arrays.equals(args,new String[]{"--version"})) { System.out.println("rtk 0.45.0");return; }
    if(args.length>=2&&args[0].equals("init")&&args[1].equals("-g")) {
      var home=Path.of(System.getenv(Arrays.asList(args).contains("--codex")?"CODEX_HOME":"HOME"));
      if(!home.toAbsolutePath().startsWith(Path.of(System.getProperty("java.class.path"))))throw new AssertionError("RTK fixture HOME escaped its directory");
      Files.createDirectories(home);
      Files.writeString(home.resolve("RTK.md"),"Always prefix shell commands with rtk\n");
      Files.writeString(home.resolve("AGENTS.md"),"Fixture agent instructions\n@"+home+"/RTK.md\n");
      return;
    }
    if(args.length==2&&args[0].equals("hook")&&Arrays.asList("claude","copilot").contains(args[1])) {
      var payload=new String(System.in.readAllBytes(),StandardCharsets.UTF_8);
      if(!payload.contains("tool_name")||!payload.contains("ls -la"))throw new AssertionError("unexpected hook payload: "+payload);
      System.out.println("{\"hookSpecificOutput\":{\"updatedInput\":{\"command\":\"rtk ls -la\"}}}");
      return;
    }
    throw new IllegalArgumentException("unsupported RTK fixture command: "+Arrays.toString(args));
  }
}
