// Copyright Vespa.ai. Licensed under the terms of the Apache 2.0 license. See LICENSE in the project root.
//
// This is a Rust port of the pinned Vespa KStemmer implementation. The
// algorithm is derived from the Lucene KStem implementation, which in turn is
// derived from the CIIR KStemmer implementation. See the upstream source for
// the original Apache 2.0 and modified BSD license notices.

use std::collections::HashMap;
use std::sync::OnceLock;

use super::data;

const MAX_WORD_LEN: usize = 50;

const EXCEPTIONS: &[&str] = &[
    "aide",
    "bathe",
    "caste",
    "cute",
    "dame",
    "dime",
    "doge",
    "done",
    "dune",
    "envelope",
    "gage",
    "grille",
    "grippe",
    "lobe",
    "mane",
    "mare",
    "nape",
    "node",
    "pane",
    "pate",
    "plane",
    "pope",
    "programme",
    "quite",
    "ripe",
    "rote",
    "rune",
    "sage",
    "severe",
    "shoppe",
    "sine",
    "slime",
    "snipe",
    "steppe",
    "suite",
    "swinge",
    "tare",
    "tine",
    "tope",
    "tripe",
    "twine",
];

const DIRECT_CONFLATIONS: &[(&str, &str)] = &[
    ("aging", "age"),
    ("going", "go"),
    ("goes", "go"),
    ("lying", "lie"),
    ("using", "use"),
    ("owing", "owe"),
    ("suing", "sue"),
    ("dying", "die"),
    ("tying", "tie"),
    ("vying", "vie"),
    ("aged", "age"),
    ("used", "use"),
    ("vied", "vie"),
    ("cued", "cue"),
    ("died", "die"),
    ("eyed", "eye"),
    ("hued", "hue"),
    ("iced", "ice"),
    ("lied", "lie"),
    ("owed", "owe"),
    ("sued", "sue"),
    ("toed", "toe"),
    ("tied", "tie"),
    ("does", "do"),
    ("doing", "do"),
    ("aeronautical", "aeronautics"),
    ("mathematical", "mathematics"),
    ("political", "politics"),
    ("metaphysical", "metaphysics"),
    ("cylindrical", "cylinder"),
    ("nazism", "nazi"),
    ("ambiguity", "ambiguous"),
    ("barbarity", "barbarous"),
    ("credulity", "credulous"),
    ("generosity", "generous"),
    ("spontaneity", "spontaneous"),
    ("unanimity", "unanimous"),
    ("voracity", "voracious"),
    ("fled", "flee"),
    ("miscarriage", "miscarry"),
];

#[derive(Clone, Copy)]
struct Entry {
    root: Option<&'static str>,
    exception: bool,
}

fn dictionary() -> &'static HashMap<&'static str, Entry> {
    static DICTIONARY: OnceLock<HashMap<&'static str, Entry>> = OnceLock::new();
    DICTIONARY.get_or_init(|| {
        let mut dictionary = HashMap::with_capacity(data::WORDS.len() + 256);
        for &word in EXCEPTIONS {
            dictionary.insert(
                word,
                Entry {
                    root: None,
                    exception: true,
                },
            );
        }
        for &(word, root) in DIRECT_CONFLATIONS {
            dictionary.insert(
                word,
                Entry {
                    root: Some(root),
                    exception: false,
                },
            );
        }
        for &(word, root) in data::COUNTRY_NATIONALITY {
            dictionary.insert(
                word,
                Entry {
                    root: Some(root),
                    exception: false,
                },
            );
        }
        for &word in data::WORDS {
            dictionary.entry(word).or_insert(Entry {
                root: None,
                exception: false,
            });
        }
        for &word in data::EXTRA_WORDS {
            dictionary.entry(word).or_insert(Entry {
                root: None,
                exception: false,
            });
        }
        dictionary
    })
}

#[derive(Default)]
pub(super) struct KStemmer {
    word: Vec<u8>,
    original: Vec<u8>,
    j: isize,
    k: isize,
    matched_entry: Option<Entry>,
}

