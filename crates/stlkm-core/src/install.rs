//! Installer un projet : récupérer ses fichiers, puis jouer sa commande
//! d'installation. Le cœur ne parle à personne — il rend compte de ce qu'il
//! fait via `on_step`, que la CLI affiche et que la GUI affichera aussi.

use std::path::{Path, PathBuf};

use crate::catalog::{Project, Source};
use crate::error::{Error, Result};
use crate::github;
use crate::paths;
use crate::platform::Platform;
use crate::process;
use crate::state::{Installed, State};

/// Installe (ou réinstalle) un projet et met l'état à jour.
pub fn install(project: &Project, on_step: &mut dyn FnMut(&str)) -> Result<Installed> {
    let platform = Platform::current();
    let (dir, version) = fetch(project, platform, on_step)?;

    // Dernier contrôle avant de dire « installé » : un dossier vide veut dire
    // que le téléchargement a échoué sans le dire. Mieux vaut une erreur franche
    // qu'un projet marqué installé qui ne se lancera jamais.
    if is_empty(&dir) {
        return Err(Error::InstallEmpty(project.id.clone()));
    }

    // La fiche a le dernier mot ; sinon on regarde ce qu'il y a dans le dossier
    // (`npm ci`, `pip install -r requirements.txt`…).
    if let Some(command) = project
        .install_in(&dir)
        .as_ref()
        .and_then(|c| c.get(platform))
    {
        on_step(&format!("installation : {command}"));
        process::run(command, &dir)?;
    }

    let installed = Installed::new(version, &dir);
    let mut state = State::load()?;
    state.record(&project.id, installed.clone());
    state.save()?;

    on_step(&format!("{} installé ({})", project.name, installed.version));
    Ok(installed)
}

/// Désinstalle : supprime les fichiers gérés par le hub, jamais un dossier
/// local qui appartenait déjà à l'utilisateur.
pub fn uninstall(project: &Project, on_step: &mut dyn FnMut(&str)) -> Result<()> {
    let mut state = State::load()?;
    let installed = state
        .forget(&project.id)
        .ok_or_else(|| Error::NotInstalled(project.id.clone()))?;

    if installed.path.exists() {
        on_step(&format!("suppression de {}", installed.path.display()));

        // Windows refuse de supprimer un dossier dont un fichier est ouvert :
        // c'est presque toujours l'application elle-même, encore lancée.
        if let Err(e) = std::fs::remove_dir_all(&installed.path) {
            if !est_utilise(&e) {
                return Err(e.into());
            }

            // La suppression s'arrête au premier fichier verrouillé, mais elle
            // a déjà emporté les autres. Si le dossier ne contient plus rien
            // d'utile, l'installation est morte : on l'oublie quand même,
            // sinon elle resterait « installée » sans aucun fichier.
            if !is_empty(&installed.path) {
                return Err(Error::StillRunning(project.name.clone()));
            }
            on_step("l'application tournait encore : il en reste peut-être des fichiers");
        }
    }

    state.save()?;
    Ok(())
}

/// Vrai quand le système refuse parce qu'un fichier est ouvert ailleurs.
fn est_utilise(e: &std::io::Error) -> bool {
    // 32 = ERROR_SHARING_VIOLATION, 33 = ERROR_LOCK_VIOLATION sur Windows.
    matches!(e.raw_os_error(), Some(32) | Some(33))
        || e.kind() == std::io::ErrorKind::PermissionDenied
}

