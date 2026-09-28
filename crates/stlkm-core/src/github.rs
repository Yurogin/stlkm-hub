//! Le strict minimum de l'API GitHub : trouver la dernière release d'un dépôt
//! et l'asset qui correspond à la plateforme courante.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

const USER_AGENT: &str = "stlkm-hub";

#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    #[serde(rename = "tag_name")]
    pub tag: String,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    #[serde(default)]
    pub size: u64,
}

/// Un dépôt public, tel que GitHub le décrit.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Repo {
    pub name: String,
    pub full_name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub fork: bool,
    #[serde(default)]
    pub archived: bool,
    /// En Ko. Zéro veut dire un dépôt sans le moindre commit.
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub pushed_at: Option<String>,
    #[serde(default)]
    pub default_branch: Option<String>,
}

/// Tous les dépôts publics d'un compte, du plus récemment poussé au plus vieux.
pub fn public_repos(account: &str) -> Result<Vec<Repo>> {
    let url =
        format!("https://api.github.com/users/{account}/repos?per_page=100&sort=pushed&type=owner");
    get_json(&url)
}

/// Dernière release publiée de `proprietaire/depot`.
///
/// `allow_prerelease` autorise les pré-versions ; les brouillons sont toujours
/// écartés, ils ne sont visibles que du propriétaire.
pub fn latest_release(repo: &str, allow_prerelease: bool) -> Result<Release> {
    let url = format!("https://api.github.com/repos/{repo}/releases?per_page=20");
    let releases: Vec<Release> = get_json(&url)?;
    releases
        .into_iter()
        .find(|r| !r.draft && (allow_prerelease || !r.prerelease))
        .ok_or_else(|| Error::NoRelease(repo.to_string()))
}

impl Release {
    /// Cherche un asset par son nom, où `*` remplace n'importe quoi.
    pub fn find_asset(&self, pattern: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| matches_glob(&a.name, pattern))
    }
}

/// Le jeton, s'il y en a un : la variable d'environnement d'abord, le fichier ensuite.
///
/// Sans jeton, GitHub compte soixante demandes par heure et par adresse IP —
/// une fouille de l'atelier les épuise. Avec, cinq mille. Pour ne lire que des
/// dépôts publics, un jeton sans la moindre permission suffit : c'est celui
/// qu'il faut créer.
pub fn token() -> Option<String> {
    if let Ok(t) = std::env::var("STLKM_GITHUB_TOKEN") {
        let t = t.trim().to_string();
        if !t.is_empty() {
            return Some(t);
        }
    }
    let fichier = crate::paths::token_file().ok()?;
    let t = std::fs::read_to_string(fichier).ok()?.trim().to_string();
    (!t.is_empty()).then_some(t)
}

pub fn has_token() -> bool {
    token().is_some()
}

/// Garde le jeton, ou l'efface si la chaîne est vide.
pub fn save_token(valeur: &str) -> Result<()> {
    let fichier = crate::paths::token_file()?;
    let valeur = valeur.trim();
    if valeur.is_empty() {
        match std::fs::remove_file(&fichier) {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        }
    }
    if let Some(parent) = fichier.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&fichier, valeur)?;
    Ok(())
}

/// Ce qu'il reste comme demandes cette heure-ci. Ne compte pas dans le quota.
#[derive(Debug, Clone, Deserialize)]
pub struct Quota {
    pub limit: u32,
    pub remaining: u32,
}

pub fn quota() -> Result<Quota> {
    #[derive(Deserialize)]
    struct Enveloppe {
        resources: Ressources,
    }
    #[derive(Deserialize)]
    struct Ressources {
        core: Quota,
    }
    let e: Enveloppe = get_json("https://api.github.com/rate_limit")?;
    Ok(e.resources.core)
}

