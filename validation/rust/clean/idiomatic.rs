// Expected: exit 0
use std::fs;
use std::path::Path;

fn load_config(path: &str) -> Result<String, std::io::Error> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "missing"));
    }
    fs::read_to_string(p)
}

fn main() {
    match load_config("config.toml") {
        Ok(content) => println!("{}", content),
        Err(e) => eprintln!("Error: {}", e),
    }
}
