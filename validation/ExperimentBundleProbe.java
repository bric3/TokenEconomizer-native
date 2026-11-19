//JAVA 25
//SOURCES NativeCliProbe.java CliProbe.java ContextProbe.java ParserProbe.java ContainerProbe.java AdapterProbe.java ExtractProposerPrompts.java
//SOURCES DatasetExportProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.security.MessageDigest;
import java.util.HexFormat;
import java.util.List;
import java.util.Map;
import java.util.regex.Pattern;
import java.util.zip.ZipFile;

/** Exercises fresh experiment payloads, relocation, archive bytes and input/output guards. */
class ExperimentBundleProbe {
  static int checks;
  static String q(Object value){return ParserProbe.q(value);}
  static void require(boolean condition,String message){if(!condition)throw new AssertionError(message);checks++;}
  static NativeCliProbe.Output run(Path root,Path exe,String... args)throws Exception {
    var command=new java.util.ArrayList<>(List.of(exe.toString()));command.addAll(List.of(args));
    return NativeCliProbe.process(root,Map.of("HOME",root.toString(),"PATH","","TOKENECONOMIZER_HOME",root.resolve("state").toString(),"TE_MICROSKILLS","envfacts","TE_LOOPGUARD","off","TE_TOOLPRUNE","off"),command,120);
  }
  static void copy(Path source,Path target)throws Exception {
    Files.createDirectories(target.getParent());Files.copy(source,target,StandardCopyOption.COPY_ATTRIBUTES);
  }
  static void hashes(Path bundle)throws Exception {
    var manifest=Files.readString(bundle.resolve("manifest.json"));
    require(manifest.contains("\"benchmark_entrypoint\": \"bin/te-bench\""),"manifest omitted benchmark entry point");
    var matches=Pattern.compile("\"([^\"]+)\": \"([0-9a-f]{64})\"").matcher(manifest);int count=0;
    while(matches.find()) {
      if(matches.group(1).equals("cargo_lock_sha256"))continue;
      var hash=HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(Files.readAllBytes(bundle.resolve(matches.group(1)))));
      require(hash.equals(matches.group(2)),"manifest hash differs: "+matches.group(1));count++;
    }
    require(count==39,"manifest omitted files: "+count);
  }
  static void engine(Path bundle)throws Exception {
    try(var client=new ContextProbe.Client(bundle.resolve("bin/te-bridge"),"experiment.swebench")) {
      var digit=new String(Character.toChars(0x11de0));var upper=new String(Character.toChars(0x16ea0));var lower=new String(Character.toChars(0x16ebb));
      for(var entry:Map.of("(?ai)\\bpytest\\b",List.of("pytest", "PYTEST"),"(?ai)(.+)\\1",List.of("aéAé", "😀😀"),"\\N{SNOWMAN}",List.of("☃"),"(?ai)(\\N{LATIN SMALL LETTER E WITH ACUTE})\\1",List.of("éé"),"\\A\\w+\\Z",List.of(digit,upper),"\\A\\d+\\Z",List.of(digit),"(?i)\\U00016EA0",List.of(lower),"(?P<"+upper+">a)",List.of("a")).entrySet())for(var text:entry.getValue()) {
        var match=client.call("distiller.regex_search","\"pattern\":"+q(entry.getKey())+",\"text\":"+q(text));
        require(match.equals("{\"result\":true}"),"relocated detector matcher failed: "+match);
      }
      var rejected=client.call("distiller.regex_search","\"pattern\":"+q("(?a)\\bpytest\\b")+",\"text\":"+q("none"));
      require(rejected.equals("{\"result\":false}"),"relocated detector matcher accepted unrelated text: "+rejected);
      for(var entry:Map.of("(?i)(.)\\1",Map.of("İi",true,"Sſ",false,"Σς",false,upper+lower,true),"(?ai)(.)\\1(?ui:(.)\\2)",Map.of("aAéÉ",true,"éÉaA",false),"(?i)(.)\\1(?<=\\1\\1)",Map.of("éÉ",true,"Σς",false),"(?i)()\\1(?<=\\1)é",Map.of("é",true)).entrySet())for(var match:entry.getValue().entrySet()) {
        var result=client.call("distiller.regex_search","\"pattern\":"+q(entry.getKey())+",\"text\":"+q(match.getKey()));
        require(result.equals("{\"result\":"+match.getValue()+"}"),"relocated backreference matcher failed: "+result);
      }
      var decision=client.call("experiment.net_guard.decide","\"command\":\"echo https://github.com/probe\"");
      require(decision.contains("[true,"),"relocated guard policy failed: "+decision);
      var result=client.call("experiment.swebench.guard.install","\"name\":\"owned-fixture\"");boolean copied=false;
      while(result.contains("\"callback\"")) {
        var method=Pattern.compile("\"method\":\"([^\"]+)\"").matcher(result);require(method.find(),"callback method missing: "+result);
        var name=method.group(1);
        String value=switch(name) {
          case "config_function"->"[\"WebSearch\"]";
          case "config_attr"->result.contains("NET_GUARD_PROBE")?q("echo https://github.com/probe"):result.contains("\"kind\"")?q("claude"):q("/tmp/task-home");
          case "_exec"->{var out=result.contains("uname -m")?"aarch64\n":result.contains("mkdir -p")?"ready\n":result.contains("--install-hook")?"armed\n":"rc=2\n";yield "{\"returncode\":0,\"stdout\":"+q(out)+",\"stderr\":\"\"}";}
          case "container_run"->{require(result.contains(bundle.resolve("bin/container-helpers/arm64/te-net-guard").toString()),"helper discovery used another checkout: "+result);copied=true;yield "{\"returncode\":0,\"stdout\":\"\",\"stderr\":\"\"}";}
          default->throw new AssertionError("unexpected callback: "+result);
        };
        result=client.resume(result,name,"{\"value\":"+value+"}");
      }
      require(copied&&result.equals("{\"result\":true}"),"relocated helper discovery failed: "+result);
    }
  }
  public static void main(String[] args)throws Exception {
    var checkout=Path.of("").toAbsolutePath();var root=Files.createTempDirectory("te-experiment-bundle-").toAbsolutePath();boolean passed=false;
    try {
      var binaries=Files.createDirectories(root.resolve("inputs/bin"));
      for(var name:List.of("te","te-hook","te-bridge","te-bench","te-net-guard","te-repomap-gen","te-package"))copy(checkout.resolve("target/debug/"+name),binaries.resolve(name));
      var helpers=Files.createDirectory(root.resolve("inputs/helpers"));
      for(var architecture:List.of("arm64","amd64"))for(var name:List.of("te-net-guard","manifest.json","toolchain.txt","THIRD-PARTY-NOTICES.txt"))copy(checkout.resolve("target/container-helpers").resolve(architecture).resolve(name),helpers.resolve(architecture).resolve(name));
      var tool=binaries.resolve("te-package");var bundle=root.resolve("runtime payload.v1");
      var built=run(root,tool,"--experiments","--container-helpers",helpers.toString(),"--out",bundle.toString(),"--zip");require(built.code()==0,built.toString());hashes(bundle);
      var defaults=run(root,tool,"--experiments","--container-helpers",helpers.toString());
      require(defaults.code()==0&&Files.isRegularFile(root.resolve("dist/experiments/manifest.json")),"default output depended on the build checkout: "+defaults);
      require(Files.mismatch(bundle.resolve("assets/codex_base_prompt/0.153.1.md"),checkout.resolve("reference/upstream/experiments/te_harness/data/codex_base_prompt/0.153.1.md"))==-1,"captured Codex prompt changed");
      require(Files.mismatch(bundle.resolve("assets/webbench_undici_preload.mjs"),checkout.resolve("reference/upstream/experiments/te_harness/webbench_undici_preload.mjs"))==-1,"WebBench preload changed");
      require(Files.mismatch(bundle.resolve("LICENSE"),checkout.resolve("LICENSE"))==-1,"licence changed");
      try(var zip=new ZipFile(bundle.resolveSibling("runtime payload.v1.zip").toFile());var paths=Files.walk(bundle)) {
        for(var path:paths.filter(Files::isRegularFile).toList()) {
          var entry=zip.getEntry("runtime payload.v1/"+bundle.relativize(path).toString().replace('\\','/'));require(entry!=null,"archive omitted "+path);
          try(var input=zip.getInputStream(entry)){require(java.util.Arrays.equals(input.readAllBytes(),Files.readAllBytes(path)),"archive changed "+path);}
        }
      }
      require(run(root,tool,"--experiments","--container-helpers",helpers.toString(),"--out",bundle.toString()).code()!=0,"existing output was overwritten");hashes(bundle);
      var moved=root.resolve("relocated runtime's directory");Files.move(bundle,moved);engine(moved);
      var tasks=root.resolve("tasks.json");Files.writeString(tasks,"[{\"instance_id\":\"pkg__0\",\"problem_statement\":\"Offline fixture\"}]");
      var bench=NativeCliProbe.process(root,Map.of("PATH","","HOME",root.toString(),"TE_BENCH_PYTHON",checkout.resolve(".venv/bin/python").toString(),"TOKENECONOMIZER_ENV_FILE",root.resolve("absent.env").toString()),List.of(moved.resolve("bin/te-bench").toString(),"--arm","baseline","--tasks-file",tasks.toString(),"--results-root",root.resolve("bench-results").toString()));
      require(bench.code()==0&&bench.out().contains("tasks=1 resolved=0"),"relocated standalone benchmark failed: "+bench);
      var aggregate=NativeCliProbe.process(root,Map.of("PATH","","TE_BENCH_PYTHON","/missing-python"),List.of(moved.resolve("bin/te-bench").toString(),"aggregate",root.resolve("bench-results").toString()));
      require(aggregate.code()==0&&aggregate.out().contains("all_rows.csv: 1 task-runs"),"relocated aggregation required Python or source files: "+aggregate);
      require(Files.readString(root.resolve("bench-results/index/summary_by_cell.csv")).contains("baseline,"),"relocated aggregation omitted its cell");
      var exported=run(root,moved.resolve("bin/te-bench"),"export-workloads");
      require(exported.code()==0&&exported.out().contains("277 tasks, 6 groups"),"relocated workload export failed: "+exported);
      var checked=NativeCliProbe.process(root,Map.of("PATH","","TE_BENCH_PYTHON","/missing-python"),List.of(moved.resolve("bin/te-bench").toString(),"export-workloads","--check"));
      require(checked.code()==0&&checked.out().contains("up to date (4 domains)"),"relocated workload check required Python: "+checked);
      for(var domain:List.of("swebench","webbench","creddata","reposvul"))require(Files.mismatch(root.resolve("workloads/"+domain+".json"),checkout.resolve("reference/upstream/experiments/workloads/"+domain+".json"))==-1,"relocated workload bytes changed: "+domain);
      var planned=run(root,moved.resolve("bin/te-bench"),"sweep-plan");
      require(planned.code()==0&&planned.out().contains("task-runs=  9,972")&&!planned.out().contains("serial="),"relocated sweep estimate failed: "+planned);
      var counted=run(root,moved.resolve("bin/te-bench"),"sweep-plan","--format","counts");
      require(counted.code()==0&&counted.out().contains("sympy/sympy\t60\n"),"relocated sweep counts drifted: "+counted);
      var selected=run(root,moved.resolve("bin/te-bench"),"sweep-plan","--domain","webbench","--format","cells","--arms","serena","baseline","repomap","--repos","calculator","--seeds","0");
      require(selected.code()==0&&selected.out().equals("baseline\tcalculator\t0\n")&&selected.err().contains("serena repomap"),"relocated sweep lost scaffold filtering: "+selected);
      var manifest=root.resolve("workloads/swebench.json");var originalManifest=Files.readString(manifest);
      Files.writeString(manifest,"{\"groups\":{\"custom\":[1,2]}}");
      var custom=run(root,moved.resolve("bin/te-bench"),"sweep-plan","--format","counts");
      require(custom.code()==0&&custom.out().equals("custom\t2\n"),"relocated sweep ignored its local manifest: "+custom);
      Files.writeString(manifest,originalManifest);
      var datasetRoot=Files.createDirectory(root.resolve("dataset-export"));
      try(var source=new AdapterProbe.Client(checkout,false,"dataset_export_host.py")) {
        DatasetExportProbe.fixtures(source,datasetRoot);
        var reference=source.call(DatasetExportProbe.request("main",datasetRoot.resolve("reference"),""));
        require(!reference.contains("\"raised\""),"reference dataset fixture failed: "+reference);
      }
      var dataset=DatasetExportProbe.command(checkout,datasetRoot,moved.resolve("bin/te-bench"),"--out-dir","native");
      require(dataset.code()==0&&dataset.out().contains("wrote 277 tasks"),"relocated dataset export failed: "+dataset);
      for(var name:List.of("swebench_tasks.json","swebench_grading.json","image_tags.txt"))require(Files.mismatch(datasetRoot.resolve("native").resolve(name),datasetRoot.resolve("reference/out").resolve(name))==-1,"relocated dataset bytes changed: "+name);
      var builtin=NativeCliProbe.process(root,Map.of("PATH","","HOME",root.toString(),"TE_BENCH_PYTHON",checkout.resolve(".venv/bin/python").toString(),"TOKENECONOMIZER_ENV_FILE",root.resolve("absent.env").toString(),"ANTHROPIC_API_KEY",""),List.of(moved.resolve("bin/te-bench").toString(),"--arm","baseline","--runner","te_runtime.swebench:make_runner","--agent","claude","--tasks-file",tasks.toString(),"--results-root",root.resolve("builtin-results").toString()));
      require(builtin.code()!=0&&builtin.err().contains("ANTHROPIC_API_KEY is unset"),"built-in runner did not reach credential validation: "+builtin);
      var webBuiltin=NativeCliProbe.process(root,Map.of("PATH","","HOME",root.toString(),"TE_BENCH_PYTHON",checkout.resolve(".venv/bin/python").toString(),"TOKENECONOMIZER_ENV_FILE",root.resolve("absent.env").toString(),"ANTHROPIC_API_KEY",""),List.of(moved.resolve("bin/te-bench").toString(),"--arm","baseline","--runner","te_runtime.webbench:make_runner","--agent","claude","--tasks-file",tasks.toString(),"--results-root",root.resolve("web-builtin-results").toString()));
      require(webBuiltin.code()!=0&&webBuiltin.err().contains("ANTHROPIC_API_KEY is unset"),"built-in WebBench did not reach credential validation: "+webBuiltin);
      var controller=NativeCliProbe.process(root,Map.of("PATH","","HOME",root.toString()),List.of(checkout.resolve(".venv/bin/python").toString(),"-B","-c","""
          import builtins, sys
          original_import=builtins.__import__
          def guarded(name,*args,**kwargs):
              if name.split('.')[0] in ('tokeneconomizer','te_bench','te_harness','native_plugin'):
                  raise AssertionError('packaged controller imported reference/test code: '+name)
              return original_import(name,*args,**kwargs)
          builtins.__import__=guarded
          sys.path.insert(0,sys.argv[1])
          from te_runtime.transport import Bridge
          from te_runtime.nudge_guard import bind
          from te_runtime.codex_install import bind as install_bindings
          from te_runtime.usage import bind as usage_bindings
          from te_runtime.providers import routing
          from te_runtime.optimizer import bind as optimizer_bindings
          from te_runtime.scaffolds import bind as scaffold_bindings
          from te_runtime.runner_setup import bind as setup_bindings
          from te_runtime.swebench_task import bind_network
          from te_runtime.swebench_guard import bind as guard_bindings
          from te_runtime.swebench_runner import bind as runner_bindings
          from te_runtime.swebench import bind as swebench_bindings
          from te_runtime.driver import Driver
          from types import SimpleNamespace
          import json, shlex, shutil, subprocess
          from pathlib import Path
          with Bridge(sys.argv[2]) as bridge:
              sdk=bind(bridge.request)
              calls=[]
              def writer(digest):
                  calls.append(digest)
                  return dict(text='fixture redirect',_tokens=4)
              guard=sdk.NudgeGuard(nudge_writer=writer,resume_header=':')
              assert guard.monitor('invalid') is False
              guard._stop('nudge','offline fixture')
              assert guard.continuation('task') == 'task:fixture redirect'
              assert guard.segment == 1 and guard.nudge_writer_tokens == 4 and len(calls) == 1
              codex=install_bindings(bridge)
              capture=Path(sys.argv[1]).parent/'assets/codex_base_prompt/0.153.1.md'
              assert codex.base_prompt_for('codex-cli 0.153.1') == capture.read_text(encoding='utf-8')
              assert codex.base_prompt_for('unknown') is None
              usage_logs=[]
              accounting=usage_bindings(bridge.request,usage_logs.append)
              row=accounting.codex.row_usage('{"type":"turn.completed","usage":{"input_tokens":10,"cached_input_tokens":3,"output_tokens":2}}')
              assert row['input_tokens'] == 7 and row['tokens_total'] == 12
              assert accounting.copilot.stream_output_tokens('{"type":"assistant.turn_start","data":{"outputTokens":4}}') == 4
              recovery=accounting.claude._UsageRecovery()
              recovery.observe(dict(type='assistant',message=dict(id='m1',usage=dict(input_tokens=5,output_tokens=1))))
              recovered=recovery.synthesize('owned fixture')
              assert recovered['usage_incomplete'] and recovered['usage']['output_tokens'] == 0 and len(usage_logs) == 1
              providers=routing(bridge,lambda message:None)
              endpoint=providers.codex.resolve_codex_routing(dict(OPENAI_API_KEY='fixture',OPENAI_BASE_URL=' https://bundle.invalid/v1/// '))
              assert endpoint == dict(base_url='https://bundle.invalid/v1',api_key='fixture')
              environment=dict(ANTHROPIC_AUTH_TOKEN='ignored fixture',keep='fixture')
              assert providers.claude.agent_credential_env(environment,api_key='fixture') is environment
              assert environment == dict(keep='fixture',ANTHROPIC_API_KEY='fixture')
              optimizer=optimizer_bindings(bridge)
              log=Path(sys.argv[1]).parent.parent/'optimizer.jsonl'
              optimizer.claude.os=SimpleNamespace(environ=dict(TE_OPTIMIZER_LOG=str(log)),path=__import__('os').path,makedirs=__import__('os').makedirs)
              optimizer.claude.set_optimizer_context(agent='codex',cell='fixture/repo_s0')
              optimizer.claude._log_optimizer_call('gpt-5.6-sol','Reply with the single word: ok','ok',dict(input_tokens=10,output_tokens=2))
              row=__import__('json').loads(log.read_text(encoding='utf-8'))
              assert row['kind'] == 'preflight' and row['agent'] == 'codex' and row['cell'] == 'fixture/repo_s0'
              assert row['response'] == 'ok' and row['usage'] == dict(input_tokens=10,output_tokens=2)
              assert optimizer.codex._responses_text(dict(output_text='bundle answer')) == 'bundle answer'
              assert optimizer.copilot.build_argv('fixture',log_dir='/fixture/logs')[:3] == ['copilot','-p','fixture']
              assert optimizer.codex.run_env()['CODEX_HOME'] == '/opt/codex-home'
              log.unlink()
              scaffolds=scaffold_bindings(bridge)
              assert scaffolds.registry.get(None) is None
              for arm,name in [('trimech-rtk','rtk'),('codex-trimech-repomap','repomap'),('serena','serena')]:
                  assert scaffolds.registry.for_arm(arm).name == name
              work=Path(sys.argv[1]).parent.parent/'scaffold-fixture'
              home=work/'home'
              home.mkdir(parents=True)
              profile=scaffolds.base.AgentProfile('codex',str(home),'AGENTS.md',(),lambda cfg:['--mcp-config',cfg])
              assert profile.mcp_args('fixture.json') == ['--mcp-config','fixture.json']
              def copy_in(source,destination):
                  target=work/'AGENTS.md' if destination == '/testbed/AGENTS.md' else Path(destination)
                  assert target.is_relative_to(work)
                  shutil.copyfile(source,target)
                  return subprocess.CompletedProcess([],0)
              def execute(command,timeout,env=None):
                  assert command.startswith('mkdir -p ')
                  return subprocess.CompletedProcess([],0)
              context=scaffolds.base.ScaffoldContext(profile=profile,container=None,task={},run_dir=str(work),
                  exec_=execute,copy_in=copy_in,host_run=None)
              assert context.write_instructions('bundle scaffold instructions')
              assert (work/'AGENTS.md').read_text(encoding='utf-8') == 'bundle scaffold instructions'
              cfg=context.write_mcp_config(dict(fixture=dict(command='owned-fixture')))
              assert json.loads(Path(cfg).read_text(encoding='utf-8')) == dict(mcpServers=dict(fixture=dict(command='owned-fixture')))
              assert not list(work.glob('_scaffold_*'))
              shutil.rmtree(work)
              class Interface:
                  def optimizer_call(self,prompt,model='agent'): raise AssertionError('unbound optimizer')
                  def _preflight_optimizer_endpoint(self): raise AssertionError('unbound preflight')
                  def _segment(self,name,prompt,transcript_fh,exclude_tools,max_turns,guard,mcp_config=None): raise AssertionError('unbound segment')
                  def _merge_segments(self,a,b): raise AssertionError('unbound segment merge')
                  def _session_log_text(self,name): raise AssertionError('unbound session log')
                  def _scaffold_context(self,name,task,run_dir_path): raise AssertionError('unbound scaffold context')
                  def __init__(self,agent='claude',model=None,grading_path=None,scaffold=None):
                      raise AssertionError('unbound constructor interface executed')
                  @property
                  def NET_GUARD_SCRIPT(self): raise AssertionError('unbound script')
                  @property
                  def NET_GUARD_LOG(self): raise AssertionError('unbound log')
                  @staticmethod
                  def _net_guard_merge_program(entry): raise AssertionError('unbound program')
                  def _answer_key_attempts(self,run_dir_path): raise AssertionError('unbound attempts')
                  def _write_hook_entry(self,name,python,path): raise AssertionError('unbound writer')
                  def _register_net_guard(self,name,python): raise AssertionError('unbound registration')
              class Claude(Interface): pass
              class Copilot(Interface): pass
              class Codex(Interface): pass
              host=SimpleNamespace(ClaudeSwebenchRunner=Claude,CopilotSwebenchRunner=Copilot,CodexSwebenchRunner=Codex,
                  os=SimpleNamespace(environ=dict(TE_OPTIMIZER_MODEL='refused-fixture')),
                  resolve_anthropic_key=lambda:'fixture-key')
              setup_bindings(bridge,host)
              runner=object.__new__(Claude)
              try: runner.__init__()
              except RuntimeError as error: assert str(error).startswith('TE_OPTIMIZER_MODEL is no longer honoured:')
              else: raise AssertionError('constructor accepted the optimizer override')
              assert runner.api_key == 'fixture-key' and runner.model == 'claude-sonnet-5'
              assert not hasattr(runner,'optimizer_model')
              def network_tool_block(): raise AssertionError('unbound network interface executed')
              host=SimpleNamespace(network_tool_block=network_tool_block)
              bind_network(bridge,host)
              import os
              previous=os.environ.pop('TE_ALLOW_NETWORK_TOOLS',None)
              try: assert 'WebSearch' in host.network_tool_block()
              finally:
                  if previous is not None: os.environ['TE_ALLOW_NETWORK_TOOLS']=previous
              host=SimpleNamespace(ClaudeSwebenchRunner=Claude,CopilotSwebenchRunner=Copilot,CodexSwebenchRunner=Codex,
                  json=json,answer_key_attempts=lambda path:dict(path=path))
              guard_bindings(bridge,host)
              runner=object.__new__(Claude)
              runner.scaffold_profile=SimpleNamespace(home='/owned/guard-home')
              assert runner.NET_GUARD_SCRIPT == '/owned/guard-home/net_guard.py'
              assert runner.NET_GUARD_LOG == '/owned/guard-home/net_guard_blocks.jsonl'
              assert runner._answer_key_attempts('fixture') == dict(path='fixture')
              entry=dict(matcher='Bash',hooks=[dict(type='command',command='/owned/net_guard')])
              program=Claude._net_guard_merge_program(entry)
              path=Path(sys.argv[1]).parent.parent/'compat-guard/settings.json'
              for repeat in range(2):
                  result=subprocess.run([sys.executable,'-c',program,str(path)],capture_output=True,text=True,timeout=10)
                  assert result.returncode == 0 and result.stdout == 'armed'+chr(10),result
              assert json.loads(path.read_text(encoding='utf-8')) == dict(hooks=dict(PreToolUse=[entry]))
              native_path=path.with_name('native.json')
              helper=str(Path(sys.argv[2]).with_name('te-net-guard'))
              result=subprocess.run([sys.executable,'-c',program,'--native-helper',helper,str(native_path)],capture_output=True,text=True,timeout=10)
              assert result.returncode == 0 and result.stdout == 'armed'+chr(10),result
              native_entry=json.loads(native_path.read_text(encoding='utf-8'))['hooks']['PreToolUse'][0]
              assert shlex.split(native_entry['hooks'][0]['command']) == [helper]
              shutil.rmtree(path.parent)
              def make_runner(agent=None,scaffold=None,**kwargs): raise AssertionError('unbound factory')
              def _cleanup_containers(*_a): raise AssertionError('unbound cleanup')
              def _install_signal_cleanup(): raise AssertionError('unbound signals')
              host.make_runner,host._cleanup_containers,host._install_signal_cleanup=make_runner,_cleanup_containers,_install_signal_cleanup
              host.__name__='owned.harness'
              host.os,host.scaffolds_pkg,host._LIVE_CONTAINERS=os,scaffolds.registry,set()
              host._run=lambda *args,**kwargs:None
              host.claude_bin=lambda:None
              host.cli_version=lambda binary:None
              host._import_swebench=lambda:None
              host._log=lambda message:None
              host.subprocess=SimpleNamespace(PIPE=-1,run=lambda *args,**kwargs:SimpleNamespace(returncode=0,stdout=bytes([0,255,128]),stderr=b''))
              runner_bindings(bridge,host)
              previous=Claude._segment
              assert not runner_bindings(bridge,host) and Claude._segment is previous
              runner._exec=lambda *args,**kwargs:SimpleNamespace(stdout='owned session log')
              assert runner._session_log_text('container') == 'owned session log'
              context=runner._scaffold_context('container',dict(instance_id='fixture'),'/run')
              assert context.read_bytes('fixture') == bytes([0,255,128])
              assert context._read_bytes.__doc__.startswith('Raw stdout of a container script.')
              assert host._cleanup_containers() is None
              host._install_signal_cleanup=lambda:None
              host.RUNNERS=dict(claude=lambda **kwargs:kwargs)
              assert host.make_runner(agent='CLAUDE',scaffold='rtk',model='fixture') == dict(agent='claude',scaffold='rtk',model='fixture')
              assembled=swebench_bindings(bridge)
              assert sorted(assembled.RUNNERS) == ['claude','codex','copilot']
              for name,cls in assembled.RUNNERS.items():
                  assert cls.scaffold_profile.kind == name
                  for descriptor in vars(cls).values():
                      function=descriptor.fget if isinstance(descriptor,property) else descriptor.__func__ if isinstance(descriptor,staticmethod) else descriptor
                      if assembled.inspect.isfunction(function): assert hasattr(function,'__wrapped__'),function
              driver=Driver(bridge,Path(sys.argv[1]).parent)
              loaded=driver._import_module('te_runtime.swebench')
              assert driver._import_module('te_runtime.swebench') is loaded
              loaded._install_signal_cleanup=lambda:None
              loaded.RUNNERS['claude']=lambda **kwargs:kwargs
              assert driver.load_runner('te_runtime.swebench:make_runner','claude','fixture','rtk') == dict(agent='claude',model='fixture',scaffold='rtk')
              web=driver._import_module('te_runtime.webbench')
              assert driver._import_module('te_runtime.webbench') is web
              assert web.codex is loaded.codex and web.copilot is loaded.copilot
              assert web.WebBenchRunner.NET_GUARD_HOOK_KEY == 'PreToolUse'
              assert web.CopilotWebBenchRunner.NET_GUARD_HOOK_KEY == 'preToolUse'
              for cls in (web._AgentHandler,web._EvalSession,*web.RUNNERS.values()):
                  for descriptor in vars(cls).values():
                      function=descriptor.__func__ if isinstance(descriptor,staticmethod) else descriptor
                      if web.inspect.isfunction(function): assert hasattr(function,'__wrapped__'),function
              web._install_signal_cleanup=lambda:None
              web.RUNNERS['claude']=lambda **kwargs:kwargs
              assert driver.load_runner('te_runtime.webbench:make_runner','claude','fixture','rtk') == dict(agent='claude',model='fixture',scaffold='rtk')
          print('packaged SDK bindings passed')
          """,moved.resolve("sdk").toString(),moved.resolve("bin/te-bridge").toString()));
      require(controller.code()==0&&controller.out().equals("packaged SDK bindings passed\n"),"relocated SDK binding failed: "+controller);
      try(var paths=Files.walk(moved)){require(paths.filter(Files::isRegularFile).count()==40,"benchmark command changed the runtime payload");}
      var bootstrap=moved.resolve("sdk/bench.py");var bootstrapBytes=Files.readAllBytes(bootstrap);Files.delete(bootstrap);
      var absent=NativeCliProbe.process(root,Map.of("PATH","","TE_BENCH_PYTHON",checkout.resolve(".venv/bin/python").toString()),List.of(moved.resolve("bin/te-bench").toString(),"--help"));
      require(absent.code()!=0&&absent.err().contains("SDK binding is missing"),"missing bundled SDK selected development files: "+absent);Files.write(bootstrap,bootstrapBytes);
      require(run(root,moved.resolve("bin/te"),"--help").code()==0,"relocated native CLI failed");
      var patch=root.resolve("patch.json");Files.writeString(patch,"{\"keep\":1}");var settings=root.resolve("settings.json");
      require(run(root,moved.resolve("bin/te-net-guard"),"--merge-json",settings.toString(),patch.toString()).out().equals("merged\n"),"relocated native merge failed");
      var invalid=helpers.resolve("amd64/manifest.json");var before=Files.readString(invalid);Files.writeString(invalid,before.replace("\"sha256\": \"","\"sha256\": \"bad"));
      var rejected=root.resolve("rejected");require(run(root,tool,"--experiments","--container-helpers",helpers.toString(),"--out",rejected.toString()).code()!=0&&!Files.exists(rejected),"invalid helper created an output");Files.writeString(invalid,before);
      require(run(root,tool,"--experiments","--container-helpers",helpers.toString(),"--out",helpers.resolve("overlap").toString()).code()!=0,"input/output overlap was accepted");
      for(var options:List.of(List.of("--experiments"),List.of("--experiments=true"),List.of("--experiments","--droid"),List.of("--experiments","--source","missing"),List.of("--container-helpers",helpers.toString())))require(run(root,tool,options.toArray(String[]::new)).code()!=0,"invalid arguments accepted: "+options);
      passed=true;
    }finally{if(passed)ContainerProbe.clear(root);else System.err.println("Probe artifacts: "+root);}
    System.out.println("Experiment runtime bundle passed "+checks+" checks; embedded source assets, manifest hashes, ZIP bytes, relocation, native services, default helper discovery and output guards passed with an empty PATH");
  }
}
