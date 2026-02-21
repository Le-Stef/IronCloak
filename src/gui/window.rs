// Fenetre egui pour le controle d'IronCloak.
// Affiche le statut de connexion Tor, le port SOCKS5, un selecteur de langue,
// un bouton Appliquer qui sauvegarde dans le fichier TOML,
// et un bouton Redemarrer qui relance l'application avec la nouvelle config.
// La fenetre reste au-dessus des autres et possede l'icone de l'application.

use std::sync::Arc;
use eframe::egui;
use crate::config::IronCloakConfig;
use crate::gui::state::AppState;

/// Icone PNG embarquee pour la fenetre
const WINDOW_ICON_PNG: &[u8] = include_bytes!("../../icon_256_on.png");

/// Les langues disponibles avec leur code et libelle
const LANGUAGES: &[(&str, &str)] = &[
    ("en", "English"),
    ("fr", "Francais"),
    ("es", "Espanol"),
];

/// Charge l'icone PNG et la convertit en IconData pour egui
fn load_window_icon() -> egui::IconData {
    let img = image::load_from_memory(WINDOW_ICON_PNG)
        .expect("Erreur de decodage de l'icone PNG")
        .into_rgba8();
    let (w, h) = img.dimensions();
    egui::IconData {
        rgba: img.into_raw(),
        width: w,
        height: h,
    }
}

/// Lance la fenetre egui. Bloquant jusqu'a la fermeture de la fenetre.
pub fn run_window(state: Arc<AppState>) {
    let icon = load_window_icon();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([380.0, 280.0])
            .with_resizable(false)
            .with_always_on_top()
            .with_icon(Arc::new(icon)),
        ..Default::default()
    };

    let _ = eframe::run_native(
        &crate::t!("gui.window_title"),
        options,
        Box::new(move |_cc| Ok(Box::new(IronCloakApp::new(state)))),
    );
}

/// Application egui principale
struct IronCloakApp {
    state: Arc<AppState>,
    port_input: String,
    selected_lang_index: usize,
    /// Index precedent de la langue pour detecter les changements
    prev_lang_index: usize,
    status_message: Option<(String, bool)>,
    /// Indique que la config a ete modifiee et sauvegardee (affiche le bouton Redemarrer)
    needs_restart: bool,
}

impl IronCloakApp {
    fn new(state: Arc<AppState>) -> Self {
        // Initialiser le port affiche : le port en attente s'il existe, sinon le port courant
        let pending = state.get_pending_port();
        let port_input = if pending > 0 {
            pending.to_string()
        } else {
            state.get_port().to_string()
        };

        // Trouver l'index de la langue courante
        let current_lang = state.get_language();
        let selected_lang_index = LANGUAGES.iter()
            .position(|(code, _)| *code == current_lang)
            .unwrap_or(0);

        // Si un port en attente existe, on a deja des changements non appliques
        let needs_restart = pending > 0 && pending != state.get_port();

        Self {
            state,
            port_input,
            selected_lang_index,
            prev_lang_index: selected_lang_index,
            status_message: None,
            needs_restart,
        }
    }

    /// Sauvegarde les changements dans le fichier TOML
    fn save_config(&mut self) {
        let new_port: u16 = match self.port_input.trim().parse() {
            Ok(p) if p > 0 => p,
            _ => {
                self.status_message = Some(("Invalid port".to_string(), false));
                return;
            }
        };

        let (lang_code, _) = LANGUAGES[self.selected_lang_index];
        let config_path = &self.state.config_path;

        // Charger la config existante, appliquer les modifications, sauvegarder
        let mut config = IronCloakConfig::load(config_path)
            .unwrap_or_default();

        config.proxy.listen_port = new_port;
        config.logging.language = Some(lang_code.to_string());

        match config.save(config_path) {
            Ok(()) => {
                // Mettre a jour le port en attente dans l'etat partage
                let current_port = self.state.get_port();
                if new_port != current_port {
                    self.state.set_pending_port(new_port);
                    self.needs_restart = true;
                } else {
                    self.state.set_pending_port(0);
                }

                // Mettre a jour la langue dans l'etat partage
                let current_lang = self.state.get_language();
                if lang_code != current_lang {
                    self.state.set_language(lang_code.to_string());
                    self.needs_restart = true;
                }

                tracing::info!("{}", crate::t!("gui.saved"));
                self.status_message = Some((crate::t!("gui.saved"), true));
            }
            Err(e) => {
                tracing::error!("{}", crate::t!("gui.save_failed", e));
                self.status_message = Some((crate::t!("gui.save_failed", e), false));
            }
        }
    }

