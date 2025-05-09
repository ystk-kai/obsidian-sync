use std::env;

use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub couchdb: CouchDbConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CouchDbConfig {
    pub url: String,
    pub username: String,
    pub password: String,
}

impl AppConfig {
    pub fn load() -> Result<Self, ConfigError> {
        // Get the environment (default is development)
        let env = env::var("RUN_ENV").unwrap_or_else(|_| "development".into());

        // Start with default configuration
        let config = Config::builder()
            // Load default configuration from files
            .add_source(File::with_name("config/default").required(false))
            // Load environment-specific configuration
            .add_source(File::with_name(&format!("config/{}", env)).required(false))
            // Add environment variables (with prefix APP_)
            .add_source(Environment::with_prefix("APP").separator("_"))
            // Override with specific environment variables for CouchDB
            .add_source(Environment::with_prefix("COUCHDB").separator("_"))
            .build()?;

        // Deserialize into our config struct
        config.try_deserialize()
    }

    /// Create a config object from environment variables directly (for containerized deployment)
    pub fn from_env() -> Self {
        let couchdb_url =
            env::var("COUCHDB_URL").unwrap_or_else(|_| "http://couchdb:5984".to_string());

        // Parse the CouchDB URL to extract auth if present
        let mut username = env::var("COUCHDB_USER").unwrap_or_else(|_| "admin".to_string());
        let mut password = env::var("COUCHDB_PASSWORD").unwrap_or_else(|_| "secret".to_string());

        if couchdb_url.contains('@') {
            if let Ok(url) = url::Url::parse(&couchdb_url) {
                if !url.username().is_empty() {
                    username = url.username().to_string();
                    if let Some(pass) = url.password() {
                        password = pass.to_string();
                    }
                }
            }
        }

        // Clean the URL if it contains auth
        let clean_url = if couchdb_url.contains('@') {
            if let Ok(mut url) = url::Url::parse(&couchdb_url) {
                url.set_username("").unwrap_or_default();
                url.set_password(None).unwrap_or_default();
                url.to_string()
            } else {
                couchdb_url
            }
        } else {
            couchdb_url
        };

        // Ensure the URL ends with a slash
        let url_with_slash = if clean_url.ends_with('/') {
            clean_url
        } else {
            format!("{}/", clean_url)
        };

        AppConfig {
            server: ServerConfig {
                host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
                port: env::var("PORT")
                    .ok()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(3000),
            },
            couchdb: CouchDbConfig {
                url: url_with_slash,
                username,
                password,
            },
        }
    }
}
