//JAVA 25
//SOURCES AdapterProbe.java ExtractProposerPrompts.java ParserProbe.java CliProbe.java ContextProbe.java ContainerProbe.java

import java.nio.file.Files;
import java.io.IOException;
import java.nio.file.FileVisitResult;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.nio.file.SimpleFileVisitor;
import java.nio.file.attribute.BasicFileAttributes;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.List;

/** Exercises source/native TemporaryDirectory cleanup with real file permissions and local agents. */
class CleanupProbe {
  static int checks,failures;
  static String q(Object value) { return ParserProbe.q(value); }
  static String shell(String value) { return "'" + value.replace("'", "'\\''") + "'"; }
  static void permissions(Path path,String mode) throws Exception { Files.setPosixFilePermissions(path,PosixFilePermissions.fromString(mode)); }
  static void child() throws Exception {
    var cwd = Path.of("").toAbsolutePath();
    var log = Path.of(System.getenv("TE_CLEANUP_LOG"));
    var kind = System.getenv("TE_CLEANUP_CASE");
    Files.writeString(log,cwd.toString());
    var outside = Files.createDirectory(log.resolveSibling("outside"));
    Files.writeString(outside.resolve("keep.txt"),"keep");
    permissions(outside.resolve("keep.txt"),"r--------");
    permissions(outside,"r-x------");
    if (kind.equals("root-symlink") || kind.equals("root-file")) {
      Files.move(cwd,log.resolveSibling("original"));
      if (kind.equals("root-file")) Files.writeString(cwd,"replacement file");
      else Files.createSymbolicLink(cwd,outside);
    } else if (kind.equals("root-missing")) {
      Files.delete(cwd);
    } else {
      var folder = Files.createDirectories(cwd.resolve("blocked/nested"));
      Files.writeString(folder.resolve("data"),"temporary");
      Files.createSymbolicLink(folder.resolve("external"),outside);
      Files.createSymbolicLink(folder.resolve("broken"),outside.resolve("missing"));
      switch (kind) {
        case "file-readonly" -> permissions(folder.resolve("data"),"r--------");
        case "directory-no-read" -> permissions(cwd.resolve("blocked"),"-wx------");
        case "directory-no-write" -> permissions(folder,"r-x------");
        case "directory-no-search" -> permissions(folder,"rw-------");
        case "directory-none" -> permissions(folder,"---------");
        case "root-none" -> permissions(cwd,"---------");
        case "multiple" -> {
          var other = Files.createDirectories(cwd.resolve("other/child"));
          Files.writeString(other.resolve("data"),"temporary");
          permissions(cwd.resolve("other"),"---------");
          permissions(folder,"r-x------");
        }
        case "deep-limited" -> {
          var deepest = cwd;
          for (int i=0;i<50;i++) deepest = Files.createDirectory(deepest.resolve("d"));
          Files.writeString(deepest.resolve("data"),"temporary");
          permissions(cwd,"---------");
        }
        case "base-locked" -> permissions(cwd.getParent(),"r-x------");
        case "immutable-locked-root", "immutable-locked-symlink" -> {
          var path = kind.equals("immutable-locked-root") ? cwd : folder.resolve("external");
          if (kind.equals("immutable-locked-root")) permissions(cwd,"---------");
          if (new ProcessBuilder("/bin/chflags","-h","uchg",path.toString()).inheritIO().start().waitFor()!=0) throw new AssertionError("Could not lock owned fixture");
          if (kind.equals("immutable-locked-symlink")) permissions(folder,"---------");
        }
        case "immutable-file", "immutable-directory", "immutable-symlink" -> {
          var path = kind.equals("immutable-file") ? folder.resolve("data") : kind.equals("immutable-directory") ? folder : folder.resolve("external");
          if (new ProcessBuilder("/bin/chflags","-h","uchg",path.toString()).inheritIO().start().waitFor()!=0) throw new AssertionError("Could not set the owned fixture's immutable flag");
        }
      }
    }
    System.out.print("{\"result\":\"ok\"}");
  }
  static void compare(AdapterProbe.Client nativeCode,AdapterProbe.Client source,Path root,Path cli,String api,String kind) throws Exception {
    var cases = Files.createDirectory(root.resolve(api+"-"+kind));
    var expected = "";
    var expectedExists = false;
    for (boolean nativeMode : List.of(false,true)) {
      var folder = Files.createDirectory(cases.resolve(nativeMode?"native":"source"));
      var log = folder.resolve("workspace");
      var request = "{\"api\":"+q(api)+",\"case\":"+q(kind)+",\"cli\":"+q(cli)+",\"root\":"+q(folder)+",\"log\":"+q(log)+"}";
      var actual = (nativeMode?nativeCode:source).call(request);
      var workspace = Path.of(Files.readString(log));
      var exists = Files.exists(workspace,LinkOption.NOFOLLOW_LINKS);
      if (!Files.readString(folder.resolve("outside/keep.txt")).equals("keep")
          || !Files.getPosixFilePermissions(folder.resolve("outside")).equals(PosixFilePermissions.fromString("r-x------"))
          || !Files.getPosixFilePermissions(folder.resolve("outside/keep.txt")).equals(PosixFilePermissions.fromString("r--------")))
        throw new AssertionError("Cleanup touched an external symlink target: "+kind);
      if (!nativeMode) {
        expected = actual; expectedExists = exists;
        var retained = List.of("root-symlink","root-file","base-locked").contains(kind)
            || api.equals("copilot") && List.of("directory-no-write","directory-none").contains(kind);
        if (exists!=retained) throw new AssertionError("Source fixture did not demonstrate expected cleanup: "+api+" "+kind+", retained="+exists);
      }
      else {
        if (!actual.equals(expected) || exists!=expectedExists) {
          System.out.println(api+" "+kind+": native="+actual+", retained="+exists+"; source="+expected+", retained="+expectedExists);
          failures++;
        }
        checks++;
      }
      if (kind.equals("base-locked")) {
        if (!Files.getPosixFilePermissions(workspace.getParent()).equals(PosixFilePermissions.fromString("r-x------"))) throw new AssertionError("Cleanup changed the temporary directory's parent permissions");
        permissions(workspace.getParent(),"rwx------");
      }
    }
  }
  static void clear(Path root) throws IOException {
    Files.setPosixFilePermissions(root,PosixFilePermissions.fromString("rwx------"));
    Files.walkFileTree(root,new SimpleFileVisitor<>() {
      public FileVisitResult preVisitDirectory(Path dir,BasicFileAttributes attrs) throws IOException {
        Files.setPosixFilePermissions(dir,PosixFilePermissions.fromString("rwx------"));
        return FileVisitResult.CONTINUE;
      }
      public FileVisitResult visitFile(Path file,BasicFileAttributes attrs) throws IOException { Files.delete(file); return FileVisitResult.CONTINUE; }
      public FileVisitResult visitFileFailed(Path file,IOException error) throws IOException {
        if (Files.isDirectory(file,LinkOption.NOFOLLOW_LINKS)) { clear(file); return FileVisitResult.CONTINUE; }
        throw error;
      }
      public FileVisitResult postVisitDirectory(Path dir,IOException error) throws IOException { if(error!=null)throw error; Files.delete(dir); return FileVisitResult.CONTINUE; }
    });
  }
  public static void main(String[] args) throws Exception {
    if (args.length>0 && args[0].equals("--child")) { child(); return; }
    var checkout = Path.of("").toAbsolutePath();
    var root = Files.createTempDirectory(checkout.resolve("validation/results"),"cleanup-");
    var base = Files.createDirectory(root.resolve("workspaces"));
    var cli = root.resolve("cursor-fixture");
    Files.writeString(cli,"#!/bin/sh\nexec "+shell(Path.of(System.getProperty("java.home"),"bin/java").toString())+" -cp "+shell(System.getProperty("java.class.path"))+" CleanupProbe --child \"$@\"\n");
    permissions(cli,"rwx------");
    boolean passed = false;
    try (var nativeCode = new AdapterProbe.Client(checkout,true,"cleanup_host.py","--base="+base);
         var source = new AdapterProbe.Client(checkout,false,"cleanup_host.py","--base="+base)) {
      for (var kind : List.of("normal","file-readonly","directory-no-read","directory-no-write","directory-no-search","directory-none","root-none","multiple","root-symlink","root-file","root-missing","base-locked")) compare(nativeCode,source,root,cli,"cursor",kind);
      if (System.getProperty("os.name").equals("Mac OS X"))
        for (var kind : List.of("immutable-file","immutable-directory","immutable-symlink","immutable-locked-root","immutable-locked-symlink")) compare(nativeCode,source,root,cli,"cursor",kind);
      for (var kind : List.of("normal","file-readonly","directory-no-write","directory-none","root-symlink")) compare(nativeCode,source,root,cli,"copilot",kind);
      try (var limitedNative = new AdapterProbe.Client(checkout,true,"cleanup_host.py","--base="+base,"--limit=64");
           var limitedSource = new AdapterProbe.Client(checkout,false,"cleanup_host.py","--base="+base,"--limit=64")) {
        compare(limitedNative,limitedSource,root,cli,"cursor","deep-limited");
      }
      if (failures!=0) throw new AssertionError(failures+" cleanup mismatches in "+checks+" cases");
      passed = true;
    } finally { if (passed) clear(root); else System.err.println("Probe artifacts: "+root); }
    System.out.println("Temporary-directory cleanup matches source in "+checks+" permission, flag and symlink cases; external data and permissions are unchanged");
  }
}
