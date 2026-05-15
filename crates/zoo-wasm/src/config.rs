use serde::Deserialize;

#[derive(Deserialize)]
pub struct ZooConfig {
    pub base_url: String,
}
