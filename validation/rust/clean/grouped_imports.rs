// Expected: exit 0
// Grouped imports are not decomposed (known limitation).
use std::{io, fs};

fn read_file(path: &str) -> io::Result<String> {
    fs::read_to_string(path)
}

fn main() {
    match read_file("test.txt") {
        Ok(s) => println!("{}", s),
        Err(e) => eprintln!("{}", e),
    }
}
