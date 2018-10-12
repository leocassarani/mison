use json::Value;
use std::collections::HashSet;

mod json;

pub struct Query {
    field_set: HashSet<String>,
}

impl Query {
    pub fn new(fields: Vec<impl Into<String>>) -> Self {
        let mut field_set = HashSet::with_capacity(fields.len());

        for field in fields {
            field_set.insert(field.into());
        }

        Query { field_set }
    }

    pub fn run(&self, json: String) -> Record {
        Record::new(json.into_bytes(), self.field_set.clone())
    }
}

pub struct Record {
    bytes: Vec<u8>,
    fields: HashSet<String>,
    colons: ColonBitmaps,
    pos: usize,
}

impl Record {
    fn new(bytes: Vec<u8>, fields: HashSet<String>) -> Self {
        let colons = ColonBitmaps::generate(&bytes);

        Record {
            bytes,
            fields,
            colons,
            pos: 0,
        }
    }

    fn key_preceding(&self, colon: usize) -> Option<String> {
        let mut start = 0;
        let mut end = 0;
        let mut inside = false;

        for i in (0..colon).rev() {
            if self.bytes[i] == b'"' {
                if inside {
                    start = i + 1; // TODO: check if safe
                    break;
                } else {
                    inside = true;
                    end = i;
                }
            }
        }

        if start >= end {
            return None;
        }

        String::from_utf8(self.bytes[start..end].to_vec()).ok()
    }

    fn value_following(&self, _colon: usize) -> Option<Value> {
        Some(Value::Null)
    }
}

impl Iterator for Record {
    type Item = (String, Value);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.fields.is_empty() {
                return None;
            }

            if self.pos >= self.colons.len() {
                return None;
            }

            let colon = self.colons.indices[self.pos];
            let key = self.key_preceding(colon)?;
            let value = self.value_following(colon)?;

            self.pos += 1;

            if self.fields.remove(&key) {
                return Some((key, value));
            }
        }
    }
}

struct ColonBitmaps {
    indices: Vec<usize>,
}

impl ColonBitmaps {
    fn generate(_bytes: &[u8]) -> Self {
        ColonBitmaps {
            indices: vec![14, 46, 84, 97, 124, 142, 161, 182, 208, 231],
        }
    }

    fn len(&self) -> usize {
        self.indices.len()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
