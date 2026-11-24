import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import net.sourceforge.jeuclid.context.LayoutContextImpl;
import net.sourceforge.jeuclid.converter.Converter;

/**
 * Renders MathML files to SVG for the canon comparison. Reads a plan file of
 * `source<TAB>target` lines; every line renders independently and a render
 * failure writes `<target>.error` with the message instead of the SVG.
 */
public class CanonRender {
    public static void main(String[] args) throws Exception {
        int rendered = 0;
        int errored = 0;
        for (String line : Files.readAllLines(Path.of(args[0]), StandardCharsets.UTF_8)) {
            if (line.isBlank()) {
                continue;
            }
            int split = line.indexOf('\t');
            Path source = Path.of(line.substring(0, split));
            Path target = Path.of(line.substring(split + 1));
            try {
                String mathml = Files.readString(source, StandardCharsets.UTF_8)
                    .replaceAll("(?s)^\\s*<\\?xml[^?]*\\?>", "").trim();
                var dimension = Converter.getInstance().convert(mathml,
                    java.nio.file.Files.newOutputStream(target), Converter.TYPE_SVG,
                    new LayoutContextImpl(LayoutContextImpl.getDefaultLayoutContext()));
                if (dimension == null) {
                    throw new IllegalStateException("render returned no result");
                }
                rendered += 1;
            } catch (Exception failure) {
                errored += 1;
                Files.writeString(Path.of(target + ".error"), String.valueOf(failure.getMessage()) + "\n");
            }
        }
        System.out.println("rendered " + rendered + ", errored " + errored);
    }
}
