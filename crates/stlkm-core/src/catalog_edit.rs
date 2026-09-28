//! Écrire dans le catalogue sans l'écraser.
//!
//! Le fichier appartient à l'utilisateur : il y met ses commentaires, ses
//! résumés, l'ordre qui lui plaît. On se contente donc d'ajouter des blocs à la
//! fin et d'en retirer, ligne à ligne. Rien n'est jamais re-généré.

use std::path::Path;

use crate::catalog::{Catalog, Launch, Project, Source};
use crate::error::{Error, Result};
use crate::platform::{Platform, PlatformValue};

/// Ajoute un projet à la fin du catalogue.
pub fn append(path: &Path, project: &Project) -> Result<()> {
    let existing = std::fs::read_to_string(path)?;
    if Catalog::parse(&existing)?.get(&project.id).is_ok() {
        return Err(Error::DuplicateId(project.id.clone()));
    }

    let mut updated = existing;
    if !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(&render(project));

    // On ne remplace le fichier que si le résultat est encore lisible.
    Catalog::parse(&updated)?;
    std::fs::write(path, updated)?;
    Ok(())
}

/// Retire un projet du catalogue. Retourne `false` s'il n'y était pas.
///
/// Le fichier est découpé en blocs `[[project]]` et recopié tel quel, bloc par
/// bloc : tout ce que l'utilisateur a écrit autour survit intact.
pub fn remove(path: &Path, id: &str) -> Result<bool> {
    let existing = std::fs::read_to_string(path)?;
    let lines: Vec<&str> = existing.lines().collect();
    let is_header = |line: &str| line.trim() == "[[project]]";

    let mut out = String::with_capacity(existing.len());
    let mut cursor = 0;

    /// Les lignes vides et les commentaires qui terminent un bloc n'en font pas
    /// partie : ils annoncent le suivant. On les met de côté pour les donner au
    /// bloc d'après — ou les jeter avec lui s'il est supprimé.
    fn detacher<'a>(bloc: &mut Vec<&'a str>) -> Vec<&'a str> {
        let mut annonce = Vec::new();
        while bloc
            .last()
            .is_some_and(|l| l.trim().is_empty() || l.trim().starts_with('#'))
        {
            annonce.insert(0, bloc.pop().unwrap());
        }
        annonce
    }

    // Tout ce qui précède le premier projet : schéma, commentaires d'entête.
    let mut preamble: Vec<&str> = Vec::new();
    while cursor < lines.len() && !is_header(lines[cursor]) {
        preamble.push(lines[cursor]);
        cursor += 1;
    }
    let mut annonce = detacher(&mut preamble);
    for line in preamble {
        out.push_str(line);
        out.push('\n');
    }

    let mut removed = false;
    while cursor < lines.len() {
        let start = cursor;
        cursor += 1;
        while cursor < lines.len() && !is_header(lines[cursor]) {
            cursor += 1;
        }

        let mut block: Vec<&str> = lines[start..cursor].to_vec();
        let suivante = detacher(&mut block);

        let block_id = block
            .iter()
            .find_map(|l| l.trim().strip_prefix("id = "))
            .map(|value| value.trim().trim_matches('"'));

        if block_id == Some(id) {
            // Le commentaire qui l'annonçait parlait de lui : il part aussi.
            removed = true;
        } else {
            for line in annonce.iter().chain(block.iter()) {
                out.push_str(line);
                out.push('\n');
            }
        }
        annonce = suivante;
    }

    for line in &annonce {
        out.push_str(line);
        out.push('\n');
    }

    if !removed {
        return Ok(false);
    }

    // On ne remplace le fichier que si le résultat est encore lisible.
    Catalog::parse(&out)?;
    std::fs::write(path, out)?;
    Ok(true)
}

/// Restreint une fiche aux hubs indiqués, ou la remet partout avec une liste vide.
///
/// La fiche est réécrite : si le fichier n'en avait pas — le projet venait des dépôts —
/// elle y entre, avec ce que le hub avait deviné. C'est le prix à payer pour ranger un
/// projet, et ça se relit.
pub fn set_platforms(path: &Path, project: &Project, platforms: Vec<Platform>) -> Result<()> {
    let mut fiche = project.clone();
    fiche.platforms = platforms;
    fiche.hidden = false; // jamais écrit : c'est une constatation, pas un réglage
    remove(path, &project.id)?;
    append(path, &fiche)
}

