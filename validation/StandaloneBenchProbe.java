//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java NativeCliProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;

/** Runs every pinned arm through the standalone SDK and checks the native executable. */
class StandaloneBenchProbe {
  static int checks;
  static String q(Object value){return ParserProbe.q(value);}
  public static void main(String[] args)throws Exception {
    var root=Files.createTempDirectory(Path.of("validation/results"),"standalone-bench-").toAbsolutePath();boolean passed=false;
    try(var nativeCode=new AdapterProbe.Client(Path.of("").toAbsolutePath(),true,"standalone_driver_host.py");var source=new AdapterProbe.Client(Path.of("").toAbsolutePath(),false,"driver_e2e_host.py")) {
      var arms=List.of("baseline","baseline-low","baseline-medium","inject-only","untool-only","spiral-only","spiral-only-web","trimech","trimech-web","codex-untool","codex-spiral","codex-spiral-postgreen","codex-spiral-defensive","codex-combo","codex-inject","codex-inject-untool","codex-inject-envfacts","codex-inject-distiller","codex-trimech","codex-trimech-rtk","codex-trimech-repomap","rtk","serena","repomap","trimech-rtk","trimech-rtk-web","trimech-repomap");
      for(var arm:arms)for(var agent:List.of("claude","copilot","codex")) {
        var request="{\"root\":"+q(root.resolve("case"))+",\"arm\":"+q(arm)+",\"agent\":"+q(agent)+",\"count\":10}";
        var actual=nativeCode.call(request);var expected=source.call(request);
        if(!actual.equals(expected)) {
          Files.writeString(root.resolve("actual.json"),actual);Files.writeString(root.resolve("expected.json"),expected);
          int difference=0;while(difference<actual.length()&&difference<expected.length()&&actual.charAt(difference)==expected.charAt(difference))difference++;
          throw new AssertionError("Mismatch "+arm+" / "+agent+" at "+difference+"\nNative: "+actual.substring(Math.max(0,difference-100),Math.min(actual.length(),difference+400))+"\nSource: "+expected.substring(Math.max(0,difference-100),Math.min(expected.length(),difference+400)));
        }
        if(!actual.contains("\"value\":null")||!actual.contains("tasks=10 resolved=6"))throw new AssertionError("offline run did not complete: "+actual);
        checks++;
      }
      var cli=Path.of("target/debug/te-bench").toAbsolutePath();var python=Path.of(".venv/bin/python").toAbsolutePath();
      var environment=Map.of("PATH","","HOME",root.toString(),"TE_BENCH_PYTHON",python.toString(),"TOKENECONOMIZER_ENV_FILE",root.resolve("absent.env").toString());
      var help=NativeCliProbe.process(root,environment,List.of(cli.toString(),"--help"));
      if(help.code()!=0||!help.out().contains("usage: te-bench")||!help.out().contains("synthetic transcripts"))throw new AssertionError(help);checks++;
      var missing=NativeCliProbe.process(root,environment,List.of(cli.toString()));if(missing.code()!=2)throw new AssertionError(missing);checks++;
      Files.writeString(root.resolve("bench_fixture.py"),"""
          import dataclasses, json, os, signal, sys
          from pathlib import Path
          class Runner:
              def begin_stream(self,tasks,**info):
                  Path(os.environ['TE_BENCH_FIXTURE'],'begun').write_text(str(len(tasks)))
                  if os.environ.get('TE_BENCH_INTERRUPT') == 'term':
                      def interrupted(number,frame): raise SystemExit(17)
                      signal.signal(signal.SIGTERM,interrupted)
              def end_stream(self):
                  Path(os.environ['TE_BENCH_FIXTURE'],'closed').write_text(type(sys.exception()).__name__ if sys.exception() else 'None')
              def optimizer_call(self,prompt,model='agent'):
                  return '{"stuck":false,"confidence":0.9,"reason":"fixture"}',40.0
              def run_task(self,task,*,run_dir_path,**kwargs):
                  interrupt=os.environ.get('TE_BENCH_INTERRUPT')
                  if interrupt: os.kill(os.getpid(),signal.SIGTERM if interrupt == 'term' else signal.SIGINT)
                  path=Path(run_dir_path);path.mkdir(parents=True,exist_ok=True)
                  (path/'transcript.jsonl').write_text('')
                  row=dict(resolved=True,num_turns=2,tokens_total=100)
                  policy=kwargs.get('codex_spiral_policy')
                  if policy is not None:
                      assert dataclasses.is_dataclass(policy)
                      row['policy_checked']=policy.at_threshold(7).min_tool_turns == 7
                  judge=kwargs.get('spiral_judge')
                  if judge is not None:
                      result=judge('fixture digest');row['spiral_judge_tokens']=result['_tokens']
                  writer=kwargs.get('spiral_nudge_writer')
                  if writer is not None:
                      result=writer('fixture digest');row['spiral_nudge_writer_tokens']=result['_tokens']
                  return row,str(path)
          def make_runner(agent=None,model=None,scaffold=None): return Runner()
          """);
      var tasks=root.resolve("external-tasks.json");Files.writeString(tasks,"[{\"instance_id\":\"external\",\"problem_statement\":\"Offline fixture\"}]");
      var external=new java.util.HashMap<>(environment);external.put("PYTHONPATH",root.toString());external.put("TE_BENCH_FIXTURE",root.toString());
      for(var arm:List.of("baseline","trimech-web","codex-spiral")) {
        var destination=root.resolve("external-"+arm);
        var result=NativeCliProbe.process(root,external,List.of(cli.toString(),"--arm",arm,"--agent",arm.equals("codex-spiral")?"codex":"claude","--runner","bench_fixture:make_runner","--tasks-file",tasks.toString(),"--results-root",destination.toString()));
        if(result.code()!=0||!result.out().contains("tasks=1 resolved=1")||!Files.readString(root.resolve("closed")).equals("None"))throw new AssertionError("external factory or native policy callback failed: "+result);checks++;
        if(arm.equals("codex-spiral"))try(var paths=Files.walk(destination)) {
          var row=paths.filter(path->path.getFileName().toString().equals("row.json")).findFirst().orElseThrow();
          if(!Files.readString(row).contains("\"policy_checked\": true"))throw new AssertionError("Codex policy adapter failed");checks++;
        }
      }
      for(var signal:List.of("int","term")) {
        external.put("TE_BENCH_INTERRUPT",signal);
        var result=NativeCliProbe.process(root,external,List.of(cli.toString(),"--arm","baseline","--runner","bench_fixture:make_runner","--tasks-file",tasks.toString(),"--results-root",root.resolve("interrupted-"+signal).toString()));
        if(result.code()!=(signal.equals("int")?130:17)||!Files.readString(root.resolve("closed")).equals(signal.equals("int")?"KeyboardInterrupt":"SystemExit"))throw new AssertionError("interrupted runner was not closed: "+result);checks++;
      }
      passed=true;
    }finally{if(passed)ContainerProbe.clear(root);else System.err.println("Probe artifacts: "+root);}
    System.out.println("Standalone benchmark passed "+checks+" checks; all 27 arms on all three agents match the pinned source with reference imports forbidden, and the native executable preserves help/error status");
  }
}
