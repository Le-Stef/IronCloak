// Icone systray Windows avec menu contextuel.
// Utilise tray-icon pour l'icone et une boucle de messages Win32.
// L'icone change selon l'etat de connexion Tor (on/off).
// Double-clic sur l'icone ouvre la fenetre de configuration.

#![cfg(windows)]

use std::sync::Arc;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIconBuilder, TrayIconEvent, Icon,
};

use crate::gui::state::AppState;

// Icones PNG embarquees dans le binaire
const ICON_ON_PNG: &[u8] = include_bytes!("../../icon_256_on.png");
const ICON_OFF_PNG: &[u8] = include_bytes!("../../icon_256_off.png");

/// Decode un PNG en Icon compatible tray-icon
fn load_icon(png_data: &[u8]) -> Icon {
    let img = image::load_from_memory(png_data)
        .expect("Erreur de decodage de l'icone PNG")
        .into_rgba8();
    let (w, h) = img.dimensions();
    Icon::from_rgba(img.into_raw(), w, h).expect("Erreur de creation de l'icone")
}

/// Lance la boucle systray Windows. Bloquant jusqu'a la demande de fermeture.
pub fn run_tray(state: Arc<AppState>) {
    let icon_on = load_icon(ICON_ON_PNG);
    let icon_off = load_icon(ICON_OFF_PNG);

    // Construction du menu contextuel
    let status_item = MenuItem::new(crate::t!("gui.disconnected"), false, None);
    let configure_item = MenuItem::new(crate::t!("gui.configure"), true, None);
    let quit_item = MenuItem::new(crate::t!("gui.quit"), true, None);

    let menu = Menu::new();
    let _ = menu.append(&status_item);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&configure_item);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&quit_item);

    // Creation de l'icone systray (demarre en mode "off")
    let _tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip(format!("IronCloak :{}", state.get_port()))
        .with_icon(icon_off.clone())
        .build()
        .expect("Erreur de creation du systray");

    let configure_id = configure_item.id().clone();
    let quit_id = quit_item.id().clone();

    // Stocker l'ID du menu "Quitter" dans l'etat partage
    // pour que la fenetre egui puisse traiter cet evenement pendant qu'elle est ouverte
    state.set_tray_quit_menu_id(quit_id.as_ref().to_string());

    let mut was_connected = false;

    // Boucle de messages Win32 non-bloquante
    loop {
        // Traitement des messages Windows (necessaire pour le systray)
        unsafe {
            let mut msg: winapi::um::winuser::MSG = std::mem::zeroed();
            while winapi::um::winuser::PeekMessageW(
                &mut msg,
                std::ptr::null_mut(),
                0,
                0,
                winapi::um::winuser::PM_REMOVE,
            ) != 0
            {
                winapi::um::winuser::TranslateMessage(&msg);
                winapi::um::winuser::DispatchMessageW(&msg);
            }
        }

        // Verifier les evenements de clic sur l'icone (double-clic = ouvrir config)
        let mut open_config = false;
        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if matches!(event, TrayIconEvent::DoubleClick { .. }) {
                open_config = true;
            }
        }

        // Verifier les evenements du menu
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == configure_id {
                open_config = true;
            } else if event.id == quit_id {
                state.request_quit();
            }
        }

        // Ouvrir la fenetre de configuration si demande
        if open_config && !state.should_quit() {
            let state_clone = Arc::clone(&state);
            crate::gui::window::run_window(state_clone);
        }

        // Verifier si on doit quitter
        if state.should_quit() {
            break;
        }

        // Mise a jour de l'icone selon l'etat de connexion
        let connected = state.is_connected();
        if connected != was_connected {
            was_connected = connected;
            let new_icon = if connected {
                icon_on.clone()
            } else {
                icon_off.clone()
            };
            let _ = _tray_icon.set_icon(Some(new_icon));

            let status_text = if connected {
                crate::t!("gui.connected")
            } else {
                crate::t!("gui.disconnected")
            };
            status_item.set_text(status_text);
        }

        // Attendre 50ms pour ne pas saturer le CPU
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
