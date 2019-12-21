use dirs::home_dir;
use ssb_crypto::{PublicKey, SecretKey, generate_longterm_keypair};
use ssb_keyfile::load_keys_from_path;
use std::path::PathBuf;

const DEFAULT_SSB_DIR: &'static str = ".ssb";

pub struct SsbConfig {
    config_dir: PathBuf,
    public_key: PublicKey,
    secret_key: SecretKey,
}

impl SsbConfig {

    pub fn default() -> SsbConfig {
        SsbConfig::from_dir(DEFAULT_SSB_DIR)
    }

    pub fn from_dir(ssb_dir: &str) -> SsbConfig {
        let mut config_dir = home_dir().expect("Cannot find home directory.");
        config_dir.push(ssb_dir);

        let mut keyfile = config_dir.clone();
        keyfile.push("secret");

        let (public_key, secret_key) = load_or_generate_keys(keyfile);

        SsbConfig {
            config_dir,
            public_key,
            secret_key,
        }
    }

    pub fn keys(&self) -> (&PublicKey, &SecretKey) {
        (&self.public_key, &self.secret_key)
    }

}

fn load_or_generate_keys(path: PathBuf) -> (PublicKey, SecretKey) {
    load_keys_from_path(path).unwrap_or(generate_longterm_keypair())
}
