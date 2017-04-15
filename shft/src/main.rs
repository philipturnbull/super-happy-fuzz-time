extern crate clap;
extern crate rand;
extern crate libshft;

use clap::{Arg, App, SubCommand};
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

fn do_fuzz<'buf>(parsed_file: &ParsedFile<'buf>, iterations: usize) {
    let mut rng = isaac::Isaac64Rng::from_seed(&[1, 2, 3, 4]);
    for i in 0..iterations {
        let fuzzed = fuzz_one(parsed_file, &mut rng, 5);

        let out_filename = format!("out/{}.pdf", i);
        let mut file = File::create(out_filename).expect("oops");
        file.write_all(&fuzzed[..]).expect("oops")
    }
}

fn parse_input_file<'buf>(config_filename: &str, buf: &'buf [u8]) -> ParsedFile<'buf> {
    let grammar = Grammar::from_path(config_filename);
    slurp(&grammar, buf)
}

fn main() {
    let app = App::new("super-happy-fuzz-time")
        .arg(Arg::with_name("INPUT")
            .help("File to fuzz")
            .long("input")
            .short("i")
            .number_of_values(1)
            .required(true))
        .arg(Arg::with_name("CONFIG")
            .help("Config file")
            .long("config")
            .short("c")
            .number_of_values(1)
            .required(true))
        .subcommand(
            SubCommand::with_name("dump")
                .help("Parse and dump input file"))
        .subcommand(
            SubCommand::with_name("fuzz")
                .help("Fuzz input file"));
    let matches = app.clone().get_matches();

    let config_filename = matches.value_of("CONFIG").expect("impossible");
    let input_filename = matches.value_of("INPUT").expect("impossible");

    match matches.subcommand() {
        ("dump", _) => {
            let buf = read_file(input_filename).expect("read_file");
            let parsed_file = parse_input_file(config_filename, &buf[..]);
            println!("{}", parsed_file.dump());
        },
        ("fuzz", _) => {
            let buf = read_file(input_filename).expect("read_file");
            let parsed_file = parse_input_file(config_filename, &buf[..]);
            do_fuzz(&parsed_file, 10);
        },
        _ => {
                let mut out = io::stdout();
                app.write_help(&mut out).expect("app.write_help");
                write!(out, "\n\nMust provide 'dump' or 'fuzz'\n").expect("write newline");
        },
    }
}
