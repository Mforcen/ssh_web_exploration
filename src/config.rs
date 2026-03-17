use serde::Deserialize;

#[derive(Deserialize, Debug, Default)]
pub struct WebexServeUserConfig {
    pub user: String,
    pub pass_hash: String,
}

#[derive(Deserialize, Debug, Default)]
pub struct WebexServeConfig {
    #[serde(default)]
    pub bind: Option<String>,
    #[serde(default)]
    pub index_path: Option<String>,
    #[serde(default)]
    pub auth: Option<WebexServeUserConfig>,
}

#[derive(Deserialize, Debug)]
pub struct WebexPeerConfig {
    pub hostname: String,
    #[serde(default)]
    pub root: Option<String>,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub key_path: Option<String>,
}

#[derive(Deserialize, Debug, Default)]
pub struct WebexLocalConfig {
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct WebexConfig {
    #[serde(default)]
    pub serve: WebexServeConfig,
    pub peer: WebexPeerConfig,
    #[serde(default)]
    pub local: WebexLocalConfig,
}
