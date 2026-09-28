//! Le catalogue : la liste de tes projets, telle qu'elle vit dans le repo
//! `stlkm-catalog`. Un seul fichier TOML, écrit à la main, lu par le hub.

use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::platform::{Platform, PlatformValue};

/// Version de schéma que ce hub sait lire.
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Catalog {
    pub schema: u32,
    /// Identifiants des dépôts que le hub n'affiche pas.
    #[serde(default)]
    pub hidden: Vec<String>,
    #[serde(default, rename = "project")]
    pub projects: Vec<Project>,
}

impl Project {
    /// Ce projet doit-il apparaître dans le hub de cette plateforme ?
    pub fn visible_on(&self, platform: Platform) -> bool {
        self.platforms.is_empty() || self.platforms.contains(&platform)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Project {
    /// Identifiant stable, utilisé en ligne de commande et comme nom de dossier.
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Emoji ou chemin d'icône, pour la GUI.
    #[serde(default)]
    pub icon: Option<String>,

    /// D'où vient le projet.
    pub source: Source,
    /// Commande à jouer une fois les fichiers en place (`npm ci`, `cargo build`…).
    #[serde(default)]
    pub install: Option<PlatformValue>,
    /// Ce que « Lancer » veut dire pour ce projet. Facultatif : quand la fiche
    /// ne le dit pas, le hub le devine en regardant les fichiers installés.
    #[serde(default)]
    pub launch: Option<Launch>,

    /// Les hubs où ce projet a sa place. Vide — le cas courant — veut dire
    /// partout : un projet n'a pas à se déclarer pour apparaître. On ne s'en
    /// sert que pour ce qui n'aurait aucun sens ailleurs, comme le hub mobile
    /// dans le répertoire du PC.
    #[serde(default)]
    pub platforms: Vec<Platform>,

    /// Retiré du répertoire, mais encore installé sur cette machine. Jamais
    /// écrit dans le fichier : c'est une constatation, pas un réglage.
    #[serde(skip)]
    pub hidden: bool,
}

/// Où le hub va chercher les fichiers du projet.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Source {
    /// Un asset attaché à une release GitHub. La version = le tag.
    GithubRelease {
        /// `proprietaire/depot`
        repo: String,
        /// Nom de l'asset à télécharger (peut contenir `*`).
        asset: PlatformValue,
        #[serde(default)]
        prerelease: bool,
    },
    /// Le dépôt lui-même, cloné puis mis à jour. La version = le commit.
    GithubRepo {
        repo: String,
        #[serde(default)]
        branch: Option<String>,
    },
}

impl Source {
    /// `proprietaire/depot`, quelle que soit la façon dont le projet arrive.
    pub fn repo(&self) -> Option<&str> {
        match self {
            Source::GithubRelease { repo, .. } | Source::GithubRepo { repo, .. } => Some(repo),
        }
    }
}

/// Les trois façons de lancer un projet.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Launch {
    /// Un exécutable ou une app fenêtrée : on le démarre, point.
    Process {
        exec: PlatformValue,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        cwd: Option<String>,
    },
    /// Un outil en ligne de commande : on ouvre un terminal dessus.
    Terminal {
        command: PlatformValue,
        #[serde(default)]
        cwd: Option<String>,
    },
    /// Un dossier de fichiers statiques : le hub le sert lui-même, sans
    /// dépendance à installer, et ouvre le navigateur dessus.
    Static {
        #[serde(default)]
        dir: Option<String>,
        #[serde(default)]
        port: Option<u16>,
    },
    /// Une app web : on démarre le serveur, on attend, on ouvre le navigateur.
    Server {
        command: PlatformValue,
        #[serde(default)]
        cwd: Option<String>,
        /// Adresse à ouvrir une fois le serveur prêt.
        url: String,
        /// Motif attendu sur la sortie du serveur avant d'ouvrir le navigateur.
        #[serde(default)]
        ready: Option<String>,
    },
}

impl Catalog {
    /// Un répertoire sans rien dedans.
    pub fn empty() -> Self {
        Catalog {
            schema: SCHEMA_VERSION,
            hidden: Vec::new(),
            projects: Vec::new(),
        }
    }

