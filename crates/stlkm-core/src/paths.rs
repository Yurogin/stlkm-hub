//! Où le hub range ses affaires. Un seul endroit décide, pour que la CLI et la
//! GUI voient exactement les mêmes projets installés.

use std::path::PathBuf;

use directories::ProjectDirs;

use crate::error::{Error, Result};

/// Chemin absolu et lisible. Windows préfixe ses chemins canoniques de
/// `\\?\`, que les autres outils digèrent mal.
pub fn absolute(path: impl AsRef<std::path::Path>) -> PathBuf {
    let path = path.as_ref();
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let text = canonical.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) => PathBuf::from(rest),
        None => canonical,
    }
}

fn dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("", "STLKM", "stlkm").ok_or(Error::NoHome)
}

/// Racine des données : état, catalogue en cache.
pub fn data_dir() -> Result<PathBuf> {
    Ok(dirs()?.data_dir().to_path_buf())
}

/// Dossier où sont installés les projets, un sous-dossier par `id`.
pub fn apps_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("apps"))
}

pub fn app_dir(id: &str) -> Result<PathBuf> {
    Ok(apps_dir()?.join(id))
}

/// Les réglages du hub.
pub fn config_file() -> Result<PathBuf> {
    Ok(data_dir()?.join("config.toml"))
}

/// Le jeton GitHub, s'il y en a un. Volontairement à part de `config.toml` :
/// un fichier de réglages se montre, se copie, se colle dans un message —
/// pas celui-ci.
pub fn token_file() -> Result<PathBuf> {
    Ok(data_dir()?.join("github-token"))
}

/// Ce qui est installé et en quelle version.
pub fn state_file() -> Result<PathBuf> {
    Ok(data_dir()?.join("state.json"))
}

/// Dossier de travail jetable : archives téléchargées, catalogue en cache.
pub fn cache_dir() -> Result<PathBuf> {
    Ok(dirs()?.cache_dir().to_path_buf())
}

/// Dernière liste connue des dépôts du compte.
pub fn repos_cache() -> Result<PathBuf> {
    Ok(dirs()?.cache_dir().join("repos.json"))
}

/// Dernière copie connue du catalogue distant.
pub fn catalog_cache() -> Result<PathBuf> {
    Ok(dirs()?.cache_dir().join("catalog.toml"))
}
