//! Les dépôts publics d'un compte GitHub, vus comme des applications.
//!
//! C'est la source du répertoire : pousser un projet sur GitHub suffit à le
//! faire apparaître dans le hub. Rien à déclarer, rien à tenir à jour.
//!
//! On ne devine pas ici comment lancer le projet : on le saura en regardant
//! ses fichiers une fois installé (voir [`crate::catalog::Project::launch_in`]).

use crate::catalog::{Project, Source};
use crate::error::Result;
use crate::github::{self, Repo};
use crate::guess::slug;

/// Sujet GitHub à poser sur un dépôt pour qu'il n'apparaisse jamais.
pub const TOPIC_CACHE: &str = "stlkm-ignore";

/// Les dépôts d'un compte, transformés en fiches.
pub fn projects(account: &str) -> Result<Vec<Project>> {
    Ok(from_repos(&github::public_repos(account)?))
}

pub fn from_repos(repos: &[Repo]) -> Vec<Project> {
    repos
        .iter()
        .filter(|r| is_showable(r))
        .map(from_repo)
        .collect()
}

/// Ce qu'on écarte : les copies d'autrui, les dépôts rangés au placard, ceux
/// que tu as explicitement masqués, et la tuyauterie du hub lui-même.
///
/// On ne se sert **pas** de la taille annoncée par GitHub pour deviner si un
/// dépôt est vide : elle est calculée en différé et reste à zéro un moment
/// après le premier push. S'en servir faisait disparaître les projets tout
/// juste publiés, sans rien dire. Un dépôt réellement vide est repéré à
/// l'installation, où le message est clair.
fn is_showable(repo: &Repo) -> bool {
    !repo.fork
        && !repo.archived
        && !repo.topics.iter().any(|t| t == TOPIC_CACHE)
        && !is_plumbing(&repo.name)
}

/// Le dépôt du répertoire et celui du hub ne sont pas des applications.
fn is_plumbing(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name == "stlkm-catalog" || name == "stlkm-hub" || name == "stlkm"
}

fn from_repo(repo: &Repo) -> Project {
    Project {
        id: slug(&repo.name),
        name: repo.name.clone(),
        summary: repo.description.clone().unwrap_or_default(),
        tags: tags(repo),
        icon: None,
        platforms: Vec::new(),   // un dépôt découvert n'exclut aucun hub
        source: Source::GithubRepo {
            repo: repo.full_name.clone(),
            branch: repo.default_branch.clone(),
        },
        install: None,
        // Décidés après installation, en regardant les fichiers.
        launch: None,
        hidden: false,
    }
}

/// Le langage principal et les sujets du dépôt font de bonnes étiquettes.
fn tags(repo: &Repo) -> Vec<String> {
    let mut tags = Vec::new();
    if let Some(langue) = &repo.language {
        tags.push(langue.to_ascii_lowercase());
    }
    for sujet in &repo.topics {
        if sujet != TOPIC_CACHE && !tags.contains(sujet) {
            tags.push(sujet.clone());
        }
    }
    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    fn depot(name: &str) -> Repo {
        Repo {
            name: name.to_string(),
            full_name: format!("moi/{name}"),
            description: Some("un outil".into()),
            language: Some("Python".into()),
            topics: Vec::new(),
            fork: false,
            archived: false,
            size: 42,
            pushed_at: None,
            default_branch: Some("main".into()),
        }
    }

    #[test]
    fn un_depot_devient_une_fiche() {
        let projets = from_repos(&[depot("cleanFiles")]);
        assert_eq!(projets.len(), 1);
        assert_eq!(projets[0].id, "cleanfiles");
        assert_eq!(projets[0].name, "cleanFiles");
        assert_eq!(projets[0].tags, vec!["python".to_string()]);
        // Le lancement reste à deviner une fois le projet installé.
        assert!(projets[0].launch.is_none());
    }

    #[test]
    fn on_ecarte_ce_qui_nest_pas_une_application() {
        let mut copie = depot("copie");
        copie.fork = true;
        let mut range = depot("range");
        range.archived = true;
        let mut cache = depot("cache");
        cache.topics = vec![TOPIC_CACHE.to_string()];

        let projets = from_repos(&[copie, range, cache, depot("stlkm-catalog")]);
        assert!(projets.is_empty());
    }

    #[test]
    fn un_depot_tout_juste_pousse_apparait_quand_meme() {
        // GitHub calcule la taille en différé : elle vaut encore 0 juste après
        // le premier push. S'y fier faisait disparaître le projet en silence.
        let mut frais = depot("fnfCLI");
        frais.size = 0;

        let projets = from_repos(&[frais]);
        assert_eq!(projets.len(), 1);
        assert_eq!(projets[0].id, "fnfcli");
    }
}
