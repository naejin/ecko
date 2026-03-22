// Expected: exit 0
// Trait imports used implicitly via method calls (allowlisted).
use std::io::Write;
use std::fmt::Display;

fn write_to_file(path: &str, data: &[u8]) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    file.write_all(data)?;
    Ok(())
}

fn print_value<T: Display>(value: T) {
    println!("Value: {}", value);
}

fn main() {
    let _ = write_to_file("test.txt", b"hello");
    print_value(42);
}
