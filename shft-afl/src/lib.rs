#![feature(drop_types_in_const)]
extern crate libc;
extern crate rand;
extern crate libshft;

use libc::{c_void, size_t};
use rand::isaac;

use libshft::grammar::Grammar;
use libshft::parse::{ParsedFile, slurp};
use libshft::fuzz;

static mut GRAMMAR: Option<Grammar> = None;
static mut PARSED_FILE: Option<ParsedFile> = None;
static mut RNG: Option<isaac::Isaac64Rng> = None;

static mut FUZZ_CONFIG: Option<fuzz::FuzzConfig> = None;

#[no_mangle]
pub unsafe extern fn afl_fuzz_init() -> size_t {
    RNG = Some(isaac::Isaac64Rng::new_unseeded());
    GRAMMAR = Some(Grammar::from_path("/home/phil/super-happy-fuzz-time/config.yml"));

    FUZZ_CONFIG = Some(fuzz::FuzzConfig {
        max_mutations: 5,
        max_duplications: 5,
        valid_actions: fuzz::default_actions(),
    });
    0
}

#[no_mangle]
pub unsafe extern fn afl_parse_one(in_buf: *const c_void, in_len: size_t) -> size_t {
    if in_buf.is_null() || in_len == 0 {
        1
    } else {
        let in_slice = std::slice::from_raw_parts(in_buf as *const u8, in_len as usize);
        PARSED_FILE = Some(slurp(GRAMMAR.as_mut().unwrap(), in_slice));
        0
    }
}

#[no_mangle]
pub unsafe extern fn afl_fuzz_one(out_buf: *mut c_void, out_len: size_t) -> size_t {
    if out_buf.is_null() || out_len == 0 {
        0
    } else {
        let result = fuzz::fuzz_one(PARSED_FILE.as_mut().unwrap(), RNG.as_mut().unwrap(), FUZZ_CONFIG.as_mut().unwrap());

        match result {
            Some(fuzzed_file) => {
                let out_slice = std::slice::from_raw_parts_mut(out_buf as *mut u8, out_len as usize);
                let mut serialized = fuzz::SliceSerializer::new(out_slice);
                fuzzed_file.serialize(&mut serialized);
                serialized.bytes_written()
            },
            None => 0,
        }
    }
}
