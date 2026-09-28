//! Le répertoire : la liste des applications que le hub propose.
//!
//! Elle vient de deux endroits, dans cet ordre :
//!
//! 1. **Les dépôts publics du compte** — la source. Pousser un projet sur
//!    GitHub suffit à le faire apparaître.
//! 2. **Le fichier `catalog.toml`** — facultatif, et seulement pour corriger :
//!    un nom plus joli, une icône, une commande de lancement que le hub n'a
//!    pas su deviner. Une fiche du fichier remplace celle du dépôt.
//!
//! Tout est gardé en cache : le hub s'ouvre même sans réseau.

use std::path::PathBuf;

use crate::catalog::{Catalog, Project, SCHEMA_VERSION};
use crate::config::{Config, Mode};
use crate::discover;
use crate::error::{Error, Result};
use crate::github::{self, Repo};
use crate::paths;
use crate::platform::Platform;

/// D'où vient ce qui est affiché, pour pouvoir le dire à l'écran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// Les dépôts du compte, plus le fichier local du propriétaire.
    Local(PathBuf),
    /// Les dépôts du compte, plus le fichier publié.
    Publie(String),
}

impl Origin {
    pub fn label(&self) -> String {
        match self {
            Origin::Local(path) => path.display().to_string(),
            Origin::Publie(compte) => format!("dépôts publics de {compte}"),
        }
    }
}

/// Le répertoire, sans forcément aller sur le réseau : on lit le cache s'il
/// existe. C'est [`refresh`] qui va chercher les nouveautés.
pub fn load() -> Result<Catalog> {
    build(false)
}

/// Va rechercher la dernière version : les dépôts du compte et le fichier.
pub fn refresh() -> Result<Catalog> {
    build(true)
}

fn build(force: bool) -> Result<Catalog> {
    let config = Config::load()?;
    let decouverts = discovered(&config, force)?;
    let corrections = overrides(&config, force)?;
    let installes = crate::state::State::load().unwrap_or_default();

    let mut projects = merge(decouverts, corrections.projects);

    // Un dépôt masqué disparaît — sauf s'il est encore installé ici, ou pour le
    // propriétaire qui doit pouvoir le remettre : on le garde, marqué « retiré ».
    // Le propriétaire voit tout : c'est dans l'Atelier qu'il range. Les autres ne voient
    // ni ce qui est masqué, ni ce qui ne concerne pas leur plateforme — sauf ce qu'ils ont
    // encore installé, qu'on ne fait jamais disparaître sous leurs pieds.
    let proprietaire = config.mode().is_owner();
    let ici = Platform::current();
    projects.retain(|p| {
        proprietaire
            || installes.is_installed(&p.id)
            || (!corrections.hidden.contains(&p.id) && p.visible_on(ici))
    });
    for projet in &mut projects {
        projet.hidden = corrections.hidden.contains(&projet.id);
    }

    Ok(Catalog {
        schema: SCHEMA_VERSION,
        hidden: corrections.hidden,
        projects,
    })
}

/// Les fiches du fichier remplacent celles devinées ; celles qui ne
/// correspondent à aucun dépôt s'ajoutent à la fin.
fn merge(decouverts: Vec<Project>, corrections: Vec<Project>) -> Vec<Project> {
    let mut sortie: Vec<Project> = decouverts
        .into_iter()
        .map(|projet| {
            corrections
                .iter()
                .find(|c| c.id == projet.id)
                .cloned()
                .unwrap_or(projet)
        })
        .collect();

    for correction in corrections {
        if !sortie.iter().any(|p| p.id == correction.id) {
            sortie.push(correction);
        }
    }
    sortie
}

// ── Les dépôts du compte ────────────────────────────────────────────────────

fn discovered(config: &Config, force: bool) -> Result<Vec<Project>> {
    let cache = paths::repos_cache()?;

    if !force {
        if let Ok(raw) = std::fs::read_to_string(&cache) {
            if let Ok(repos) = serde_json::from_str::<Vec<Repo>>(&raw) {
                return Ok(discover::from_repos(&repos));
            }
        }
    }

    let repos = github::public_repos(&config.account())?;
    if let Some(parent) = cache.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&cache, serde_json::to_string(&repos)?).ok();
    Ok(discover::from_repos(&repos))
}

