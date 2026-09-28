//! Les formes que la fenêtre reçoit. Le cœur a ses types ; la fenêtre a besoin
//! de texte déjà prêt à afficher, et rien d'autre.

use serde::Serialize;

use stlkm_core::update::{Check, Status};
use stlkm_core::{Launch, Plan, Platform, Project, Source, State};

/// Ce que la fenêtre a besoin de savoir pour décider si l'Atelier existe.
#[derive(Serialize)]
pub struct ModeView {
    pub mode: String,
    pub owner: bool,
    /// D'où vient le répertoire affiché.
    pub origin: String,
    /// Le compte GitHub dont les dépôts sont proposés.
    pub account: String,
}

#[derive(Serialize)]
pub struct ProjectView {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub kind: String,
    pub icon: Option<String>,
    pub tags: Vec<String>,
    pub installed: bool,
    pub version: Option<String>,
    pub path: Option<String>,
    pub source: String,
    pub launch: String,
    pub running: bool,
    /// Vrai quand lancer ouvre une adresse (le hub sait alors l'arrêter).
    pub servable: bool,
    /// Vrai quand la fiche vient du fichier de corrections.
    pub from_file: bool,
    /// Retiré du répertoire, mais encore installé ici.
    pub hidden: bool,
    /// Les hubs où la fiche a sa place. Vide = partout.
    pub platforms: Vec<String>,
}

impl ProjectView {
    pub fn new(project: &Project, state: &State, running: bool, from_file: bool) -> Self {
        let platform = Platform::current();
        let installed = state.get(&project.id);

        ProjectView {
            id: project.id.clone(),
            name: project.name.clone(),
            summary: project.summary.clone(),
            kind: project.kind_label().to_string(),
            icon: project.icon.clone(),
            tags: project.tags.clone(),
            installed: installed.is_some(),
            version: installed.map(|i| i.version.clone()),
            path: installed.map(|i| i.path.display().to_string()),
            source: describe_source(&project.source, platform),
            launch: describe_launch(&project.launch, platform),
            running,
            from_file,
            hidden: project.hidden,
            platforms: project.platforms.iter().map(|p| p.as_str().to_string()).collect(),
            servable: matches!(
                project.launch,
                Some(Launch::Server { .. }) | Some(Launch::Static { .. })
            ),
        }
    }
}

#[derive(Serialize)]
pub struct CheckView {
    pub id: String,
    pub name: String,
    pub current: Option<String>,
    pub latest: Option<String>,
    /// Un mot simple pour la fenêtre : absent, a-jour, dispo, local, inconnu.
    pub status: &'static str,
    pub label: String,
}

impl From<Check> for CheckView {
    fn from(check: Check) -> Self {
        let status = match check.status {
            Status::Absent => "absent",
            Status::UpToDate => "a-jour",
            Status::Available => "dispo",
            Status::Removed => "retire",
            Status::Unknown(_) => "inconnu",
        };
        CheckView {
            label: check.status.label(),
            id: check.id,
            name: check.name,
            current: check.current,
            latest: check.latest,
            status,
        }
    }
}

#[derive(Serialize)]
pub struct PlanView {
    pub repo_dir: String,
    pub remote: Option<String>,
    pub changes: Vec<String>,
    pub warnings: Vec<WarningView>,
    pub blocked: bool,
    pub empty: bool,
}

#[derive(Serialize)]
pub struct WarningView {
    pub line: usize,
    pub reason: String,
}

impl From<Plan> for PlanView {
    fn from(plan: Plan) -> Self {
        PlanView {
            repo_dir: plan.repo_dir.display().to_string(),
            remote: plan.remote.clone(),
            blocked: plan.is_blocked(),
            empty: plan.is_empty(),
            changes: plan.changes.clone(),
            warnings: plan
                .warnings
                .iter()
                .map(|w| WarningView {
                    line: w.line,
                    reason: w.reason.clone(),
                })
                .collect(),
        }
    }
}

fn describe_source(source: &Source, platform: Platform) -> String {
    match source {
        Source::GithubRelease { repo, asset, .. } => match asset.get(platform) {
            Some(asset) => format!("release {repo} ({asset})"),
            None => "rien pour cette plateforme".to_string(),
        },
        Source::GithubRepo { repo, .. } => format!("dépôt {repo}"),
    }
}

fn describe_launch(launch: &Option<Launch>, platform: Platform) -> String {
    let Some(launch) = launch else {
        return "deviné une fois installé".to_string();
    };
    match launch {
        Launch::Process { exec, .. } => exec.get(platform).unwrap_or("—").to_string(),
        Launch::Terminal { command, .. } => command.get(platform).unwrap_or("—").to_string(),
        Launch::Static { dir, .. } => format!("dossier {}", dir.as_deref().unwrap_or(".")),
        Launch::Server { command, url, .. } => {
            format!("{}  →  {url}", command.get(platform).unwrap_or("—"))
        }
    }
}

/// Une appli Android trouvée par l'atelier mobile.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MobileView {
    pub id: String,
    pub name: String,
    pub tag: String,
    pub apk: String,
    pub autres: Vec<String>,
    /// La même release sert aussi un PC : la fiche reste dans les deux hubs.
    pub aussi_pc: bool,
    /// Ce que la fiche dit aujourd'hui. Vide = elle est dans les deux hubs.
    pub platforms: Vec<String>,
}

impl MobileView {
    pub fn new(t: &stlkm_core::mobile::Trouvaille, platforms: Vec<String>) -> Self {
        MobileView {
            id: t.id.clone(),
            name: t.name.clone(),
            tag: t.tag.clone(),
            apk: t.apk.clone(),
            autres: t.autres_apk.clone(),
            aussi_pc: t.aussi_pc,
            platforms,
        }
    }
}

/// L'état du jeton GitHub, sans jamais rendre le jeton lui-même.
#[derive(Debug, Clone, Serialize)]
pub struct TokenView {
    pub present: bool,
    pub limite: u32,
    pub restant: u32,
}
