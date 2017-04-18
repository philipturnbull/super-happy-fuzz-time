extern crate serde;
extern crate serde_yaml;

use std::fs::File;
use std::io;
use std::io::Read;
use std::path::Path;

#[derive(Deserialize)]
struct ConfigFormat {
    delims: Vec<(String, String)>,
    breaks: Vec<String>,
    whitespace: Vec<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum GrammarDef {
    Breaker(Vec<u8>),
    Delim(Vec<u8>, Vec<u8>),
}

#[derive(Debug)]
pub struct Grammar {
    pub defs: Vec<GrammarDef>,
    pub whitespace: Vec<Vec<u8>>,
}

impl Grammar {
    pub fn new(defs: Vec<GrammarDef>, whitespace: Vec<Vec<u8>>) -> Self {
        Grammar {
            defs: defs,
            whitespace: whitespace,
        }
    }

    pub fn from_path<P: AsRef<Path>>(path: P) -> io::Result<Grammar> {
        let mut f = File::open(path)?;
        let mut s = String::new();
        f.read_to_string(&mut s).expect("read_to_string");

        let cfg = serde_yaml::from_str::<ConfigFormat>(&s).unwrap();

        let mut defs = Vec::new();
        let mut whitespace = Vec::new();

        for (start_pattern, end_pattern) in cfg.delims {
            defs.push(GrammarDef::Delim(start_pattern.into_bytes(), end_pattern.into_bytes()));
        }

        for pattern in cfg.whitespace {
            whitespace.push(pattern.into_bytes())
        }

        for pattern in cfg.breaks {
            defs.push(GrammarDef::Breaker(pattern.into_bytes()))
        }

        Ok(Grammar::new(defs, whitespace))
    }
}
