#![recursion_limit = "1024"]

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate error_chain;

pub mod error;
pub mod grammar;
pub mod parse;
pub mod fuzz;
