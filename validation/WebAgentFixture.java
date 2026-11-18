import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

/** Owned agent process: emits usage, edits one file and delays shutdown on stop cases. */
class WebAgentFixture {
  public static void main(String[] args) throws Exception {
    var agent=System.getenv("TE_FIXTURE_AGENT");
    var mode=System.getenv().getOrDefault("TE_FIXTURE_MODE","normal");
    if(!agent.equals("copilot")) {
      var prompt=new String(System.in.readAllBytes(),StandardCharsets.UTF_8);
      if(!prompt.contains("fixture prompt")&&!prompt.contains("<task>"))throw new AssertionError("stdin prompt missing");
    } else if(!List.of(args).contains("--log-dir"))throw new AssertionError("Copilot log directory missing");
    var usage="{\"input_tokens\":10,\"cache_read_input_tokens\":20,\"cache_creation_input_tokens\":30,\"output_tokens\":4}";
    if(agent.equals("copilot")) {
      int at=List.of(args).indexOf("--log-dir");
      Files.writeString(Path.of(args[at+1],"fixture.log"),"{\"usage\": {\"prompt_tokens\":60,\"completion_tokens\":4,\"total_tokens\":64,\"prompt_tokens_details\":{\"cached_tokens\":20,\"cache_creation_tokens\":30}}}\n");
    }
    var answer=Path.of("src/a.js");
    if(Files.exists(answer))Files.writeString(answer,"fixture answer");
    if(!mode.equals("normal"))Runtime.getRuntime().addShutdownHook(new Thread(()->{try{Thread.sleep(60_000);}catch(InterruptedException error){Thread.currentThread().interrupt();}}));
    System.out.println("   ");
    if(agent.equals("claude")) {
      var log=Path.of(System.getenv("HOME"),".claude/projects/fixture/session.jsonl");
      Files.createDirectories(log.getParent());
      Files.writeString(log,"{\"type\":\"assistant\",\"message\":{\"id\":\"fixture-message\",\"usage\":"+usage+"}}\n");
      System.out.println("{\"type\":\"assistant\",\"message\":{\"id\":\"fixture-message\",\"usage\":"+usage+"}}");
    } else if(agent.equals("copilot"))System.out.println("{\"type\":\"assistant.turn_start\"}");
    else System.out.println("{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\"}}");
    System.out.flush();
    if(!mode.equals("normal"))Thread.sleep(60_000);
    if(agent.equals("claude"))System.out.println("{\"type\":\"result\",\"num_turns\":1,\"usage\":"+usage+"}");
    else if(agent.equals("copilot"))System.out.println("{\"type\":\"assistant.message\",\"data\":{\"outputTokens\":4}}");
    else System.out.println("{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":60,\"cached_input_tokens\":20,\"cache_write_input_tokens\":30,\"output_tokens\":4}}");
    System.err.println("fixture stderr é😀");
  }
}
