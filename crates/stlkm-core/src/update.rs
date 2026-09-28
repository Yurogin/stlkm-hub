//! Savoir si un projet installé a pris du retard, et le rattraper.

use crate::catalog::{Catalog, Project, Source};
use crate::error::Result;
use crate::github;
use crate::install;
use crate::process;
use crate::state::{Installed, State};

/// Le verdict pour un projet.
#[derive(Debug, Clone)]
pub struct Check {
    pub id: String,
    pub name: String,
    /// Version installée, `None` si le projet n'est pas installé.
    pub current: Option<String>,
    /// Dernière version disponible, `None` si la question n'a pas de sens
    /// (dossier local) ou si le réseau n'a pas répondu.
    pub latest: Option<String>,
    pub status: Status,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Pas installé : rien à mettre à jour.
    Absent,
    /// À jour.
    UpToDate,
    /// Une nouvelle version existe.
    Available,
    /// Retiré du répertoire alors qu'il est encore installé ici.
    Removed,
    /// La vérification a échoué (réseau, dépôt privé, pas de release).
    Unknown(String),
}

/// Vérifie un projet.
pub fn check(project: &Project, state: &State) -> Check {
    let current = state.get(&project.id).map(|i| i.version.clone());

    let Some(installed) = state.get(&project.id) else {
        return Check {
            id: project.id.clone(),
            name: project.name.clone(),
            current: None,
            latest: None,
            status: Status::Absent,
        };
    };

    // Retiré du répertoire : inutile de chercher une mise à jour, il n'y en
    // aura plus. C'est le moment de proposer de le désinstaller.
    if project.hidden {
        return Check {
            id: project.id.clone(),
            name: project.name.clone(),
            current,
            latest: None,
            status: Status::Removed,
        };
    }

    let (latest, status) = match &project.source {
        Source::GithubRelease {
            repo, prerelease, ..
        } => match github::latest_release(repo, *prerelease) {
            Ok(release) => {
                let same = release.tag == installed.version;
                (
                    Some(release.tag),
                    if same {
                        Status::UpToDate
                    } else {
                        Status::Available
                    },
                )
            }
            Err(e) => (None, Status::Unknown(e.to_string())),
        },

        Source::GithubRepo { repo, branch } => {
            match remote_head(repo, branch.as_deref(), installed) {
                Ok((short, same)) => (
                    Some(short),
                    if same {
                        Status::UpToDate
                    } else {
                        Status::Available
                    },
                ),
                Err(e) => (None, Status::Unknown(e.to_string())),
            }
        }
    };

    Check {
        id: project.id.clone(),
        name: project.name.clone(),
        current,
        latest,
        status,
    }
}

/// Vérifie tout le catalogue d'un coup.
pub fn check_all(catalog: &Catalog, state: &State) -> Vec<Check> {
    catalog.projects.iter().map(|p| check(p, state)).collect()
}

/// Applique la mise à jour. Pour un dépôt c'est un `git pull`, pour une
/// release c'est un nouveau téléchargement : dans les deux cas, réinstaller.
pub fn update(project: &Project, on_step: &mut dyn FnMut(&str)) -> Result<Installed> {
    install::install(project, on_step)
}

/// Compare le dernier commit du dépôt distant à celui qui est installé.
fn remote_head(repo: &str, branch: Option<&str>, installed: &Installed) -> Result<(String, bool)> {
    let url = format!("https://github.com/{repo}.git");
    let reference = branch.unwrap_or("HEAD");
    let output = process::capture("git", &["ls-remote", &url, reference], &installed.path)?;

    let remote = output
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_string();
    let short = remote.chars().take(7).collect::<String>();

    // La version installée est un commit court : on compare sur sa longueur.
    let same = !remote.is_empty() && remote.starts_with(installed.version.trim());
    Ok((short, same))
}

impl Status {
    pub fn label(&self) -> String {
        match self {
            Status::Absent => "pas installé".to_string(),
            Status::UpToDate => "à jour".to_string(),
            Status::Available => "mise à jour dispo".to_string(),
            Status::Removed => "retiré du répertoire".to_string(),
            Status::Unknown(why) => format!("indéterminé ({why})"),
        }
    }
}
