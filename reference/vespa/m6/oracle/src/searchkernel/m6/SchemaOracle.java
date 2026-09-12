package searchkernel.m6;

import com.yahoo.schema.ApplicationBuilder;
import com.yahoo.schema.derived.DerivedConfiguration;
import com.yahoo.schema.derived.SummaryClass;
import com.yahoo.schema.derived.SummaryClassField;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;

/** Thin semantic projection over the pinned Vespa Java config-model. */
public final class SchemaOracle {
    private static String q(String s) {
        StringBuilder b = new StringBuilder("\"");
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            switch (c) {
                case '\\' -> b.append("\\\\");
                case '"' -> b.append("\\\"");
                case '\n' -> b.append("\\n");
                case '\r' -> b.append("\\r");
                case '\t' -> b.append("\\t");
                default -> {
                    if (c < 0x20) b.append(String.format("\\u%04x", (int)c));
                    else b.append(c);
                }
            }
        }
        return b.append('"').toString();
    }

    private static String bool(boolean value) { return value ? "true" : "false"; }

    public static void main(String[] args) throws Exception {
        if (args.length != 3) {
            System.err.println("usage: SchemaOracle SCENARIO_ID SCHEMA_NAME SCHEMA.sd");
            System.exit(2);
        }
        String id = args[0];
        String schemaName = args[1];
        String schemaText = Files.readString(Path.of(args[2]));

        ApplicationBuilder builder = new ApplicationBuilder((level, message) -> {});
        builder.addSchema(schemaText);
        var application = builder.build(true);
        var schema = application.schemas().get(schemaName);
        if (schema == null) throw new IllegalArgumentException("schema not found: " + schemaName);
        var derived = new DerivedConfiguration(schema, application.rankProfileRegistry());

        var indexFields = new ArrayList<>(derived.getIndexSchema().getFields());
        indexFields.sort(Comparator.comparing(f -> f.getName()));

        var attrs = new ArrayList<>(derived.getAttributeFields().attributes());
        attrs.sort(Comparator.comparing(a -> a.getName()));

        SummaryClass defaultSummary = derived.getSummaries().asList().stream()
                .filter(s -> s.getName().equals("default"))
                .findFirst().orElseThrow();
        List<SummaryClassField> summaryFields = new ArrayList<>(defaultSummary.fields().values());
        summaryFields.sort(Comparator.comparing(SummaryClassField::getName));

        StringBuilder out = new StringBuilder();
        out.append('{');
        out.append("\"id\":").append(q(id)).append(',');
        out.append("\"schema\":").append(q(schemaName)).append(',');

        out.append("\"index_fields\":[");
        for (int i = 0; i < indexFields.size(); i++) {
            var f = indexFields.get(i);
            if (i != 0) out.append(',');
            out.append('{')
               .append("\"name\":").append(q(f.getName())).append(',')
               .append("\"datatype\":").append(q(f.getType())).append(',')
               .append("\"collection\":").append(q(f.getCollectionType())).append(',')
               .append("\"prefix\":").append(bool(f.hasPrefix())).append(',')
               .append("\"positions\":true,")
               .append("\"interleaved_features\":").append(bool(f.useInterleavedFeatures()))
               .append('}');
        }
        out.append("],");

        out.append("\"attributes\":[");
        for (int i = 0; i < attrs.size(); i++) {
            var a = attrs.get(i);
            if (i != 0) out.append(',');
            out.append('{')
               .append("\"name\":").append(q(a.getName())).append(',')
               .append("\"datatype\":").append(q(a.getType().getExportAttributeTypeName())).append(',')
               .append("\"collection\":").append(q(a.getCollectionType().getName()))
               .append('}');
        }
        out.append("],");

        out.append("\"summary_fields\":[");
        for (int i = 0; i < summaryFields.size(); i++) {
            var f = summaryFields.get(i);
            if (i != 0) out.append(',');
            out.append('{')
               .append("\"name\":").append(q(f.getName())).append(',')
               .append("\"command\":").append(q(f.getCommand())).append(',')
               .append("\"source\":").append(q(f.getSource()))
               .append('}');
        }
        out.append("]}");
        System.out.println(out);
    }
}
