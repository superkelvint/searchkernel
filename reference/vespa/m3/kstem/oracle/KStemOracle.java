import com.yahoo.language.simple.kstem.KStemmer;

import java.io.BufferedReader;
import java.io.BufferedWriter;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.nio.charset.StandardCharsets;

/**
 * Thin oracle over the pinned Vespa KStemmer.
 *
 * Input:  one UTF-8 term per line
 * Output: input<TAB>stem per line
 */
public final class KStemOracle {
    private KStemOracle() {}

    public static void main(String[] args) throws Exception {
        var stemmer = new KStemmer();
        var in = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
        var out = new BufferedWriter(new OutputStreamWriter(System.out, StandardCharsets.UTF_8));

        String term;
        while ((term = in.readLine()) != null) {
            if (term.indexOf('\t') >= 0) {
                throw new IllegalArgumentException("corpus terms may not contain tabs");
            }
            out.write(term);
            out.write('\t');
            out.write(stemmer.stem(term));
            out.newLine();
        }
        out.flush();
    }
}
