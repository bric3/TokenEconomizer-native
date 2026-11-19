//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java NativeCliProbe.java CliProbe.java ContextProbe.java ParserProbe.java ContainerProbe.java LaunchProbe.java

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.ArrayList;
import java.util.Base64;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

/** Native/source scanner, deletion and standalone developer-tool checks. */
class DeadCodeProbe {
  static Path checkout, root;
  static int checks;
  static String q(Object value) { return ParserProbe.q(value); }
  static void require(boolean condition, String message) { if (!condition) throw new AssertionError(message); checks++; }
  static void git(Path repo, String... args) throws Exception {
    var command = new ArrayList<>(List.of(LaunchProbe.tool("git").toString(), "-C", repo.toString())); command.addAll(List.of(args));
    var result = NativeCliProbe.process(repo, Map.of("PATH", "/usr/bin:/bin", "GIT_CONFIG_GLOBAL", "/dev/null", "GIT_CONFIG_NOSYSTEM", "1"), command);
    if (result.code() != 0) throw new AssertionError(result);
  }
  static Path fixture(Map<String,String> files) throws Exception {
    var repo = Files.createTempDirectory(root, "repo-");
    for (var file : files.entrySet()) { var path = repo.resolve(file.getKey()); Files.createDirectories(path.getParent()); Files.writeString(path, file.getValue()); }
    git(repo, "init", "-q"); git(repo, "add", "."); return repo;
  }
  static Map<String,String> contents(Path repo) throws Exception {
    var result = new TreeMap<String,String>();
    try (var paths = Files.walk(repo)) {
      for (var path : paths.filter(Files::isRegularFile).filter(path -> !path.startsWith(repo.resolve(".git"))).toList())
        result.put(repo.relativize(path).toString(), Base64.getEncoder().encodeToString(Files.readAllBytes(path)));
    }
    return result;
  }
  static String call(AdapterProbe.Client client, Path repo, String operation, String extra) throws Exception {
    return client.call("{\"root\":" + q(repo) + ",\"operation\":" + q(operation) + extra + "}");
  }
  static String compare(AdapterProbe.Client nativeCode, AdapterProbe.Client source, Path repo, String operation, String extra) throws Exception {
    var before = contents(repo);
    var expected = call(source, repo, operation, extra); var expectedFiles = contents(repo);
    for (var file : before.entrySet()) Files.write(repo.resolve(file.getKey()), Base64.getDecoder().decode(file.getValue()));
    var actual = call(nativeCode, repo, operation, extra);
    require(actual.equals(expected), "Dead-code " + operation + " differs\nNative: " + actual + "\nSource: " + expected);
    require(contents(repo).equals(expectedFiles), "Dead-code file edits differ\nNative: " + contents(repo) + "\nSource: " + expectedFiles);
    return actual;
  }
  static String scanFix(AdapterProbe.Client nativeCode, AdapterProbe.Client source, String code) throws Exception {
    var repo = fixture(Map.of("scripts/tool.py", code));
    var scanned = compare(nativeCode, source, repo, "scan", "");
    compare(nativeCode, source, repo, "fix", "");
    compare(nativeCode, source, repo, "sweep", "");
    return scanned;
  }
  static void reports(AdapterProbe.Client nativeCode, AdapterProbe.Client source) throws Exception {
    var repo = fixture(Map.of("scripts/café.py", "VALUE='😀'\ndef orphan(): pass\n"));
    for (var args : List.of("[\"--root\"," + q(repo) + ",\"--json\"]", "[\"--ro=" + repo + "\",\"--js\"]", "[\"--root\"," + q(repo) + ",\"--fix\",\"--json\"]"))
      compare(nativeCode, source, repo, "main", ",\"argv\":" + args);
    for (var args : List.of("[\"--help\"]", "[\"-h\"]", "[\"--unknown\"]", "[\"--root\"]", "[\"--root\",\"--json\"]", "[\"--json=1\"]", "[\"--\"]", "[\"--\",\"--json\"]"))
      compare(nativeCode, source, repo, "main", ",\"argv\":" + args);
    for (var items : List.of("[]", "[{\"kind\":\"constant\",\"status\":\"unreferenced\",\"path\":\"a.py\",\"line\":1,\"name\":\"A\",\"removable\":false},{\"kind\":\"function\",\"status\":\"test-only\",\"path\":\"a.py\",\"line\":2,\"name\":\"used\",\"removable\":true},{\"kind\":\"method\",\"status\":\"unreferenced\",\"path\":\"b.py\",\"line\":3,\"name\":\"Used.orphan\",\"removable\":true,\"removed\":true,\"note\":\"reason\"}]")) {
      compare(nativeCode, source, repo, "counts", ",\"arguments\":{\"items\":" + items + "}");
      for (boolean fixed : List.of(false, true)) {
        var request = "{\"operation\":\"markdown\",\"arguments\":{\"items\":" + items + ",\"fixed\":" + fixed + "}}";
        // Table padding follows this repository's Markdown convention.
        var expected = source.call(request).replaceAll(" *\\| *", "|").replaceAll("-{3,}", "---");
        var actual = nativeCode.call(request).replaceAll(" *\\| *", "|").replaceAll("-{3,}", "---");
        require(actual.equals(expected), "Markdown report differs\nNative: " + actual + "\nSource: " + expected);
      }
    }
    scanFix(nativeCode, source, "VALUE=0x" + "f".repeat(2048) + "\n");
    scanFix(nativeCode, source, "VALUE=" + "a+".repeat(300) + "a\n");
  }
  static void safety(AdapterProbe.Client nativeCode, AdapterProbe.Client source) throws Exception {
    var decorated = "from dataclasses import dataclass\n\n@dataclass\nclass Unused:\n    def orphan(self):\n        pass\n    pass\n\n\ndef live():\n    pass\n\nlive()\n";
    var repo = fixture(Map.of("scripts/tool.py", decorated));
    compare(nativeCode, source, repo, "scan", "");
    var fixed = compare(nativeCode, source, repo, "fix", "");
    require(Files.readString(repo.resolve("scripts/tool.py")).equals("from dataclasses import dataclass\n\n\ndef live():\n    pass\n\nlive()\n") && !fixed.contains("\"raised\""), "Native decorated-class deletion lost a live definition: " + fixed);
    var shared = "A = 1; B = 2\nprint(B)\n";
    repo = fixture(Map.of("scripts/tool.py", shared)); compare(nativeCode, source, repo, "scan", "");
    var old = call(source, repo, "fix", "");
    require(Files.readString(repo.resolve("scripts/tool.py")).equals("print(B)\n"), "Source shared-line regression did not reproduce: " + old);
    Files.writeString(repo.resolve("scripts/tool.py"), shared);
    fixed = call(nativeCode, repo, "fix", "");
    require(Files.readString(repo.resolve("scripts/tool.py")).equals(shared) && fixed.contains("same line"), "Native deletion removed a referenced neighbouring statement: " + fixed);
    for (var chained : List.of("A = B = 1\nprint(B)\n", "A = B = 1\nprint('live')\n")) {
      repo = fixture(Map.of("scripts/tool.py", chained)); compare(nativeCode, source, repo, "scan", "");
      fixed = compare(nativeCode, source, repo, "fix", "");
      require(Files.readString(repo.resolve("scripts/tool.py")).equals(chained), "Native deletion removed a chained binding: " + fixed);
    }
  }
  static void standalone() throws Exception {
    var work = fixture(Map.of("scripts/tool.py", "def orphan():\n    pass\n\ndef main():\n    pass\n\nmain()\n"));
    var bin = Files.createDirectory(root.resolve("relocated tool's directory")); var tool = bin.resolve("te-dead-code");
    Files.copy(checkout.resolve("target/debug/te-dead-code"), tool, StandardCopyOption.COPY_ATTRIBUTES);
    var env = new java.util.HashMap<>(Map.of("HOME", work.toString(), "PATH", "", "TE_PYTHON", "/missing-python"));
    var help = NativeCliProbe.process(work, env, List.of(tool.toString(), "--help"));
    require(help.code() == 0 && help.out().contains("--fix"), "Help required Python, Git or a checkout: " + help);
    var invalid = NativeCliProbe.process(work, env, List.of(tool.toString(), "--unknown"));
    require(invalid.code() == 2, "Argument refusal required Python: " + invalid);
    env.put("TE_PYTHON", checkout.resolve(".venv/bin/python").toString()); env.put("PATH", LaunchProbe.tool("git").getParent().toString());
    var scanned = NativeCliProbe.process(work, env, List.of(tool.toString(), "--json"));
    require(scanned.code() == 0 && scanned.out().contains("\"name\": \"orphan\""), "Relocated scanner depended on source files: " + scanned);
    Files.setPosixFilePermissions(work.resolve("scripts/tool.py"), PosixFilePermissions.fromString("rwx------"));
    var fixed = NativeCliProbe.process(work, env, List.of(tool.toString(), "--fix", "--json"));
    require(fixed.code() == 0 && !Files.readString(work.resolve("scripts/tool.py")).contains("orphan") && Files.isExecutable(work.resolve("scripts/tool.py")), "Relocated fix lost executable mode or failed: " + fixed);
    var checked = NativeCliProbe.process(work, env, List.of(tool.toString(), "--json"));
    require(checked.code() == 0 && checked.out().contains("\"items\": []"), "Fixed repository did not become clean: " + checked);
  }
  public static void main(String[] args) throws Exception {
    checkout = Path.of("").toAbsolutePath(); root = Files.createTempDirectory(checkout.resolve("validation/results"), "dead-code-"); boolean passed = false;
    try (var nativeCode = new AdapterProbe.Client(checkout, true, "dead_code_host.py"); var source = new AdapterProbe.Client(checkout, false, "dead_code_host.py")) {
      for (var code : List.of("", "# no definitions\n", "def unused():\n    pass\n", "async def unused():\n    pass\n", "def used():\n    pass\nused()\n", "class Empty:\n    pass\n", "class Used:\n    def only(self):\n        pass\nprint(Used)\n", "class Used:\n    'doc'\n    def a(self): pass\n    def b(self): pass\nprint(Used)\n", "class Used:\n    1\n    def a(self): pass\nprint(Used)\n", "@registry.register\ndef hook(): pass\n", "@cache()\ndef old(): pass\n", "__all__=['public']\ndef public(): pass\ndef orphan(): pass\n", "__all__=[]\n__all__ += ['public']\ndef public(): pass\n", "(X,Y)=(1,2)\n_CONST=3\n__OTHER=4\n", "CALLED=f()\nLAMBDA=lambda: f()\n", "X: int\n", "X: int=3\n", "def invalid(:\n", "def café(): pass\n", "__all__=['café']\ndef café(): pass\n", "VALUE='😀é'\n", "VALUE='\\ud800'\n", "VALUE=b'abc'\n", "VALUE=1e999\n", "VALUE=3j\n", "# comment\r\ndef unused():\r\n    pass\r\n\r\n", "# comment\ndef unused():\n    pass", "def a():\n    pass\n\n\ndef b():\n    return a()\n", "class Base(Child):\n    def one(self): pass\nclass Child(Base):\n    def two(self): pass\nprint(Base,Child)\n")) scanFix(nativeCode, source, code);
      for (var base : List.of("json.JSONEncoder", "collections.abc.Mapping", "logging.handlers.RotatingFileHandler", "http.server.BaseHTTPRequestHandler", "cmd.Cmd", "ast.NodeVisitor", "ast.NodeTransformer", "unknown.NodeVisitor", "dict[str,str]"))
        scanFix(nativeCode, source, "import json, collections.abc, logging.handlers, http.server, cmd, ast\nclass Used(" + base + "):\n    def default(self, value): pass\n    def do_CUSTOM(self): pass\n    def visit_Custom(self): pass\n    def help_custom(self): pass\n    def stray(self): pass\nprint(Used)\n");
      for (var dynamic : List.of("getattr(self,kind)", "getattr(cls,'on_'+kind)", "getattr(self,'fixed')")) scanFix(nativeCode,source,"class Used:\n    def handle(self,kind): return " + dynamic + "()\n    def on_start(self): pass\n    def fixed(self): pass\nprint(Used().handle)\n");
      var repo = fixture(Map.of("scripts/a.py", "def exported(): pass\ndef tested(): pass\ndef unused(): pass\n", "docs/note.txt", "exported\n", "tests/test_a.py", "tested\n", "scripts/dead_code_allowlist.txt", "scripts/a.py::unused\u2028"));
      compare(nativeCode, source, repo, "scan", "");
      var invalidUtf = fixture(Map.of("scripts/tool.py", "def orphan(): pass\n", "data.bin", "placeholder")); Files.write(invalidUtf.resolve("data.bin"),new byte[]{(byte)0xff,0,1});
      compare(nativeCode,source,invalidUtf,"scan","");
      var symlink = fixture(Map.of("scripts/tool.py", "def orphan(): pass\n")); Files.createSymbolicLink(symlink.resolve("alias.py"),Path.of("scripts/tool.py")); git(symlink,"add","alias.py"); compare(nativeCode,source,symlink,"scan","");
      for (int limit : List.of(-1,0,1,2,20)) {
        repo=fixture(Map.of("scripts/tool.py","HELPER=1\ndef helper(): return HELPER\ndef orphan(): return helper()\ndef main(): pass\nmain()\n"));
        compare(nativeCode,source,repo,"sweep",",\"max_passes\":"+limit);
      }
      safety(nativeCode,source); reports(nativeCode,source); standalone(); passed=true;
      System.out.println("Verified " + checks + " dead-code source comparisons, file edits, deletion safeguards and relocated CLI checks");
    } finally {if(passed)ContainerProbe.clear(root);else System.err.println("Dead-code fixtures: " + root);}
  }
}