/// Range ce que l'atelier mobile a trouvé : la fiche dit quel APK prendre, et
/// n'apparaît plus que sur mobile quand la release n'offre rien pour le PC.
pub fn range_mobile(path: &Path, project: &Project, apk: &str, aussi_pc: bool) -> Result<()> {
    let mut fiche = project.clone();
    fiche.hidden = false;
    fiche.platforms = if aussi_pc {
        Vec::new()
    } else {
        vec![Platform::Android]
    };
    fiche.source = avec_apk(&fiche.source, apk);
    remove(path, &project.id)?;
    append(path, &fiche)
}

/// La même source, en y ajoutant l'APK à prendre dans la release. Un projet
/// suivi par clonage devient un projet suivi par release : c'est ce qui
/// s'installe sur un téléphone.
fn avec_apk(source: &Source, apk: &str) -> Source {
    let (repo, actuel, prerelease) = match source {
        Source::GithubRelease {
            repo,
            asset,
            prerelease,
        } => (repo.clone(), Some(asset.clone()), *prerelease),
        Source::GithubRepo { repo, .. } => (repo.clone(), None, false),
    };

    let (windows, linux) = match actuel {
        // « pareil partout » visait le PC : on garde la valeur des deux côtés.
        Some(PlatformValue::Same(v)) => (Some(v.clone()), Some(v)),
        Some(PlatformValue::PerPlatform { windows, linux, .. }) => (windows, linux),
        None => (None, None),
    };

    Source::GithubRelease {
        repo,
        asset: PlatformValue::PerPlatform {
            windows,
            linux,
            android: Some(apk.to_string()),
        },
        prerelease,
    }
}

/// Le commentaire qui annonce la liste des dépôts masqués. Le hub le gère
/// entièrement : il l'écrit et l'efface avec la ligne qu'il accompagne.
const ANNONCE_MASQUES: &str = "# Dépôts que le répertoire n'affiche pas";

/// Masque un dépôt : il disparaît du répertoire, chez toi comme chez les gens
/// qui ont installé le hub. Retourne `false` s'il était déjà masqué.
pub fn hide(path: &Path, id: &str) -> Result<bool> {
    let mut masques = Catalog::from_path(path)?.hidden;
    if masques.iter().any(|m| m == id) {
        return Ok(false);
    }
    masques.push(id.to_string());
    set_hidden(path, &masques)?;
    Ok(true)
}

/// Remet un dépôt masqué dans le répertoire.
pub fn unhide(path: &Path, id: &str) -> Result<bool> {
    let mut masques = Catalog::from_path(path)?.hidden;
    let avant = masques.len();
    masques.retain(|m| m != id);
    if masques.len() == avant {
        return Ok(false);
    }
    set_hidden(path, &masques)?;
    Ok(true)
}

/// Réécrit la seule ligne `hidden = [...]`, sans toucher au reste du fichier.
fn set_hidden(path: &Path, ids: &[String]) -> Result<()> {
    let existing = std::fs::read_to_string(path)?;
    let ligne = format!(
        "hidden = [{}]",
        ids.iter().map(|i| quote(i)).collect::<Vec<_>>().join(", ")
    );

    let mut sortie = String::with_capacity(existing.len() + ligne.len());
    let mut posee = false;

    for l in existing.lines() {
        let debut = l.trim_start();

        // On efface d'abord tout l'ancien bloc — la ligne *et* son commentaire.
        // Les garder séparément laissait un commentaire orphelin à chaque
        // retrait, et un de plus à chaque fois qu'on remettait la ligne.
        if debut == ANNONCE_MASQUES || (debut.starts_with("hidden") && debut.contains('=')) {
            continue;
        }

        sortie.push_str(l);
        sortie.push('\n');

        // Puis on le réécrit une seule fois, juste après le schéma.
        if !posee && !ids.is_empty() && debut.starts_with("schema") {
            sortie.push('\n');
            sortie.push_str(ANNONCE_MASQUES);
            sortie.push('\n');
            sortie.push_str(&ligne);
            sortie.push('\n');
            posee = true;
        }
    }

    // Les suppressions laissent des lignes vides en trop.
    while sortie.contains("\n\n\n") {
        sortie = sortie.replace("\n\n\n", "\n\n");
    }

    // On ne remplace le fichier que si le résultat est encore lisible.
    Catalog::parse(&sortie)?;
    std::fs::write(path, sortie)?;
    Ok(())
}

