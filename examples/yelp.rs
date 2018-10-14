extern crate mison;

use mison::{Query, Value};

use std::fs::File;
use std::io::{BufRead, BufReader, Result};

fn main() -> Result<()> {
    let file = File::open("examples/yelp.json")?;
    let query = Query::new(vec!["name", "stars"]);

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
