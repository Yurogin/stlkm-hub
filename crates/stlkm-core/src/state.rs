//! Ce que le hub a installé, et en quelle version.
//!
//! Un seul fichier JSON, partagé par la CLI et la GUI : les deux voient donc
//! exactement les mêmes projets installés.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::paths;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub installed: BTreeMap<String, Installed>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Installed {
    /// Tag de release, commit court, ou `local` pour un dossier déjà présent.
    pub version: String,
    /// Où sont les fichiers du projet.
    pub path: PathBuf,
    /// Date d'installation, en secondes depuis 1970.
    pub installed_at: u64,
}

impl Installed {
    pub fn new(version: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Installed {
            version: version.into(),
            path: path.into(),
            installed_at: now(),
        }
    }
}

impl State {
    /// Lit l'état sur disque. Un fichier absent ou corrompu donne un état vide
    /// plutôt qu'une erreur : le hub doit démarrer même après un accident.
    pub fn load() -> Result<Self> {
        Self::load_from(paths::state_file()?)
    }

    pub fn load_from(path: impl AsRef<Path>) -> Result<Self> {
        match std::fs::read_to_string(path) {
            // Un éditeur Windows peut coller un BOM en tête du fichier. Sans ce
            // retrait, il devient illisible et le hub oublie tout ce qui est
            // installé — c'est arrivé une fois, ça suffit.
            Ok(raw) => Ok(serde_json::from_str(sans_bom(&raw)).unwrap_or_default()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(State::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(paths::state_file()?)
    }

    pub fn save_to(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&Installed> {
        self.installed.get(id)
    }

    pub fn is_installed(&self, id: &str) -> bool {
        self.installed.contains_key(id)
    }

    pub fn record(&mut self, id: impl Into<String>, installed: Installed) {
        self.installed.insert(id.into(), installed);
    }

    pub fn forget(&mut self, id: &str) -> Option<Installed> {
        self.installed.remove(id)
    }
}

/// Retire la marque d'ordre des octets (BOM) que Windows ajoute volontiers en
/// tête des fichiers texte. Ni JSON ni TOML ne l'acceptent.
pub fn sans_bom(texte: &str) -> &str {
    texte.strip_prefix('\u{feff}').unwrap_or(texte)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_etat_absent_nest_pas_une_erreur() {
        let state = State::load_from("ce-fichier-nexiste-pas.json").expect("état vide");
        assert!(state.installed.is_empty());
    }

    #[test]
    fn un_etat_corrompu_redemarre_a_vide() {
        let dir = std::env::temp_dir().join("stlkm-test-etat");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("state.json");
        std::fs::write(&file, "{ ceci n'est pas du json").unwrap();

        let state = State::load_from(&file).expect("état vide");
        assert!(state.installed.is_empty());
        std::fs::remove_file(&file).ok();
    }

    #[test]
    fn un_bom_ne_fait_pas_tout_oublier() {
        let dir = std::env::temp_dir().join("stlkm-test-bom");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("state.json");

        let mut state = State::default();
        state.record("demo", Installed::new("v1", &dir));
        state.save_to(&file).unwrap();

        // Réécriture avec un BOM, comme le ferait un outil Windows.
        let contenu = std::fs::read_to_string(&file).unwrap();
        std::fs::write(&file, format!("\u{feff}{contenu}")).unwrap();

        let relu = State::load_from(&file).expect("état relu");
        assert!(
            relu.is_installed("demo"),
            "un BOM ne doit pas effacer les installations"
        );
        std::fs::remove_file(&file).ok();
    }

    #[test]
    fn enregistre_et_relit() {
        let dir = std::env::temp_dir().join("stlkm-test-etat-2");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("state.json");

        let mut state = State::default();
        state.record("demo", Installed::new("v1.2.0", &dir));
        state.save_to(&file).unwrap();

        let relu = State::load_from(&file).unwrap();
        assert_eq!(relu.get("demo").unwrap().version, "v1.2.0");
        std::fs::remove_file(&file).ok();
    }
}
