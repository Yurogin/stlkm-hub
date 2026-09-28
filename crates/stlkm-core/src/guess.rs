//! Deviner ce qu'on peut faire d'un projet en regardant ses fichiers.
//!
//! Le hub ne demande rien à personne : une fois le dépôt téléchargé, il ouvre
//! le dossier et décide comment installer et lancer. La fiche du répertoire
//! peut toujours corriger la devinette, mais elle n'est pas obligatoire.

use std::path::{Path, PathBuf};

use crate::catalog::Launch;
use crate::platform::PlatformValue;

/// Ce qu'on a compris d'un dossier.
#[derive(Debug, Clone)]
pub struct Recipe {
    /// Comment lancer le projet.
    pub launch: Launch,
    /// Ce qu'il faut faire avant, une seule fois (`npm ci`, `pip install`…).
    pub install: Option<PlatformValue>,
}

/// Examine un dossier et propose une façon de l'installer et de le lancer.
pub fn recipe(dir: &Path) -> Option<Recipe> {
    let files = Files::read(dir)?;
    let nom = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    // Node
    if let Some(raw) = files.read_text("package.json") {
        let install = Some(PlatformValue::Same("npm ci".into()));
        // Electron ouvre sa propre fenêtre : pas de navigateur à ouvrir.
        if raw.contains("\"electron\"") {
            let script = if raw.contains("\"start\"") { "npm start" } else { "npm run dev" };
            return Some(Recipe {
                launch: Launch::Terminal {
                    command: PlatformValue::Same(script.into()),
                    cwd: None,
                },
                install,
            });
        }
        if raw.contains("\"dev\"") {
            return Some(Recipe {
                launch: Launch::Server {
                    command: PlatformValue::Same("npm run dev".into()),
                    cwd: None,
                    url: format!("http://localhost:{}", port(&raw)),
                    ready: Some("Local:".into()),
                },
                install,
            });
        }
        if raw.contains("\"start\"") {
            return Some(Recipe {
                launch: Launch::Terminal {
                    command: PlatformValue::Same("npm start".into()),
                    cwd: None,
                },
                install,
            });
        }
    }

    // PHP
    if files.has("composer.json") || files.has("index.php") {
        let racine = if files.has("public") { "public" } else { "." };
        return Some(Recipe {
            launch: Launch::Server {
                command: PlatformValue::Same(format!("php -S localhost:8080 -t {racine}")),
                cwd: None,
                url: "http://localhost:8080".into(),
                ready: None,
            },
            install: files
                .has("composer.json")
                .then(|| PlatformValue::Same("composer install".into())),
        });
    }

    // Python
    if let Some(entree) = python_entry(&files, &nom) {
        return Some(Recipe {
            launch: Launch::Terminal {
                command: PlatformValue::PerPlatform {
                    windows: Some(format!("python {entree}")),
                    linux: Some(format!("python3 {entree}")),
                    android: None,
                },
                cwd: None,
            },
            install: files
                .has("requirements.txt")
                .then(|| PlatformValue::Same("pip install -r requirements.txt".into())),
        });
    }

    // Rust
    if files.has("Cargo.toml") {
        return Some(Recipe {
            launch: Launch::Terminal {
                command: PlatformValue::Same("cargo run --release".into()),
                cwd: None,
            },
            install: Some(PlatformValue::Same("cargo build --release".into())),
        });
    }

    // Un dossier de pages, servi par le hub lui-même.
    if files.has("index.html") {
        return Some(Recipe {
            launch: Launch::Static {
                dir: None,
                port: None,
            },
            install: None,
        });
    }

    // Un exécutable posé là.
    if let Some(exe) = files.with_extension("exe").first() {
        return Some(Recipe {
            launch: Launch::Process {
                exec: PlatformValue::Same((*exe).to_string()),
                args: Vec::new(),
                cwd: None,
            },
            install: None,
        });
    }

    None
}

/// Le contenu direct d'un dossier, lu une seule fois.
struct Files {
    names: Vec<String>,
    dir: PathBuf,
}

impl Files {
    fn read(path: &Path) -> Option<Self> {
        let names = std::fs::read_dir(path)
            .ok()?
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        Some(Files {
            names,
            dir: path.to_path_buf(),
        })
    }

