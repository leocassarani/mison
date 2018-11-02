extern crate mison;

use mison::{KeyPath, Query, Value};

use std::fs::File;
use std::io::{BufRead, BufReader, Result};

fn main() -> Result<()> {
    let file = File::open("examples/yelp.json")?;

    let query = Query::new(vec![
        KeyPath::Field("name".to_owned()),
        KeyPath::Field("stars".to_owned()),
        KeyPath::Nested(vec!["hours".to_owned(), "Saturday".to_owned()]),
    ]);

    for record in BufReader::new(file).lines() {
        for (key, value) in query.run(record?) {
            match value {
                Value::String(s) => println!("{}: {}", key, s),
                Value::Number(n) => println!("{}: {}", key, n),
                _ => println!("{}: {:?}", key, value),
            }
        }
    }

    Ok(())
}
