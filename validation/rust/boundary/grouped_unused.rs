// Expected: exit 0
// Known limitation: grouped imports are not checked individually.
use std::{io, fs};

fn main() {
    let _ = io::stdin();
    // fs is unused but grouped imports are not decomposed
}