impl KStemmer {
    fn final_char(&self) -> u8 {
        self.word[self.k as usize]
    }

    fn penult_char(&self) -> u8 {
        self.word[self.k as usize - 1]
    }

    fn is_vowel(&self, index: isize) -> bool {
        !self.is_consonant(index)
    }

    fn is_consonant(&self, index: isize) -> bool {
        match self.word[index as usize] {
            b'a' | b'e' | b'i' | b'o' | b'u' => false,
            b'y' if index > 0 => !self.is_consonant(index - 1),
            _ => true,
        }
    }

    fn stem_len(&self) -> usize {
        (self.j + 1) as usize
    }

    fn ends_in(&mut self, suffix: &[u8]) -> bool {
        if suffix.len() as isize > self.k {
            return false;
        }
        self.j = self.k;
        let start = self.word.len() - suffix.len();
        if self.word[start..] != *suffix {
            return false;
        }
        self.j = start as isize - 1;
        true
    }

    fn word_str(&self) -> &str {
        std::str::from_utf8(&self.word).expect("KStemmer contains ASCII input")
    }

    fn word_in_dict(&mut self) -> Option<Entry> {
        if let Some(entry) = self.matched_entry {
            return Some(entry);
        }
        let entry = dictionary().get(self.word_str()).copied();
        if let Some(entry) = entry {
            if !entry.exception {
                self.matched_entry = Some(entry);
            }
        }
        entry
    }

    fn lookup(&mut self) -> bool {
        self.matched_entry = dictionary().get(self.word_str()).copied();
        self.matched_entry.is_some()
    }

    fn set_len(&mut self, length: usize) {
        if length > self.word.len() {
            if length <= self.original.len() {
                self.word
                    .extend_from_slice(&self.original[self.word.len()..length]);
            } else {
                self.word.resize(length, 0);
            }
        } else {
            self.word.truncate(length);
        }
    }

    fn append(&mut self, bytes: &[u8]) {
        self.word.extend_from_slice(bytes);
    }

    fn set_suffix(&mut self, suffix: &[u8]) {
        self.set_len((self.j + 1) as usize);
        self.append(suffix);
        self.k = self.j + suffix.len() as isize;
    }

    fn double_consonant(&self, index: isize) -> bool {
        index >= 1
            && self.word[index as usize] == self.word[index as usize - 1]
            && self.is_consonant(index)
    }

    fn vowel_in_stem(&self) -> bool {
        (0..self.stem_len() as isize).any(|index| self.is_vowel(index))
    }

    fn matched(&self) -> bool {
        self.matched_entry.is_some()
    }

    fn plural(&mut self) {
        if self.final_char() != b's' {
            return;
        }

        if self.ends_in(b"ies") {
            self.set_len((self.j + 3) as usize);
            self.k -= 1;
            if self.lookup() {
                return;
            }
            self.k += 1;
            self.word.push(b's');
            self.set_suffix(b"y");
            self.lookup();
        } else if self.ends_in(b"es") {
            self.set_len((self.j + 2) as usize);
            self.k -= 1;
            let try_e = self.j > 0
                && !(self.word[self.j as usize] == b's' && self.word[self.j as usize - 1] == b's');
            if try_e && self.lookup() {
                return;
            }

            self.set_len((self.j + 1) as usize);
            self.k -= 1;
            if self.lookup() {
                return;
            }

            self.word.push(b'e');
            self.k += 1;
            if !try_e {
                self.lookup();
            }
        } else if self.word.len() > 3 && self.penult_char() != b's' && !self.ends_in(b"ous") {
            self.set_len(self.k as usize);
            self.k -= 1;
            self.lookup();
        }
    }

