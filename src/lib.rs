use std::collections::HashSet;

pub struct Query<'a> {
    field_set: HashSet<&'a str>,
}

impl<'a> Query<'a> {
    pub fn new(fields: Vec<&'a str>) -> Self {
        let mut field_set = HashSet::with_capacity(fields.len());

        for field in fields {
            field_set.insert(field);
        }

        Query { field_set }
    }

    pub fn run(&self, json: String) -> Record {
        Record {
            bytes: json.into_bytes(),
            fields: self.field_set.clone(),
        }
    }
}

pub struct Record<'a> {
    bytes: Vec<u8>,
    fields: HashSet<&'a str>,
}

impl<'a> Iterator for Record<'a> {
    type Item = (&'a str, String);

    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
