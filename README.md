# IronCloak - Proxy SOCKS5 vers Tor

Application Rust qui fait transiter le trafic TCP a travers le réseau Tor via un proxy SOCKS5 local. L'application s'intègre dans la zone de notification Windows (Systray) et propose une interface graphique de configuration.

## Description

IronCloak démarre un serveur SOCKS5 local et route toutes les connexions entrantes a travers un circuit Tor. Le client Tor est embarqué dans l'application grâce a `arti-client` (implémentation Tor en Rust) : aucune installation externe de Tor n'est nécessaire.

L'application fonctionne sans fenêtre console. Une icône dans la zone de notification indique l'état de la connexion Tor (connecte/déconnecté) et donne accès à la fenêtre de configuration.

## Fonctionnalités

- **Proxy SOCKS5 local** : écoute sur `127.0.0.1:9150` par défaut, configurable
- **Client Tor embarque** : bootstrap automatique via `arti-client`, pas de dépendance externe
- **Systray Windows** : icône avec changement d'état (on/off), menu contextuel, double-clic pour configurer
- **Interface graphique** : fenêtre pour modifier le port, la langue, voir le statut de connexion
- **Internationalisation** : anglais, français, espagnol : changement de langue avec apercu instantané
- **Rotation des logs** : journaux quotidiens organisés par année/mois (`logs/2026/02/ironcloak.2026-02-21`)
- **Rejet des IP directes** : option `dns_reject_ip` pour forcer le passage des requêtes DNS par Tor
- **Redémarrage depuis l'interface** : bouton pour relancer l'application après un changement de configuration

## Structure du projet

```
IronCloak/
├── src/
│   ├── main.rs          # Point d'entrée, runtime tokio, lancement GUI
│   ├── config.rs         # Dé-sérialisation TOML, sauvegarde de la configuration
│   ├── tor.rs            # Bootstrap du client Tor via arti-client
│   ├── socks.rs          # Serveur SOCKS5, relais bidirectionnel via Tor
│   ├── i18n.rs           # Internationalisation (chargement JSON, macro t!())
│   └── gui/
│       ├── mod.rs        # Dispatch plateforme (Systray Windows / fenêtre Linux) et désolé, je n'ai pas de Mac
│       ├── state.rs      # Etat partagé entre GUI et tokio (atomics)
│       ├── tray.rs       # Icône systray Windows, boucle messages Win32
│       └── window.rs     # Fenêtre "egui" (configuration, statut)
├── langs/
│   ├── en.json           # Traductions en anglais
│   ├── fr.json           # Traductions en français
│   └── es.json           # Traductions en espagnol
├── icon_256_on.png       # Icône Systray Tor connecté
├── icon_256_off.png      # Icône Systray Tor déconnecté
├── ironcloak.toml        # Fichier de configuration
├── Cargo.toml            # Dépendances Rust
└── README.md             # Ce fichier
```

## Installation et utilisation

### Prérequis

- Rust 1.93+ ([Installation](https://www.rust-lang.org/tools/install))

- soit Windows 10/11 (Systray)
- soit Linux (seule la fenêtre est affichée)

### Compilation

```bash
cargo build --release
```

Le binaire se trouve dans `target/release/ironcloak.exe` (~12 Mo). La compilation est longue la première fois (~500 crates nécessaires à `arti-client`). Sur une machine avec peu de mémoire ou peu de bande passante, limitez les jobs :

```bash
CARGO_BUILD_JOBS=2 cargo build --release
```

### Exécution

```bash
./target/release/ironcloak.exe
```

Ou avec un fichier de configuration spécifique :

```bash
./target/release/ironcloak.exe --config /chemin/vers/ironcloak.toml
```

## Configuration

Fichier `ironcloak.toml` :

```toml
[proxy]
# Adresse d'écoute en local
listen_addr = "127.0.0.1"
# Port SOCKS5
listen_port = 9150
# Rejeter les requêtes avec des IP brutes (force le DNS via Tor)
dns_reject_ip = true

[tor]
# Répertoire pour l'état et le cache de Tor
data_dir = "./data/arti"

[logging]
# Niveau de traces : debug | info | warn | error
level = "info"
# Répertoire des journaux
log_dir = "./logs"
# Langue des messages : en | fr | es
language = "fr"
```

Le port et la langue peuvent aussi être modifiés depuis la fenêtre de configuration (clic-droit sur l'icône Systray puis "Configurer", ou double-clic sur l'icône). Les changements sont sauvegardés dans le fichier TOML et appliqués au prochain redémarrage.

## Architecture

### Threads

- **Thread principal** : interface graphique (Systray Windows ou fenêtre "egui" Linux)
- **Thread secondaire** : runtime tokio avec le bootstrap Tor et le serveur SOCKS5

La communication entre les deux threads passe par un `AppState` partagé contenant des types atomiques (`AtomicBool`, `AtomicU16`).

### Traitement d'une connexion

1. Le client se connecte au proxy SOCKS5 local
2. Le handshake SOCKS5 est finalisé (sans authentification)
3. L'adresse de destination est extraite de la requête SOCKS5
4. Un flux Tor est ouvert vers la destination via `arti-client`
5. Un relais bidirectionnel est mis en place entre le client et le circuit Tor
6. Le relais se termine quand l'une des deux parties ferme la connexion

### Internationalisation

Les traductions sont stockées dans les fichiers JSON idoines (`langs/*.json`) et embarquées dans le binaire via `include_str!`. Un aplatissement en clefs à points (`tor.connected`, `socks.listening`) permet un accès rapide. La macro `t!()` fournit l'accès aux messages avec support des arguments positionnels :

```rust
tracing::info!("{}", t!("socks.listening", &bind_addr));
```

## Dépendances principales

| Crate | Rôle |
|-------|------|
| `arti-client` | Client Tor embarqué |
| `fast-socks5` | Serveur SOCKS5 |
| `tokio` | Runtime asynchrone |
| `eframe` / `egui` | Interface graphique |
| `tray-icon` | Icône Systray Windows |
| `tracing` | Journalisation structurée |
| `serde` / `toml` | Configuration TOML |
| `clap` | Arguments en ligne de commande |

## Auteur

- Le-Stef

## Licence

Licence Apache 2.0 - voir le fichier [LICENSE](LICENSE) pour plus de détails