    fn past_tense(&mut self) {
        if self.word.len() <= 4 {
            return;
        }

        if self.ends_in(b"ied") {
            self.set_len((self.j + 3) as usize);
            self.k -= 1;
            if self.lookup() {
                return;
            }
            self.k += 1;
            self.word.push(b'd');
            self.set_suffix(b"y");
            self.lookup();
            return;
        }

        if self.ends_in(b"ed") && self.vowel_in_stem() {
            self.set_len((self.j + 2) as usize);
            self.k = self.j + 1;
            if let Some(entry) = self.word_in_dict() {
                if !entry.exception {
                    return;
                }
            }

            self.set_len((self.j + 1) as usize);
            self.k = self.j;
            if self.lookup() {
                return;
            }

            if self.double_consonant(self.k) {
                self.set_len(self.k as usize);
                self.k -= 1;
                if self.lookup() {
                    return;
                }
                let repeated = self.word[self.k as usize];
                self.k += 1;
                self.word.push(repeated);
                self.lookup();
                return;
            }

            if self.word[0] == b'u' && self.word[1] == b'n' {
                self.word.extend_from_slice(b"ed");
                self.k += 2;
                return;
            }

            self.set_len((self.j + 1) as usize);
            self.word.push(b'e');
            self.k = self.j + 1;
        }
    }

    fn aspect(&mut self) {
        if self.word.len() <= 5 {
            return;
        }

        if self.ends_in(b"ing") && self.vowel_in_stem() {
            self.word[self.j as usize + 1] = b'e';
            self.set_len((self.j + 2) as usize);
            self.k = self.j + 1;
            if let Some(entry) = self.word_in_dict() {
                if !entry.exception {
                    return;
                }
            }

            self.set_len(self.k as usize);
            self.k -= 1;
            if self.lookup() {
                return;
            }

            if self.double_consonant(self.k) {
                self.k -= 1;
                self.set_len((self.k + 1) as usize);
                if self.lookup() {
                    return;
                }
                let repeated = self.word[self.k as usize];
                self.k += 1;
                self.word.push(repeated);
                self.lookup();
                return;
            }

            if self.j > 0 && self.is_consonant(self.j) && self.is_consonant(self.j - 1) {
                self.k = self.j;
                self.set_len((self.k + 1) as usize);
                return;
            }

            self.set_len((self.j + 1) as usize);
            self.word.push(b'e');
            self.k = self.j + 1;
        }
    }

    fn ity_endings(&mut self) {
        let old_k = self.k;
        if self.ends_in(b"ity") {
            self.set_len((self.j + 1) as usize);
            self.k = self.j;
            if self.lookup() {
                return;
            }
            self.word.push(b'e');
            self.k = self.j + 1;
            if self.lookup() {
                return;
            }
            self.word[self.j as usize + 1] = b'i';
            self.append(b"ty");
            self.k = old_k;

            if self.j > 0
                && self.word[self.j as usize - 1] == b'i'
                && self.word[self.j as usize] == b'l'
            {
                self.set_len((self.j - 1) as usize);
                self.append(b"le");
                self.k = self.j;
                self.lookup();
                return;
            }

            if self.j > 0
                && self.word[self.j as usize - 1] == b'i'
                && self.word[self.j as usize] == b'v'
            {
                self.set_len((self.j + 1) as usize);
                self.word.push(b'e');
                self.k = self.j + 1;
                self.lookup();
                return;
            }

            if self.j > 0
                && self.word[self.j as usize - 1] == b'a'
                && self.word[self.j as usize] == b'l'
            {
                self.set_len((self.j + 1) as usize);
                self.k = self.j;
                self.lookup();
                return;
            }

            self.lookup();
            self.set_len((self.j + 1) as usize);
            self.k = self.j;
        }
    }

    fn nce_endings(&mut self) {
        let old_k = self.k;
        if self.ends_in(b"nce") {
            let word_char = self.word[self.j as usize];
            if word_char != b'e' && word_char != b'a' {
                return;
            }
            self.set_len(self.j as usize);
            self.word.push(b'e');
            self.k = self.j;
            if self.lookup() {
                return;
            }
            self.set_len(self.j as usize);
            self.k = self.j - 1;
            if self.lookup() {
                return;
            }
            self.word.push(word_char);
            self.append(b"nce");
            self.k = old_k;
        }
    }

