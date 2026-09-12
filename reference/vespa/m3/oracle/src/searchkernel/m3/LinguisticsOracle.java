// Oracle adapter only. Linguistic behavior comes from the pinned Vespa Java implementation.
package searchkernel.m3;

import com.yahoo.language.Language;
import com.yahoo.language.opennlp.OpenNlpLinguistics;
import com.yahoo.language.process.StemList;
import com.yahoo.language.process.StemMode;
import com.yahoo.language.process.Token;
import com.yahoo.language.process.Tokenizer;

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

public final class LinguisticsOracle {
    private record DocumentTerm(String original, String term, int byteStart, int byteLen, int position) {}

    private static StemMode parseStemMode(String value) {
        return switch (value) {
            case "none" -> StemMode.NONE;
            case "default" -> StemMode.DEFAULT;
            default -> throw new IllegalArgumentException("unsupported stemming mode: " + value);
        };
    }

    private static byte[] fromHex(String hex) {
        if ((hex.length() & 1) != 0) throw new IllegalArgumentException("odd hex length");
        byte[] out = new byte[hex.length() / 2];
        for (int i = 0; i < out.length; i++) {
            int hi = Character.digit(hex.charAt(i * 2), 16);
            int lo = Character.digit(hex.charAt(i * 2 + 1), 16);
            if (hi < 0 || lo < 0) throw new IllegalArgumentException("invalid hex");
            out[i] = (byte)((hi << 4) | lo);
        }
        return out;
    }

    private static String json(String value) {
        StringBuilder out = new StringBuilder(value.length() + 16);
        out.append('"');
        for (int i = 0; i < value.length();) {
            int cp = value.codePointAt(i);
            i += Character.charCount(cp);
            switch (cp) {
                case '"' -> out.append("\\\"");
                case '\\' -> out.append("\\\\");
                case '\b' -> out.append("\\b");
                case '\f' -> out.append("\\f");
                case '\n' -> out.append("\\n");
                case '\r' -> out.append("\\r");
                case '\t' -> out.append("\\t");
                default -> {
                    if (cp < 0x20) out.append(String.format("\\u%04x", cp));
                    else out.appendCodePoint(cp);
                }
            }
        }
        return out.append('"').toString();
    }

    private static int utf8Length(String value) {
        return value.getBytes(StandardCharsets.UTF_8).length;
    }

    private static void collectDocumentTerms(String input, Token token, List<DocumentTerm> out) {
        if (!token.isSpecialToken()) {
            if (token.getNumComponents() > 0) {
                for (int i = 0; i < token.getNumComponents(); i++) {
                    collectDocumentTerms(input, token.getComponent(i), out);
                }
                return;
            }
            if (!token.isIndexable()) return;
        }

        String term = token.getTokenString();
        if (term == null || term.trim().isEmpty()) return;

        int from = Math.toIntExact(token.getOffset());
        int utf16Len = token.getOrig().length();
        if (from < 0 || from > input.length() || from + utf16Len > input.length()) {
            throw new IllegalStateException("Vespa token span outside input: " + token);
        }

        String original = input.substring(from, from + utf16Len);
        if (!original.equals(token.getOrig())) {
            throw new IllegalStateException(
                    "Vespa token original does not match source span: '" + token.getOrig() + "' vs '" + original + "'");
        }

        int byteStart = utf8Length(input.substring(0, from));
        int byteLen = utf8Length(original);
        out.add(new DocumentTerm(original, term, byteStart, byteLen, out.size()));
    }

    @SuppressWarnings("deprecation")
    private static String documentJson(String id, String input, StemMode stemMode, boolean removeAccents,
                                       OpenNlpLinguistics linguistics) {
        Tokenizer tokenizer = linguistics.getTokenizer();
        Iterable<Token> tokens = tokenizer.tokenize(input, Language.ENGLISH, stemMode, removeAccents);
        List<DocumentTerm> terms = new ArrayList<>();
        for (Token token : tokens) collectDocumentTerms(input, token, terms);

        StringBuilder out = new StringBuilder();
        out.append("{\"id\":").append(json(id)).append(",\"terms\":[");
        for (int i = 0; i < terms.size(); i++) {
            if (i > 0) out.append(',');
            DocumentTerm t = terms.get(i);
            out.append("{\"original\":").append(json(t.original()))
               .append(",\"term\":").append(json(t.term()))
               .append(",\"byte_start\":").append(t.byteStart())
               .append(",\"byte_len\":").append(t.byteLen())
               .append(",\"position\":").append(t.position())
               .append('}');
        }
        return out.append("]}").toString();
    }

    @SuppressWarnings("deprecation")
    private static String queryJson(String id, String input, StemMode stemMode, boolean removeAccents,
                                    OpenNlpLinguistics linguistics) {
        List<StemList> stems = linguistics.getStemmer().stem(input, Language.ENGLISH, stemMode, removeAccents);
        StringBuilder out = new StringBuilder();
        out.append("{\"id\":").append(json(id)).append(",\"terms\":[");
        for (int i = 0; i < stems.size(); i++) {
            if (i > 0) out.append(',');
            StemList list = stems.get(i);
            out.append("{\"stems\":[");
            for (int j = 0; j < list.size(); j++) {
                if (j > 0) out.append(',');
                out.append(json(list.get(j)));
            }
            out.append("],\"position\":").append(i).append('}');
        }
        return out.append("]}").toString();
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 1 || !(args[0].equals("document") || args[0].equals("query"))) {
            System.err.println("usage: LinguisticsOracle document|query");
            System.exit(2);
        }
        String mode = args[0];
        OpenNlpLinguistics linguistics = new OpenNlpLinguistics();
        BufferedReader reader = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
        String line;
        while ((line = reader.readLine()) != null) {
            if (line.isEmpty()) continue;
            String[] parts = line.split("\\t", -1);
            if (parts.length != 4) throw new IllegalArgumentException("invalid input protocol line");
            String id = parts[0];
            StemMode stemMode = parseStemMode(parts[1]);
            boolean removeAccents = switch (parts[2]) {
                case "0" -> false;
                case "1" -> true;
                default -> throw new IllegalArgumentException("remove_accents must be 0 or 1");
            };
            String input = new String(fromHex(parts[3]), StandardCharsets.UTF_8);
            String output = mode.equals("document")
                    ? documentJson(id, input, stemMode, removeAccents, linguistics)
                    : queryJson(id, input, stemMode, removeAccents, linguistics);
            System.out.println(output);
        }
    }
}
