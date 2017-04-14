extern crate clap;
extern crate rand;
extern crate libshft;

use clap::{Arg, App};
use rand::SeedableRng;
use rand::isaac;
use std::io;
use std::io::{Read, Write};
use std::fs::File;
use std::path::Path;
use libshft::grammar::Grammar;
use libshft::parse::slurp;
use libshft::fuzz::fuzz_one;

#[cfg(test)]
fn roundtrip(grammar: &Grammar, buf: &[u8]) -> bool {
    let parsed_file = slurp(grammar, buf);
    let ff = FuzzFile::new(&parsed_file);
    println!("parsed_file = {:#?}", parsed_file);
    let serialized = ff.serialize();
    println!("serialized = {:#?}", serialized);
    serialized == buf
}

#[test]
fn test_tokenizer() {
    let grammar = Grammar::new(vec![GrammarDef::Tokenizer(vec![b' '])]);
    let buf = b"1 2 3";
    assert!(roundtrip(&grammar, buf))
}

#[test]
fn test_two_tokenizers() {
    let grammar = Grammar::new(vec![
        GrammarDef::Tokenizer(vec![b' ']),
        GrammarDef::Tokenizer(vec![b'|']),
    ]);
    let buf = b"1 2|3 |4";
    assert!(roundtrip(&grammar, buf))
}

#[test]
fn test_delim() {
    let grammar = Grammar::new(vec![
        GrammarDef::Delim(vec![b'<', b'<'], vec![b'>', b'>']),
    ]);
    assert!(roundtrip(&grammar, b"1<<2<<3>>4>>5"));
    assert!(roundtrip(&grammar, b"1<<2<<3>>4"));
    assert!(roundtrip(&grammar, b"1<<2>>3>>4"));
    assert!(roundtrip(&grammar, b"1<<2"));
    assert!(roundtrip(&grammar, b"1>>2"));
}

fn read_file<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
    let mut f = File::open(path)?;
    let mut buffer = Vec::new();

    match f.read_to_end(&mut buffer) {
        Ok(_) => Ok(buffer),
        Err(err) => Err(err),
    }
}

fn main() {
    let matches = App::new("super-happy-fuzz-time")
        .arg(Arg::with_name("INPUT")
            .help("File to fuzz")
            .required(true)
            .index(1))
        .arg(Arg::with_name("CONFIG")
            .help("Config file")
            .long("config")
            .number_of_values(1)
            .required(true))
        .arg(Arg::with_name("dump")
            .help("Dump input file")
            .long("dump"))
        .get_matches();

    let input_filename = matches.value_of("INPUT").unwrap();
    let config_filename = matches.value_of("CONFIG").unwrap();

    if let Ok(buf) = read_file(input_filename) {
        let grammar = Grammar::from_path(config_filename);
        let parsed = slurp(&grammar, &buf);

        if matches.occurrences_of("dump") != 0 {
            let mut x = String::new();
            parsed.dump(&mut x).expect("parsed.dump");
            println!("{}", x);
        } else {
            let mut rng = isaac::Isaac64Rng::from_seed(&[1, 2, 3, 4]);
            for i in 0..50000 {
                let fuzzed = fuzz_one(&parsed, &mut rng, 5);

                let out_filename = format!("out/{}.pdf", i);
                let mut file = File::create(out_filename).expect("oops");
                file.write_all(&fuzzed[..]).expect("oops")
            }
        }
    }
}