    fn ness_endings(&mut self) {
        if self.ends_in(b"ness") {
            self.set_len((self.j + 1) as usize);
            self.k = self.j;
            if self.word[self.j as usize] == b'i' {
                self.word[self.j as usize] = b'y';
            }
            self.lookup();
        }
    }

    fn ism_endings(&mut self) {
        if self.ends_in(b"ism") {
            self.set_len((self.j + 1) as usize);
            self.k = self.j;
            self.lookup();
        }
    }

    fn ment_endings(&mut self) {
        let old_k = self.k;
        if self.ends_in(b"ment") {
            self.set_len((self.j + 1) as usize);
            self.k = self.j;
            if self.lookup() {
                return;
            }
            self.append(b"ment");
            self.k = old_k;
        }
    }

    fn ize_endings(&mut self) {
        let old_k = self.k;
        if self.ends_in(b"ize") {
            self.set_len((self.j + 1) as usize);
            self.k = self.j;
            if self.lookup() {
                return;
            }
            self.word.push(b'i');

            if self.double_consonant(self.j) {
                self.set_len(self.j as usize);
                self.k = self.j - 1;
                if self.lookup() {
                    return;
                }
                let repeated = self.word[self.j as usize - 1];
                self.word.push(repeated);
            }

            self.set_len((self.j + 1) as usize);
            self.word.push(b'e');
            self.k = self.j + 1;
            if self.lookup() {
                return;
            }
            self.set_len((self.j + 1) as usize);
            self.append(b"ize");
            self.k = old_k;
        }
    }

    fn ncy_endings(&mut self) {
        if self.ends_in(b"ncy") {
            let index = self.j as usize;
            if self.word[index] != b'e' && self.word[index] != b'a' {
                return;
            }
            self.word[index + 2] = b't';
            self.set_len(index + 3);
            self.k = self.j + 2;
            if self.lookup() {
                return;
            }
            self.word[index + 2] = b'c';
            self.word.push(b'e');
            self.k = self.j + 3;
            self.lookup();
        }
    }

    fn ble_endings(&mut self) {
        let old_k = self.k;
        if self.ends_in(b"ble") {
            let word_char = self.word[self.j as usize];
            if word_char != b'a' && word_char != b'i' {
                return;
            }
            self.set_len(self.j as usize);
            self.k = self.j - 1;
            if self.lookup() {
                return;
            }

            if self.double_consonant(self.k) {
                self.set_len(self.k as usize);
                self.k -= 1;
                if self.lookup() {
                    return;
                }
                let repeated = self.word[self.k as usize];
                self.k += 1;
                self.word.push(repeated);
            }

            self.set_len(self.j as usize);
            self.word.push(b'e');
            self.k = self.j;
            if self.lookup() {
                return;
            }
            self.set_len(self.j as usize);
            self.append(b"ate");
            self.k = self.j + 2;
            if self.lookup() {
                return;
            }
            self.set_len(self.j as usize);
            self.word.push(word_char);
            self.append(b"ble");
            self.k = old_k;
        }
    }

    fn ic_endings(&mut self) {
        if self.ends_in(b"ic") {
            self.set_len((self.j + 3) as usize);
            self.append(b"al");
            self.k = self.j + 4;
            if self.lookup() {
                return;
            }
            self.word[self.j as usize + 1] = b'y';
            self.set_len((self.j + 2) as usize);
            self.k = self.j + 1;
            if self.lookup() {
                return;
            }
            self.word[self.j as usize + 1] = b'e';
            if self.lookup() {
                return;
            }
            self.set_len((self.j + 1) as usize);
            self.k = self.j;
            if self.lookup() {
                return;
            }
            self.append(b"ic");
            self.k = self.j + 2;
        }
    }