    /// Lit un catalogue depuis du TOML et vérifie qu'il tient debout.
    pub fn parse(toml_src: &str) -> Result<Self> {
        let catalog: Catalog = toml::from_str(crate::state::sans_bom(toml_src))?;
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        Self::parse(&std::fs::read_to_string(path)?)
    }

    fn validate(&self) -> Result<()> {
        if self.schema > SCHEMA_VERSION {
            return Err(Error::Schema {
                found: self.schema,
                max: SCHEMA_VERSION,
            });
        }
        let mut seen = HashSet::new();
        for project in &self.projects {
            if !seen.insert(project.id.as_str()) {
                return Err(Error::DuplicateId(project.id.clone()));
            }
        }
        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<&Project> {
        self.projects
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| Error::UnknownProject(id.to_string()))
    }
}

impl Project {
    /// Étiquette courte du type de projet, pour l'affichage.
    pub fn kind_label(&self) -> &'static str {
        match &self.launch {
            Some(Launch::Process { .. }) => "app",
            Some(Launch::Terminal { .. }) => "cli",
            Some(Launch::Static { .. }) | Some(Launch::Server { .. }) => "web",
            None => "?",
        }
    }

    /// Comment lancer le projet installé dans `dir`.
    ///
    /// La fiche a le dernier mot ; sinon on regarde ce qu'il y a dans le
    /// dossier.
    pub fn launch_in(&self, dir: &std::path::Path) -> Option<Launch> {
        if let Some(launch) = &self.launch {
            return Some(launch.clone());
        }
        crate::guess::recipe(dir).map(|r| r.launch)
    }

    /// Ce qu'il faut faire après avoir récupéré les fichiers.
    pub fn install_in(&self, dir: &std::path::Path) -> Option<PlatformValue> {
        if let Some(install) = &self.install {
            return Some(install.clone());
        }
        crate::guess::recipe(dir).and_then(|r| r.install)
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
schema = 1

[[project]]
id = "demo"
name = "Démo"
source = { kind = "github-release", repo = "moi/demo", asset = "demo-win.zip" }
launch = { kind = "process", exec = { windows = "demo.exe", linux = "demo" } }
"#;

    const SANS_LANCEMENT: &str = r#"
schema = 1

[[project]]
id = "devine-moi"
name = "Devine-moi"
source = { kind = "github-repo", repo = "moi/devine-moi" }
"#;

    #[test]
    fn une_fiche_sans_plateforme_est_partout() {
        let toml = "schema = 1

[[project]]
id = \"x\"
name = \"X\"
source = { kind = \"github-repo\", repo = \"moi/x\" }
";
        let p = &Catalog::parse(toml).unwrap().projects[0];
        assert!(p.visible_on(Platform::Windows));
        assert!(p.visible_on(Platform::Android));
    }

    #[test]
    fn une_fiche_mobile_ne_concerne_pas_le_pc() {
        let toml = "schema = 1

[[project]]
id = \"x\"
name = \"X\"
platforms = [\"android\"]
source = { kind = \"github-repo\", repo = \"moi/x\" }
";
        let p = &Catalog::parse(toml).unwrap().projects[0];
        assert!(!p.visible_on(Platform::Windows));
        assert!(p.visible_on(Platform::Android));
    }

    #[test]
    fn lit_un_projet_minimal() {
        let catalog = Catalog::parse(SAMPLE).expect("catalogue valide");
        let demo = catalog.get("demo").expect("projet demo");
        assert_eq!(demo.name, "Démo");
        assert_eq!(demo.kind_label(), "app");
    }

    #[test]
    fn une_fiche_peut_ne_rien_dire_du_lancement() {
        let catalog = Catalog::parse(SANS_LANCEMENT).expect("catalogue valide");
        let projet = catalog.get("devine-moi").expect("projet");
        assert!(projet.launch.is_none());
        assert_eq!(projet.kind_label(), "?");
    }

    #[test]
    fn refuse_les_identifiants_en_double() {
        let doubled = format!("{SAMPLE}{}", SAMPLE.trim_start_matches("\nschema = 1\n"));
        assert!(matches!(
            Catalog::parse(&doubled),
            Err(Error::DuplicateId(_))
        ));
    }

    #[test]
    fn refuse_un_schema_trop_recent() {
        let future = SAMPLE.replace("schema = 1", "schema = 99");
        assert!(matches!(Catalog::parse(&future), Err(Error::Schema { .. })));
    }
}
