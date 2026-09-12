# DocumentId contract

A `DocumentId` is an opaque Unicode string. Search Kernel does not Unicode-normalize or case-fold it.

Valid IDs:

- contain 1 to 1024 UTF-8 bytes;
- are not Unicode-whitespace-only;
- contain no C0 control character (`U+0000..U+001F`) and no `U+007F`.

All other Unicode scalar values are preserved exactly. Canonically equivalent Unicode strings are distinct IDs.