    fn ion_endings(&mut self) {
        let old_k = self.k;
        if !self.ends_in(b"ion") {
            return;
        }

        if self.ends_in(b"ization") {
            self.set_len((self.j + 3) as usize);
            self.word.push(b'e');
            self.k = self.j + 3;
            self.lookup();
            return;
        }

        if self.ends_in(b"ition") {
            self.set_len((self.j + 1) as usize);
            self.word.push(b'e');
            self.k = self.j + 1;
            if self.lookup() {
                return;
            }
            self.set_len((self.j + 1) as usize);
            self.append(b"ition");
            self.k = old_k;
        } else if self.ends_in(b"ation") {
            self.set_len((self.j + 3) as usize);
            self.word.push(b'e');
            self.k = self.j + 3;
            if self.lookup() {
                return;
            }

            self.set_len((self.j + 1) as usize);
            self.word.push(b'e');
            self.k = self.j + 1;
            if self.lookup() {
                return;
            }

            self.set_len((self.j + 1) as usize);
            self.k = self.j;
            if self.lookup() {
                return;
            }

            self.set_len((self.j + 1) as usize);
            self.append(b"ation");
            self.k = old_k;
        }

        if self.ends_in(b"ication") {
            self.set_len((self.j + 1) as usize);
            self.word.push(b'y');
            self.k = self.j + 1;
            if self.lookup() {
                return;
            }
            self.set_len((self.j + 1) as usize);
            self.append(b"ication");
            self.k = old_k;
        }

        self.j = self.k - 3;
        self.set_len((self.j + 1) as usize);
        self.word.push(b'e');
        self.k = self.j + 1;
        if self.lookup() {
            return;
        }

        self.set_len((self.j + 1) as usize);
        self.k = self.j;
        if self.lookup() {
            return;
        }

        self.set_len((self.j + 1) as usize);
        self.append(b"ion");
        self.k = old_k;
    }

    fn er_or_endings(&mut self) {
        let old_k = self.k;
        if self.final_char() != b'r' {
            return;
        }

        if self.ends_in(b"izer") {
            self.set_len((self.j + 4) as usize);
            self.k = self.j + 3;
            self.lookup();
            return;
        }

        if self.ends_in(b"er") || self.ends_in(b"or") {
            let word_char = self.word[self.j as usize + 1];
            if self.double_consonant(self.j) {
                self.set_len(self.j as usize);
                self.k = self.j - 1;
                if self.lookup() {
                    return;
                }
                let repeated = self.word[self.j as usize - 1];
                self.word.push(repeated);
            }

            if self.word[self.j as usize] == b'i' {
                self.word[self.j as usize] = b'y';
                self.set_len((self.j + 1) as usize);
                self.k = self.j;
                if self.lookup() {
                    return;
                }
                self.word[self.j as usize] = b'i';
                self.word.push(b'e');
            }

            if self.word[self.j as usize] == b'e' {
                self.set_len(self.j as usize);
                self.k = self.j - 1;
                if self.lookup() {
                    return;
                }
                self.word.push(b'e');
                self.set_len((self.j + 2) as usize);
            }

            self.set_len((self.j + 2) as usize);
            self.k = self.j + 1;
            if self.lookup() {
                return;
            }
            self.set_len((self.j + 1) as usize);
            self.k = self.j;
            if self.lookup() {
                return;
            }
            self.word.push(b'e');
            self.k = self.j + 1;
            if self.lookup() {
                return;
            }
            self.set_len((self.j + 1) as usize);
            self.word.push(word_char);
            self.word.push(b'r');
            self.k = old_k;
        }
    }

