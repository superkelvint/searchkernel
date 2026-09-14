package searchkernel.m7;

import com.yahoo.language.Language;
import com.yahoo.language.opennlp.OpenNlpLinguistics;
import com.yahoo.prelude.Index;
import com.yahoo.prelude.IndexFacts;
import com.yahoo.prelude.IndexModel;
import com.yahoo.prelude.SearchDefinition;
import com.yahoo.prelude.query.AndItem;
import com.yahoo.prelude.query.CompositeItem;
import com.yahoo.prelude.query.IntItem;
import com.yahoo.prelude.query.Item;
import com.yahoo.prelude.query.Limit;
import com.yahoo.prelude.query.NotItem;
import com.yahoo.prelude.query.OrItem;
import com.yahoo.prelude.query.PhraseItem;
import com.yahoo.prelude.query.TermItem;
import com.yahoo.prelude.query.TrueItem;
import com.yahoo.search.query.QueryTree;
import com.yahoo.search.query.parser.Parsable;
import com.yahoo.search.query.parser.ParserEnvironment;
import com.yahoo.search.yql.YqlParser;

public final class QueryOracle {
    private static String q(String s) {
        StringBuilder b=new StringBuilder("\"");
        for (int i=0;i<s.length();i++) {
            char c=s.charAt(i);
            switch(c) {
                case '\\' -> b.append("\\\\");
                case '"' -> b.append("\\\"");
                case '\n' -> b.append("\\n");
                case '\r' -> b.append("\\r");
                case '\t' -> b.append("\\t");
                default -> { if (c<0x20) b.append(String.format("\\u%04x",(int)c)); else b.append(c); }
            }
        }
        return b.append('"').toString();
    }
    private static String fieldOf(TermItem t) { return t.getIndexName(); }
    private static String bound(Limit l) {
        if (l.isInfinite()) return "{\"kind\":\"unbounded\"}";
        return "{\"kind\":"+q(l.isInclusive()?"included":"excluded")+",\"value\":"+l.number().longValue()+"}";
    }
    private static String list(String kind, CompositeItem c) {
        StringBuilder b=new StringBuilder("{\"kind\":").append(q(kind)).append(",\"children\":[");
        for(int i=0;i<c.getItemCount();i++){ if(i>0)b.append(','); b.append(canon(c.getItem(i))); }
        return b.append("]}").toString();
    }
    private static String canonNot(NotItem n) {
        Item pos=n.getPositiveItem();
        if (n.negativeItems().size()!=1) throw new IllegalArgumentException("M7 oracle supports one negative child per NotItem");
        String neg="{\"kind\":\"not\",\"child\":"+canon(n.negativeItems().get(0))+"}";
        if (pos instanceof TrueItem) return neg;
        return "{\"kind\":\"and\",\"children\":["+canon(pos)+","+neg+"]}";
    }
    private static String canon(Item item) {
        if (item instanceof NotItem n) return canonNot(n);
        if (item instanceof AndItem a) return list("and",a);
        if (item instanceof OrItem o) return list("or",o);
        if (item instanceof PhraseItem p) {
            StringBuilder text=new StringBuilder();
            for(int i=0;i<p.getItemCount();i++) {
                if(i>0) text.append(' ');
                Item child=p.getItem(i);
                if (!(child instanceof TermItem t)) throw new IllegalArgumentException("non-term phrase child: "+child.getClass());
                text.append(t.stringValue());
            }
            return "{\"kind\":\"phrase\",\"field\":"+q(p.getIndexName())+",\"text\":"+q(text.toString())+"}";
        }
        if (item instanceof IntItem i) {
            Limit lo=i.getFromLimit(), hi=i.getToLimit();
            if (lo.isInfinite() && hi.isInfinite()) return "{\"kind\":\"exists\",\"field\":"+q(i.getIndexName())+"}";
            if (!lo.isInfinite() && !hi.isInfinite() && lo.isInclusive() && hi.isInclusive() && lo.number().longValue()==hi.number().longValue())
                return "{\"kind\":\"equals\",\"field\":"+q(i.getIndexName())+",\"value\":"+lo.number().longValue()+"}";
            return "{\"kind\":\"range\",\"field\":"+q(i.getIndexName())+",\"lower\":"+bound(lo)+",\"upper\":"+bound(hi)+"}";
        }
        if (item instanceof TermItem t) return "{\"kind\":\"term\",\"field\":"+q(fieldOf(t))+",\"text\":"+q(t.stringValue())+"}";
        if (item instanceof TrueItem) return "{\"kind\":\"true\"}";
        throw new IllegalArgumentException("unsupported oracle item: "+item.getClass().getName()+" "+item);
    }
    private static IndexFacts facts(String schema, String fieldSpec) {
        SearchDefinition sd=new SearchDefinition(schema);
        for(String token: fieldSpec.split(",")) {
            if(token.isBlank()) continue;
            String[] p=token.split(":",2);
            Index idx=new Index(p[0]);
            if (p[1].equals("text")) { idx.setString(true); idx.setIndex(true); }
            else if (p[1].equals("integer")) { idx.setInteger(true); idx.setAttribute(true); }
            else throw new IllegalArgumentException("unsupported field type "+p[1]);
            sd.addIndex(idx);
        }
        return new IndexFacts(new IndexModel(sd)).freeze();
    }
    public static void main(String[] args) {
        if(args.length!=4) throw new IllegalArgumentException("usage: QueryOracle ID SCHEMA FIELD_SPEC YQL");
        String id=args[0], schema=args[1], fieldSpec=args[2], yql=args[3];
        ParserEnvironment env=new ParserEnvironment()
                .setIndexFacts(facts(schema,fieldSpec))
                .setLinguistics(new OpenNlpLinguistics());
        QueryTree tree=new YqlParser(env).parse(new Parsable().setQuery(yql).setLanguage(Language.ENGLISH));
        System.out.println("{\"id\":"+q(id)+",\"query\":"+canon(tree.getRoot())+",\"schema\":"+q(schema)+"}");
    }
}