// ── Le fichier de corrections ───────────────────────────────────────────────

fn overrides(config: &Config, force: bool) -> Result<Catalog> {
    // Le propriétaire édite son fichier : il fait foi, tout de suite.
    if let Some(path) = config.editable_catalog() {
        return Catalog::from_path(path);
    }

    let cache = paths::catalog_cache()?;
    if !force {
        if let Ok(catalog) = Catalog::from_path(&cache) {
            return Ok(catalog);
        }
    }

    // Quand l'utilisateur clique « Actualiser », on passe par l'API : elle
    // reflète les publications tout de suite, là où le CDN resservait le
    // fichier d'avant pendant cinq minutes. Au démarrage, le CDN suffit — il
    // est plus rapide et ne compte pas dans les quotas.
    let recu = if force {
        github::fetch_file(&config.repo(), "catalog.toml", config.branch())
    } else {
        match config.catalog_url() {
            Some(url) => github::fetch_text(&url),
            None => return Ok(Catalog::empty()),
        }
    };

    match recu {
        Ok(raw) => {
            // On valide avant d'écrire : un fichier cassé en ligne ne doit pas
            // remplacer une copie locale qui marche.
            let catalog = Catalog::parse(&raw)?;
            if let Some(parent) = cache.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&cache, raw).ok();
            Ok(catalog)
        }
        // Pas de fichier de corrections publié : ce n'est pas une erreur, le
        // répertoire tient debout avec les seuls dépôts.
        Err(Error::NotPublished(_)) => Ok(Catalog::empty()),
        Err(e) => Err(e),
    }
}

// ── Renseignements ──────────────────────────────────────────────────────────

pub fn origin() -> Result<Origin> {
    let config = Config::load()?;
    match config.editable_catalog() {
        Some(path) => Ok(Origin::Local(path)),
        None => Ok(Origin::Publie(config.account())),
    }
}

/// Les identifiants qui viennent du fichier de corrections — les seuls qu'on
/// puisse en retirer. Les autres viennent des dépôts GitHub.
pub fn override_ids() -> Result<Vec<String>> {
    let config = Config::load()?;
    Ok(overrides(&config, false)?
        .projects
        .into_iter()
        .map(|p| p.id)
        .collect())
}

/// Le fichier de corrections à modifier. Réservé au propriétaire.
pub fn editable_path() -> Result<PathBuf> {
    Config::load()?.editable_catalog().ok_or(Error::NotOwner)
}

pub fn mode() -> Result<Mode> {
    Ok(Config::load()?.mode())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{Launch, Source};
    use crate::platform::PlatformValue;

    fn projet(id: &str, name: &str, launch: Option<Launch>) -> Project {
        Project {
            id: id.into(),
            name: name.into(),
            summary: String::new(),
            tags: Vec::new(),
            icon: None,
            platforms: Vec::new(),
            source: Source::GithubRepo {
                repo: format!("moi/{id}"),
                branch: None,
            },
            install: None,
            launch,
            hidden: false,
        }
    }

    #[test]
    fn une_correction_remplace_la_fiche_devinee() {
        let devine = projet("outil", "outil", None);
        let corrige = projet(
            "outil",
            "Mon Bel Outil",
            Some(Launch::Terminal {
                command: PlatformValue::Same("python main.py".into()),
                cwd: None,
            }),
        );

        let sortie = merge(vec![devine], vec![corrige]);
        assert_eq!(sortie.len(), 1, "pas de doublon");
        assert_eq!(sortie[0].name, "Mon Bel Outil");
        assert!(sortie[0].launch.is_some());
    }

    #[test]
    fn une_fiche_sans_depot_sajoute_quand_meme() {
        let sortie = merge(
            vec![projet("un", "Un", None)],
            vec![projet("deux", "Deux", None)],
        );
        assert_eq!(sortie.len(), 2);
        assert_eq!(sortie[1].id, "deux");
    }

    #[test]
    fn sans_correction_les_depots_passent_tels_quels() {
        let sortie = merge(vec![projet("un", "Un", None)], Vec::new());
        assert_eq!(sortie.len(), 1);
        assert_eq!(sortie[0].name, "Un");
    }
}
