//JAVA 25

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.file.FileVisitResult;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.SimpleFileVisitor;
import java.nio.file.StandardCopyOption;
import java.nio.file.attribute.BasicFileAttributes;
import java.security.MessageDigest;
import java.util.Comparator;
import java.util.HexFormat;
import java.util.List;

/** Builds static Linux helpers from a minimal Docker context, without host toolchain changes. */
class BuildContainerHelpers {
  static void copyTree(Path source,Path destination)throws Exception {
    Files.walkFileTree(source,new SimpleFileVisitor<>() {
      @Override public FileVisitResult preVisitDirectory(Path directory,BasicFileAttributes attributes)throws java.io.IOException {
        Files.createDirectories(destination.resolve(source.relativize(directory)));return FileVisitResult.CONTINUE;
      }
      @Override public FileVisitResult visitFile(Path file,BasicFileAttributes attributes)throws java.io.IOException {
        Files.copy(file,destination.resolve(source.relativize(file)));return FileVisitResult.CONTINUE;
      }
    });
  }
  static void verifyElf(Path binary,String architecture)throws Exception {
    var bytes=Files.readAllBytes(binary);var elf=ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN);
    if(bytes.length<64||elf.getInt(0)!=0x464c457f||bytes[4]!=2||bytes[5]!=1)throw new AssertionError("expected a 64-bit little-endian ELF helper");
    if(Short.toUnsignedInt(elf.getShort(18))!=(architecture.equals("amd64")?62:183))throw new AssertionError("helper architecture differs from its directory");
    long offset=elf.getLong(32);int size=Short.toUnsignedInt(elf.getShort(54)),count=Short.toUnsignedInt(elf.getShort(56));
    for(int index=0;index<count;index++)if(elf.getInt(Math.toIntExact(offset+(long)index*size))==3)throw new AssertionError("helper requires a dynamic interpreter");
  }
  public static void main(String[] args)throws Exception {
    var root=Path.of("").toAbsolutePath();var output=Files.createDirectories(root.resolve("target/container-helpers"));
    var architectures=args.length==0?List.of("arm64","amd64"):List.of(args);
    for(var architecture:architectures)if(!List.of("arm64","amd64").contains(architecture))throw new IllegalArgumentException("usage: BuildContainerHelpers.java [arm64|amd64 ...]");
    var context=Files.createTempDirectory(output,"source-");
    try {
      Files.copy(root.resolve("Cargo.toml"),context.resolve("Cargo.toml"));Files.copy(root.resolve("Cargo.lock"),context.resolve("Cargo.lock"));
      Files.copy(root.resolve("LICENSE"),context.resolve("LICENSE"));
      copyTree(root.resolve("vendor/serde_json/src"),context.resolve("vendor/serde_json/src"));
      for(var name:List.of("Cargo.toml","build.rs"))Files.copy(root.resolve("vendor/serde_json").resolve(name),context.resolve("vendor/serde_json").resolve(name));
      copyTree(root.resolve("src"),context.resolve("src"));Files.createDirectory(context.resolve("validation"));
      Files.copy(root.resolve("validation/console_probe.rs"),context.resolve("validation/console_probe.rs"));
      Files.copy(root.resolve("scripts/container-helpers.Dockerfile"),context.resolve("Dockerfile"));
      for(var architecture:architectures) {
        var destination=output.resolve(architecture);
        var process=new ProcessBuilder("docker","build","--platform","linux/"+architecture,"--output","type=local,dest="+destination,context.toString()).directory(root.toFile()).inheritIO().start();
        if(process.waitFor()!=0)throw new AssertionError("container helper build failed: "+architecture);
        var binary=destination.resolve("te-net-guard");verifyElf(binary,architecture);
        var digest=HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(Files.readAllBytes(binary)));
        var lock=HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(Files.readAllBytes(context.resolve("Cargo.lock"))));
        var notices=context.resolve("src/package/THIRD-PARTY-NOTICES.txt");
        if(!Files.readString(notices).contains("Cargo.lock SHA-256: "+lock))throw new AssertionError("third-party notices differ from the compiled lockfile");
        Files.copy(notices,destination.resolve("THIRD-PARTY-NOTICES.txt"),StandardCopyOption.REPLACE_EXISTING);
        Files.writeString(destination.resolve("manifest.json"),"{\n  \"platform\": \"linux/"+architecture+"\",\n  \"binary\": \"te-net-guard\",\n  \"sha256\": \""+digest+"\",\n  \"cargo_lock_sha256\": \""+lock+"\"\n}\n");
        System.out.println("Built static linux/"+architecture+" helper: "+binary);
      }
    }finally{try(var paths=Files.walk(context)){for(var path:paths.sorted(Comparator.reverseOrder()).toList())Files.delete(path);}}
  }
}