    fn ly_endings(&mut self) {
        let old_k = self.k;
        if self.ends_in(b"ly") {
            self.word[self.j as usize + 2] = b'e';
            if self.lookup() {
                return;
            }
            self.word[self.j as usize + 2] = b'y';
            self.set_len((self.j + 1) as usize);
            self.k = self.j;
            if self.lookup() {
                return;
            }

            if self.j > 0
                && self.word[self.j as usize - 1] == b'a'
                && self.word[self.j as usize] == b'l'
            {
                return;
            }

            self.append(b"ly");
            self.k = old_k;
            if self.j > 0
                && self.word[self.j as usize - 1] == b'a'
                && self.word[self.j as usize] == b'b'
            {
                self.word[self.j as usize + 2] = b'e';
                self.k = self.j + 2;
                return;
            }

            if self.word[self.j as usize] == b'i' {
                self.set_len(self.j as usize);
                self.word.push(b'y');
                self.k = self.j;
                if self.lookup() {
                    return;
                }
                self.set_len(self.j as usize);
                self.append(b"ily");
                self.k = old_k;
            }

            self.set_len((self.j + 1) as usize);
            self.k = self.j;
        }
    }

    fn al_endings(&mut self) {
        let old_k = self.k;
        if self.word.len() < 4 {
            return;
        }
        if self.ends_in(b"al") {
            self.set_len((self.j + 1) as usize);
            self.k = self.j;
            if self.lookup() {
                return;
            }

            if self.double_consonant(self.j) {
                self.set_len(self.j as usize);
                self.k = self.j - 1;
                if self.lookup() {
                    return;
                }
                let repeated = self.word[self.j as usize - 1];
                self.word.push(repeated);
            }

            self.set_len((self.j + 1) as usize);
            self.word.push(b'e');
            self.k = self.j + 1;
            if self.lookup() {
                return;
            }

            self.set_len((self.j + 1) as usize);
            self.append(b"um");
            self.k = self.j + 2;
            if self.lookup() {
                return;
            }

            self.set_len((self.j + 1) as usize);
            self.append(b"al");
            self.k = old_k;

            if self.j > 0
                && self.word[self.j as usize - 1] == b'i'
                && self.word[self.j as usize] == b'c'
            {
                self.set_len((self.j - 1) as usize);
                self.k = self.j - 2;
                if self.lookup() {
                    return;
                }
                self.set_len((self.j - 1) as usize);
                self.word.push(b'y');
                self.k = self.j - 1;
                if self.lookup() {
                    return;
                }
                self.set_len((self.j - 1) as usize);
                self.append(b"ic");
                self.k = self.j;
                self.lookup();
                return;
            }

            if self.word[self.j as usize] == b'i' {
                self.set_len(self.j as usize);
                self.k = self.j - 1;
                if self.lookup() {
                    return;
                }
                self.append(b"ial");
                self.k = old_k;
                self.lookup();
            }
        }
    }

    fn ive_endings(&mut self) {
        let old_k = self.k;
        if self.ends_in(b"ive") {
            self.set_len((self.j + 1) as usize);
            self.k = self.j;
            if self.lookup() {
                return;
            }

            self.word.push(b'e');
            self.k = self.j + 1;
            if self.lookup() {
                return;
            }
            self.set_len((self.j + 1) as usize);
            self.append(b"ive");

            if self.j > 0
                && self.word[self.j as usize - 1] == b'a'
                && self.word[self.j as usize] == b't'
            {
                self.word[self.j as usize - 1] = b'e';
                self.set_len(self.j as usize);
                self.k = self.j - 1;
                if self.lookup() {
                    return;
                }
                self.set_len((self.j - 1) as usize);
                if self.lookup() {
                    return;
                }
                self.append(b"ative");
                self.k = old_k;
            }

            self.word[self.j as usize + 2] = b'o';
            self.word[self.j as usize + 3] = b'n';
            self.k = self.j + 3;
            if self.lookup() {
                return;
            }
            self.word[self.j as usize + 2] = b'v';
            self.word[self.j as usize + 3] = b'e';
            self.k = old_k;
        }
    }

