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
use libshft::parse::{ParsedFile, slurp};
use libshft::fuzz::fuzz_one;

#[cfg(test)]
mod test {
    use libshft::grammar::{Grammar, GrammarDef};
    use libshft::parse::slurp;
    use libshft::fuzz::FuzzFile;

    fn roundtrip(grammar: &Grammar, buf: &[u8]) -> bool {
        let parsed_file = slurp(grammar, buf);
        println!("parsed = {:?}", parsed_file.dump());
        let ff = FuzzFile::new(&parsed_file);
        let serialized = ff.serialize();
        println!("serialized = {:#?}", serialized);
        serialized == buf
    }

    #[test]
    fn test_whitespace() {
        let grammar = Grammar::new(vec![], vec![b" ".to_vec()]);
        let buf = b"1 2 3";
        assert!(roundtrip(&grammar, buf))
    }

    #[test]
    fn test_two_whitespaces() {
        let grammar = Grammar::new(vec![], vec![b" ".to_vec(), b"\r\n".to_vec()]);
        let buf = b"1 2\r\n3 \r\n4";
        assert!(roundtrip(&grammar, buf))
    }

    #[test]
    fn test_delim() {
        let grammar = Grammar::new(vec![
            GrammarDef::Delim(vec![b'<', b'<'], vec![b'>', b'>']),
        ], vec![]);
        assert!(roundtrip(&grammar, b"1<<2<<3>>4>>5"));
        assert!(roundtrip(&grammar, b"1<<2<<3>>4"));
        assert!(roundtrip(&grammar, b"1<<2>>3>>4"));
        assert!(roundtrip(&grammar, b"1<<2"));
        assert!(roundtrip(&grammar, b"1>>2"));
    }
}

fn read_file<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
    let mut f = File::open(path)?;
    let mut buffer = Vec::new();

    match f.read_to_end(&mut buffer) {
        Ok(_) => Ok(buffer),
        Err(err) => Err(err),
    }
}

fn do_fuzz<'buf>(parsed_file: &ParsedFile<'buf>) {
    let mut rng = isaac::Isaac64Rng::from_seed(&[1, 2, 3, 4]);
    for i in 0..50000 {
        let fuzzed = fuzz_one(parsed_file, &mut rng, 5);

        let out_filename = format!("out/{}.pdf", i);
        let mut file = File::create(out_filename).expect("oops");
        file.write_all(&fuzzed[..]).expect("oops")
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
        let parsed_file = slurp(&grammar, &buf);

        if matches.occurrences_of("dump") != 0 {
            println!("{}", parsed_file.dump());
        } else {
            do_fuzz(&parsed_file);
        }
    }
}