/// Met les fichiers en place et retourne où ils sont, avec leur version.
fn fetch(
    project: &Project,
    platform: Platform,
    on_step: &mut dyn FnMut(&str),
) -> Result<(PathBuf, String)> {
    match &project.source {
        Source::GithubRepo { repo, branch } => {
            let dir = paths::app_dir(&project.id)?;
            clone_or_pull(repo, branch.as_deref(), &dir, on_step)?;

            // `--git-dir` colle git au dépôt du projet. Sans ça il remonte les
            // dossiers parents et finit par répondre pour un dépôt qui n'a
            // rien à voir — le dossier personnel de l'utilisateur, par exemple.
            if is_empty(&dir) {
                return Err(Error::EmptyRepo(repo.clone()));
            }

            let git_dir = dir.join(".git");
            let version = process::capture(
                "git",
                &["--git-dir", &git_dir.to_string_lossy(), "rev-parse", "--short", "HEAD"],
                &dir,
            )
            .unwrap_or_else(|_| "inconnue".to_string());
            Ok((dir, version))
        }

        Source::GithubRelease {
            repo,
            asset,
            prerelease,
        } => {
            let pattern = asset
                .get(platform)
                .ok_or_else(|| Error::UnsupportedPlatform {
                    project: project.id.clone(),
                    platform: platform.as_str(),
                })?;

            on_step(&format!("recherche de la dernière release de {repo}"));
            let release = github::latest_release(repo, *prerelease)?;
            let found = release
                .find_asset(pattern)
                .ok_or_else(|| Error::NoAsset {
                    repo: repo.clone(),
                    tag: release.tag.clone(),
                    pattern: pattern.to_string(),
                })?;

            let dir = paths::app_dir(&project.id)?;
            let archive = paths::cache_dir()?.join(&found.name);
            github::download(&found.browser_download_url, &archive, on_step)?;

            // On repart d'un dossier propre : une ancienne version pourrait
            // laisser des fichiers que la nouvelle n'écrase pas.
            if dir.exists() {
                std::fs::remove_dir_all(&dir)?;
            }
            std::fs::create_dir_all(&dir)?;

            on_step(&format!("extraction de {}", found.name));
            extract(&archive, &dir)?;
            std::fs::remove_file(&archive).ok();

            Ok((dir, release.tag.clone()))
        }
    }
}

/// Vrai si le dossier ne contient aucun fichier du projet.
///
/// Le `.git` ne compte pas : cloner un dépôt sans le moindre commit donne un
/// dossier qui n'a que ça, et ce n'est pas une installation.
fn is_empty(dir: &Path) -> bool {
    match std::fs::read_dir(dir) {
        Ok(entries) => !entries
            .filter_map(|e| e.ok())
            .any(|e| e.file_name() != ".git"),
        Err(_) => true,
    }
}

/// Clone le dépôt, ou le met à jour s'il est déjà là.
fn clone_or_pull(
    repo: &str,
    branch: Option<&str>,
    dir: &Path,
    on_step: &mut dyn FnMut(&str),
) -> Result<()> {
    if dir.join(".git").exists() {
        on_step(&format!("mise à jour de {repo}"));
        return process::run_program("git", &["pull", "--ff-only"], dir);
    }

    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    let parent = dir.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;

    on_step(&format!("clonage de {repo}"));
    let url = format!("https://github.com/{repo}.git");
    let target = dir.to_string_lossy().into_owned();
    let mut args = vec!["clone"];
    if let Some(branch) = branch {
        args.push("--branch");
        args.push(branch);
    }
    args.push(&url);
    args.push(&target);
    process::run_program("git", &args, parent)
}

/// Décompresse une archive. Un fichier qui n'en est pas une est simplement
/// déposé tel quel — c'est le cas d'un `.exe` publié seul.
fn extract(archive: &Path, dir: &Path) -> Result<()> {
    let name = archive
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();

    if name.ends_with(".zip") {
        let file = std::fs::File::open(archive)?;
        let mut zip =
            zip::ZipArchive::new(file).map_err(|e| Error::Archive(e.to_string()))?;
        zip.extract(dir).map_err(|e| Error::Archive(e.to_string()))?;
        return Ok(());
    }

    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        let file = std::fs::File::open(archive)?;
        let decoder = flate2::read::GzDecoder::new(file);
        tar::Archive::new(decoder)
            .unpack(dir)
            .map_err(|e| Error::Archive(e.to_string()))?;
        return Ok(());
    }

    let dest = dir.join(archive.file_name().unwrap_or_default());
    std::fs::copy(archive, dest)?;
    Ok(())
}
