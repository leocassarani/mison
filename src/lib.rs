pub use json::Value;
use std::collections::{HashMap, VecDeque};

mod bitmaps;
mod json;

pub struct Query {
    field_set: FieldSet,
}

impl Query {
    pub fn new(fields: Vec<Vec<String>>) -> Self {
        let field_set = FieldSet::new(fields);
        Query { field_set }
    }

    pub fn run(&self, json: String) -> impl Iterator<Item = (String, Value)> {
        Record::new(json.into_bytes(), self.field_set.clone())
    }
}

#[derive(Clone)]
enum Field {
    Simple,
    Nested(FieldSet),
}

#[derive(Clone)]
struct FieldSet {
    fields: HashMap<String, Field>,
}

impl FieldSet {
    fn new(key_paths: Vec<Vec<String>>) -> Self {
        let mut fields = HashMap::with_capacity(key_paths.len());

        for key_path in key_paths {
            if let Some((head, tail)) = key_path.split_first() {
                let value = if tail.is_empty() {
                    Field::Simple
                } else {
                    Field::Nested(FieldSet::new(vec![tail.to_vec()]))
                };

                fields.insert(head.to_owned(), value);
            }
        }

        FieldSet { fields }
    }

    fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    fn max_depth(&self) -> usize {
        2
    }

    fn remove(&mut self, key: &str) -> Option<Field> {
        self.fields.remove(key)
    }
}

struct Record {
    bytes: Vec<u8>,
    fields: FieldSet,
    colons: VecDeque<usize>,
}

impl Record {
    fn new(bytes: Vec<u8>, fields: FieldSet) -> Self {
        let colons = bitmaps::LeveledColons::build(&bytes, fields.max_depth());

        Record {
            bytes,
            fields,
            colons: VecDeque::from(colons.positions(0)),
        }
    }

    fn key_preceding(&self, colon: usize) -> Option<String> {
        let mut start = 0;
        let mut end = 0;

        // Skip all whitespace until we find the index of the closing quote.
        for i in (0..colon).rev() {
            match self.bytes[i] {
                b'"' => {
                    end = i;
                    break;
                }
                b'\t' | b'\n' | b'\r' | b' ' => {}
                _ => {
                    return None;
                }
            }
        }

        // Keep going backwards until we find the index of the opening quote.
        for i in (0..end).rev() {
            if self.bytes[i] == b'"' {
                // If we've found a quote, we need to check if it's preceded by
                // a backslash character, which would mean it's an escaped quote
                // in the middle of the string rather than a structural quote.
                if i > 0 && self.bytes[i - 1] == b'\\' {
                    // TODO: check if the backslash is itself preceded by a backslash.
                    // If there is an odd number of backslashes, then we need to skip
                    // this quote, otherwise it is in fact the opening quote.
                    continue;
                }

                // The string starts at the character following the opening quote.
                // In the case of an empty string, start will equal end, which will
                // correctly produce an empty range.
                start = i + 1;
                break;
            }
        }

        // If we failed to find an opening quote, or if we somehow ended up with
        // an invalid range, then we failed to parse this key.
        if start == 0 || start > end {
            return None;
        }

        String::from_utf8(self.bytes[start..end].to_vec()).ok()
    }

    fn value_following(&self, colon: usize) -> Option<Value> {
        let start = colon + 1;

        if start < self.bytes.len() {
            Value::parse(&self.bytes[start..])
        } else {
            None
        }
    }
}

impl Iterator for Record {
    type Item = (String, Value);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.fields.is_empty() {
                return None;
            }

            let colon = self.colons.pop_front()?;
            let key = self.key_preceding(colon)?;

            match self.fields.remove(&key) {
                Some(Field::Simple) => {
                    let value = self.value_following(colon)?;
                    return Some((key, value));
                }
                Some(Field::Nested(_fields)) => {
                    return Some((key, Value::String("{ ... }".to_owned())));
                }
                None => {}
            }
        }
    }
}
