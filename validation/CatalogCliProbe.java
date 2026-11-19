//JAVA 25
//SOURCES ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java AdapterProbe.java ExtractProposerPrompts.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.Arrays;
import java.util.List;
import java.util.Map;

/** Runs the source and standalone native capture commands with a local Docker executable fixture. */
class CatalogCliProbe {
  record Output(int code, String out, String err) {}
  static String shell(String value) { return "'"+value.replace("'", "'\\''")+"'"; }
  static void fixture(String[] arguments) throws Exception {
    var args = Arrays.asList(arguments).subList(1,arguments.length);
    Files.writeString(Path.of(System.getenv("TE_CATALOG_FIXTURE_LOG")), ParserProbe.array(args)+"\n",StandardOpenOption.CREATE,StandardOpenOption.APPEND);
    if (args.getFirst().equals("cp")) {
      var helper = Path.of(args.get(1));
      var bytes = Files.readAllBytes(helper);
      if(bytes.length<64||bytes[0]!=127||bytes[1]!='E'||bytes[2]!='L'||bytes[3]!='F') throw new AssertionError("capture copied a non-native recorder");
    } else if (args.contains("claude")) {
      if ("1".equals(System.getenv("TE_CATALOG_SLOW_AGENT"))) Thread.sleep(30_000);
      System.out.println("{\"type\":\"system\",\"tools\":[\"Write\"]}");
    }
    else if (args.getLast().equals("uname -m")) System.out.println("aarch64");
    else if (args.getLast().contains("echo ready")) System.out.println("ready");
    else if (args.getLast().startsWith("curl ")) System.out.println("0");
    else if (args.getLast().startsWith("cat /tmp/tools.json")) System.out.println("{\"Write\":\"write é😀 files\"}");
  }
  static Output process(Path root, Map<String,String> environment, List<String> command) throws Exception {
    var out = Files.createTempFile(root,"stdout-",".log");
    var err = Files.createTempFile(root,"stderr-",".log");
    var builder = new ProcessBuilder(command).redirectOutput(out.toFile()).redirectError(err.toFile());
    builder.environment().putAll(environment);
    var process = builder.start();
    if (!process.waitFor(60,java.util.concurrent.TimeUnit.SECONDS)) { process.destroyForcibly(); throw new AssertionError("Capture fixture timed out"); }
    return new Output(process.exitValue(),Files.readString(out),Files.readString(err));
  }
  static int interruption(Path root, Map<String,String> environment, List<String> command) throws Exception {
    var log = Path.of(environment.get("TE_CATALOG_FIXTURE_LOG"));
    var snapshot = Path.of(environment.get("TE_CATALOG_OUT"));
    Files.deleteIfExists(log); Files.deleteIfExists(snapshot);
    var error = Files.createTempFile(root,"interrupt-",".log");
    var builder = new ProcessBuilder(command).redirectOutput(ProcessBuilder.Redirect.DISCARD).redirectError(error.toFile());
    builder.environment().putAll(environment);
    builder.environment().put("TE_CATALOG_SLOW_AGENT","1");
    var process = builder.start();
    try {
      long deadline = System.nanoTime()+java.util.concurrent.TimeUnit.SECONDS.toNanos(15);
      while (!Files.exists(log) || !Files.readString(log).contains("\"claude\",\"-p\"")) {
        if (!process.isAlive() || System.nanoTime()>deadline) throw new AssertionError("Capture did not reach its agent: "+Files.readString(error));
        Thread.sleep(10);
      }
      if (new ProcessBuilder("/bin/kill","-INT",Long.toString(process.pid())).start().waitFor()!=0) throw new AssertionError("Could not interrupt owned fixture");
      if (!process.waitFor(15,java.util.concurrent.TimeUnit.SECONDS)) throw new AssertionError("Interrupted capture did not finish");
      var calls = Files.readString(log);
      if (process.exitValue()==0 || !Files.readString(error).contains("KeyboardInterrupt") || Files.exists(snapshot)
          || calls.lastIndexOf("[\"rm\",\"-f\",\"te-catalog-capture\"]")<calls.indexOf("\"claude\",\"-p\""))
        throw new AssertionError("Interrupted capture did not clean up: "+calls+"\n"+Files.readString(error));
      return process.exitValue();
    } finally {
      var children = process.descendants().toList();
      process.destroyForcibly();
      for (var child : children) child.destroyForcibly();
      process.waitFor();
    }
  }
  static void standalone(Path checkout, Path root, Path executable) throws Exception {
    Files.createDirectories(root);
    var docker = root.resolve("docker");
    Files.writeString(docker,"#!/bin/sh\nexec "+shell(Path.of(System.getProperty("java.home"),"bin/java").toString())+" -cp "+shell(System.getProperty("java.class.path"))+" CatalogCliProbe --docker-fixture \"$@\"\n");
    if (!docker.toFile().setExecutable(true)) throw new AssertionError("Docker fixture is not executable");
    var snapshot = root.resolve("tools.json");
    var log = root.resolve("docker.jsonl");
    var environment = Map.of("ANTHROPIC_API_KEY","fixture-key","TE_CATALOG_OUT",snapshot.toString(),
        "TE_CATALOG_INSTANCE","pydata__xarray-6461","TE_CATALOG_DESC_CHARS","300","TE_CATALOG_PORT","8770",
        "TE_CLAUDE_BIN",root.resolve("claude").toString(),"PATH",root.toString(),"PYTHONPATH","",
        "TE_BENCH_PYTHON",checkout.resolve(".venv/bin/python").toString(),"TE_CATALOG_FIXTURE_LOG",log.toString());
    var sourceEnvironment = new java.util.HashMap<>(environment);
    sourceEnvironment.put("PYTHONPATH",checkout.resolve("reference/upstream")+":"+checkout.resolve("reference/upstream/experiments"));
    var expected = process(root,sourceEnvironment,List.of(checkout.resolve(".venv/bin/python").toString(),"-m","te_harness.capture_catalog"));
    var sourceSnapshot = Files.readString(snapshot); Files.delete(snapshot); Files.delete(log);
    var actual = process(root,environment,List.of(executable.toString(),"capture-catalog"));
    if (!actual.equals(expected) || actual.code()!=0 || !Files.readString(snapshot).equals(sourceSnapshot))
      throw new AssertionError("Standalone capture differs\nNative: "+actual+"\nSource: "+expected);
    var calls = Files.readString(log);
    if (!calls.contains("\"cp\"") || !calls.contains("te-net-guard") || !calls.contains("chmod 755 /tmp/te-net-guard"))
      throw new AssertionError("Capture did not deploy its static recorder helper: "+calls);
    var interruptedSource = interruption(root,sourceEnvironment,List.of(checkout.resolve(".venv/bin/python").toString(),"-m","te_harness.capture_catalog"));
    var interruptedNative = interruption(root,environment,List.of(executable.toString(),"capture-catalog"));
    if (interruptedSource!=interruptedNative) throw new AssertionError("Capture interruption exit status differs");
  }
  public static void main(String[] args) throws Exception {
    if (args.length>0 && args[0].equals("--docker-fixture")) { fixture(args); return; }
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"),"catalog-cli-");
    boolean passed = false;
    try { standalone(checkout,root,checkout.resolve("target/debug/te-bench")); passed = true; }
    finally { if (passed) ContainerProbe.clear(root); else System.err.println("Probe artifacts: "+root); }
    System.out.println("Standalone capture matches source output and snapshot bytes, deploys the verified native recorder helper and preserves Ctrl-C cleanup");
  }
}
