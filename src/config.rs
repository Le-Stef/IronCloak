// Configuration de l'application IronCloak
// Deserialise le fichier TOML avec des valeurs par defaut pour chaque section.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Configuration racine de l'application
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IronCloakConfig {
    #[serde(default)]
    pub proxy: ProxyConfig,
    #[serde(default)]
    pub tor: TorConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
}

/// Configuration du proxy SOCKS5
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProxyConfig {
    #[serde(default = "default_listen_addr")]
    pub listen_addr: String,
    #[serde(default = "default_listen_port")]
    pub listen_port: u16,
    #[serde(default = "default_true")]
    pub dns_reject_ip: bool,
}

/// Configuration du client Tor (repertoire de donnees)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TorConfig {
    #[serde(default = "default_data_dir")]
    pub data_dir: String,
}

/// Configuration du logging (niveau, repertoire, langue)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default = "default_log_dir")]
    pub log_dir: String,
    /// Langue des messages de trace : "en", "fr", "es" (defaut : "en")
    #[serde(default)]
    pub language: Option<String>,
}

fn default_listen_addr() -> String {
    "127.0.0.1".to_string()
}

fn default_listen_port() -> u16 {
    9150
}

fn default_true() -> bool {
    true
}

fn default_data_dir() -> String {
    "./data/arti".to_string()
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_dir() -> String {
    "./logs".to_string()
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            listen_addr: default_listen_addr(),
            listen_port: default_listen_port(),
            dns_reject_ip: default_true(),
        }
    }
}

impl Default for TorConfig {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            log_dir: default_log_dir(),
            language: None,
        }
    }
}

impl IronCloakConfig {
    /// Sauvegarde la configuration dans un fichier TOML.
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .context("Failed to serialize config to TOML")?;
        std::fs::write(path, content)
            .with_context(|| format!("Failed to write config file: {}", path.display()))?;
        Ok(())
    }

    /// Charge la configuration depuis un fichier TOML.
    /// Si le fichier n'existe pas, utilise les valeurs par defaut.
    pub fn load(path: &Path) -> Result<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path)
                .with_context(|| {
                    crate::i18n::get_with_args("config.read_failed", &[&path.display().to_string()])
                })?;
            let config: IronCloakConfig = toml::from_str(&content)
                .with_context(|| crate::t!("config.parse_failed").to_string())?;
            Ok(config)
        } else {
            tracing::warn!("{}", crate::t!("config.file_not_found", path.display()));
            Ok(Self::default())
        }
    }
}

impl Default for IronCloakConfig {
    fn default() -> Self {
        Self {
            proxy: ProxyConfig::default(),
            tor: TorConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}
