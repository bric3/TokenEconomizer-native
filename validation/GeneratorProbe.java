//JAVA 25
//SOURCES ContextProbe.java ProcessInputProbe.java

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Map;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.Executors;
import java.util.regex.Pattern;

/** Native generator parity against the original generator through an isolated SDK fixture. */
class GeneratorProbe {
  static String quote(Object value) { return ProcessInputProbe.quote(value); }
  static void require(boolean ok, String message) { if (!ok) throw new AssertionError(message); }
  record Result(int code, String out, String err) { }
  static Result run(List<String> argv, Path cwd, Map<String,String> extra) throws Exception {
    var builder = new ProcessBuilder(argv).directory(cwd.toFile());
    builder.environment().putAll(extra);
    var process = builder.start();
    process.getOutputStream().close();
    try (var reader = Executors.newVirtualThreadPerTaskExecutor()) {
      var out = reader.submit(() -> process.getInputStream().readAllBytes());
      var err = reader.submit(() -> process.getErrorStream().readAllBytes());
      if (!process.waitFor(20,TimeUnit.SECONDS)) {
        process.descendants().forEach(ProcessHandle::destroyForcibly);
        process.destroyForcibly().waitFor();
        throw new AssertionError("Generator stayed alive");
      }
      return new Result(process.exitValue(),new String(out.get(2,TimeUnit.SECONDS),StandardCharsets.UTF_8),new String(err.get(2,TimeUnit.SECONDS),StandardCharsets.UTF_8));
    }
  }
  static String field(String json, String key) {
    var match = Pattern.compile(quote(key) + ": \"([^\"]+)\"").matcher(json);
    require(match.find(),json);
    return match.group(1);
  }
  static void file(Path repo, String name, int bytes) throws Exception {
    var path = repo.resolve(name);
    Files.createDirectories(path.getParent());
    Files.write(path,new byte[bytes]);
  }
  static String check(Path root, Path nativeBin, Path python, Path original, Path repo, List<String> args) throws Exception {
    var record = root.resolve("sdk.json");
    var env = Map.of("PYTHONPATH",root.resolve("sdk").toString(),"SDK_RECORD",record.toString());
    var command = new ArrayList<>(List.of(nativeBin.toString(),"--python",python.toString()));
    command.addAll(args);
    var result = run(command,root,env);
    require(result.code == 0 && result.out.equals("MAP é😀\n") && result.err.contains("Initial repo scan"),result.toString());
    var observed = Files.readString(record);
    require(!Files.exists(Path.of(field(observed,"script"))),"Native SDK workspace was retained");
    var referenceCommand = new ArrayList<>(List.of(python.toString(),original.toString()));
    referenceCommand.addAll(args);
    var reference = run(referenceCommand,root,env);
    require(reference.code == 0 && reference.out.equals(result.out),reference.toString());
    var expected = Files.readString(record);
    // The temporary script path and PID identify the transport rather than the map request.
    var strip = "\"(?:script|pid)\": (?:\"[^\"]*\"|[0-9]+),? ?";
    require(observed.replaceAll(strip,"").equals(expected.replaceAll(strip,"")),"Native request differs from the original:\n" + observed + "\n" + expected);
    return observed;
  }
  public static void main(String[] args) throws Exception {
    if (args.length > 0 && args[0].equals("--heartbeat")) {
      var root = Path.of(args[1]);
      for (int i = 0; ; i++) { Files.writeString(root.resolve("heartbeat"),Integer.toString(i)); Thread.sleep(25); }
    }
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"),"generator-");
    var repo = Files.createDirectory(root.resolve("repo"));
    var python = checkout.resolve(".venv/bin/python");
    var nativeBin = checkout.resolve("target/debug/te-repomap-gen");
    var original = checkout.resolve("reference/upstream/tokeneconomizer/plugin/repomap_gen.py");
    try {
      var sdk = Files.createDirectories(root.resolve("sdk/aider"));
      Files.writeString(sdk.resolve("__init__.py"),"");
      Files.writeString(sdk.resolve("io.py"),"class InputOutput:\n    def __init__(self): print('IO WARNING')\n");
      Files.writeString(sdk.resolve("models.py"),"class Model:\n    def __init__(self, name):\n        assert name == 'gpt-4o'\n        print('MODEL WARNING')\n");
      Files.writeString(sdk.resolve("repomap.py"),"""
          import json, os, subprocess, sys, time
          from pathlib import Path
          if os.environ.get('SDK_IMPORT_NOISE'): print('IMPORT WARNING')
          class RepoMap:
              TAGS_CACHE_DIR = 'DEFAULT'
              def __init__(self, **config):
                  assert config['verbose'] is False
                  self.config = config
                  print('Initial repo scan')
              def get_repo_map(self, chat, other, mentioned_idents):
                  assert chat == [] and isinstance(mentioned_idents, set)
                  assert isinstance(self.config['map_tokens'], int)
                  Path(os.environ['SDK_RECORD']).write_text(json.dumps(dict(files=other,
                      repo=self.config['root'], budget=self.config['map_tokens'],
                      cache=self.TAGS_CACHE_DIR, idents=sorted(mentioned_idents),
                      script=sys.argv[0], pid=os.getpid()), ensure_ascii=False), encoding='utf-8')
                  print('RANK WARNING')
                  if os.environ.get('SDK_FAIL'): raise RuntimeError('SDK broke')
                  if os.environ.get('SDK_SIGNAL'): os.kill(os.getpid(), 15)
                  if os.environ.get('SDK_SLEEP'):
                      subprocess.Popen([os.environ['PROBE_JAVA'], '-cp', os.environ['PROBE_CP'],
                                        'GeneratorProbe', '--heartbeat', os.environ['PROBE_ROOT']])
                      time.sleep(10)
                  return None if os.environ.get('SDK_EMPTY') else 'MAP é😀\\n'
          """);
      for (var name : List.of("pkg/core.py","pkg/util.txt","space name.py","line\nname.py","ü.py","pkg/noisy.min.js","pkg/noisy.bundle.js","pkg/noisy-min.js","pkg/noisy.min.css","vendor/external.py","vendorish/own.py","static/source.py",".github/config.py","build-tools/local.py","pkg/cache.egg-info/one.py","docs/direct.py","docs/subdir/deep.py","pkg/__pycache__/compiled.py")) file(repo,name,10);
      file(repo,"pkg/at-limit.py",256*1024);
      file(repo,"pkg/over-limit.py",256*1024+1);
      Files.createSymbolicLink(repo.resolve("linked-file.py"),Path.of("pkg/core.py"));
      Files.createSymbolicLink(repo.resolve("linked-dir"),Path.of("pkg"));
      Files.createSymbolicLink(repo.resolve("dangling.py"),Path.of("missing"));
      require(run(List.of("git","init","-q"),repo,Map.of()).code == 0,"git init failed");
      require(run(List.of("git","add","-A"),repo,Map.of()).code == 0,"git add failed");
      file(repo,"untracked.py",10);
      var git = check(root,nativeBin,python,original,repo,List.of(repo.toString(),"512","Engine,,Engine","--git","--cache","tags"));
      for (var name : List.of("pkg/core.py","pkg/util.txt","space name.py","line\nname.py","ü.py","pkg/at-limit.py","vendorish/own.py","linked-file.py","linked-dir")) require(git.contains(quote(repo.resolve(name))),"Tracked file missing: " + name);
      for (var name : List.of("untracked.py","pkg/over-limit.py","pkg/noisy.min.js","vendor/external.py","static/source.py",".github/config.py","build-tools/local.py","docs/direct.py","dangling.py")) require(!git.contains(quote(repo.resolve(name))),"Excluded file entered map: " + name);
      require(git.contains("\"cache\": " + quote(root.resolve("tags"))) && git.contains("\"idents\": [\"\", \"Engine\"]"),git);
      var walked = check(root,nativeBin,python,original,repo,List.of("repo"));
      require(walked.contains("\"budget\": 2048") && walked.contains("\"cache\": \"DEFAULT\""),walked);
      for (var name : List.of("untracked.py","pkg/over-limit.py","vendor/external.py","static/source.py","docs/direct.py","dangling.py")) require(walked.contains(quote(repo.resolve(name))),"Walked file missing: " + name);
      for (var name : List.of("pkg/util.txt",".github/config.py","build-tools/local.py","docs/subdir/deep.py","linked-dir/core.py")) require(!walked.contains(quote(repo.resolve(name))),"Walk followed an excluded path: " + name);
      for (var budget : List.of("٥_١٢","𝟝𝟙𝟚","100000000000000000000000001")) check(root,nativeBin,python,original,repo,List.of(repo.toString(),budget));

      var record = root.resolve("sdk.json");
      var env = Map.of("PYTHONPATH",root.resolve("sdk").toString(),"SDK_RECORD",record.toString(),"SDK_IMPORT_NOISE","1");
      var command = List.of(nativeBin.toString(),"--python",python.toString(),repo.toString());
      var noisy = run(command,root,env);
      require(noisy.code == 0 && noisy.out.equals("MAP é😀\n") && noisy.err.contains("IMPORT WARNING"),noisy.toString());
      var failed = run(command,root,Map.of("PYTHONPATH",root.resolve("sdk").toString(),"SDK_RECORD",record.toString(),"SDK_FAIL","1"));
      require(failed.code != 0 && failed.out.isEmpty() && failed.err.contains("SDK broke"),failed.toString());
      var empty = run(command,root,Map.of("PYTHONPATH",root.resolve("sdk").toString(),"SDK_RECORD",record.toString(),"SDK_EMPTY","1"));
      require(empty.code == 0 && empty.out.isEmpty(),empty.toString());
      var signal = run(command,root,Map.of("PYTHONPATH",root.resolve("sdk").toString(),"SDK_RECORD",record.toString(),"SDK_SIGNAL","1"));
      require(signal.code == 143,signal.toString());
      var invalid = run(List.of(nativeBin.toString(),repo.toString(),"1__2"),root,env);
      require(invalid.code != 0 && invalid.out.isEmpty() && invalid.err.contains("invalid map token budget"),invalid.toString());

      var wrapper = root.resolve("python");
      Files.writeString(wrapper,"#!/bin/sh\nPYTHONPATH='" + root.resolve("sdk").toString().replace("'","'\\''") + "' exec '" + python.toString().replace("'","'\\''") + "' \"$@\"\n");
      Files.setPosixFilePermissions(wrapper,PosixFilePermissions.fromString("rwx------"));
      var childEnv = "{\"SDK_RECORD\":" + quote(record) + ",\"SDK_SLEEP\":\"1\",\"PROBE_ROOT\":" + quote(root) + ",\"PROBE_JAVA\":" + quote(Path.of(System.getProperty("java.home"),"bin/java")) + ",\"PROBE_CP\":" + quote(System.getProperty("java.class.path")) + "}";
      try (var client = new ContextProbe.Client(checkout.resolve("target/debug/te-bridge"),"repomap")) {
        var timed = client.call("repomap.generate","\"ctx\":{\"base\":" + quote(root.resolve("state/host")) + ",\"repo\":{\"root\":" + quote(repo) + ",\"key\":\"r\",\"label\":\"r\",\"remote\":\"\"},\"config\":{\"repomap_tokens\":512},\"env\":" + childEnv + "},\"generator\":" + quote(nativeBin) + ",\"python\":" + quote(wrapper) + ",\"idents\":[],\"timeout\":2");
        require(timed.contains("timed out after 2s"),timed);
        var snapshot = Files.readString(record);
        require(!Files.exists(Path.of(field(snapshot,"script"))),"Timed-out SDK workspace was retained");
        var heartbeat = Files.readString(root.resolve("heartbeat"));
        Thread.sleep(250);
        require(heartbeat.equals(Files.readString(root.resolve("heartbeat"))),"SDK descendant survived the generator deadline");
      }
      System.out.println("Native generator matches original file selection and SDK requests; Unicode budgets, console routing, signals and descendant timeout cleanup passed");
    } finally {
      try (var paths = Files.walk(root)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path); }
    }
  }
}
