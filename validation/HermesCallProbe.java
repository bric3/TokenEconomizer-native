//JAVA 25
//SOURCES ContextProbe.java ProcessInputProbe.java

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Comparator;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import java.util.regex.Pattern;

/** Native Hermes charging and recovery against a local CLI fixture and SQLite database. */
class HermesCallProbe {
  static final String PROMPT = "Native é😀\n&%PATH% a'b";
  static String quote(Object value) { return ProcessInputProbe.quote(value); }
  static String literal(String value) { return "'" + value.replace("'", "''") + "'"; }
  static void require(boolean ok, String message) { if (!ok) throw new AssertionError(message); }
  static String sql(Path db, String query) throws Exception {
    var child = new ProcessBuilder("/usr/bin/sqlite3", "-cmd", ".timeout 5000", db.toString()).redirectError(ProcessBuilder.Redirect.INHERIT).start();
    child.getOutputStream().write(query.getBytes(StandardCharsets.UTF_8));
    child.getOutputStream().close();
    var result = new String(child.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
    require(child.waitFor(5, TimeUnit.SECONDS) && child.exitValue() == 0, "SQLite fixture failed");
    return result.trim();
  }
  static void fixture(String[] args) throws Exception {
    var root = Path.of(System.getenv("PROBE_ROOT"));
    var db = Path.of(System.getenv("HERMES_HOME"), "state.db");
    require(System.getenv("TE_SIDECALL") == null && System.getenv("TE_HERMES_MODE") == null && System.getenv("PYTHONPATH") == null, "The gateway inherited a hook override");
    var argv = Arrays.asList(args).subList(1, args.length);
    if (argv.getFirst().equals("sessions")) {
      require(argv.subList(0, 3).equals(List.of("sessions", "delete", "--yes")), argv.toString());
      var sid = argv.getLast();
      var count = sql(db, "SELECT COUNT(*) FROM session_model_usage WHERE session_id=" + literal(sid) + ";");
      Files.writeString(root.resolve("deleted"), sid + ":" + count + "\n", StandardOpenOption.CREATE, StandardOpenOption.APPEND);
      if (!Files.exists(root.resolve("keep"))) sql(db, "DELETE FROM session_model_usage WHERE session_id=" + literal(sid) + "; DELETE FROM sessions WHERE id=" + literal(sid) + ";");
      return;
    }
    require(argv.subList(0, 10).equals(List.of("chat", "-Q", "--oneshot", "--source", "tool", "--ignore-rules", "--max-turns", "1", "-t", "bot_room")), argv.toString());
    require(argv.get(argv.indexOf("--model") + 1).equals("small"), "The model changed");
    var query = Path.of(argv.get(argv.indexOf("--query-file") + 1));
    var cwd = Path.of("").toAbsolutePath();
    require(Files.isSameFile(cwd, query.getParent()) && Files.getPosixFilePermissions(cwd).equals(PosixFilePermissions.fromString("rwx------")), "The prompt workspace is not private");
    Files.writeString(root.resolve("cwd"), cwd.toString());
    var prompt = Files.readString(query);
    require(!argv.contains(prompt), "The prompt reached argv");
    var sid = prompt.equals(PROMPT) ? "side" : prompt;
    var replyModel = sid.equals("side") ? "claude-haiku-4-5" : "small";
    var titleModel = sid.equals("side") ? "gpt-5.4" : "cheap";
    double now = System.currentTimeMillis() / 1000.0;
    sql(db, "INSERT INTO sessions VALUES (" + literal(sid) + ",'tool','tool'," + now + "," + (sid.equals("late") ? "NULL" : now) + "," + (sid.equals("delayed") ? "NULL" : "'title'") + ");"
        + "INSERT INTO messages VALUES (" + literal(sid) + ",'user'," + literal(prompt) + ");"
        + "INSERT INTO session_model_usage VALUES (" + literal(sid) + "," + literal(replyModel) + ",'',10,3,1,2);"
        + (sid.equals("delayed") ? "" : "INSERT INTO session_model_usage VALUES (" + literal(sid) + "," + literal(titleModel) + ",'title_generation',1,0,0,1);"));
    if (sid.equals("late")) { Thread.sleep(10_000); return; }
    if (sid.equals("delayed")) {
      new ProcessBuilder(Path.of(System.getProperty("java.home"), "bin/java").toString(), "-cp", System.getProperty("java.class.path"), "HermesCallProbe", "--title", db.toString())
          .directory(root.toFile()).redirectOutput(ProcessBuilder.Redirect.DISCARD).redirectError(ProcessBuilder.Redirect.DISCARD).start();
    }
    System.out.print("hermes reply");
    System.err.print("note\n\nsession_id: " + sid + "\n");
  }
  static double charge(String result) {
    var match = Pattern.compile("\\\"value\\\":\\[\\\"hermes reply\\\",([^\\]]+)\\]").matcher(result);
    require(match.find(), result);
    return Double.parseDouble(match.group(1));
  }
  public static void main(String[] args) throws Exception {
    if (args.length > 0 && args[0].equals("--title")) {
      Thread.sleep(350);
      sql(Path.of(args[1]), "INSERT INTO session_model_usage VALUES ('delayed','cheap','title_generation',1,0,0,1);");
      return;
    }
    if (args.length > 0 && args[0].equals("--fixture")) { fixture(args); return; }
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"), "hermes-call-");
    var home = Files.createDirectory(root.resolve("home"));
    var db = home.resolve("state.db");
    sql(db, "CREATE TABLE sessions (id TEXT PRIMARY KEY, source TEXT, created_source TEXT, started_at REAL, ended_at REAL, title TEXT);"
        + "CREATE TABLE messages (session_id TEXT, role TEXT, content TEXT);"
        + "CREATE TABLE session_model_usage (session_id TEXT, model TEXT, task TEXT, input_tokens INTEGER, cache_read_tokens INTEGER, cache_write_tokens INTEGER, output_tokens INTEGER);");
    var cli = root.resolve("fixture-hermes");
    var java = Path.of(System.getProperty("java.home"), "bin/java");
    Files.writeString(cli, "#!/bin/sh\nexec '" + java.toString().replace("'", "'\\''") + "' -cp '" + System.getProperty("java.class.path").replace("'", "'\\''") + "' HermesCallProbe --fixture \"$@\"\n");
    Files.setPosixFilePermissions(cli, PosixFilePermissions.fromString("rwx------"));
    var common = "\"cli\":" + quote(cli) + ",\"model\":\"small\",\"root\":" + quote(root.resolve("state")) + ",\"home\":" + quote(root)
        + ",\"process_cwd\":" + quote(root) + ",\"hermes_limits\":{\"title_wait\":0},\"env\":{\"HERMES_HOME\":" + quote(home)
        + ",\"PROBE_ROOT\":" + quote(root) + ",\"TE_SIDECALL\":\"1\",\"TE_HERMES_MODE\":\"cli\",\"PYTHONPATH\":\"private\"}";
    try (var client = new ProcessInputProbe.Client(checkout.resolve("target/debug/te-bridge"))) {
      var first = client.call("sidecall.hermes_call", common + ",\"prompt\":" + quote(PROMPT));
      require(charge(first) == 28.55, first);
      ContextProbe.expect(Files.readString(root.resolve("deleted")), "side:2\n");
      require(sql(db, "SELECT COUNT(*) FROM sessions;").equals("0"), "The priced session was retained");
      require(!Files.exists(Path.of(Files.readString(root.resolve("cwd")))), "The completed workspace was retained");
      var delayed = client.call("sidecall.hermes_call", common.replace("\"title_wait\":0", "\"title_wait\":2") + ",\"prompt\":\"delayed\"");
      require(charge(delayed) == 30.55 && Files.readString(root.resolve("deleted")).contains("delayed:2\n"), "Late title usage was lost: " + delayed);
      var timed = client.call("sidecall.hermes_call", common + ",\"prompt\":\"late\",\"timeout\":0.5");
      require(timed.contains("\"value\":[\"\",null]"), timed);
      var ledger = root.resolve("state/hosts/hermes/sidecalls.json");
      require(Files.readString(ledger).contains("\"charged\": false") && sql(db, "SELECT COUNT(*) FROM sessions WHERE id='late';").equals("1"), "A timed-out session lost uncharged usage");
      require(!Files.exists(Path.of(Files.readString(root.resolve("cwd")))), "The timed-out workspace was retained");
      sql(db, "UPDATE sessions SET ended_at=1 WHERE id='late';");
      var next = client.call("sidecall.hermes_call", common + ",\"prompt\":\"next\"");
      require(charge(next) == 61.1, next);
      require(Files.readString(root.resolve("deleted")).contains("late:2\n"), "Deferred usage was not read before deletion");
      Files.writeString(root.resolve("keep"), "keep");
      var stuck = client.call("sidecall.hermes_call", common + ",\"prompt\":\"stuck\"");
      require(charge(stuck) == 30.55 && Files.readString(ledger).contains("\"charged\": true"), stuck);
      Files.delete(root.resolve("keep"));
      var last = client.call("sidecall.hermes_call", common + ",\"prompt\":\"last\"");
      require(charge(last) == 30.55 && sql(db, "SELECT COUNT(*) FROM sessions;").equals("0"), "A failed delete was charged twice: " + last);
      var start = new CountDownLatch(1);
      try (var pool = Executors.newVirtualThreadPerTaskExecutor()) {
        var jobs = new ArrayList<java.util.concurrent.Future<?>>();
        for (int i = 0; i < 8; i++) {
          int n = i;
          jobs.add(pool.submit(() -> {
            start.await();
            try (var other = new ProcessInputProbe.Client(checkout.resolve("target/debug/te-bridge"))) {
              var result = other.call("sidecall._hermes_defer", common + ",\"session_id\":\"parallel-" + n + "\",\"left\":{\"charged\":false}");
              require(result.contains("\"value\":null"), result);
            }
            return null;
          }));
        }
        start.countDown();
        for (var job : jobs) job.get(15, TimeUnit.SECONDS);
      }
      require(Pattern.compile("\\\"charged\\\": false").matcher(Files.readString(ledger)).results().count() == 8, "Concurrent ledger updates were lost");
      System.out.println("Native Hermes query files, per-model pricing, charge-before-delete, timeout recovery, once-only charging and ledger locking passed");
    } finally {
      try (var paths = Files.walk(root)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path); }
    }
  }
}
