//JAVA 25

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.HexFormat;

/** Verifies every upstream file against the pinned Git tree, including symlinks. */
class VerifyReference {
  public static void main(String[] args) throws Exception {
    var root = Path.of(args.length == 0 ? "." : args[0]).toAbsolutePath().normalize();
    var snapshot = root.resolve("reference/upstream");
    var count = 0;

    for (var entry : Files.readAllLines(root.resolve("reference/tree.txt"))) {
      var fields = entry.split("[ \\t]", 4);
      var path = snapshot.resolve(fields[3]);
      var bytes = Files.isSymbolicLink(path)
          ? Files.readSymbolicLink(path).toString().getBytes(StandardCharsets.UTF_8)
          : Files.readAllBytes(path);

      // Upstream .gitattributes exports *.cmd with CRLF; Git blobs store LF.
      if (fields[3].endsWith(".cmd")) {
        bytes = new String(bytes, StandardCharsets.UTF_8).replace("\r\n", "\n")
            .getBytes(StandardCharsets.UTF_8);
      }
      var digest = MessageDigest.getInstance("SHA-1");
      digest.update(("blob " + bytes.length + "\0").getBytes(StandardCharsets.US_ASCII));
      var actual = HexFormat.of().formatHex(digest.digest(bytes));

      if (!actual.equals(fields[2])) {
        throw new AssertionError("Upstream content changed: " + fields[3]);
      }
      count++;
    }

    if (count == 0) {
      throw new AssertionError("The upstream tree manifest is empty");
    }
    System.out.println("Verified " + count + " upstream files against the pinned Git tree");
  }
}