/// Une demande à l'API, signée quand un jeton est là.
fn demande(url: &str) -> ureq::Request {
    let req = ureq::get(url)
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json");
    match token() {
        Some(t) => req.set("Authorization", &format!("Bearer {t}")),
        None => req,
    }
}

fn get_json<T: serde::de::DeserializeOwned>(url: &str) -> Result<T> {
    let response = demande(url).call().map_err(|e| Error::Http(e.to_string()))?;
    Ok(response.into_json()?)
}

/// Récupère un fichier texte (le catalogue publié, par exemple).
pub fn fetch_text(url: &str) -> Result<String> {
    match ureq::get(url).set("User-Agent", USER_AGENT).call() {
        Ok(response) => Ok(response.into_string()?),
        // Un 404 sur le catalogue veut dire une seule chose, et « échec
        // réseau : status code 404 » ne l'explique à personne.
        Err(ureq::Error::Status(404, _)) => Err(Error::NotPublished(url.to_string())),
        Err(e) => Err(Error::Http(e.to_string())),
    }
}

/// Récupère un fichier d'un dépôt **en passant par l'API**.
///
/// `raw.githubusercontent.com` garde une copie pendant environ cinq minutes,
/// et ignore les paramètres qu'on ajoute pour l'en empêcher : après une
/// publication, il resservait l'ancien fichier. L'API, elle, ne garde le sien
/// qu'une minute et reflète les publications tout de suite.
pub fn fetch_file(repo: &str, chemin: &str, branche: &str) -> Result<String> {
    let url = format!("https://api.github.com/repos/{repo}/contents/{chemin}?ref={branche}");
    match demande(&url)
        .set("Accept", "application/vnd.github.raw")
        .call()
    {
        Ok(response) => Ok(response.into_string()?),
        Err(ureq::Error::Status(404, _)) => Err(Error::NotPublished(url)),
        Err(e) => Err(Error::Http(e.to_string())),
    }
}

/// Télécharge une URL dans un fichier, en signalant l'avancement.
pub fn download(url: &str, dest: &std::path::Path, on_step: &mut dyn FnMut(&str)) -> Result<()> {
    on_step(&format!("téléchargement de {url}"));
    let response = ureq::get(url)
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| Error::Http(e.to_string()))?;

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::File::create(dest)?;
    std::io::copy(&mut response.into_reader(), &mut file)?;
    Ok(())
}

/// Motif à la `MBomb-*-x64.zip`. Seul `*` est reconnu.
fn matches_glob(name: &str, pattern: &str) -> bool {
    let name = name.to_ascii_lowercase();
    let pattern = pattern.to_ascii_lowercase();
    if !pattern.contains('*') {
        return name == pattern;
    }

    let parts: Vec<&str> = pattern.split('*').collect();
    let mut cursor = 0usize;

    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        // Le premier morceau doit coller au début, le dernier à la fin.
        if index == 0 {
            if !name.starts_with(part) {
                return false;
            }
            cursor = part.len();
        } else if index == parts.len() - 1 {
            return name[cursor..].ends_with(part);
        } else {
            match name[cursor..].find(part) {
                Some(at) => cursor += at + part.len(),
                None => return false,
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_motif_sans_etoile_est_exact() {
        assert!(matches_glob("app.zip", "app.zip"));
        assert!(!matches_glob("app.zip", "app.tar.gz"));
    }

    #[test]
    fn letoile_remplace_nimporte_quoi() {
        assert!(matches_glob("MBomb-1.4.2-windows-x64.zip", "MBomb-*-windows-x64.zip"));
        assert!(matches_glob("MBomb-windows.zip", "*windows*"));
        assert!(!matches_glob("MBomb-linux.zip", "*windows*"));
        assert!(!matches_glob("autre.zip", "MBomb-*.zip"));
    }

    #[test]
    fn la_casse_ne_compte_pas() {
        assert!(matches_glob("MBOMB-WINDOWS.ZIP", "mbomb-*.zip"));
    }
}
