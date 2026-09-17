from searchkernel import Index, Text, Keyword, Float, field

index = Index.create(
    "./tmp/products",
    fields={
        "title": Text(stored=True),
        "brand": Keyword(
            stored=True,
            filterable=True,
            facetable=True,
        ),
        "price": Float(
            stored=True,
            filterable=True,
            sortable=True,
        ),
    },
)

index.put({
    "_id": "1",
    "title": "Wireless headphones",
    "brand": "Acme",
    "price": 99.0,
})

index.put({
    "_id": "2",
    "title": "Wired headphones",
    "brand": "Example",
    "price": 39.0,
})

results = index.search(
    "wireless",
    filter=field("price") <= 100,
    facets=["brand"],
    limit=10,
)

assert results.hits[0]["_id"] == "1"

index.update("1", {"price": 89.0})

assert index.get("1")["price"] == 89.0

index.delete("2")

index.close()

index = Index.open("./tmp/products")

results = index.search("headphones")

assert [hit["_id"] for hit in results.hits] == ["1"]

index.close()
