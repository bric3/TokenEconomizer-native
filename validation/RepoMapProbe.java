//JAVA 25
//SOURCES ContextProbe.java ProcessInputProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.Arrays;
import java.util.Comparator;
import java.util.List;

/** Native repository-map state and orchestration; the generator is a local CLI fixture. */
class RepoMapProbe {
  static String quote(Object value) { return ProcessInputProbe.quote(value); }
  static void require(boolean ok, String message) { if (!ok) throw new AssertionError(message); }
  static String context(Path root, String python, boolean sync) {
    return "{\"base\":" + quote(root.resolve("state/project/claude")) + ",\"repo\":{\"root\":" + quote(root.resolve("repo with space")) + ",\"key\":\"project\",\"label\":\"project\",\"remote\":\"\"},\"config\":{\"repomap_python\":" + quote(python) + ",\"repomap_tokens\":2048,\"repomap_timeout\":1},\"env\":{\"PROBE_ROOT\":" + quote(root) + ",\"PYTHONPATH\":\"must be removed\"" + (sync ? ",\"TE_FOLD_SYNC\":\"1\"" : "") + "}}";
  }
  static String common(Path root, String python, boolean sync) {
    return "\"ctx\":" + context(root,python,sync) + ",\"process_cwd\":" + quote(root) + ",\"home\":" + quote(root) + ",\"process_env\":{\"GIT_CEILING_DIRECTORIES\":" + quote(root) + "},\"generator\":" + quote(Path.of("target/debug/te-repomap-gen").toAbsolutePath()) + ",\"interpreter\":" + quote(root.resolve("good"));
  }
  static void fixture(String[] args) throws Exception {
    var root = Path.of(System.getenv("PROBE_ROOT"));
    var mode = args[1];
    var argv = Arrays.asList(args).subList(2,args.length);
    require("1".equals(System.getenv("TE_SIDECALL")) && System.getenv("PYTHONPATH") == null, "Generator environment changed");
    Files.writeString(root.resolve("calls"),mode + " " + argv.getFirst() + "\n",StandardOpenOption.CREATE,StandardOpenOption.APPEND);
    if (argv.getFirst().equals("-c")) {
      require(argv.equals(List.of("-c","import aider.repomap")),argv.toString());
      if (mode.equals("bad")) System.exit(1);
      return;
    }
    require(argv.size() == 7 && Path.of(argv.getFirst()).getFileName().toString().equals("aider_sdk.py") && Files.exists(Path.of(argv.getFirst())) && argv.get(2).equals("2048") && argv.get(4).equals("--git") && argv.get(5).equals("--cache"),argv.toString());
    require(Files.isSameFile(Path.of(""),root.resolve("repo with space")), "Generator cwd changed");
    require(Path.of(argv.getLast()).equals(root.resolve("state/project/repomap-tags")), "Tags cache entered the repository");
    Files.writeString(root.resolve("pid"),Long.toString(ProcessHandle.current().pid()));
    if (mode.equals("slow") && !argv.get(3).isEmpty()) { Thread.sleep(10_000); return; }
    if (mode.equals("error")) {
      System.err.print(" \u001clead" + "é😀".repeat(200) + " tail\u001f ");
      System.exit(2);
    }
    System.out.print("MAP é😀 [" + argv.get(3) + "]\n");
  }
  public static void main(String[] args) throws Exception {
    if (args.length > 0 && args[0].equals("--fixture")) { fixture(args); return; }
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"),"repomap-");
    Files.createDirectory(root.resolve("repo with space"));
    for (var mode : List.of("good","bad","slow","error")) {
      var wrapper = root.resolve(mode);
      Files.writeString(wrapper,"#!/bin/sh\nexec '" + Path.of(System.getProperty("java.home"),"bin/java").toString().replace("'","'\\''") + "' -cp '" + System.getProperty("java.class.path").replace("'","'\\''") + "' RepoMapProbe --fixture " + mode + " \"$@\"\n");
      Files.setPosixFilePermissions(wrapper,PosixFilePermissions.fromString("rwx------"));
    }
    var good = root.resolve("good").toString();
    var normal = common(root,good,false);
    var stateFile = root.resolve("state/project/repomap.json");
    try (var client = new ContextProbe.Client(checkout.resolve("target/debug/te-bridge"),"repomap")) {
      var missing = client.call("repomap.read_state",normal);
      require(missing.contains("\"value\":{}") && !Files.exists(stateFile.getParent()),missing);
      var ids = client.call("repomap.prompt_idents","\"text\":" + quote("`abc.` `a.b` `zz` aa_bb\u0345cc_dd αee_ff ee_ffβ Ⅳgg_hh"));
      require(ids.contains("\"value\":[\"\",\"aa_bb\",\"b\",\"cc_dd\"]"),ids);
      var many = new StringBuilder();
      for (int i = 99; i >= 0; i--) many.append("`id_").append(String.format("%03d",i)).append("` ");
      ids = client.call("repomap.prompt_idents","\"text\":" + quote(many));
      require(ids.contains("id_000") && ids.contains("id_059") && !ids.contains("id_060"),ids);
      var state = "{\"commit\":\"previous\",\"python\":" + quote(good) + ",\"map\":\"cached é😀\",\"chars\":9,\"tokens_budget\":2048,\"tag\":\"keep\"}";
      client.call("repomap._write_state",normal + ",\"d\":" + state);
      require(client.call("repomap.cached_block",normal).contains("cached é😀"),"Cached map disappeared");
      require(client.call("repomap.stale",normal).contains("\"value\":true"),"An old commit stayed fresh");
      var bad = common(root,root.resolve("bad").toString(),false);
      require(client.call("repomap.find_python",bad).contains("\"value\":null"),"A named interpreter was replaced");
      var unavailable = client.call("repomap.refresh",bad);
      require(unavailable.contains("cannot import aider") && unavailable.contains("cached é😀") && unavailable.contains("\"tag\":\"keep\""),unavailable);
      var refreshed = client.call("repomap.refresh",normal);
      require(refreshed.contains("MAP é😀 []") && refreshed.contains("\"chars\":10") && refreshed.contains("\"error\":null"),refreshed);
      require(client.call("repomap.stale",normal).contains("\"value\":false"),"The current commit and budget stayed stale");
      var ranked = client.call("repomap.for_prompt",normal + ",\"prompt\":\"Fix `Foo.bar` in ConfigLoader and do_work\"");
      require(ranked.contains("MAP é😀 [ConfigLoader,bar,do_work]") && ranked.contains("\"prompt\"]"),ranked);
      require(Files.readString(stateFile).contains("MAP ") && !Files.readString(stateFile).contains("ConfigLoader"),"Prompt ranking overwrote the cached map");
      int before = Files.readAllLines(root.resolve("calls")).size();
      require(client.call("repomap.for_prompt",normal + ",\"prompt\":\"fix stuff\"").contains("\"cached\"]"),"A prompt with no identifiers lost its cache");
      require(Files.readAllLines(root.resolve("calls")).size() == before,"A plain prompt started the generator");

      state = "{\"commit\":\"previous\",\"python\":" + quote(root.resolve("slow")) + ",\"map\":\"old cache\",\"tokens_budget\":2048}";
      client.call("repomap._write_state",normal + ",\"d\":" + state);
      var timed = client.call("repomap.for_prompt",normal + ",\"prompt\":\"Fix `do_work`\"");
      require(timed.contains("old cache") && timed.contains("\"cached\"]"),timed);
      require(!ProcessHandle.of(Long.parseLong(Files.readString(root.resolve("pid")))).map(ProcessHandle::isAlive).orElse(false),"Timed-out generator survived");
      var failed = client.call("repomap.refresh",common(root,root.resolve("error").toString(),false));
      var error = "lead" + "é😀".repeat(200) + " tail";
      var tail = error.substring(error.offsetByCodePoints(0,error.codePointCount(0,error.length()) - 300));
      require(failed.contains("\"error\":" + quote(tail)) && failed.contains("old cache") && failed.contains("\"commit\":\"previous\""),failed);
      var pending = client.call("repomap.spawn_refresh",normal);
      require(pending.contains("repomap.log") && pending.contains("\"repomap\",\"refresh\""),pending);
      require(client.resume(pending,"spawn","{\"value\":null}").contains("\"value\":null"),"Spawn result changed");
      require(client.call("repomap.spawn_refresh",common(root,good,true)).contains("MAP é😀 []"),"Synchronous refresh did not run");

      var fallback = common(root,"",false);
      before = Files.readAllLines(root.resolve("calls")).size();
      pending = client.call("repomap.find_python",fallback);
      pending = client.resume(pending,"which","{\"value\":null}");
      var found = client.resume(pending,"which","{\"value\":" + quote(good) + "}");
      require(found.contains("\"value\":" + quote(good)) && Files.readAllLines(root.resolve("calls")).size() == before + 1,found);
      var aider = root.resolve("aider");
      Files.writeString(aider,"#!" + good + " -I\n");
      found = client.resume(client.call("repomap._aider_command_python",normal),"which","{\"value\":" + quote(aider) + "}");
      require(found.contains("\"value\":" + quote(good)),found);
      Files.writeString(aider,"#!/usr/bin/env python3\n");
      require(client.resume(client.call("repomap._aider_command_python",normal),"which","{\"value\":" + quote(aider) + "}").contains("\"value\":null"),"An env shebang became an interpreter");
      Files.writeString(stateFile,"{\"python\":true,\"map\":\"cached\"}");
      require(client.call("repomap.find_python",fallback).contains("interpreter must be text"),"A malformed interpreter was silently replaced");
      require(client.call("repomap.for_prompt",normal + ",\"prompt\":\"plain prompt\"").contains("\"cached\"]"),"An unused malformed interpreter broke cached context");
      Files.writeString(stateFile,"[]");
      require(client.call("repomap.read_state",normal).contains("\"value\":{}"),"Non-object state was accepted");
      try (var paths = Files.list(stateFile.getParent())) { require(paths.noneMatch(p -> p.toString().endsWith(".tmp")),"Temporary state file survived a successful replace"); }
      System.out.println("Native map state, interpreter precedence, Unicode identifiers and error bounds, ranked prompts, timeout fallback and refresh scheduling passed");
    } finally {
      try (var paths = Files.walk(root)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path); }
    }
  }
}