    fn has(&self, name: &str) -> bool {
        self.names.iter().any(|n| n.eq_ignore_ascii_case(name))
    }

    fn with_extension(&self, ext: &str) -> Vec<&str> {
        let suffixe = format!(".{ext}");
        self.names
            .iter()
            .filter(|n| n.to_ascii_lowercase().ends_with(&suffixe))
            .map(String::as_str)
            .collect()
    }

    fn read_text(&self, name: &str) -> Option<String> {
        std::fs::read_to_string(self.dir.join(name)).ok()
    }
}

/// Le port par défaut dépend de l'outil de développement utilisé.
fn port(package_json: &str) -> u16 {
    if package_json.contains("\"vite\"") {
        5173
    } else if package_json.contains("\"next\"") || package_json.contains("react-scripts") {
        3000
    } else {
        8080
    }
}

/// Choisit le fichier python à lancer parmi ceux du dossier.
fn python_entry<'a>(files: &'a Files, nom_du_dossier: &str) -> Option<&'a str> {
    let scripts = files.with_extension("py");
    if scripts.is_empty() {
        return None;
    }
    let preferes = [
        "main.py".to_string(),
        "app.py".to_string(),
        "__main__.py".to_string(),
        format!("{}.py", nom_du_dossier.to_ascii_lowercase()),
    ];
    for nom in &preferes {
        if let Some(trouve) = scripts.iter().find(|s| s.eq_ignore_ascii_case(nom)) {
            return Some(trouve);
        }
    }
    // Un seul script : pas d'ambiguïté. Plusieurs : on ne devine pas.
    (scripts.len() == 1).then(|| scripts[0])
}

/// Nom de dépôt vers identifiant utilisable en ligne de commande.
pub fn slug(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_nettoie_les_noms_de_depot() {
        assert_eq!(slug("cleanFiles"), "cleanfiles");
        assert_eq!(slug("Force Brut"), "force-brut");
        assert_eq!(slug("Text-To-file.crx"), "text-to-file-crx");
    }

    #[test]
    fn devine_le_port_selon_loutil() {
        assert_eq!(port(r#"{"devDependencies":{"vite":"^5"}}"#), 5173);
        assert_eq!(port(r#"{"dependencies":{"next":"14"}}"#), 3000);
        assert_eq!(port(r#"{"dependencies":{}}"#), 8080);
    }

    #[test]
    fn un_dossier_de_pages_est_servi_par_le_hub() {
        let dir = std::env::temp_dir().join("stlkm-test-devine-html");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("index.html"), "<h1>salut</h1>").unwrap();

        let recette = recipe(&dir).expect("une recette");
        assert!(matches!(recette.launch, Launch::Static { .. }));
        assert!(recette.install.is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn un_script_python_souvre_dans_un_terminal() {
        let dir = std::env::temp_dir().join("stlkm-test-devine-py");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("main.py"), "print('salut')").unwrap();
        std::fs::write(dir.join("requirements.txt"), "requests\n").unwrap();

        let recette = recipe(&dir).expect("une recette");
        match recette.launch {
            Launch::Terminal { command, .. } => {
                assert_eq!(
                    command.get(crate::platform::Platform::Windows),
                    Some("python main.py")
                );
            }
            autre => panic!("attendu un terminal, obtenu {autre:?}"),
        }
        assert!(recette.install.is_some(), "les dépendances sont installées");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn une_appli_electron_ne_passe_pas_par_le_navigateur() {
        let dir = std::env::temp_dir().join("stlkm-test-devine-electron");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("package.json"),
            r#"{"scripts":{"dev":"electron-vite dev","start":"electron-vite preview"},"devDependencies":{"electron":"^33","vite":"^5"}}"#,
        )
        .unwrap();

        let recette = recipe(&dir).expect("une recette");
        match recette.launch {
            Launch::Terminal { command, .. } => {
                assert_eq!(command.get(crate::platform::Platform::Windows), Some("npm start"));
            }
            autre => panic!("attendu un terminal, obtenu {autre:?}"),
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn un_dossier_sans_rien_de_connu_ne_donne_rien() {
        let dir = std::env::temp_dir().join("stlkm-test-devine-vide");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("notes.txt"), "rien").unwrap();

        assert!(recipe(&dir).is_none());
        std::fs::remove_dir_all(&dir).ok();
    }
}
