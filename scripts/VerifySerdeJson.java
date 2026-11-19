//JAVA 25

import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HexFormat;
import java.util.Set;
import java.util.TreeSet;

/** Checks the vendored source against the checksum-pinned published crate. */
class VerifySerdeJson {
  static final String CRATE = "serde_json-1.0.151";
  static final String SHA256 = "c841b55ecdae098c80dcae9cf767f6f8a0c2cdb3416bbef72181df4d0fe73f14";
  static final Set<String> PATCHED = Set.of("Cargo.toml", "Cargo.toml.orig", "src/de.rs", "src/number.rs", "src/raw.rs", "src/ser.rs", "src/value/de.rs", "src/lexical/math.rs");
  public static void main(String[] args) throws Exception {
    if (args.length > 1) throw new IllegalArgumentException("usage: VerifySerdeJson.java [PUBLISHED_CRATE]");
    Path archive;
    if (args.length == 1) archive = Path.of(args[0]);
    else {
      var cache = Path.of(System.getenv().getOrDefault("CARGO_HOME", Path.of(System.getProperty("user.home"), ".cargo").toString()), "registry/cache");
      var candidates = new ArrayList<Path>();
      try (var registries = Files.list(cache)) {
        for (var registry : registries.toList()) {
          var candidate = registry.resolve(CRATE + ".crate");
          if (Files.isRegularFile(candidate)) candidates.add(candidate);
        }
      }
      if (candidates.size() != 1) throw new IllegalStateException("Pass the published " + CRATE + ".crate archive");
      archive = candidates.getFirst();
    }
    var digest = HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(Files.readAllBytes(archive)));
    if (!SHA256.equals(digest)) throw new AssertionError("Published crate checksum differs");
    var temporary = Files.createTempDirectory("te-serde-json-");
    int files = 0;
    try {
      if (new ProcessBuilder("tar", "-xzf", archive.toAbsolutePath().toString(), "-C", temporary.toString()).inheritIO().start().waitFor() != 0) throw new AssertionError("Cannot unpack published crate");
      var original = temporary.resolve(CRATE); var vendor = Path.of("vendor/serde_json");
      var changed = new TreeSet<String>(); var published = new TreeSet<String>();
      try (var paths = Files.walk(original)) {
        for (var file : paths.filter(Files::isRegularFile).toList()) {
          var relative = original.relativize(file); var name = relative.toString().replace('\\', '/');
          published.add(name); files++;
          if (!Files.isRegularFile(vendor.resolve(relative))) throw new AssertionError("Missing vendored file: " + name);
          if (Files.mismatch(file, vendor.resolve(relative)) != -1) changed.add(name);
        }
      }
      if (!changed.equals(PATCHED)) throw new AssertionError("Unexpected upstream changes: " + changed);
      try (var paths = Files.walk(vendor)) {
        for (var file : paths.filter(Files::isRegularFile).filter(path -> !path.startsWith(vendor.resolve("target"))).toList()) {
          var name = vendor.relativize(file).toString().replace('\\', '/');
          if (!published.contains(name) && !Set.of("UPSTREAM.md", "Cargo.lock").contains(name)) throw new AssertionError("Unexpected vendored file: " + name);
        }
      }
    } finally {
      try (var paths = Files.walk(temporary)) { for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.delete(path); }
    }
    System.out.println("Verified " + files + " published files, " + PATCHED.size() + " patched files and archive SHA-256 " + digest);
  }
}
