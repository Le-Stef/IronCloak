// Module GUI — dispatch selon la plateforme.
// Windows : icone systray + fenetre egui a la demande
// Linux : fenetre egui directement

pub mod state;
pub mod window;

#[cfg(windows)]
pub mod tray;

use std::sync::Arc;
use state::AppState;

/// Lance l'interface graphique appropriee selon la plateforme.
/// Cette fonction est bloquante et doit etre appelee sur le thread principal.
pub fn run_gui(state: Arc<AppState>) {
    #[cfg(windows)]
    {
        tray::run_tray(state);
    }

    #[cfg(not(windows))]
    {
        window::run_window(state);
    }
}
