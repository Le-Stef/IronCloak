// Bootstrap du client Tor via arti-client.
// Configure les repertoires de cache et d'etat, puis demarre la connexion au reseau Tor.

use std::sync::Arc;

use anyhow::{Context, Result};
use arti_client::{TorClient, TorClientConfig};
use tor_config_path::CfgPath;
use tor_rtcompat::PreferredRuntime;

use crate::config::IronCloakConfig;

/// Demarre et connecte le client Tor avec la configuration fournie.
/// Retourne un client Tor pret a l'emploi, enveloppe dans un Arc pour le partage entre threads.
pub async fn bootstrap_tor(config: &IronCloakConfig) -> Result<Arc<TorClient<PreferredRuntime>>> {
    tracing::info!("{}", crate::t!("tor.configuring"));

    let data_dir = &config.tor.data_dir;
    let cache_path = format!("{}/cache", data_dir);
    let state_path = format!("{}/state", data_dir);

    // Construire la configuration avec les chemins de stockage
    let mut builder = TorClientConfig::builder();
    builder
        .storage()
        .cache_dir(CfgPath::new(cache_path))
        .state_dir(CfgPath::new(state_path));

    let tor_config = builder
        .build()
        .context(crate::t!("tor.build_config_failed").to_string())?;

    tracing::info!("{}", crate::t!("tor.bootstrapping"));

    // Creer et amorcer le client Tor (peut prendre plusieurs secondes)
    let tor_client = TorClient::create_bootstrapped(tor_config)
        .await
        .context(crate::t!("tor.bootstrap_failed").to_string())?;

    tracing::info!("{}", crate::t!("tor.bootstrap_complete"));

    Ok(Arc::new(tor_client))
}
