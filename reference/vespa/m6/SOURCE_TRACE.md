# M6 source trace

Pinned Vespa revision:

```text
2300ef0412bdb76f227ae3b497c8ca5e73c5da4e
```

Java config-model authority:

```text
config-model/src/main/java/com/yahoo/schema/ApplicationBuilder.java
config-model/src/main/java/com/yahoo/schema/derived/DerivedConfiguration.java
config-model/src/main/java/com/yahoo/schema/derived/IndexSchema.java
config-model/src/main/java/com/yahoo/schema/derived/AttributeFields.java
config-model/src/main/java/com/yahoo/schema/derived/Summaries.java
config-model/src/main/java/com/yahoo/schema/derived/SummaryClass.java
config-model/src/main/java/com/yahoo/schema/derived/SummaryClassField.java
config-model/src/main/java/com/yahoo/schema/document/Attribute.java
```

Useful upstream derivation examples:

```text
config-model/src/test/java/com/yahoo/schema/SchemaTestCase.java
```

Native document-model/bootstrap paths to inspect during implementation:

```text
searchcore/src/vespa/searchcore/proton/server/documentdb.cpp
searchcore/src/vespa/searchcore/proton/server/documentdbconfigmanager.cpp
searchcore/src/tests/proton/documentdb/
```

Generated-config definitions/consumers to inspect:

```text
configdefinitions/src/vespa/search/indexschema.def
configdefinitions/src/vespa/search/attributes.def
configdefinitions/src/vespa/search/summary.def
searchcore/src/vespa/searchcore/proton/server/documentdb.cpp
```

The Java oracle is deliberately a thin semantic projection over `DerivedConfiguration`; it is not a reimplementation of config-model.
