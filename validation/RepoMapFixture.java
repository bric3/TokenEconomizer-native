import java.nio.file.Files;
import java.nio.file.Path;

/** Generator process fixture: checks arguments and emits only a deterministic map. */
class RepoMapFixture {
  public static void main(String[] args)throws Exception {
    if(args.length!=3||!args[1].equals("2048")||args[2].isBlank())throw new AssertionError("generator arguments changed");
    var root=Path.of(args[0]);
    if(!Files.readString(root.resolve("keep.py")).contains("Widget"))throw new AssertionError("source archive missing");
    if(!Files.isRegularFile(root.resolve("nested/a b.py")))throw new AssertionError("space in source name lost");
    System.out.println("keep.py: Widget");
    System.out.println("nested/a b.py: Widget");
  }
}
