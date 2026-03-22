// Expected: exit 0
// Test module imports must not mask main-code usage.
use std::path::Path;

pub fn is_config(path: &str) -> bool {
    Path::new(path).extension().map_or(false, |e| e == "toml")
}

fn main() {
    println!("{}", is_config("config.toml"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toml() {
        assert!(is_config("config.toml"));
    }

    #[test]
    fn test_not_toml() {
        assert!(!is_config("config.json"));
    }
}
