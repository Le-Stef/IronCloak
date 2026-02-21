// Module d'internationalisation (i18n)
// Charge les traductions depuis des fichiers JSON embarques dans le binaire
// et fournit une macro t!() pour acceder aux messages traduits.
// Utilise un RwLock pour permettre le changement de langue a chaud.

use std::collections::HashMap;
use std::sync::RwLock;

// Fichiers JSON embarques dans le binaire
const EN_JSON: &str = include_str!("../langs/en.json");
const FR_JSON: &str = include_str!("../langs/fr.json");
const ES_JSON: &str = include_str!("../langs/es.json");

// Singleton global contenant les traductions chargees (remplacable via RwLock)
static I18N: RwLock<Option<I18nStore>> = RwLock::new(None);

/// Stockage des traductions pour la langue selectionnee et le fallback anglais
struct I18nStore {
    current: HashMap<String, String>,
    fallback: HashMap<String, String>,
}

/// Initialise ou reinitialise le systeme i18n avec la langue demandee.
/// Peut etre appele plusieurs fois pour changer de langue.
pub fn init(language: &str) {
    let current_json = match language {
        "fr" => FR_JSON,
        "es" => ES_JSON,
        _ => EN_JSON,
    };

    let current = flatten_json(current_json);
    let fallback = if language == "en" {
        current.clone()
    } else {
        flatten_json(EN_JSON)
    };

    let mut store = I18N.write().unwrap();
    *store = Some(I18nStore { current, fallback });
}

/// Recupere un message traduit par sa cle pointee (ex: "tor.connected").
/// Retourne le fallback anglais si la cle n'existe pas dans la langue courante.
pub fn get(key: &str) -> String {
    let store = I18N.read().unwrap();
    let store = store.as_ref().expect("i18n non initialise — appeler i18n::init() d'abord");
    if let Some(val) = store.current.get(key) {
        val.clone()
    } else if let Some(val) = store.fallback.get(key) {
        val.clone()
    } else {
        key.to_string()
    }
}

/// Recupere un message traduit et remplace les arguments positionnels {0}, {1}, etc.
pub fn get_with_args(key: &str, args: &[&str]) -> String {
    let template = get(key);
    let mut result = template;
    for (i, arg) in args.iter().enumerate() {
        result = result.replace(&format!("{{{}}}", i), arg);
    }
    result
}

/// Aplatit un JSON imbrique en cles pointees.
/// Ex: {"tor": {"connected": "ok"}} → {"tor.connected": "ok"}
fn flatten_json(json_str: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str) {
        flatten_value(&value, "", &mut map);
    }
    map
}

fn flatten_value(value: &serde_json::Value, prefix: &str, map: &mut HashMap<String, String>) {
    match value {
        serde_json::Value::Object(obj) => {
            for (key, val) in obj {
                let new_prefix = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", prefix, key)
                };
                flatten_value(val, &new_prefix, map);
            }
        }
        serde_json::Value::String(s) => {
            map.insert(prefix.to_string(), s.clone());
        }
        _ => {}
    }
}

/// Macro pour acceder facilement aux traductions.
/// Usage : t!("tor.connected") ou t!("socks.listening", bind_addr)
#[macro_export]
macro_rules! t {
    ($key:expr) => {
        $crate::i18n::get($key)
    };
    ($key:expr, $($arg:expr),+) => {{
        let args: Vec<String> = vec![$($arg.to_string()),+];
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        $crate::i18n::get_with_args($key, &refs)
    }};
}
