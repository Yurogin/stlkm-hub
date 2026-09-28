//! Publier le catalogue : le commiter et le pousser sur GitHub.
//!
//! Rien ici ne part tout seul. `plan` dit ce qui serait fait, `publish` le fait
//! — et c'est l'appelant (la CLI aujourd'hui, la GUI demain) qui demande
//! confirmation entre les deux.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::process;
use crate::secrets::{self, Finding};

/// Ce qu'une publication ferait, avant de la faire.
#[derive(Debug, Clone)]
pub struct Plan {
    /// Le dépôt qui contient le catalogue.
    pub repo_dir: PathBuf,
    pub catalog: PathBuf,
    /// `proprietaire/depot`, quand un remote GitHub est configuré.
    pub remote: Option<String>,
    /// Les fichiers modifiés, tels que git les voit.
    pub changes: Vec<String>,
    /// Ce qui ressemble à un secret dans le catalogue. Bloque la publication.
    pub warnings: Vec<Finding>,
}

impl Plan {
    /// Rien à pousser : le catalogue est déjà à jour sur GitHub.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    pub fn is_blocked(&self) -> bool {
        !self.warnings.is_empty()
    }
}

/// Prépare la publication du catalogue et vérifie qu'elle est sans danger.
pub fn plan(catalog: &Path) -> Result<Plan> {
    if !catalog.is_file() {
        return Err(Error::MissingCatalog(catalog.display().to_string()));
    }
    let catalog = crate::paths::absolute(catalog);
    let repo_dir = repo_root(&catalog)?;

    let content = std::fs::read_to_string(&catalog)?;
    let warnings = secrets::sniff(&content);

    // Seul le catalogue nous intéresse : le dépôt peut contenir autre chose,
    // et publier ne doit jamais emporter des fichiers voisins au passage.
    let tracked = relative_path(&repo_dir, &catalog);
    let status = process::capture("git", &["status", "--porcelain", "--", &tracked], &repo_dir)?;
    let changes = status
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();

    let remote = process::capture("git", &["remote", "get-url", "origin"], &repo_dir)
        .ok()
        .and_then(|url| short_repo(&url));

    Ok(Plan {
        repo_dir,
        catalog,
        remote,
        changes,
        warnings,
    })
}

/// Commite et pousse. Refuse tant qu'un secret est signalé.
pub fn publish(plan: &Plan, message: &str, on_step: &mut dyn FnMut(&str)) -> Result<()> {
    if plan.is_blocked() {
        return Err(Error::SecretFound(plan.warnings.len()));
    }
    if plan.is_empty() {
        on_step("rien à publier, le catalogue est déjà à jour");
        return Ok(());
    }

    let file = relative_path(&plan.repo_dir, &plan.catalog);

    on_step(&format!("git add {file}"));
    process::run_program("git", &["add", "--", &file], &plan.repo_dir)?;

    // Le chemin en fin de commande limite le commit à ce seul fichier : sans
    // lui, tout ce qui traîne dans l'index partirait avec.
    on_step(&format!("git commit -m \"{message}\" -- {file}"));
    process::run_program(
        "git",
        &["commit", "-m", message, "--", &file],
        &plan.repo_dir,
    )?;

    match &plan.remote {
        Some(remote) => {
            on_step(&format!("git push vers {remote}"));
            process::run_program("git", &["push"], &plan.repo_dir)?;
            on_step("catalogue publié");
        }
        None => {
            on_step("commit fait, mais aucun remote : rien n'a été poussé");
        }
    }
    Ok(())
}

/// Remonte les dossiers jusqu'au dépôt git qui contient le catalogue.
fn repo_root(catalog: &Path) -> Result<PathBuf> {
    let mut dir = catalog.parent().unwrap_or(Path::new("."));
    loop {
        if dir.join(".git").exists() {
            return Ok(dir.to_path_buf());
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => {
                return Err(Error::NoCatalogRepo(catalog.display().to_string()));
            }
        }
    }
}

/// Chemin du catalogue vu depuis la racine du dépôt, en séparateurs git.
fn relative_path(repo_dir: &Path, catalog: &Path) -> String {
    catalog
        .strip_prefix(repo_dir)
        .unwrap_or(catalog)
        .to_string_lossy()
        .replace('\\', "/")
}

fn short_repo(url: &str) -> Option<String> {
    let rest = url
        .trim()
        .strip_prefix("https://github.com/")
        .or_else(|| url.trim().strip_prefix("git@github.com:"))?;
    Some(
        rest.trim_end_matches(".git")
            .trim_end_matches('/')
            .to_string(),
    )
}
