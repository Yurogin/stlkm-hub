use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use stlkm_core::update::Status;
use stlkm_core::{Catalog, Config, Launch, Platform, Project, Source, State};

#[derive(Parser)]
#[command(name = "stlkm", version, about = "Le hub de tes projets")]
struct Cli {
    /// Fichier de corrections à lire au lieu de celui configuré
    #[arg(long, global = true, value_name = "FICHIER")]
    catalog: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Liste les applications du répertoire
    List {
        /// Ne garder que celles portant ce tag
        #[arg(long)]
        tag: Option<String>,
    },
    /// Affiche le détail d'une application
    Show { id: String },
    /// Installe une application
    Install { id: String },
    /// Lance une application installée
    Run { id: String },
    /// Affiche le dossier d'une application installée
    Where { id: String },
    /// Ouvre ce dossier dans l'explorateur de fichiers
    Open { id: String },
    /// Sans identifiant : liste les mises à jour. Avec : l'applique.
    Update { id: Option<String> },
    /// Retire une application installée
    Uninstall { id: String },
    /// Ce qui est installé sur cette machine
    Status,
    /// Retire un dépôt du répertoire (il n'apparaîtra plus chez personne)
    Hide { id: String },
    /// Remet un dépôt masqué dans le répertoire
    Unhide { id: String },
    /// Va rechercher la dernière version du répertoire
    Refresh,
    /// Affiche ou change les réglages du hub
    Config {
        /// Dépôt GitHub qui publie le répertoire (proprietaire/depot)
        #[arg(long)]
        repo: Option<String>,
        /// Clone local du fichier de corrections — c'est lui qui fait de toi
        /// le propriétaire
        #[arg(long, value_name = "FICHIER")]
        catalog_file: Option<PathBuf>,
    },
    /// Commite et pousse le répertoire sur GitHub
    Publish {
        /// Message de commit
        #[arg(short, long)]
        message: Option<String>,
        /// Ne pas demander confirmation
        #[arg(long)]
        yes: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("stlkm : {err}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> stlkm_core::Result<()> {
    match cli.command {
        Command::List { tag } => list(&load_catalog(cli.catalog)?, tag.as_deref()),
        Command::Show { id } => show(load_catalog(cli.catalog)?.get(&id)?),
        Command::Install { id } => install(load_catalog(cli.catalog)?.get(&id)?)?,
        Command::Run { id } => launch(load_catalog(cli.catalog)?.get(&id)?)?,
        Command::Where { id } => where_is(&id, false)?,
        Command::Open { id } => where_is(&id, true)?,
        Command::Update { id } => update(&load_catalog(cli.catalog)?, id.as_deref())?,
        Command::Uninstall { id } => uninstall(load_catalog(cli.catalog)?.get(&id)?)?,
        Command::Status => status(&load_catalog(cli.catalog)?)?,
        Command::Hide { id } => hide(catalog_path(cli.catalog)?, &id)?,
        Command::Unhide { id } => unhide(catalog_path(cli.catalog)?, &id)?,
        Command::Refresh => refresh()?,
        Command::Config { repo, catalog_file } => configure(repo, catalog_file)?,
        Command::Publish { message, yes } => publish(catalog_path(cli.catalog)?, message, yes)?,
    }
    Ok(())
}

/// Le répertoire : les dépôts publics du compte, corrigés par le fichier.
fn load_catalog(path: Option<PathBuf>) -> stlkm_core::Result<Catalog> {
    match path {
        Some(path) => Catalog::from_path(path),
        None => stlkm_core::repository::load(),
    }
}

/// Le fichier que l'on peut modifier. Sans clone local, il n'y en a pas.
fn catalog_path(path: Option<PathBuf>) -> stlkm_core::Result<PathBuf> {
    match path {
        Some(path) => Ok(path),
        None => stlkm_core::repository::editable_path(),
    }
}

/// Ce que le cœur raconte pendant qu'il travaille.
fn steps() -> impl FnMut(&str) {
    |message: &str| println!("  {message}")
}

// ── Consultation ────────────────────────────────────────────────────────────

fn list(catalog: &Catalog, tag: Option<&str>) {
    let state = State::load().unwrap_or_default();
    let projects: Vec<&Project> = catalog
        .projects
        .iter()
        .filter(|p| tag.is_none_or(|t| p.tags.iter().any(|pt| pt == t)))
        .collect();

    if projects.is_empty() {
        println!("Le répertoire est vide.");
        return;
    }

    let id_width = projects.iter().map(|p| p.id.chars().count()).max().unwrap();
    let name_width = projects
        .iter()
        .map(|p| p.name.chars().count())
        .max()
        .unwrap();

    for project in projects {
        let mark = if state.is_installed(&project.id) {
            "✓"
        } else {
            " "
        };
        let retire = if project.hidden { "  (retiré)" } else { "" };
        println!(
            "{mark} {:<id_width$}  {:<3}  {:<name_width$}  {}{retire}",
            project.id,
            project.kind_label(),
            project.name,
            project.summary,
        );
    }
}

fn show(project: &Project) {
    let platform = Platform::current();
    let state = State::load().unwrap_or_default();

    println!("{}  [{}]", project.name, project.kind_label());
    if !project.summary.is_empty() {
        println!("{}", project.summary);
    }
    println!();
    println!("  id       {}", project.id);
    if !project.tags.is_empty() {
        println!("  tags     {}", project.tags.join(", "));
    }
    println!("  source   {}", describe_source(&project.source, platform));

    match state.get(&project.id) {
        Some(installed) => {
            // Installé : on peut montrer ce que « Lancer » fera vraiment,
            // devinette comprise.
            let devine = project.launch.is_none();
            let launch = project.launch_in(&installed.path);
            let texte = describe_launch(&launch, platform);
            if devine {
                println!("  lancer   {texte}  (deviné)");
            } else {
                println!("  lancer   {texte}");
            }
            println!("  version  {}", installed.version);
            println!("  dossier  {}", installed.path.display());
        }
        None => {
            println!("  lancer   {}", describe_launch(&project.launch, platform));
            println!("  installé non");
        }
    }

    if project.hidden {
        println!();
        println!("  Retiré du répertoire : tu peux le désinstaller.");
    }
}

fn status(catalog: &Catalog) -> stlkm_core::Result<()> {
    let state = State::load()?;
    if state.installed.is_empty() {
        println!("Rien d'installé pour l'instant.");
        return Ok(());
    }
    for (id, installed) in &state.installed {
        let name = catalog.get(id).map(|p| p.name.as_str()).unwrap_or("?");
        println!(
            "{id:<18}  {:<10}  {name}  —  {}",
            installed.version,
            installed.path.display()
        );
    }
    Ok(())
}

/// Où sont les fichiers, et au besoin ouvrir le dossier.
fn where_is(id: &str, ouvrir: bool) -> stlkm_core::Result<()> {
    let state = State::load()?;
    let installed = state
        .get(id)
        .ok_or_else(|| stlkm_core::Error::NotInstalled(id.to_string()))?;

    println!("{}", installed.path.display());
    if ouvrir {
        stlkm_core::process::reveal(&installed.path)?;
    }
    Ok(())
}

// ── Actions ─────────────────────────────────────────────────────────────────

fn install(project: &Project) -> stlkm_core::Result<()> {
    println!("Installation de {}", project.name);
    stlkm_core::install(project, &mut steps())?;
    Ok(())
}

fn uninstall(project: &Project) -> stlkm_core::Result<()> {
    println!("Retrait de {}", project.name);
    stlkm_core::uninstall(project, &mut steps())?;
    println!("  retiré");
    Ok(())
}

fn launch(project: &Project) -> stlkm_core::Result<()> {
    println!("Lancement de {}", project.name);
    let mut session = stlkm_core::run(project, &mut steps())?;

    if session.is_detached() {
        println!("  c'est parti (le projet vit dans sa propre fenêtre)");
        return Ok(());
    }

    println!("  Ctrl-C pour arrêter");
    session.wait()?;
    Ok(())
}

fn update(catalog: &Catalog, id: Option<&str>) -> stlkm_core::Result<()> {
    let state = State::load()?;

    let Some(id) = id else {
        for check in stlkm_core::check_all(catalog, &state) {
            let version = match (&check.current, &check.latest) {
                (Some(current), Some(latest)) if check.status == Status::Available => {
                    format!("{current} → {latest}")
                }
                (Some(current), _) => current.clone(),
                (None, _) => "—".to_string(),
            };
            println!("{:<18}  {:<20}  {}", check.id, version, check.status.label());
        }
        return Ok(());
    };

    let project = catalog.get(id)?;
    println!("Mise à jour de {}", project.name);
    stlkm_core::update::update(project, &mut steps())?;
    Ok(())
}

// ── Le répertoire ───────────────────────────────────────────────────────────

fn hide(catalog: PathBuf, id: &str) -> stlkm_core::Result<()> {
    if stlkm_core::catalog_edit::hide(&catalog, id)? {
        println!("« {id} » ne fait plus partie du répertoire.");
        println!("Publie pour que ça vaille aussi chez les autres : stlkm publish");
    } else {
        println!("« {id} » en était déjà retiré.");
    }
    Ok(())
}

fn unhide(catalog: PathBuf, id: &str) -> stlkm_core::Result<()> {
    if stlkm_core::catalog_edit::unhide(&catalog, id)? {
        println!("« {id} » est de retour dans le répertoire.");
    } else {
        println!("« {id} » n'en était pas retiré.");
    }
    Ok(())
}

fn refresh() -> stlkm_core::Result<()> {
    let catalog = stlkm_core::repository::refresh()?;
    println!(
        "Répertoire à jour : {} application(s) — {}",
        catalog.projects.len(),
        stlkm_core::repository::origin()?.label()
    );
    Ok(())
}

fn configure(repo: Option<String>, catalog_file: Option<PathBuf>) -> stlkm_core::Result<()> {
    let mut config = Config::load()?;
    let touche = repo.is_some() || catalog_file.is_some();

    if let Some(repo) = repo {
        config.catalog_repo = Some(repo);
    }
    if let Some(file) = catalog_file {
        config.catalog_path = Some(file);
    }
    if touche {
        config.save()?;
    }

    println!("mode        {}", config.mode().as_str());
    println!("compte      {}", config.account());
    println!("dépôt       {}", config.repo());
    println!(
        "corrections {}",
        config
            .editable_catalog()
            .map(|p| p.display().to_string())
            .or_else(|| config.catalog_url())
            .unwrap_or_else(|| "—".to_string())
    );
    Ok(())
}

fn publish(catalog: PathBuf, message: Option<String>, yes: bool) -> stlkm_core::Result<()> {
    let plan = stlkm_core::plan(&catalog)?;

    println!("Dépôt   {}", plan.repo_dir.display());
    match &plan.remote {
        Some(remote) => println!("Remote  {remote}"),
        None => println!("Remote  aucun — le commit restera local"),
    }

    if !plan.warnings.is_empty() {
        println!();
        println!("Publication bloquée, ça ressemble à des secrets :");
        for finding in &plan.warnings {
            println!("  ligne {} : {}", finding.line, finding.reason);
        }
        return Ok(());
    }

    if plan.is_empty() {
        println!("Le répertoire n'a pas bougé, rien à publier.");
        return Ok(());
    }

    println!();
    for change in &plan.changes {
        println!("  {change}");
    }

    let message = message.unwrap_or_else(|| "répertoire : mise à jour".to_string());
    if !yes && !confirm(&format!("Publier avec le message « {message} » ?"))? {
        println!("Annulé, rien n'a été poussé.");
        return Ok(());
    }

    stlkm_core::publish::publish(&plan, &message, &mut steps())?;
    Ok(())
}

/// Question fermée, par défaut non : on ne pousse jamais sur un malentendu.
fn confirm(question: &str) -> stlkm_core::Result<bool> {
    use std::io::Write;
    print!("{question} [o/N] ");
    std::io::stdout().flush()?;

    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_lowercase().as_str(),
        "o" | "oui" | "y" | "yes"
    ))
}

