// Expected: exit 0
// Deserialize used via #[derive(Deserialize)] -- not a literal name reference.
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub name: String,
    pub port: u16,
}

fn main() {
    let _cfg = AppConfig { name: "app".to_string(), port: 8080 };
}
