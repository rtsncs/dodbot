use serde_derive::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub token: String,
    pub db_string: String,
    pub lava_address: String,
    pub lava_port: u16,
    pub lava_password: String,
    pub spotify_id: String,
    pub spotify_secret: String,
    pub genius_token: String,
}
