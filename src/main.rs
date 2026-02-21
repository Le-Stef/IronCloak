// Point d'entree principal d'IronCloak.
// Le thread principal gere l'interface graphique (systray Windows ou fenetre egui Linux).
// Un thread secondaire execute le runtime tokio pour le bootstrap Tor et le serveur SOCKS5.

// En mode release sur Windows, masquer la console
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod gui;
mod i18n;
mod socks;
mod tor;

use std::path::PathBuf;
use std::sync::Arc;

use chrono::Local;
use clap::Parser;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use config::IronCloakConfig;
use gui::state::AppState;

#[derive(Parser, Debug)]
#[command(name = "ironcloak", about = "SOCKS5 proxy routing traffic through Tor")]
struct Cli {
    /// Chemin vers le fichier de configuration
    #[arg(short, long, default_value = "ironcloak.toml")]
    config: PathBuf,
}

fn main() {
    // Parser les arguments CLI
    let cli = Cli::parse();

    // Initialiser i18n avec l'anglais par defaut (avant le chargement de la config)
    i18n::init("en");

    // Charger la configuration
    let config = match IronCloakConfig::load(&cli.config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Configuration error: {}", e);
            std::process::exit(1);
        }
    };

    // Reinitialiser i18n avec la langue configuree
    let language = config.logging.language.as_deref().unwrap_or("en");
    i18n::init(language);

    // Initialiser le logging (fichier uniquement sur Windows release, stdout + fichier sinon)
    let filter_str = &config.logging.level;

    // Calculer le repertoire mensuel de logs : {log_dir}/AAAA/MM/
    let now = Local::now();
    let log_dir = PathBuf::from(&config.logging.log_dir)
        .join(now.format("%Y").to_string())
        .join(now.format("%m").to_string());
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("Failed to create log directory: {}", e);
        std::process::exit(1);
    }

    // Appender de fichier avec rotation quotidienne dans le repertoire mensuel
    let file_appender = tracing_appender::rolling::daily(&log_dir, "ironcloak");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = fmt::layer()
        .with_ansi(false)
        .with_target(false)
        .with_writer(non_blocking);

    let filter = EnvFilter::try_new(filter_str)
        .unwrap_or_else(|_| EnvFilter::new("info"));

    // Sur Linux (ou en mode debug), ajouter aussi la sortie stdout
    #[cfg(not(windows))]
    {
        let stdout_layer = fmt::layer()
            .with_ansi(false)
            .with_target(false);

        tracing_subscriber::registry()
            .with(filter)
            .with(stdout_layer)
            .with(file_layer)
            .init();
    }

    #[cfg(windows)]
    {
        #[cfg(debug_assertions)]
        {
            let stdout_layer = fmt::layer()
                .with_ansi(false)
                .with_target(false);

            tracing_subscriber::registry()
                .with(filter)
                .with(stdout_layer)
                .with(file_layer)
                .init();
        }

        #[cfg(not(debug_assertions))]
        {
            tracing_subscriber::registry()
                .with(filter)
                .with(file_layer)
                .init();
        }
    }

    tracing::info!("{}", t!("app.starting"));
    let bind_addr = format!("{}:{}", config.proxy.listen_addr, config.proxy.listen_port);
    tracing::info!("{}", t!("app.proxy_will_listen", &bind_addr));
    tracing::info!("{}", t!("app.config_loaded", language));

    // Creer l'etat partage entre GUI et tokio
    let state = Arc::new(AppState::new(
        config.proxy.listen_port,
        cli.config.clone(),
        language.to_string(),
    ));
    let state_for_runtime = Arc::clone(&state);

    // Lancer le runtime tokio sur un thread secondaire
    let config_clone = config.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Echec de creation du runtime tokio");
        rt.block_on(async move {
            run_backend(config_clone, state_for_runtime).await;
        });
    });

    // Thread principal : lancer l'interface graphique (bloquant)
    gui::run_gui(state);
}

/// Logique backend : bootstrap Tor puis lance le serveur SOCKS5
async fn run_backend(config: IronCloakConfig, state: Arc<AppState>) {
    // Bootstrap Tor
    let tor_client = match tor::bootstrap_tor(&config).await {
        Ok(client) => {
            // Marquer comme connecte pour l'interface graphique
            state.set_connected(true);
            client
        }
        Err(e) => {
            tracing::error!("{}", t!("app.runtime_error", e));
            return;
        }
    };

    // Lancer le serveur SOCKS5 avec surveillance de l'arret
    tokio::select! {
        result = socks::run_socks_server(&config, tor_client) => {
            if let Err(e) = result {
                tracing::error!("{}", t!("socks.server_error", e));
            }
        }
        _ = wait_for_quit(Arc::clone(&state)) => {
            tracing::info!("{}", t!("app.shutdown"));
        }
    }
}

/// Attend que l'etat passe en mode "quit" (demande depuis l'interface graphique)
async fn wait_for_quit(state: Arc<AppState>) {
    loop {
        if state.should_quit() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}
