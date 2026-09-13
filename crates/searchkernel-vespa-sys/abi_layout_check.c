#include <stddef.h>
#include <stdint.h>

#include "searchkernel_vespa.h"

_Static_assert(sizeof(((sk_error *)0)->code) == sizeof(int32_t),
               "sk_error.code must be int32_t");
_Static_assert(offsetof(sk_error, code) == 0,
               "sk_error.code offset changed");
_Static_assert(offsetof(sk_error, message) == sizeof(int32_t),
               "sk_error.message offset changed");
_Static_assert(sizeof(((sk_error *)0)->message) == 512,
               "sk_error.message capacity changed");
_Static_assert(sizeof(sk_error) == sizeof(int32_t) + 512,
               "sk_error has unexpected padding");

_Static_assert(offsetof(sk_text_occurrence, term) == 0,
               "sk_text_occurrence.term offset changed");
_Static_assert(offsetof(sk_text_occurrence, term_length) >
                   offsetof(sk_text_occurrence, term),
               "sk_text_occurrence.term_length offset changed");
_Static_assert(offsetof(sk_text_occurrence, byte_start) >
                   offsetof(sk_text_occurrence, term_length),
               "sk_text_occurrence.byte_start offset changed");
_Static_assert(offsetof(sk_text_occurrence, byte_length) >
                   offsetof(sk_text_occurrence, byte_start),
               "sk_text_occurrence.byte_length offset changed");
_Static_assert(offsetof(sk_text_occurrence, position) >
                   offsetof(sk_text_occurrence, byte_length),
               "sk_text_occurrence.position offset changed");

_Static_assert(offsetof(sk_text_query_term, term) == 0,
               "sk_text_query_term.term offset changed");
_Static_assert(offsetof(sk_text_query_term, term_length) >
                   offsetof(sk_text_query_term, term),
               "sk_text_query_term.term_length offset changed");
_Static_assert(offsetof(sk_text_query_term, position) >
                   offsetof(sk_text_query_term, term_length),
               "sk_text_query_term.position offset changed");
