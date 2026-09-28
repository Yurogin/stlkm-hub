//! Les réglages du hub — et surtout ce qui sépare le propriétaire du répertoire
//! de ceux qui ne font que l'utiliser.
//!
//! Le propriétaire a un clone local du dépôt catalogue : c'est lui qui écrit.
//! Tous les autres lisent le catalogue publié sur GitHub, sans jamais pouvoir
//! le modifier. La vraie barrière n'est pas cet interrupteur mais GitHub :
//! publier demande un droit d'écriture sur le dépôt, que personne d'autre n'a.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::paths;

/// Le répertoire qu'une installation neuve va lire.
///
/// Inscrit dans le programme : quelqu'un qui installe le hub voit les
/// applications tout de suite, sans rien avoir à configurer.
pub const DEFAULT_CATALOG_REPO: &str = "Yurogin/stlkm-catalog";

/// Qui est devant le hub.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Toi : tu édites le répertoire et tu le publies.
    Proprietaire,
    /// Tout le monde : on installe, on lance, on met à jour. C'est tout.
    Utilisateur,
}

impl Mode {
    pub fn is_owner(self) -> bool {
        self == Mode::Proprietaire
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Proprietaire => "proprietaire",
            Mode::Utilisateur => "utilisateur",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    /// Dépôt GitHub qui publie le répertoire, sous la forme `proprietaire/depot`.
    #[serde(default)]
    pub catalog_repo: Option<String>,
    /// Branche à lire. `main` si rien n'est dit.
    #[serde(default)]
    pub branch: Option<String>,
    /// Clone local du dépôt catalogue. Sa présence fait le propriétaire.
    #[serde(default)]
    pub catalog_path: Option<PathBuf>,
}

impl Config {
    pub fn load() -> Result<Self> {
        Self::load_from(paths::config_file()?)
    }

    pub fn load_from(path: impl AsRef<Path>) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(raw) => Ok(toml::from_str(&raw)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = paths::config_file()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, self.to_toml())?;
        Ok(())
    }

    /// Le fichier reste lisible et modifiable à la main.
    fn to_toml(&self) -> String {
        let mut out = String::from("# Réglages du hub STLKM\n");
        if let Some(repo) = &self.catalog_repo {
            out.push_str(&format!("catalog_repo = \"{repo}\"\n"));
        }
        if let Some(branch) = &self.branch {
            out.push_str(&format!("branch = \"{branch}\"\n"));
        }
        if let Some(path) = &self.catalog_path {
            out.push_str(&format!(
                "catalog_path = \"{}\"\n",
                path.display().to_string().replace('\\', "\\\\")
            ));
        }
        out
    }

    /// Le catalogue que l'on peut écrire, s'il existe vraiment sur ce disque.
    pub fn editable_catalog(&self) -> Option<PathBuf> {
        if let Some(from_env) = std::env::var_os("STLKM_CATALOG") {
            let path = PathBuf::from(from_env);
            return path.is_file().then_some(path);
        }
        self.catalog_path.clone().filter(|p| p.is_file())
    }

    /// Le dépôt qui publie le répertoire.
    pub fn repo(&self) -> String {
        std::env::var("STLKM_CATALOG_REPO")
            .ok()
            .or_else(|| self.catalog_repo.clone())
            .unwrap_or_else(|| DEFAULT_CATALOG_REPO.to_string())
    }

    /// Le compte GitHub dont on propose les dépôts : le propriétaire du dépôt
    /// du répertoire. Un seul réglage sert aux deux.
    pub fn account(&self) -> String {
        self.repo()
            .split('/')
            .next()
            .unwrap_or(DEFAULT_CATALOG_REPO)
            .to_string()
    }

    /// La branche lue sur le dépôt du répertoire.
    pub fn branch(&self) -> &str {
        self.branch.as_deref().unwrap_or("main")
    }

    /// L'adresse du catalogue publié, pour ceux qui ne font que le lire.
    pub fn catalog_url(&self) -> Option<String> {
        Some(format!(
            "https://raw.githubusercontent.com/{}/{}/catalog.toml",
            self.repo(),
            self.branch()
        ))
    }

    pub fn mode(&self) -> Mode {
        match self.editable_catalog() {
            Some(_) => Mode::Proprietaire,
            None => Mode::Utilisateur,
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sans_clone_local_on_est_utilisateur() {
        let config = Config {
            catalog_repo: Some("moi/stlkm-catalog".into()),
            catalog_path: Some(PathBuf::from("ce-chemin-nexiste-pas.toml")),
            ..Config::default()
        };
        assert_eq!(config.mode(), Mode::Utilisateur);
    }

    #[test]
    fn une_installation_neuve_connait_deja_le_repertoire() {
        // Sans aucun réglage, le hub doit savoir où regarder.
        let url = Config::default().catalog_url().expect("une adresse par défaut");
        assert!(url.contains(DEFAULT_CATALOG_REPO));
    }

    #[test]
    fn ladresse_publiee_se_construit_depuis_le_depot() {
        let config = Config {
            catalog_repo: Some("moi/stlkm-catalog".into()),
            ..Config::default()
        };
        assert_eq!(
            config.catalog_url().as_deref(),
            Some("https://raw.githubusercontent.com/moi/stlkm-catalog/main/catalog.toml")
        );
    }

    #[test]
    fn le_compte_se_deduit_du_depot() {
        assert_eq!(Config::default().account(), "Yurogin");
        let autre = Config {
            catalog_repo: Some("quelquun/son-repertoire".into()),
            ..Config::default()
        };
        assert_eq!(autre.account(), "quelquun");
    }

    #[test]
    fn le_fichier_ecrit_se_relit() {
        let config = Config {
            catalog_repo: Some("moi/depot".into()),
            branch: Some("prod".into()),
            catalog_path: Some(PathBuf::from("C:\\Users\\moi\\catalog.toml")),
        };
        let relu: Config = toml::from_str(&config.to_toml()).expect("relecture");
        assert_eq!(relu.branch.as_deref(), Some("prod"));
        assert_eq!(
            relu.catalog_path,
            Some(PathBuf::from("C:\\Users\\moi\\catalog.toml"))
        );
    }
}