// ── Affichage ───────────────────────────────────────────────────────────────

fn describe_source(source: &Source, platform: Platform) -> String {
    match source {
        Source::GithubRelease { repo, asset, .. } => match asset.get(platform) {
            Some(asset) => format!("release github {repo} ({asset})"),
            None => "— rien pour cette plateforme".to_string(),
        },
        Source::GithubRepo { repo, branch } => match branch {
            Some(branch) => format!("dépôt github {repo} ({branch})"),
            None => format!("dépôt github {repo}"),
        },
    }
}

fn describe_launch(launch: &Option<Launch>, platform: Platform) -> String {
    let missing = "— rien pour cette plateforme".to_string();
    let Some(launch) = launch else {
        return "deviné une fois installé".to_string();
    };
    match launch {
        Launch::Process { exec, args, .. } => match exec.get(platform) {
            Some(exec) if args.is_empty() => exec.to_string(),
            Some(exec) => format!("{exec} {}", args.join(" ")),
            None => missing,
        },
        Launch::Terminal { command, .. } => match command.get(platform) {
            Some(command) => format!("terminal : {command}"),
            None => missing,
        },
        Launch::Static { dir, port } => format!(
            "servir {}  sur le port {}",
            dir.as_deref().unwrap_or("."),
            port.map_or("auto".to_string(), |p| p.to_string())
        ),
        Launch::Server { command, url, .. } => match command.get(platform) {
            Some(command) => format!("{command}  puis  {url}"),
            None => missing,
        },
    }
}