    /// Relance l'application : spawn un nouveau processus puis demande l'arret du courant
    fn restart_app(&self) {
        let exe = std::env::current_exe().expect("Impossible de determiner le chemin de l'executable");
        let config_path = &self.state.config_path;

        // Lancer un nouveau processus avec le meme fichier de config
        let _ = std::process::Command::new(&exe)
            .arg("--config")
            .arg(config_path)
            .spawn();

        // Demander l'arret du processus courant
        self.state.request_quit();
    }

    /// Traite les evenements du menu systray pendant que la fenetre est ouverte (Windows)
    /// Permet de quitter l'application meme si la fenetre de config est affichee
    fn drain_tray_menu_events(&self) {
        #[cfg(windows)]
        {
            use tray_icon::menu::MenuEvent;
            if let Some(ref quit_id) = self.state.get_tray_quit_menu_id() {
                while let Ok(event) = MenuEvent::receiver().try_recv() {
                    if event.id.as_ref() == quit_id.as_str() {
                        self.state.request_quit();
                    }
                }
            }

            // Drainer aussi les evenements de clic sur l'icone pour eviter l'accumulation
            use tray_icon::TrayIconEvent;
            while TrayIconEvent::receiver().try_recv().is_ok() {}
        }
    }
}

impl eframe::App for IronCloakApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Rafraichir automatiquement toutes les secondes pour mettre a jour le statut
        ctx.request_repaint_after(std::time::Duration::from_secs(1));

        // Traiter les evenements systray (quit depuis le menu pendant que la fenetre est ouverte)
        self.drain_tray_menu_events();

        // Detecter le changement de langue dans la liste deroulante → apercu instantane
        if self.selected_lang_index != self.prev_lang_index {
            let (lang_code, _) = LANGUAGES[self.selected_lang_index];
            crate::i18n::init(lang_code);
            self.prev_lang_index = self.selected_lang_index;
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(crate::t!("gui.window_title"));
            ui.add_space(10.0);

            // Statut de connexion Tor avec indicateur colore
            let connected = self.state.is_connected();
            ui.horizontal(|ui| {
                ui.label(format!("{}: ", crate::t!("gui.status")));
                if connected {
                    ui.colored_label(egui::Color32::from_rgb(0, 180, 0), crate::t!("gui.connected"));
                } else {
                    ui.colored_label(egui::Color32::from_rgb(220, 0, 0), crate::t!("gui.disconnected"));
                }
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            // Champ de saisie du port SOCKS5
            ui.horizontal(|ui| {
                ui.label(crate::t!("gui.port_label"));
                ui.add(egui::TextEdit::singleline(&mut self.port_input).desired_width(80.0));

                // Afficher le port en attente s'il differe du port courant
                let current_port = self.state.get_port();
                if let Ok(input_port) = self.port_input.trim().parse::<u16>() {
                    if input_port != current_port {
                        ui.label(
                            egui::RichText::new(crate::t!("gui.pending_port", input_port))
                                .small()
                                .color(egui::Color32::from_rgb(180, 140, 0)),
                        );
                    }
                }
            });

            ui.add_space(8.0);

            // Selecteur de langue (le changement est applique instantanement a l'affichage)
            ui.horizontal(|ui| {
                ui.label(crate::t!("gui.language_label"));
                egui::ComboBox::from_id_salt("lang_combo")
                    .selected_text(LANGUAGES[self.selected_lang_index].1)
                    .show_ui(ui, |ui| {
                        for (i, (_code, label)) in LANGUAGES.iter().enumerate() {
                            ui.selectable_value(&mut self.selected_lang_index, i, *label);
                        }
                    });
            });

            ui.add_space(10.0);

            // Boutons Appliquer et Redemarrer sur la meme ligne
            ui.horizontal(|ui| {
                if ui.button(crate::t!("gui.apply")).clicked() {
                    self.save_config();
                }

                if self.needs_restart {
                    if ui.button(
                        egui::RichText::new(crate::t!("gui.restart")).color(egui::Color32::from_rgb(220, 120, 0))
                    ).clicked() {
                        self.save_config();
                        self.restart_app();
                    }
                }
            });

            ui.add_space(5.0);

            // Message de statut (succes en vert, erreur en rouge)
            if let Some((ref msg, success)) = self.status_message {
                let color = if success {
                    egui::Color32::from_rgb(0, 160, 0)
                } else {
                    egui::Color32::from_rgb(220, 0, 0)
                };
                ui.label(egui::RichText::new(msg.as_str()).small().color(color));
            }

            if self.needs_restart {
                ui.add_space(3.0);
                ui.label(
                    egui::RichText::new(crate::t!("gui.restart_required"))
                        .small()
                        .color(egui::Color32::GRAY),
                );
            }
        });

        // Si l'application doit quitter, fermer la fenetre
        if self.state.should_quit() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
