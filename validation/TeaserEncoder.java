//JAVA 25

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.HexFormat;
import java.util.List;

/** Records actual encoder inputs; native runs additionally invoke the real ffmpeg. */
class TeaserEncoder {
  public static void main(String[] args)throws Exception {
    var arguments=List.of(args);var input=Path.of(arguments.get(arguments.indexOf("-i")+1)).getParent();var output=Path.of(args[args.length-1]);
    var root=Path.of(System.getenv("TE_FFMPEG_CAPTURE"));Files.createDirectories(root);
    Files.writeString(root.resolve("scratch.txt"),input.toString());
    var frames=Files.createDirectories(root.resolve("frames"));var manifest=new StringBuilder();
    try(var paths=Files.list(input)) {
      for(var path:paths.filter(path->path.toString().endsWith(".png")).sorted().toList()) {
        var bytes=Files.readAllBytes(path);var hash=HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
        manifest.append(path.getFileName()).append(' ').append(hash).append('\n');
        Files.copy(path,frames.resolve(path.getFileName()),StandardCopyOption.REPLACE_EXISTING);
      }
    }
    Files.writeString(root.resolve("frames.sha256"),manifest);
    var command=new StringBuilder();
    for(var arg:args)command.append(arg.equals(input.resolve("%04d.png").toString())?"FRAMES/%04d.png":arg.equals(output.toString())?"OUT/"+output.getFileName():arg).append('\n');
    Files.writeString(root.resolve(output.getFileName()+".args"),command);
    var mode=System.getenv("TE_FFMPEG_MODE");
    if("fail".equals(mode)) {System.err.println("owned encoder refusal");System.exit(23);}
    if("real".equals(mode)) {
      var actual=new ArrayList<>(List.of(System.getenv("TE_FFMPEG_REAL")));actual.addAll(arguments);
      var process=new ProcessBuilder(actual).inheritIO().start();System.exit(process.waitFor());
    }
    Files.writeString(output,"owned encoder fixture\n");
  }
}
