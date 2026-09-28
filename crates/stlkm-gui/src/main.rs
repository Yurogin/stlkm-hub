// La fenêtre n'ouvre pas de console noire derrière elle en version finale.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! La GUI du hub. Elle ne sait rien faire par elle-même : tout passe par
//! `stlkm-core`, exactement comme la CLI. Ici on ne fait que présenter.

mod views;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, State};

use stlkm_core::{Catalog, Session};

use views::{MobileView, TokenView, CheckView, ModeView, PlanView, ProjectView};

/// Ce qui tourne en ce moment, lancé depuis la fenêtre.
#[derive(Default)]
struct Running(Mutex<HashMap<String, Session>>);

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(Running::default())
        .invoke_handler(tauri::generate_handler![
            projects,
            install,
            uninstall,
            launch,
            stop,
            running,
            updates,
            update_one,
            hide,
            unhide,
            set_platforms,
            scan_mobile,
            file_mobile,
            token_state,
            set_token,
            reveal,
            hub_version,
            hub_check,
            hub_install,
            publish_info,
            publish_now,
            mode,
            refresh,
        ])
        .run(tauri::generate_context!())
        .expect("la fenêtre n'a pas pu s'ouvrir");
}

// ── Catalogue ───────────────────────────────────────────────────────────────

/// Le répertoire affiché : le fichier local du propriétaire, ou le catalogue
/// publié pour tous les autres.
fn catalog() -> Result<Catalog, String> {
    stlkm_core::repository::load().map_err(|e| e.to_string())
}

/// Le fichier que l'on peut modifier. Absent pour un simple utilisateur.
fn catalog_path() -> Result<PathBuf, String> {
    stlkm_core::repository::editable_path().map_err(|e| e.to_string())
}

// ── Le hub lui-même ─────────────────────────────────────────────────────────

/// La version installée du hub.
#[tauri::command]
fn hub_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