/// Rend un projet en TOML, dans le style du catalogue écrit à la main :
/// les champs simples en haut, les tables en ligne.
pub fn render(project: &Project) -> String {
    let mut out = String::new();
    out.push_str("\n[[project]]\n");
    out.push_str(&format!("id = {}\n", quote(&project.id)));
    out.push_str(&format!("name = {}\n", quote(&project.name)));
    if !project.summary.is_empty() {
        out.push_str(&format!("summary = {}\n", quote(&project.summary)));
    }
    if !project.tags.is_empty() {
        let tags: Vec<String> = project.tags.iter().map(|t| quote(t)).collect();
        out.push_str(&format!("tags = [{}]\n", tags.join(", ")));
    }
    if let Some(icon) = &project.icon {
        out.push_str(&format!("icon = {}\n", quote(icon)));
    }
    if !project.platforms.is_empty() {
        let noms: Vec<String> = project.platforms.iter().map(|p| quote(p.as_str())).collect();
        out.push_str(&format!("platforms = [{}]
", noms.join(", ")));
    }
    out.push_str(&format!("source = {}\n", render_source(&project.source)));
    if let Some(install) = &project.install {
        out.push_str(&format!("install = {}\n", render_value(install)));
    }
    // Une fiche qui ne dit rien du lancement laisse le hub deviner.
    if let Some(launch) = &project.launch {
        out.push_str(&format!("launch = {}\n", render_launch(launch)));
    }
    out
}

fn render_source(source: &Source) -> String {
    match source {
        Source::GithubRelease {
            repo,
            asset,
            prerelease,
        } => {
            let mut fields = vec![
                format!("kind = \"github-release\""),
                format!("repo = {}", quote(repo)),
                format!("asset = {}", render_value(asset)),
            ];
            if *prerelease {
                fields.push("prerelease = true".to_string());
            }
            inline(&fields)
        }
        Source::GithubRepo { repo, branch } => {
            let mut fields = vec![
                format!("kind = \"github-repo\""),
                format!("repo = {}", quote(repo)),
            ];
            if let Some(branch) = branch {
                fields.push(format!("branch = {}", quote(branch)));
            }
            inline(&fields)
        }
    }
}

fn render_launch(launch: &Launch) -> String {
    match launch {
        Launch::Process { exec, args, cwd } => {
            let mut fields = vec![
                "kind = \"process\"".to_string(),
                format!("exec = {}", render_value(exec)),
            ];
            if !args.is_empty() {
                let args: Vec<String> = args.iter().map(|a| quote(a)).collect();
                fields.push(format!("args = [{}]", args.join(", ")));
            }
            push_cwd(&mut fields, cwd);
            inline(&fields)
        }
        Launch::Terminal { command, cwd } => {
            let mut fields = vec![
                "kind = \"terminal\"".to_string(),
                format!("command = {}", render_value(command)),
            ];
            push_cwd(&mut fields, cwd);
            inline(&fields)
        }
        Launch::Static { dir, port } => {
            let mut fields = vec!["kind = \"static\"".to_string()];
            if let Some(dir) = dir {
                fields.push(format!("dir = {}", quote(dir)));
            }
            if let Some(port) = port {
                fields.push(format!("port = {port}"));
            }
            inline(&fields)
        }
        Launch::Server {
            command,
            cwd,
            url,
            ready,
        } => {
            let mut fields = vec![
                "kind = \"server\"".to_string(),
                format!("command = {}", render_value(command)),
            ];
            push_cwd(&mut fields, cwd);
            fields.push(format!("url = {}", quote(url)));
            if let Some(ready) = ready {
                fields.push(format!("ready = {}", quote(ready)));
            }
            inline(&fields)
        }
    }
}

fn push_cwd(fields: &mut Vec<String>, cwd: &Option<String>) {
    if let Some(cwd) = cwd {
        fields.push(format!("cwd = {}", quote(cwd)));
    }
}

fn render_value(value: &PlatformValue) -> String {
    match value {
        PlatformValue::Same(v) => quote(v),
        PlatformValue::PerPlatform {
            windows,
            linux,
            android,
        } => {
            let mut fields = Vec::new();
            if let Some(w) = windows {
                fields.push(format!("windows = {}", quote(w)));
            }
            if let Some(l) = linux {
                fields.push(format!("linux = {}", quote(l)));
            }
            if let Some(a) = android {
                fields.push(format!("android = {}", quote(a)));
            }
            inline(&fields)
        }
    }
}

fn inline(fields: &[String]) -> String {
    format!("{{ {} }}", fields.join(", "))
}

/// Chaîne TOML, antislashs et guillemets échappés — les chemins Windows en
/// sont pleins.
fn quote(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projet_demo() -> Project {
        Project {
            id: "demo".into(),
            name: "Démo".into(),
            summary: "Un projet".into(),
            tags: vec!["web".into()],
            icon: Some("🧪".into()),
            platforms: Vec::new(),
            source: Source::GithubRepo {
                repo: "moi/demo".into(),
                branch: None,
            },
            // Un chemin Windows, pour vérifier que les antislashs survivent.
            install: Some(PlatformValue::Same("C:\\Users\\moi\\demo".into())),
            launch: Some(Launch::Static {
                dir: None,
                port: None,
            }),
            hidden: false,
        }
    }

    #[test]
    fn le_bloc_rendu_se_relit() {
        let toml = format!("schema = 1\n{}", render(&projet_demo()));
        let catalog = Catalog::parse(&toml).expect("relecture");
        let demo = catalog.get("demo").unwrap();
        assert_eq!(demo.name, "Démo");
        assert!(matches!(demo.source, Source::GithubRepo { .. }));
    }

    #[test]
    fn les_chemins_windows_sont_echappes() {
        let rendu = render(&projet_demo());
        assert!(rendu.contains("C:\\\\Users\\\\moi\\\\demo"));
    }

    #[test]
    fn ajoute_puis_retire_sans_toucher_au_reste() {
        let dir = std::env::temp_dir().join("stlkm-test-edit");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("catalog.toml");
        std::fs::write(
            &file,
            "# mon commentaire\nschema = 1\n\n[[project]]\nid = \"garde\"\nname = \"Gardé\"\nsource = { kind = \"github-repo\", repo = \"moi/x\" }\nlaunch = { kind = \"static\" }\n",
        )
        .unwrap();

        append(&file, &projet_demo()).unwrap();
        let after_add = std::fs::read_to_string(&file).unwrap();
        assert!(after_add.contains("# mon commentaire"));
        assert_eq!(Catalog::parse(&after_add).unwrap().projects.len(), 2);

        assert!(remove(&file, "demo").unwrap());
        let after_remove = std::fs::read_to_string(&file).unwrap();
        let catalog = Catalog::parse(&after_remove).unwrap();
        assert_eq!(catalog.projects.len(), 1);
        assert!(catalog.get("garde").is_ok());
        assert!(after_remove.contains("# mon commentaire"));

        std::fs::remove_file(&file).ok();
    }

    #[test]
    fn le_commentaire_part_avec_son_projet() {
        let dir = std::env::temp_dir().join("stlkm-test-commentaires");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("catalog.toml");
        std::fs::write(
            &file,
            "# entête
schema = 1

# ── le premier ──
[[project]]
id = \"un\"
name = \"Un\"
source = { kind = \"github-repo\", repo = \"moi/x\" }
launch = { kind = \"static\" }

# ── le second ──
[[project]]
id = \"deux\"
name = \"Deux\"
source = { kind = \"github-repo\", repo = \"moi/x\" }
launch = { kind = \"static\" }
",
        )
        .unwrap();

        assert!(remove(&file, "un").unwrap());
        let reste = std::fs::read_to_string(&file).unwrap();

        assert!(reste.contains("# entête"), "l'entête du fichier reste");
        assert!(!reste.contains("le premier"), "son commentaire part avec lui");
        assert!(reste.contains("le second"), "celui du voisin reste");
        assert_eq!(Catalog::parse(&reste).unwrap().projects.len(), 1);
        std::fs::remove_file(&file).ok();
    }

    #[test]
    fn masquer_et_demasquer_ne_laisse_rien_derriere() {
        let dir = std::env::temp_dir().join("stlkm-test-masques");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("catalog.toml");
        let depart = "# mon catalogue
schema = 1
";
        std::fs::write(&file, depart).unwrap();

        // Plusieurs allers-retours : c'est là que les commentaires
        // s'accumulaient, un de plus à chaque fois.
        for _ in 0..3 {
            hide(&file, "truc").unwrap();
            unhide(&file, "truc").unwrap();
        }
        hide(&file, "truc").unwrap();

        let texte = std::fs::read_to_string(&file).unwrap();
        assert_eq!(
            texte.matches(ANNONCE_MASQUES).count(),
            1,
            "un seul commentaire, pas un par passage :
{texte}"
        );
        assert_eq!(texte.matches("hidden =").count(), 1);
        assert_eq!(Catalog::parse(&texte).unwrap().hidden, vec!["truc"]);

        unhide(&file, "truc").unwrap();
        let vide = std::fs::read_to_string(&file).unwrap();
        assert!(!vide.contains(ANNONCE_MASQUES), "plus rien quand la liste est vide");
        assert!(vide.contains("# mon catalogue"), "le reste du fichier survit");

        std::fs::remove_file(&file).ok();
    }

    #[test]
    fn retirer_un_absent_ne_change_rien() {
        let dir = std::env::temp_dir().join("stlkm-test-edit-2");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("catalog.toml");
        let source = "schema = 1\n";
        std::fs::write(&file, source).unwrap();

        assert!(!remove(&file, "inconnu").unwrap());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), source);
        std::fs::remove_file(&file).ok();
    }
}
