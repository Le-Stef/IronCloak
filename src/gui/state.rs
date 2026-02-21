// Etat partage entre le thread GUI et le thread tokio.
// Utilise des types atomiques pour la synchronisation sans verrou.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::Mutex;

/// Etat global de l'application partage entre les threads
pub struct AppState {
    /// Indique si le client Tor est connecte et pret
    pub connected: AtomicBool,
    /// Port d'ecoute actuel du serveur SOCKS5
    pub port: AtomicU16,
    /// Port en attente (sera applique au prochain redemarrage), 0 = pas de changement
    pub pending_port: AtomicU16,
    /// Signal de demande d'arret de l'application
    pub quit: AtomicBool,
    /// Chemin vers le fichier de configuration
    pub config_path: PathBuf,
    /// Langue courante de l'application
    pub language: Mutex<String>,
    /// ID du menu item "Quitter" du systray (stocke comme String pour la portabilite)
    /// Permet a la fenetre egui de traiter les evenements menu pendant qu'elle est ouverte
    pub tray_quit_menu_id: Mutex<Option<String>>,
}

impl AppState {
    /// Cree un nouvel etat avec le port initial et le chemin de config
    pub fn new(port: u16, config_path: PathBuf, language: String) -> Self {
        Self {
            connected: AtomicBool::new(false),
            port: AtomicU16::new(port),
            pending_port: AtomicU16::new(0),
            quit: AtomicBool::new(false),
            config_path,
            language: Mutex::new(language),
            tray_quit_menu_id: Mutex::new(None),
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    pub fn set_connected(&self, val: bool) {
        self.connected.store(val, Ordering::Relaxed);
    }

    pub fn get_port(&self) -> u16 {
        self.port.load(Ordering::Relaxed)
    }

    pub fn get_pending_port(&self) -> u16 {
        self.pending_port.load(Ordering::Relaxed)
    }

    pub fn set_pending_port(&self, port: u16) {
        self.pending_port.store(port, Ordering::Relaxed);
    }

    pub fn get_language(&self) -> String {
        self.language.lock().unwrap().clone()
    }

    pub fn set_language(&self, lang: String) {
        *self.language.lock().unwrap() = lang;
    }

    pub fn should_quit(&self) -> bool {
        self.quit.load(Ordering::Relaxed)
    }

    pub fn request_quit(&self) {
        self.quit.store(true, Ordering::Relaxed);
    }

    pub fn set_tray_quit_menu_id(&self, id: String) {
        *self.tray_quit_menu_id.lock().unwrap() = Some(id);
    }

    pub fn get_tray_quit_menu_id(&self) -> Option<String> {
        self.tray_quit_menu_id.lock().unwrap().clone()
    }
}