/// Y a-t-il une version plus récente publiée ? Retourne son numéro.
#[tauri::command]
async fn hub_check(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_updater::UpdaterExt;

    let updater = app.updater().map_err(|e| e.to_string())?;
    match updater.check().await {
        Ok(Some(nouvelle)) => Ok(Some(nouvelle.version)),
        Ok(None) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Télécharge et installe la nouvelle version, puis redémarre le hub.
#[tauri::command]
async fn hub_install(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_updater::UpdaterExt;

    let updater = app.updater().map_err(|e| e.to_string())?;
    let Some(nouvelle) = updater.check().await.map_err(|e| e.to_string())? else {
        return Ok(());
    };

    let progres = app.clone();
    nouvelle
        .download_and_install(
            move |recu, total| {
                let message = match total {
                    Some(total) if total > 0 => {
                        format!("téléchargement {} %", recu * 100 / total as usize)
                    }
                    _ => "téléchargement…".to_string(),
                };
                progres
                    .emit(
                        "etape",
                        serde_json::json!({ "id": "STLKM", "message": message }),
                    )
                    .ok();
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;

    app.restart();
}

/// Qui est devant la fenêtre : « proprietaire » ou « utilisateur ».
/// C'est ce qui décide si l'Atelier existe.
#[tauri::command]
fn mode() -> Result<ModeView, String> {
    let config = stlkm_core::Config::load().map_err(|e| e.to_string())?;
    Ok(ModeView {
        mode: config.mode().as_str().to_string(),
        owner: config.mode().is_owner(),
        origin: stlkm_core::repository::origin()
            .map(|o| o.label())
            .unwrap_or_else(|_| "aucun répertoire configuré".to_string()),
        account: config.account(),
    })
}

/// Va rechercher la dernière version du répertoire publié.
#[tauri::command]
async fn refresh() -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(|| {
        stlkm_core::repository::refresh()
            .map(|c| c.projects.len())
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Renvoie chaque étape du cœur à la fenêtre, qui l'affiche en direct.
fn reporter(app: AppHandle, id: String) -> impl FnMut(&str) {
    move |message: &str| {
        app.emit(
            "etape",
            serde_json::json!({ "id": id, "message": message }),
        )
        .ok();
    }
}

// ── Consultation ────────────────────────────────────────────────────────────

#[tauri::command]
fn projects(running: State<'_, Running>) -> Result<Vec<ProjectView>, String> {
    let catalog = catalog()?;
    let state = stlkm_core::State::load().unwrap_or_default();
    let live = running.0.lock().unwrap();
    let du_fichier = stlkm_core::repository::override_ids().unwrap_or_default();

    Ok(catalog
        .projects
        .iter()
        .map(|p| {
            ProjectView::new(
                p,
                &state,
                live.contains_key(&p.id),
                du_fichier.contains(&p.id),
            )
        })
        .collect())
}

#[tauri::command]
fn running(running: State<'_, Running>) -> Vec<String> {
    running.0.lock().unwrap().keys().cloned().collect()
}

#[tauri::command]
async fn updates() -> Result<Vec<CheckView>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let catalog = catalog()?;
        let state = stlkm_core::State::load().unwrap_or_default();
        Ok(stlkm_core::check_all(&catalog, &state)
            .into_iter()
            .map(CheckView::from)
            .collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── Actions ─────────────────────────────────────────────────────────────────

#[tauri::command]
async fn install(app: AppHandle, id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let catalog = catalog()?;
        let project = catalog.get(&id).map_err(|e| e.to_string())?;
        stlkm_core::install(project, &mut reporter(app.clone(), id.clone())).map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn update_one(app: AppHandle, id: String) -> Result<(), String> {
    install(app, id).await
}

#[tauri::command]
async fn uninstall(app: AppHandle, id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let catalog = catalog()?;
        let project = catalog.get(&id).map_err(|e| e.to_string())?;
        stlkm_core::uninstall(project, &mut reporter(app.clone(), id.clone())).map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Lance un projet. Retourne l'adresse ouverte, s'il y en a une.
#[tauri::command]
fn launch(app: AppHandle, running: State<'_, Running>, id: String) -> Result<Option<String>, String> {
    let catalog = catalog()?;
    let project = catalog.get(&id).map_err(|e| e.to_string())?;
    let session = stlkm_core::run(project, &mut reporter(app.clone(), id.clone())).map_err(|e| e.to_string())?;
    let url = session.url.clone();

    if session.is_detached() {
        // Le projet vit dans sa propre fenêtre : le hub n'a rien à retenir,
        // et surtout rien à tuer en refermant la session.
        std::mem::forget(session);
    } else {
        running.0.lock().unwrap().insert(id, session);
    }
    Ok(url)
}

#[tauri::command]
fn stop(running: State<'_, Running>, id: String) {
    if let Some(mut session) = running.0.lock().unwrap().remove(&id) {
        session.stop();
    }
}

// ── Atelier ─────────────────────────────────────────────────────────────────

/// Retire un dépôt du répertoire : il n'apparaîtra plus, ni ici ni chez les
/// gens qui ont installé le hub une fois la publication faite.
#[tauri::command]
fn hide(id: String) -> Result<bool, String> {
    stlkm_core::catalog_edit::hide(&catalog_path()?, &id).map_err(|e| e.to_string())
}

#[tauri::command]
fn unhide(id: String) -> Result<bool, String> {
    stlkm_core::catalog_edit::unhide(&catalog_path()?, &id).map_err(|e| e.to_string())
}

/// Range une fiche : « seulement sur mobile », « seulement sur PC », ou partout
/// avec une liste vide. Ce que l'Atelier appelle ses deux listes.
#[tauri::command]
fn set_platforms(id: String, platforms: Vec<String>) -> Result<(), String> {
    let voulues: Vec<stlkm_core::Platform> = platforms
        .iter()
        .filter_map(|n| stlkm_core::Platform::parse(n))
        .collect();
    if voulues.len() != platforms.len() {
        return Err("plateforme inconnue".to_string());
    }

    let catalogue = stlkm_core::repository::load().map_err(|e| e.to_string())?;
    let projet = catalogue
        .projects
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("« {id} » n'est pas dans le répertoire"))?;

    stlkm_core::catalog_edit::set_platforms(&catalog_path()?, projet, voulues)
        .map_err(|e| e.to_string())
}

/// L'atelier mobile : va voir quelles releases contiennent un APK.
///
/// Un appel GitHub par dépôt, donc c'est long et c'est limité — d'où le bouton
/// qui le déclenche, plutôt qu'une fouille à chaque ouverture.
#[tauri::command]
async fn scan_mobile() -> Result<Vec<MobileView>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let catalogue = stlkm_core::repository::load().map_err(|e| e.to_string())?;
        let trouvees = stlkm_core::mobile::fouille(&catalogue.projects, |_, _, _| {})
            .map_err(|e| e.to_string())?;
        Ok(trouvees
            .iter()
            .map(|t| {
                let rangee = catalogue
                    .projects
                    .iter()
                    .find(|p| p.id == t.id)
                    .map(|p| p.platforms.iter().map(|x| x.as_str().to_string()).collect())
                    .unwrap_or_default();
                MobileView::new(t, rangee)
            })
            .collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Écrit dans le catalogue ce que l'atelier a trouvé pour une appli.
#[tauri::command]
fn file_mobile(id: String, apk: String, aussi_pc: bool) -> Result<(), String> {
    let catalogue = stlkm_core::repository::load().map_err(|e| e.to_string())?;
    let projet = catalogue
        .projects
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("« {id} » n'est pas dans le répertoire"))?;

    stlkm_core::catalog_edit::range_mobile(&catalog_path()?, projet, &apk, aussi_pc)
        .map_err(|e| e.to_string())
}

/// Où en est le quota GitHub, et y a-t-il un jeton. Le jeton n'est jamais rendu :
/// une fois posé, il ne ressort plus, il se remplace.
#[tauri::command]
async fn token_state() -> Result<TokenView, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let present = stlkm_core::github::has_token();
        let (limite, restant) = match stlkm_core::github::quota() {
            Ok(q) => (q.limit, q.remaining),
            Err(_) => (0, 0),
        };
        Ok(TokenView {
            present,
            limite,
            restant,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Range un jeton, ou l'efface avec une chaîne vide.
#[tauri::command]
async fn set_token(valeur: String) -> Result<TokenView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        stlkm_core::github::save_token(&valeur).map_err(|e| e.to_string())?;
        let present = stlkm_core::github::has_token();
        let (limite, restant) = match stlkm_core::github::quota() {
            Ok(q) => (q.limit, q.remaining),
            Err(e) => return Err(format!("jeton refusé par GitHub : {e}")),
        };
        Ok(TokenView {
            present,
            limite,
            restant,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Ouvre le dossier d'une application dans l'explorateur de fichiers.
#[tauri::command]
fn reveal(id: String) -> Result<String, String> {
    let state = stlkm_core::State::load().map_err(|e| e.to_string())?;
    let installed = state
        .get(&id)
        .ok_or_else(|| format!("« {id} » n'est pas installé"))?;

    stlkm_core::process::reveal(&installed.path).map_err(|e| e.to_string())?;
    Ok(installed.path.display().to_string())
}

#[tauri::command]
async fn publish_info() -> Result<PlanView, String> {
    tauri::async_runtime::spawn_blocking(|| {
        stlkm_core::plan(&catalog_path()?)
            .map(PlanView::from)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Publie pour de vrai. La fenêtre a déjà demandé confirmation ; le cœur
/// refusera quand même si un secret traîne dans le catalogue.
#[tauri::command]
async fn publish_now(app: AppHandle, message: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let plan = stlkm_core::plan(&catalog_path()?).map_err(|e| e.to_string())?;
        stlkm_core::publish::publish(&plan, &message, &mut reporter(app.clone(), "catalogue".to_string()))
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
