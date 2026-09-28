//! L'atelier mobile : quels dépôts donnent une application Android.
//!
//! Le rangement ne se décrète pas, il se lit dans les fichiers publiés. Une
//! release qui contient un `.apk` donne une appli mobile ; si elle ne contient
//! rien pour le PC, c'est une appli **seulement** mobile, qui n'a donc rien à
//! faire dans le répertoire du bureau.
//!
//! Aller voir les releases coûte un appel par dépôt, et GitHub n'en accorde que
//! soixante par heure. C'est pourquoi seul l'Atelier fouille — une fois, sur
//! demande — et écrit sa conclusion dans le catalogue : les hubs, eux, lisent le
//! résultat sans rien payer.

use crate::catalog::Project;
use crate::error::Result;
use crate::github::{self, Release};
use crate::platform::Platform;

/// Ce que l'atelier a trouvé pour un dépôt.
#[derive(Debug, Clone)]
pub struct Trouvaille {
    pub id: String,
    pub name: String,
    /// Le tag de la release regardée.
    pub tag: String,
    /// L'APK à installer (celui du téléphone quand il y a aussi celui d'une montre).
    pub apk: String,
    /// Les autres APK de la release : une montre, une variante.
    pub autres_apk: Vec<String>,
    /// Vrai quand la même release fournit aussi de quoi servir un PC.
    pub aussi_pc: bool,
}

impl Trouvaille {
    /// Les plateformes qui devraient figurer dans la fiche. Vide = partout,
    /// ce qui est le cas d'une appli qui existe des deux côtés.
    pub fn platforms(&self) -> Vec<Platform> {
        if self.aussi_pc {
            Vec::new()
        } else {
            vec![Platform::Android]
        }
    }
}

/// Ce qu'on reconnaît comme « de quoi faire tourner un PC ».
const POUR_PC: [&str; 6] = [".exe", ".msi", ".zip", ".tar.gz", ".appimage", ".deb"];

/// Un APK de montre n'est pas celui qu'on installe en premier.
fn est_montre(nom: &str) -> bool {
    let n = nom.to_ascii_lowercase();
    n.contains("montre") || n.contains("wear") || n.contains("watch")
}

/// Regarde la dernière release d'un dépôt. `None` quand elle n'a pas d'APK,
/// ou qu'il n'y a pas de release du tout.
pub fn examine(depot: &str, nom: &str, id: &str) -> Option<Trouvaille> {
    let release = github::latest_release(depot, false).ok()?;
    depuis(&release, nom, id)
}

/// La partie qui ne parle pas au réseau, pour pouvoir l'éprouver.
pub fn depuis(release: &Release, nom: &str, id: &str) -> Option<Trouvaille> {
    let apks: Vec<&str> = release
        .assets
        .iter()
        .map(|a| a.name.as_str())
        .filter(|n| n.to_ascii_lowercase().ends_with(".apk"))
        .collect();
    if apks.is_empty() {
        return None;
    }

    let principal = apks
        .iter()
        .find(|n| !est_montre(n))
        .copied()
        .unwrap_or(apks[0]);

    let aussi_pc = release.assets.iter().any(|a| {
        let n = a.name.to_ascii_lowercase();
        POUR_PC.iter().any(|ext| n.ends_with(ext))
    });

    Some(Trouvaille {
        id: id.to_string(),
        name: nom.to_string(),
        tag: release.tag.clone(),
        apk: principal.to_string(),
        autres_apk: apks
            .iter()
            .filter(|n| **n != principal)
            .map(|n| n.to_string())
            .collect(),
        aussi_pc,
    })
}

/// Passe les projets en revue et rend ceux qui donnent une appli Android.
///
/// `avance` est appelé après chaque dépôt, pour que l'écran montre où on en est.
/// Une erreur de réseau sur un dépôt ne fait pas échouer la fouille : GitHub
/// finit par refuser de répondre, et ce qu'on a déjà trouvé reste bon.
pub fn fouille(
    projects: &[Project],
    mut avance: impl FnMut(usize, usize, &str),
) -> Result<Vec<Trouvaille>> {
    let mut trouvees = Vec::new();
    let total = projects.len();
    for (i, projet) in projects.iter().enumerate() {
        avance(i + 1, total, &projet.name);
        let Some(depot) = projet.source.repo() else {
            continue;
        };
        if let Some(t) = examine(depot, &projet.name, &projet.id) {
            trouvees.push(t);
        }
    }
    Ok(trouvees)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::Asset;

    fn release(noms: &[&str]) -> Release {
        Release {
            tag: "v1.2".into(),
            prerelease: false,
            draft: false,
            assets: noms
                .iter()
                .map(|n| Asset {
                    name: (*n).into(),
                    browser_download_url: format!("https://exemple/{n}"),
                    size: 10,
                })
                .collect(),
        }
    }

    #[test]
    fn sans_apk_ce_nest_pas_une_appli_mobile() {
        assert!(depuis(&release(&["Truc.exe"]), "Truc", "truc").is_none());
    }

    #[test]
    fn lapk_du_telephone_passe_avant_celui_de_la_montre() {
        let t = depuis(
            &release(&["Klaxon-montre.apk", "Klaxon-telephone.apk"]),
            "Klaxon",
            "klaxon",
        )
        .unwrap();
        assert_eq!(t.apk, "Klaxon-telephone.apk");
        assert_eq!(t.autres_apk, vec!["Klaxon-montre.apk"]);
    }

    #[test]
    fn une_release_avec_un_exe_concerne_les_deux_hubs() {
        let t = depuis(&release(&["Klaxon-telephone.apk", "Klaxon.exe"]), "Klaxon", "klaxon").unwrap();
        assert!(t.aussi_pc);
        assert!(t.platforms().is_empty());
    }

    #[test]
    fn une_release_sans_rien_pour_le_pc_est_rangee_en_mobile() {
        let t = depuis(&release(&["STLKM.apk"]), "STLKM mobile", "stlkm-mobile").unwrap();
        assert!(!t.aussi_pc);
        assert_eq!(t.platforms(), vec![Platform::Android]);
    }
}
