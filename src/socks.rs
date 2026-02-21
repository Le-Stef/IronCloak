// Serveur SOCKS5 qui relaie les connexions a travers le reseau Tor.
// Chaque connexion entrante est traitee dans une tache tokio separee.
// Le flux bidirectionnel est assure entre le client et le circuit Tor.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use arti_client::{StreamPrefs, TorClient};
use fast_socks5::server::{Config as SocksConfig, DenyAuthentication, Socks5Server, Socks5Socket};
use fast_socks5::util::target_addr::TargetAddr;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio_stream::StreamExt;
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tokio_util::compat::FuturesAsyncWriteCompatExt;
use tor_rtcompat::PreferredRuntime;

use crate::config::IronCloakConfig;

// Compteur atomique pour identifier chaque connexion
static CONNECTION_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Lance le serveur SOCKS5 et accepte les connexions en boucle.
/// Chaque connexion est traitee dans une tache tokio independante.
pub async fn run_socks_server(
    config: &IronCloakConfig,
    tor_client: Arc<TorClient<PreferredRuntime>>,
) -> Result<()> {
    let bind_addr = format!("{}:{}", config.proxy.listen_addr, config.proxy.listen_port);
    let dns_reject_ip = config.proxy.dns_reject_ip;

    // Configuration du serveur SOCKS5 : pas de resolution DNS ni d'execution de commandes
    let mut socks_config = SocksConfig::<DenyAuthentication>::default();
    socks_config.set_dns_resolve(false);
    socks_config.set_execute_command(false);

    let server = Socks5Server::<DenyAuthentication>::bind(&bind_addr)
        .await
        .with_context(|| crate::t!("socks.bind_failed", &bind_addr))?
        .with_config(socks_config);

    tracing::info!("{}", crate::t!("socks.listening", &bind_addr));

    // Boucle d'acceptation des connexions entrantes
    let mut incoming = server.incoming();

    while let Some(socket_result) = incoming.next().await {
        let socket = match socket_result {
            Ok(socket) => socket,
            Err(e) => {
                tracing::warn!("{}", crate::t!("socks.accept_failed", e));
                continue;
            }
        };

        let conn_id = CONNECTION_COUNTER.fetch_add(1, Ordering::Relaxed);
        let tor = Arc::clone(&tor_client);

        tokio::spawn(async move {
            tracing::debug!("{}", crate::t!("socks.new_connection", conn_id));
            if let Err(e) = handle_client(socket, tor, dns_reject_ip, conn_id).await {
                tracing::warn!("{}", crate::t!("socks.connection_error", conn_id, e));
            }
            tracing::debug!("{}", crate::t!("socks.connection_closed", conn_id));
        });
    }

    Ok(())
}

/// Traite une connexion client individuelle :
/// handshake SOCKS5, connexion via Tor, puis relais bidirectionnel.
async fn handle_client(
    socket: Socks5Socket<TcpStream, DenyAuthentication>,
    tor_client: Arc<TorClient<PreferredRuntime>>,
    dns_reject_ip: bool,
    conn_id: u64,
) -> Result<()> {
    // Completer le handshake SOCKS5
    let socket = socket
        .upgrade_to_socks5()
        .await
        .map_err(|e| anyhow::anyhow!("{}", crate::t!("socks.handshake_failed", e)))?;

    let target = match socket.target_addr() {
        Some(addr) => addr.clone(),
        None => {
            anyhow::bail!("{}", crate::t!("socks.no_target"));
        }
    };

    // Extraire l'hote et le port de l'adresse cible
    let (host, port) = match &target {
        TargetAddr::Ip(sock_addr) => {
            if dns_reject_ip {
                tracing::warn!("{}", crate::t!("socks.ip_rejected", conn_id, sock_addr));
                anyhow::bail!("{}", crate::t!("socks.ip_rejected_bail"));
            }
            (sock_addr.ip().to_string(), sock_addr.port())
        }
        TargetAddr::Domain(domain, port) => (domain.clone(), *port),
    };

    tracing::info!("{}", crate::t!("socks.connecting", conn_id, &host, port));

    let prefs = StreamPrefs::new();

    // Ouvrir un flux Tor vers la destination avec un timeout de 60 secondes
    tracing::debug!("{}", crate::t!("socks.opening_stream", conn_id, &host, port));
    let tor_stream = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        tor_client.connect_with_prefs((&*host, port), &prefs),
    )
    .await
    .map_err(|_| anyhow::anyhow!("{}", crate::t!("socks.connect_timeout", conn_id, &host, port)))?
    .map_err(|e| anyhow::anyhow!("{}", crate::t!("socks.connect_failed", &host, port, e)))?;

    tracing::info!("{}", crate::t!("socks.stream_established", conn_id, &host, port));

    // Recuperer le flux TCP sous-jacent et envoyer la reponse SOCKS5 manuellement
    // (necessaire car execute_command=false signifie que la bibliotheque ne l'envoie pas)
    let mut client_stream = socket.into_inner();

    // Reponse SOCKS5 : VER=5, REP=0 (succes), RSV=0, ATYP=1 (IPv4), BND.ADDR=0.0.0.0, BND.PORT=0
    let reply = [0x05, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    client_stream.write_all(&reply).await?;
    client_stream.flush().await?;

    tracing::debug!("{}", crate::t!("socks.socks_reply_sent", conn_id));

    // Separer le DataStream en lecteur et ecrivain
    let (tor_reader, tor_writer) = tor_stream.split();

    // Convertir les AsyncRead/Write de futures en AsyncRead/Write de tokio
    let mut tor_read = tor_reader.compat();
    let mut tor_write = tor_writer.compat_write();

    // Relais bidirectionnel entre le client et Tor
    let (mut client_read, mut client_write) = tokio::io::split(client_stream);

    let (client_to_tor, tor_to_client) = tokio::join!(
        tokio::io::copy(&mut client_read, &mut tor_write),
        tokio::io::copy(&mut tor_read, &mut client_write),
    );

    match (client_to_tor, tor_to_client) {
        (Ok(up), Ok(down)) => {
            tracing::debug!("{}", crate::t!("socks.relay_complete", conn_id, up, down));
        }
        (Err(e), _) | (_, Err(e)) => {
            tracing::debug!("{}", crate::t!("socks.relay_ended", conn_id, e));
        }
    }

    Ok(())
}
