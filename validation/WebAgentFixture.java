import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

/** Owned agent process: emits usage, edits one file and delays shutdown on stop cases. */
class WebAgentFixture {
  public static void main(String[] args) throws Exception {
    var agent=System.getenv("TE_FIXTURE_AGENT");
    var mode=System.getenv().getOrDefault("TE_FIXTURE_MODE","normal");
    var arguments=List.of(args);
    if(agent.equals("copilot")){
      if(!arguments.subList(0,11).equals(List.of("-p",args[1],"--output-format","json","--allow-all-tools","--no-color","--bash-env=on","--log-level","all","--log-dir",args[10])))throw new AssertionError("Copilot invocation contract changed: "+arguments);
      if(arguments.stream().filter("--excluded-tools"::equals).count()!=2||!arguments.containsAll(List.of("web_fetch","web_search","--disable-builtin-mcps")))throw new AssertionError("Copilot isolation flags missing");
    }else if(agent.equals("codex")){
      if(!args[0].equals("exec")||!arguments.containsAll(List.of("--json","--skip-git-repo-check","--dangerously-bypass-approvals-and-sandbox"))||!arguments.getLast().equals("-"))throw new AssertionError("Codex invocation contract changed: "+arguments);
      if(mode.equals("resume")){
        if(!args[1].equals("resume")||!args[2].equals("fixture-session")||arguments.contains("-C")||arguments.contains("--color")||!arguments.contains("model_reasoning_effort=\"low\""))throw new AssertionError("Codex resume contract changed: "+arguments);
      }else if(!arguments.containsAll(List.of("--color","never","-C")))throw new AssertionError("Codex fresh-segment flags missing");
    }
    if(!agent.equals("copilot")) {
      var prompt=new String(System.in.readAllBytes(),StandardCharsets.UTF_8);
      if(!prompt.contains("fixture prompt")&&!prompt.contains("<task>"))throw new AssertionError("stdin prompt missing");
    } else if(!List.of(args).contains("--log-dir"))throw new AssertionError("Copilot log directory missing");
    if(mode.equals("config_error")){
      System.err.println(agent.equals("copilot")?"fixture model is not available":"unknown configuration fixture");
      System.exit(2);
    }
    var usage="{\"input_tokens\":10,\"cache_read_input_tokens\":20,\"cache_creation_input_tokens\":30,\"output_tokens\":4}";
    if(agent.equals("copilot")) {
      int at=List.of(args).indexOf("--log-dir");
      Files.writeString(Path.of(args[at+1],"fixture.log"),"{\"usage\": {\"prompt_tokens\":60,\"completion_tokens\":4,\"total_tokens\":64,\"prompt_tokens_details\":{\"cached_tokens\":20,\"cache_creation_tokens\":30}}}\n");
    }
    var answer=Path.of("src/a.js");
    if(Files.exists(answer))Files.writeString(answer,"fixture answer");
    boolean stopping=mode.equals("guard")||mode.equals("timeout");
    if(stopping)Runtime.getRuntime().addShutdownHook(new Thread(()->{try{Thread.sleep(60_000);}catch(InterruptedException error){Thread.currentThread().interrupt();}}));
    System.out.println("   ");
    if(agent.equals("claude")) {
      var log=Path.of(System.getenv("HOME"),".claude/projects/fixture/session.jsonl");
      Files.createDirectories(log.getParent());
      Files.writeString(log,"{\"type\":\"assistant\",\"message\":{\"id\":\"fixture-message\",\"usage\":"+usage+"}}\n");
      System.out.println("{\"type\":\"assistant\",\"message\":{\"id\":\"fixture-message\",\"usage\":"+usage+"}}");
    } else if(agent.equals("copilot"))System.out.println("{\"type\":\"assistant.turn_start\"}");
    else System.out.println("{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\"}}");
    System.out.flush();
    if(stopping)Thread.sleep(60_000);
    if(agent.equals("claude"))System.out.println("{\"type\":\"result\",\"num_turns\":1,\"usage\":"+usage+"}");
    else if(agent.equals("copilot"))System.out.println("{\"type\":\"assistant.message\",\"data\":{\"outputTokens\":4}}");
    else System.out.println("{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":60,\"cached_input_tokens\":20,\"cache_write_input_tokens\":30,\"output_tokens\":4}}");
    System.err.println("fixture stderr é😀");
  }
}
