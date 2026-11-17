//JAVA 25

import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HexFormat;
import java.util.List;
import java.util.Locale;
import java.util.regex.Pattern;

/** Copies legal texts from every locked registry crate, including build and platform dependencies. */
class ThirdPartyNotices {
  static String field(String text,String key) {
    var match=Pattern.compile("(?m)^"+Pattern.quote(key)+" = \"([^\"]*)\"$").matcher(text);
    return match.find()?match.group(1):"";
  }
  static boolean legal(Path path) {
    var name=path.getFileName().toString().toUpperCase(Locale.ROOT);
    return name.startsWith("LICENSE") || name.startsWith("LICENCE") || name.startsWith("COPYING") || name.startsWith("NOTICE") || name.startsWith("COPYRIGHT");
  }
  public static void main(String[] args) throws Exception {
    if(args.length>1 || args.length==1 && !args[0].equals("--check"))throw new IllegalArgumentException("usage: ThirdPartyNotices.java [--check]");
    var root=Path.of("").toAbsolutePath();var lock=root.resolve("Cargo.lock");
    var cache=Path.of(System.getenv().getOrDefault("CARGO_HOME",Path.of(System.getProperty("user.home"),".cargo").toString()),"registry/src");
    List<Path> registries;
    try(var directories=Files.list(cache)){registries=directories.filter(Files::isDirectory).sorted().toList();}
    var hash=HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(Files.readAllBytes(lock)));
    var text=new StringBuilder("Third-party notices for TokenEconomizer native\n\nCargo.lock SHA-256: "+hash+"\n\nThese texts cover every registry crate in the lockfile, including build and\nplatform dependencies. Some crates may not be linked into a particular binary.\nEach text below is copied from the corresponding published crate.\n");
    var packages=Pattern.compile("(?ms)^\\[\\[package\\]\\]\\n(.*?)(?=^\\[\\[package\\]\\]|\\z)").matcher(Files.readString(lock));
    int count=0;
    while(packages.find()) {
      var section=packages.group(1);
      var origin=field(section,"source");
      if(origin.isEmpty())continue;
      if(!origin.startsWith("registry+"))throw new IllegalStateException("Unsupported dependency source: "+origin);
      var id=field(section,"name")+"-"+field(section,"version");
      var candidates=registries.stream().map(registry->registry.resolve(id)).filter(Files::isDirectory).toList();
      if(candidates.size()!=1)throw new IllegalStateException("Expected one cached source for "+id+"; run cargo fetch --locked");
      var source=candidates.getFirst();var manifest=Files.readString(source.resolve("Cargo.toml"));
      var files=new ArrayList<Path>();
      try(var paths=Files.list(source)){files.addAll(paths.filter(Files::isRegularFile).filter(ThirdPartyNotices::legal).toList());}
      for(var name:List.of("license","licenses","licence","licences","LICENSES")) {
        var directory=source.resolve(name);
        if(Files.isDirectory(directory))try(var paths=Files.walk(directory)){files.addAll(paths.filter(Files::isRegularFile).toList());}
      }
      var named=field(manifest,"license-file");
      if(!named.isEmpty()) {
        var file=source.resolve(named).normalize();
        if(!file.startsWith(source) || !Files.isRegularFile(file))throw new IllegalStateException("Invalid license-file for "+id);
        files.add(file);
      }
      var ordered=files.stream().distinct().sorted(Comparator.comparing(source::relativize)).toList();
      if(ordered.isEmpty())throw new IllegalStateException("No legal text found for "+id);
      text.append("\n===============================================================================\n").append(id).append("\nDeclared licence: ").append(field(manifest,"license")).append('\n');
      for(var file:ordered)text.append("\n--- ").append(source.relativize(file).toString().replace('\\','/')).append(" ---\n").append(Files.readString(file)).append('\n');
      count++;
    }
    if(count==0)throw new IllegalStateException("No registry packages found in Cargo.lock");
    var output=root.resolve("src/package/THIRD-PARTY-NOTICES.txt");
    if(args.length==1) {
      if(!Files.readString(output).contentEquals(text))throw new IllegalStateException("Third-party notices are stale; run jbang scripts/ThirdPartyNotices.java");
    } else Files.writeString(output,text);
    System.out.println("Verified legal texts for "+count+" locked registry crates"+(args.length==0?"; wrote "+root.relativize(output):""));
  }
}