    pub(super) fn stem_word(&mut self, input: &str) -> String {
        let bytes = input.as_bytes();
        self.k = bytes.len() as isize - 1;
        if self.k <= 1 || self.k >= MAX_WORD_LEN as isize - 1 {
            return input.to_owned();
        }

        if let Some(entry) = dictionary().get(input).copied() {
            return entry.root.unwrap_or(input).to_owned();
        }

        self.original.clear();
        self.original.extend_from_slice(bytes);
        self.word.clear();
        self.word.reserve(bytes.len() + 10);
        if !bytes.iter().all(|&byte| byte.is_ascii_lowercase()) {
            return input.to_owned();
        }
        self.word.extend_from_slice(bytes);
        self.matched_entry = None;

        self.plural();
        if !self.matched() {
            self.past_tense();
        }
        if !self.matched() {
            self.aspect();
        }
        if !self.matched() {
            self.ity_endings();
        }
        if !self.matched() {
            self.ness_endings();
        }
        if !self.matched() {
            self.ion_endings();
        }
        if !self.matched() {
            self.er_or_endings();
        }
        if !self.matched() {
            self.ly_endings();
        }
        if !self.matched() {
            self.al_endings();
        }
        if !self.matched() {
            self.word_in_dict();
            self.ive_endings();
        }
        if !self.matched() {
            self.ize_endings();
        }
        if !self.matched() {
            self.ment_endings();
        }
        if !self.matched() {
            self.ble_endings();
        }
        if !self.matched() {
            self.ism_endings();
        }
        if !self.matched() {
            self.ic_endings();
        }
        if !self.matched() {
            self.ncy_endings();
        }
        if !self.matched() {
            self.nce_endings();
        }

        let stem = self
            .matched_entry
            .and_then(|entry| entry.root)
            .unwrap_or_else(|| self.word_str());
        stem.to_owned()
    }
}

// Kept as the standalone verifier entry point; production analysis reuses a
// KStemmer through `stem_word` instead.
#[allow(dead_code)]
pub(super) fn stem(input: &str) -> String {
    KStemmer::default().stem_word(input)
}

#[cfg(test)]
mod tests {
    use super::{data, stem};

    #[test]
    fn matches_pinned_kstem_examples() {
        assert_eq!(stem("running"), "running");
        assert_eq!(stem("runners"), "runner");
        assert_eq!(stem("foxes"), "fox");
        assert_eq!(stem("jumped"), "jump");
        assert_eq!(stem("generously"), "generous");
        assert_eq!(stem("relational"), "relational");
    }

    #[test]
    fn leaves_non_stemmable_terms_unchanged() {
        assert_eq!(stem("zzzzzz"), "zzzzzz");
        assert_eq!(stem("a"), "a");
        assert_eq!(stem("This"), "This");
        assert_eq!(stem(&"z".repeat(50)), "z".repeat(50));
    }

    #[test]
    fn exercises_dictionary_and_suffix_paths() {
        for &base in data::WORDS.iter().chain(data::EXTRA_WORDS.iter()) {
            for suffix in [
                "s", "ies", "es", "ed", "ied", "ing", "ity", "ness", "ion", "er", "or", "ly", "al",
                "ive", "ize", "ment", "able", "ible", "ism", "ic", "ncy", "nce",
            ] {
                let candidate = format!("{base}{suffix}");
                let _ = stem(&candidate);
                if let Some(prefix) = base.strip_suffix('e') {
                    let _ = stem(&format!("{prefix}{suffix}"));
                }
                if let Some(prefix) = base.strip_suffix("ate") {
                    let _ = stem(&format!("{prefix}{suffix}"));
                }
                if let Some(last) = base.bytes().last() {
                    let _ = stem(&format!("{base}{}{suffix}", last as char));
                }
            }

            if let Some(prefix) = base.strip_suffix("ion") {
                let _ = stem(&format!("{prefix}ive"));
            }
            if let Some(prefix) = base.strip_suffix("ize") {
                let _ = stem(&format!("{prefix}ization"));
            }
            if let Some(prefix) = base.strip_suffix('e') {
                let _ = stem(&format!("{prefix}nce"));
            }
            if let Some(prefix) = base.strip_suffix("nce") {
                let _ = stem(&format!("{prefix}ncy"));
            }
            if let Some(prefix) = base.strip_suffix("ble") {
                let _ = stem(&format!("{prefix}ility"));
            }
        }
    }
}
