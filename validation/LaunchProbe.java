//JAVA 25
//JAVA_OPTIONS --enable-native-access=ALL-UNNAMED
//SOURCES NativeCliProbe.java CliProbe.java ContextProbe.java ParserProbe.java ContainerProbe.java

import java.lang.foreign.Arena;
import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.Linker;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.TimeUnit;

/** Launch-function comparisons with fake CLIs, fake cgroups and real detached session identity. */
class LaunchProbe {
  static int checks;
  static Path checkout, root, binding, bin;
  static String sh(Object value) { return "'" + value.toString().replace("'", "'\\''") + "'"; }
  static void require(boolean condition, String message) { if (!condition) throw new AssertionError(message); }
  static int os(String name) {
    try { var linker = Linker.nativeLinker(); return (int) linker.downcallHandle(linker.defaultLookup().findOrThrow(name), FunctionDescriptor.of(ValueLayout.JAVA_INT)).invokeExact(); }
    catch (Throwable error) { throw new IllegalStateException(error); }
  }
  static void setsid(String[] args) throws Throwable {
    require(os("setsid") >= 0, "reference setsid failed");
    try (var arena = Arena.ofConfined()) {
      var argv = arena.allocate(ValueLayout.ADDRESS, args.length);
      for (int i = 1; i < args.length; i++) argv.setAtIndex(ValueLayout.ADDRESS, i - 1, arena.allocateFrom(args[i]));
      argv.setAtIndex(ValueLayout.ADDRESS, args.length - 1, MemorySegment.NULL);
      var linker = Linker.nativeLinker();
      int code = (int) linker.downcallHandle(linker.defaultLookup().findOrThrow("execvp"), FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS, ValueLayout.ADDRESS)).invokeExact(arena.allocateFrom(args[1]), argv);
      throw new AssertionError("reference execvp failed: " + code);
    }
  }
  static void copilot(String[] args) throws Exception {
    var log = Path.of(System.getenv("STUB_LOG"));
    long calls = Files.exists(log) ? Files.readAllLines(log).size() : 0;
    Files.writeString(log, ParserProbe.array(List.of(args).subList(1, args.length)) + "\n", StandardOpenOption.CREATE, StandardOpenOption.APPEND);
    var mode = System.getenv().getOrDefault("STUB_MODE", "ok");
    var argv = List.of(args);
    var model = argv.contains("--model") ? argv.get(argv.indexOf("--model") + 1) : "";
    if (mode.equals("cold") && calls == 0 || mode.equals("unsigned")) System.out.println("No authentication information found");
    else if (mode.equals("quota")) System.out.println("{\"statusCode\":402,\"errorCode\":\"quota_exceeded\"}");
    else if (mode.equals("effort")) System.out.println("Model does not support reasoning effort");
    else if (mode.equals("unserved") || mode.equals("alternate") && model.endsWith("4-8")) System.out.println("Model '" + model + "' is not available");
    else if (mode.equals("byok404")) System.out.println("Model '" + model + "' not found on provider at https://fixture.invalid (HTTP 404).");
    else System.out.println("ok");
  }
  static void record() throws Throwable {
    int pid = os("getpid"), group = os("getpgrp");
    var linker = Linker.nativeLinker();
    int session = (int) linker.downcallHandle(linker.defaultLookup().findOrThrow("getsid"), FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.JAVA_INT)).invokeExact(0);
    var previous=(MemorySegment)linker.downcallHandle(linker.defaultLookup().findOrThrow("signal"),FunctionDescriptor.of(ValueLayout.ADDRESS,ValueLayout.JAVA_INT,ValueLayout.ADDRESS)).invokeExact(1,MemorySegment.ofAddress(1));
    var data = pid + "\n" + group + "\n" + session + "\n" + Files.readString(Path.of(System.getenv("EXPECTED_PIDFILE"))).strip() + "\n" + System.getenv("TE_DETACHED") + "\n" + System.getenv("PYTHONUNBUFFERED") + "\n" + System.getenv("LOCAL_ONLY") + "\n" + (previous.address()==1) + "\n";
    Files.writeString(Path.of(System.getenv("RECORD")), data);
    System.out.println("detached stdout"); System.err.println("detached stderr");
    Files.writeString(Path.of(System.getenv("RECORD") + ".done"), "done");
  }
  static Path tool(String name) {
    return java.util.Arrays.stream(System.getenv("PATH").split(java.io.File.pathSeparator)).map(Path::of).map(path -> path.resolve(name)).filter(Files::isExecutable).findFirst().orElseThrow();
  }
  static void executable(Path path, String text) throws Exception {
    Files.writeString(path, text); Files.setPosixFilePermissions(path, PosixFilePermissions.fromString("rwx------"));
  }
  static void javaFixture(Path path, String option) throws Exception {
    executable(path, "#!/bin/sh\nexec " + sh(Path.of(System.getProperty("java.home"), "bin/java")) + " -Xrs --enable-native-access=ALL-UNNAMED -cp " + sh(System.getProperty("java.class.path")) + " LaunchProbe " + option + " \"$@\"\n");
  }
  static Map<String,String> environment() {
    return new HashMap<>(Map.of("PATH", bin + ":/usr/bin:/bin", "HOME", root.toString(), "REPO_ROOT", root.resolve("experiment").toString(),
        "TOKENECONOMIZER_ENV_FILE", root.resolve("absent.env").toString(), "COPILOT_HOST_BIN", bin.resolve("fake-copilot").toString(), "STUB_LOG", root.resolve("copilot.log").toString()));
  }
  static NativeCliProbe.Output run(Path source, String script, Map<String,String> env) throws Exception {
    var out = Files.createTempFile(root, "out-", ".log"); var err = Files.createTempFile(root, "err-", ".log");
    var process = new ProcessBuilder(tool("bash").toString(), "-uc", ". " + sh(source) + "; " + script).directory(root.toFile()).redirectOutput(out.toFile()).redirectError(err.toFile());
    process.environment().clear(); process.environment().putAll(env);
    var child = process.start();
    try {
      require(child.waitFor(20, TimeUnit.SECONDS), "launch helper timed out: " + script);
      return new NativeCliProbe.Output(child.exitValue(), Files.readString(out), Files.readString(err));
    } finally { if (child.isAlive()) child.destroyForcibly().waitFor(); Files.delete(out); Files.delete(err); }
  }
  static final String SNAPSHOT = "; _probe_status=$?; printf '\\n--state--\\n'; for _key in MODEL TE_EFFORT_APPLIES TE_EFFORT_APPLIES_FOR TE_A TE_B TE_EMPTY ANTHROPIC_BASE_URL ANTHROPIC_AUTH_TOKEN; do declare -p \"$_key\" 2>/dev/null || :; done; exit \"$_probe_status\"";
  static void compare(String script, Map<String,String> env) throws Exception {
    compare(checkout.resolve("reference/upstream/experiments/scripts/launch/_env.sh"),script,env,null);
  }
  static void compare(Path sourceFile,String script,Map<String,String> env,Path changedFile) throws Exception {
    var before=changedFile==null?null:Files.readAllBytes(changedFile);
    var log = root.resolve("copilot.log"); Files.deleteIfExists(log);
    var source = run(sourceFile, script + SNAPSHOT, env);
    var expectedFile=changedFile==null?null:Files.readAllBytes(changedFile);
    if(before!=null&&!java.util.Arrays.equals(before,expectedFile))Files.write(changedFile,before);
    var calls = Files.exists(log) ? Files.readString(log) : ""; Files.deleteIfExists(log);
    var nativeCode = run(binding, script + SNAPSHOT, env);
    var actualCalls = Files.exists(log) ? Files.readString(log) : "";
    require(source.equals(nativeCode) && calls.equals(actualCalls), script + "\nNative: " + nativeCode + "\nSource: " + source + "\nNative calls: " + actualCalls + "\nSource calls: " + calls); checks++;
    if(changedFile!=null)require(java.util.Arrays.equals(expectedFile,Files.readAllBytes(changedFile)),"CPU budget bytes differ");
  }
  static void detach() throws Exception {
    var env = environment(); var log = root.resolve("detached log's file.log");
    env.put("TE_LOG", log.toString()); env.put("RECORD", root.resolve("record").toString()); env.put("EXPECTED_PIDFILE", log.toString().replaceFirst("\\.log$", ".pid"));
    javaFixture(bin.resolve("record"), "--record");
    var script = "LOCAL_ONLY=private; te_detach fixture " + sh(bin.resolve("record")) + "; echo SHOULD_NOT_RUN";
    NativeCliProbe.Output expected = null;
    for (var source : List.of(checkout.resolve("reference/upstream/experiments/scripts/launch/_env.sh"), binding)) {
      Files.deleteIfExists(root.resolve("record.done")); Files.deleteIfExists(root.resolve("record"));
      var result = run(source, script, env);
      require(result.code() == 0 && result.out().contains("started in the background") && !result.out().contains("SHOULD_NOT_RUN"), "detach failed: " + result);
      if (expected == null) expected = result; else require(result.equals(expected), "detach output differs: " + result + "\nSource: " + expected);
      var deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(10);
      while (!Files.exists(root.resolve("record.done")) && System.nanoTime() < deadline) Thread.sleep(10);
      require(Files.exists(root.resolve("record.done")), "detached script did not finish: " + Files.readString(log));
      var data = Files.readAllLines(root.resolve("record"));
      require(data.get(0).equals(data.get(1)) && data.get(0).equals(data.get(2)) && data.get(0).equals(data.get(3)), "PID/session/group mismatch: " + data);
      require(data.subList(4, 7).equals(List.of("1", "1", "null")), "detach export scope changed: " + data);
      require(data.get(7).equals("true"),"detached child did not inherit ignored SIGHUP: "+data);
      require(Files.readString(log).equals("detached stdout\ndetached stderr\n"), "detach did not combine console streams"); checks += 3;
    }
    env.put("DETACH","0");compare("te_detach fixture " + sh(bin.resolve("record")),env);
    env.remove("DETACH");env.put("TE_DETACHED","1");compare("te_detach fixture " + sh(bin.resolve("record")),env);
    env.remove("TE_DETACHED");
    for(var failure:List.of("log","pid")) {
      Files.deleteIfExists(root.resolve("record.done"));
      var denied=root.resolve("denied-"+failure+".log");env.put("TE_LOG",denied.toString());
      Files.createDirectory(failure.equals("log")?denied:root.resolve("denied-pid.pid"));
      var result=run(binding,script,env);
      require(result.code()==2&&!result.out().contains("started")&&!Files.exists(root.resolve("record.done")),"detach continued after "+failure+" file creation failed: "+result);checks++;
    }
  }
  static void fleet() throws Exception {
    var cg=root.resolve("cgroups");
    var uid=Files.getAttribute(root,"unix:uid").toString();
    var parent=Files.createDirectories(cg.resolve("user.slice/user-"+uid+".slice/user@"+uid+".service/user.slice"));
    Files.writeString(parent.resolve("cpu.max"),"max 100000\n");Files.writeString(parent.resolve("cpu.stat"),"usage_usec 99\nnr_throttled 3\nthrottled_usec 120\n");
    var source=checkout.resolve("reference/upstream/experiments/scripts/launch/fleet_budget.sh");var env=environment();env.put("TE_CGROUP_ROOT",cg.toString());
    compare(source,"te_fleet_cgroup",env,null);
    Files.createDirectory(parent.resolve("docker-fixture.scope"));compare(source,"te_fleet_cgroup",env,null);
    for(var cores:List.of("0","none","1","16","020","0x10","-2")) compare(source,"te_fleet_cpu_cap "+sh(cores),env,parent.resolve("cpu.max"));
    Files.writeString(parent.resolve("cpu.max"),"max 100000\n");compare(source,"te_fleet_cpu_report",env,null);
    javaFixture(bin.resolve("docker"),"--docker");compare(source,"te_fleet_cpu_report",env,null);
    Files.writeString(parent.resolve("cpu.max"),"800000 100000\n");compare(source,"te_fleet_cpu_report",env,null);
    Files.setPosixFilePermissions(parent.resolve("cpu.max"),PosixFilePermissions.fromString("r--r--r--"));
    try{compare(source,"te_fleet_cpu_cap 16",env,parent.resolve("cpu.max"));}finally{Files.setPosixFilePermissions(parent.resolve("cpu.max"),PosixFilePermissions.fromString("rw-------"));}
    var absent=environment();absent.put("TE_CGROUP_ROOT",root.resolve("absent-cgroup").toString());compare(source,"te_fleet_cpu_cap 16; te_fleet_cpu_report",absent,null);
    Files.delete(parent.resolve("docker-fixture.scope"));
    var custom=Files.createDirectories(parent.getParent().resolve("custom"));Files.createDirectory(custom.resolve("docker-other.scope"));
    Files.writeString(custom.resolve("cpu.max"),"max 100000\n");
    compare(source,"te_fleet_cgroup",env,null);compare(source,"te_fleet_cpu_cap 4",env,custom.resolve("cpu.max"));
    Files.delete(custom.resolve("docker-other.scope"));Files.createDirectories(custom.resolve("too/deep/docker-ignored.scope"));
    compare(source,"te_fleet_cgroup",env,null);
  }
  static void standalone(Path project,Path work,Path executable) throws Exception {
    Files.createDirectories(work);
    var uid=Files.getAttribute(work,"unix:uid").toString();
    var cgroups=work.resolve("cgroups");
    var group=Files.createDirectories(cgroups.resolve("user.slice/user-"+uid+".slice/user@"+uid+".service/user.slice"));
    Files.writeString(group.resolve("cpu.max"),"max 100000\n");Files.writeString(group.resolve("cpu.stat"),"nr_throttled 2\nthrottled_usec 4\n");
    var tools=Files.createDirectory(work.resolve("source-tools"));
    for(var name:List.of("id","find","dirname","cat","awk")) Files.createSymbolicLink(tools.resolve(name),tool(name));
    var expected=NativeCliProbe.process(work,Map.of("PATH",tools.toString(),"TE_CGROUP_ROOT",cgroups.toString()),List.of(tool("bash").toString(),project.resolve("reference/upstream/experiments/scripts/launch/fleet_budget.sh").toString(),"16"));
    require(expected.code()==0&&expected.out().contains("capped at 16 cores"),"reference fleet fixture failed: "+expected);
    Files.writeString(group.resolve("cpu.max"),"max 100000\n");
    var env=Map.of("PATH","","TE_CGROUP_ROOT",cgroups.toString(),"TE_BENCH_PYTHON","/missing-python");
    var actual=NativeCliProbe.process(work,env,List.of(executable.toString(),"fleet-budget","16"));
    require(actual.equals(expected),"standalone fleet output differs: "+actual+"\nSource: "+expected);
    require(Files.readString(group.resolve("cpu.max")).equals("1600000 100000\n"),"standalone fleet wrote the wrong quota");
    var report=NativeCliProbe.process(work,env,List.of(executable.toString(),"fleet-budget"));
    require(report.code()==0&&report.out().startsWith("[fleet] CPU budget: 16 cores"),"standalone fleet report required utilities: "+report);checks+=3;
  }
  public static void main(String[] args) throws Throwable {
    if (args.length > 0) {
      switch (args[0]) {
        case "--copilot" -> copilot(args);
        case "--setsid" -> setsid(args);
        case "--record" -> record();
        case "--docker" -> { if (List.of(args).contains("stats")) System.out.print("50.0%\n200.0%\n"); else System.out.print("one\ntwo\n"); }
        default -> throw new IllegalArgumentException(args[0]);
      }
      return;
    }
    checkout = Path.of("").toAbsolutePath(); root = Files.createTempDirectory(checkout.resolve("validation/results"), "launch-");
    bin = Files.createDirectory(root.resolve("bin")); boolean passed = false;
    try {
      Files.createDirectories(root.resolve("experiment"));
      Files.createSymbolicLink(bin.resolve("timeout"), tool("timeout"));
      javaFixture(bin.resolve("fake-copilot"), "--copilot"); javaFixture(bin.resolve("setsid"), "--setsid");
      var generated = NativeCliProbe.process(checkout, Map.of("PYTHONPATH", checkout.resolve("validation").toString()), List.of(checkout.resolve(".venv/bin/python").toString(), "-c", "from native_launch import shell_binding; print(shell_binding())"));
      require(generated.code() == 0, generated.toString()); binding = Path.of(generated.out().strip());
      for (var model : List.of("", "auto", "agent", "copilot", "claude-opus-4-8", "claude-opus-4.8", "vendor/gpt-5.6-sol", "CLAUDE-SONNET-5", "x1-2/y3.4", "é-model-1.2")) {
        compare("te_model_tag " + sh(model), environment()); compare("te_model_alt_spelling " + sh(model), environment());
        for (var agent : List.of("claude", "copilot", "codex")) compare("te_model_for_agent " + sh(agent) + " " + sh(model), environment());
      }
      for (var text : List.of("", "ok", "QUOTA_EXCEEDED", "You have exceeded your monthly quota", "{\"errorType\" : \"quota\"}", "{\"statusCode\":402}", "{\"statusCode\":401}", "Model x is not available", "x (HTTP 404)", "not found on provider")) {
        compare("te_copilot_refused " + sh(text), environment()); compare("te_copilot_model_unserved " + sh(text), environment()); compare("te_copilot_refused_die " + sh(text) + " auth", environment());
      }
      for (var mode : List.of("ok", "cold", "unsigned", "quota", "effort", "alternate", "unserved", "byok404")) {
        var env = environment(); env.put("STUB_MODE", mode);
        compare("te_check_copilot_auth", env); compare("te_check_copilot_model claude-opus-4-8 high", env);
      }
      var missing=environment();missing.put("COPILOT_HOST_BIN",root.resolve("missing").toString());compare("te_check_copilot_auth",missing);compare("te_check_copilot_model claude-sonnet-5 high",missing);
      compare("te_check_copilot_model auto high",environment());compare("te_check_copilot_model '' high",environment());
      for (var pair : List.of(Map.of("DRYRUN", "1"), Map.of("COPILOT_PROVIDER_BASE_URL", "https://fixture.invalid"), Map.of("GH_TOKEN", "fake"), Map.of("TE_EFFORT_APPLIES", "0", "TE_EFFORT_APPLIES_FOR", "claude-opus-4-8"), Map.of("TE_STRICT_MODEL_ID", "1", "STUB_MODE", "alternate"))) {
        var env = environment(); env.putAll(pair); compare("te_check_copilot_auth", env); compare("te_check_copilot_model claude-opus-4-8 high", env);
      }
      for (var url : List.of("", "https://api.openai.com/v1", "https://US.api.openai.com/v1", "http://api.openai.com/v1", "https://api.openai.com.evil.invalid/v1", "https://fixture.invalid")) {
        var env = environment(); env.put("OPENAI_BASE_URL", url); compare("te_check_openai_base_url", env);
      }
      for (var agent : List.of("claude", "copilot", "codex", "unknown")) {
        compare("te_check_routing " + agent, environment());
        var env = environment(); env.put("RUNNER", "fixture:make_runner"); compare("te_check_routing " + agent, env);
        env.put("ANTHROPIC_API_KEY", "fake-1234"); env.put("OPENAI_API_KEY", "fake-5678"); compare("te_check_routing " + agent, env);
      }
      var env = environment(); env.put("ANTHROPIC_BASE_URL", "https://ignored.invalid"); env.put("ANTHROPIC_AUTH_TOKEN", "fake"); compare("te_drop_anthropic_endpoint", env);
      compare("te_require_key FIRST SECOND FIRST", environment());
      var file = root.resolve("loader.env"); Files.writeString(file, "# comment\nexport TE_A='é😀'\nTE_B=first\nTE_B=last\nTE_EMPTY=''\n");
      env = environment(); env.put("TOKENECONOMIZER_ENV_FILE", file.toString()); compare("te_load_env", env); compare("TE_A=local; te_load_env", env);
      compare("te_env_file", environment());
      Files.writeString(root.resolve(".env"), "TE_A=parent\n"); env = environment(); env.remove("TOKENECONOMIZER_ENV_FILE"); compare("te_env_file", env); compare("te_load_env", env);
      Files.writeString(root.resolve("experiment/.env"), "TE_A=local\n"); compare("te_env_file", env); compare("te_load_env", env);
      detach();
      fleet();
      standalone(checkout,root.resolve("standalone"),checkout.resolve("target/debug/te-bench"));
      System.out.println("Verified " + (checks-3) + " launch comparisons and detached-session checks, plus three standalone fleet checks"); passed = true;
    } finally { if (passed) { if (binding != null) Files.deleteIfExists(binding); ContainerProbe.clear(root); } else System.err.println("Preserved launch fixtures: " + root); }
  }
}
