extern crate serde;
extern crate serde_yaml;

use error::*;
use std::fmt::Display;
use std::fs::File;
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

    pub fn from_path<P: AsRef<Path> + Display>(path: P) -> Result<Grammar> {
        let f = File::open(&path).chain_err(|| format!("Failed to open grammar definition {}", path))?;
        let cfg = serde_yaml::from_reader::<_, ConfigFormat>(f).chain_err(|| "Failed to parse grammar defintion")?;

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

    pub fn delims(self: &Self) -> Vec<(Vec<u8>, Vec<u8>)> {
        self.defs.iter().filter_map(|def| {
            match def {
                &GrammarDef::Delim(ref start_pattern, ref end_pattern) => Some((start_pattern.clone(), end_pattern.clone())),
                _ => None,
            }
        }).collect()
    }
}
