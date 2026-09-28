//! Tout ce qui consiste à lancer quelque chose sur la machine : une commande,
//! un terminal, un navigateur. Windows et Linux ne s'y prennent pas pareil,
//! et c'est le seul endroit du code qui le sait.

use std::path::Path;
use std::process::{Child, Command, Stdio};

use crate::error::{Error, Result};

/// Tout ce que le hub lance en coulisses doit rester invisible.
///
/// La fenêtre du hub n'a pas de console à elle : sans ce réglage, Windows en
/// crée une pour chaque `git`, chaque `npm`, chaque `taskkill` — et l'écran
/// clignote de consoles noires. Seul [`open_terminal`] en veut une, et il la
/// demande explicitement.
fn discret(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// Prépare une commande shell (`cmd /C` sur Windows, `sh -c` ailleurs).
fn shell(command: &str) -> Command {
    let mut process = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.arg("/C");
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("-c");
        c
    };
    process.arg(command);
    discret(&mut process);
    process
}

/// Exécute une commande et attend qu'elle finisse. Échoue si le code de
/// retour n'est pas zéro : une installation ratée ne doit pas passer pour un
/// succès.
pub fn run(command: &str, dir: &Path) -> Result<()> {
    let status = shell(command).current_dir(dir).status()?;
    if status.success() {
        return Ok(());
    }
    Err(Error::CommandFailed {
        cmd: command.to_string(),
        code: status
            .code()
            .map_or_else(|| "interrompue".to_string(), |c| format!("code {c}")),
    })
}

/// Exécute un programme en lui passant ses arguments un par un. À préférer au
/// shell dès qu'un argument peut contenir des espaces : sur Windows, `cmd /C`
/// mélange ses guillemets avec les nôtres et le programme reçoit le chemin
/// entre guillemets, qu'il prend pour un nom de dossier.
pub fn run_program(program: &str, args: &[&str], dir: &Path) -> Result<()> {
    let status = discret(Command::new(program).args(args).current_dir(dir)).status()?;
    if status.success() {
        return Ok(());
    }
    Err(Error::CommandFailed {
        cmd: format!("{program} {}", args.join(" ")),
        code: status
            .code()
            .map_or_else(|| "interrompue".to_string(), |c| format!("code {c}")),
    })
}

/// Exécute un programme et récupère sa sortie standard, sans passer par le shell.
pub fn capture(program: &str, args: &[&str], dir: &Path) -> Result<String> {
    let output = discret(Command::new(program).args(args).current_dir(dir)).output()?;
    if !output.status.success() {
        return Err(Error::CommandFailed {
            cmd: format!("{program} {}", args.join(" ")),
            code: output
                .status
                .code()
                .map_or_else(|| "interrompue".to_string(), |c| format!("code {c}")),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Tue un processus **et toute sa descendance**.
///
/// Sans ça, arrêter un serveur ne marche pas : `npm run dev` est lancé par un
/// shell, le shell lance `node`, et tuer le shell laisse `node` tourner tout
/// seul dans le vide. Vérifié : le bouton « Arrêter » ne rendait pas le port.
pub fn kill_tree(child: &mut Child) {
    let pid = child.id();

    #[cfg(windows)]
    {
        // /T descend l'arbre des processus, /F ne demande pas la permission.
        let _ = discret(
            Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .stdout(Stdio::null())
                .stderr(Stdio::null()),
        )
        .status();
    }

    #[cfg(not(windows))]
    {
        // Le groupe porte le numéro de son chef : le négatif vise tout le groupe.
        let _ = Command::new("kill")
            .args(["-TERM", &format!("-{pid}")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    // Quoi qu'il arrive, on récupère le processus direct pour ne pas le laisser
    // zombie.
    let _ = child.kill();
    let _ = child.wait();
}

/// Démarre une commande sans l'attendre, en gardant sa sortie lisible.
pub fn spawn(command: &str, dir: &Path) -> Result<Child> {
    let mut process = shell(command);
    process
        .current_dir(dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Sur Linux, on met le serveur dans son propre groupe de processus pour
    // pouvoir l'arrêter en entier plus tard.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        process.process_group(0);
    }

    Ok(process.spawn()?)
}

/// Démarre un exécutable directement, sans shell (pas d'histoire de guillemets).
pub fn spawn_exec(exec: &Path, args: &[String], dir: &Path) -> Result<Child> {
    // Une vraie application a sa propre fenêtre : pas de console derrière.
    Ok(discret(Command::new(exec).args(args).current_dir(dir)).spawn()?)
}

/// Ouvre une fenêtre de terminal sur une commande.
///
/// La fenêtre se ferme quand le programme se termine normalement, et ne reste
/// ouverte que s'il a échoué — sinon elles s'accumulent à l'écran, une par
/// lancement, et il faut les fermer une à une.
pub fn open_terminal(command: &str, dir: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        // On demande directement à Windows une nouvelle console, au lieu de
        // passer par `start` : son premier argument est le titre de la fenêtre,
        // mais uniquement s'il est entre guillemets — sinon il est pris pour le
        // programme à lancer, et l'utilisateur voit « Windows ne trouve pas… ».
        const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

        Command::new("cmd")
            .args(garder_si_echec(command))
            .current_dir(dir)
            .creation_flags(CREATE_NEW_CONSOLE)
            .spawn()?;
        return Ok(());
    }

    #[cfg(not(windows))]
    {
        // Même idée : on ne retient la fenêtre que si la commande a échoué.
        let keep_open =
            format!("{command} || {{ echo; read -p 'Échec — Entrée pour fermer '; }}");
        for terminal in ["x-terminal-emulator", "gnome-terminal", "konsole", "xterm"] {
            let started = Command::new(terminal)
                .arg("-e")
                .arg("sh")
                .arg("-c")
                .arg(&keep_open)
                .current_dir(dir)
                .spawn();
            if started.is_ok() {
                return Ok(());
            }
        }
        Err(Error::CommandFailed {
            cmd: "ouverture d'un terminal".to_string(),
            code: "aucun émulateur de terminal trouvé".to_string(),
        })
    }
}

/// Les arguments de `cmd` pour lancer une commande et ne garder la fenêtre
/// que si elle a échoué.
///
/// `/S` est indispensable : sans lui, `cmd` refuse d'enlever les guillemets
/// autour d'une commande contenant `||`, et tente d'exécuter la ligne entière
/// comme si c'était un nom de programme.
#[cfg(windows)]
fn garder_si_echec(command: &str) -> [String; 3] {
    [
        "/S".to_string(),
        "/C".to_string(),
        format!("{command} || pause"),
    ]
}

/// Ouvre un dossier dans l'explorateur de fichiers.
///
/// Un lanceur doit aussi savoir dire « c'est là » : on ne lance pas toujours un
/// projet de la même façon, et il faut parfois aller voir les fichiers.
pub fn reveal(dir: &Path) -> Result<()> {
    if cfg!(windows) {
        // `explorer` rend un code non nul même quand il a ouvert la fenêtre :
        // on ne regarde donc pas son code de retour.
        discret(Command::new("explorer").arg(dir)).spawn()?;
    } else {
        Command::new("xdg-open").arg(dir).spawn()?;
    }
    Ok(())
}

/// Ouvre une adresse dans le navigateur par défaut.
pub fn open_url(url: &str) -> Result<()> {
    if cfg!(windows) {
        // Le premier argument de `start` est le titre de fenêtre : on le laisse
        // vide, sinon l'URL serait prise pour un titre.
        discret(Command::new("cmd").args(["/C", "start", "", url])).spawn()?;
    } else {
        Command::new("xdg-open").arg(url).spawn()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Les guillemets de `cmd` sont un piège : sans `/S`, une commande qui
    /// contient `||` est prise pour un nom de programme.
    #[test]
    #[cfg(windows)]
    fn la_commande_de_secours_sexecute_bien() {
        let args = garder_si_echec("echo salut");
        let sortie = Command::new("cmd")
            .args(&args)
            .output()
            .expect("cmd répond");
        let texte = String::from_utf8_lossy(&sortie.stdout);
        assert!(texte.contains("salut"), "sortie obtenue : {texte}");
        assert!(
            !texte.to_lowercase().contains("pause"),
            "une commande réussie ne doit pas retenir la fenêtre"
        );
    }

    /// Un serveur lancé par un shell doit mourir avec lui.
    ///
    /// On fait battre un cœur dans un fichier : si sa date de modification
    /// n'avance plus, le processus est bien mort. Plus fiable que compter les
    /// processus du système, où d'autres `node` peuvent traîner.
    #[test]
    fn kill_tree_emporte_les_petits_enfants() {
        if Command::new("node").arg("--version").output().is_err() {
            return; // node absent : rien à vérifier ici
        }

        let atelier = std::env::temp_dir();
        let battement = atelier.join("stlkm-battement.txt");
        std::fs::remove_file(&battement).ok();

        // Le script va dans un fichier : une commande sans guillemets ni
        // espaces traverse `cmd /C` sans se faire manger ses quotes.
        let script = atelier.join("stlkm-battement.js");
        std::fs::write(
            &script,
            "const fs = require('fs');\n\
             setInterval(() => fs.writeFileSync('stlkm-battement.txt', String(Date.now())), 200);\n",
        )
        .unwrap();

        let mut child = spawn("node stlkm-battement.js", &atelier).expect("le shell démarre");

        std::thread::sleep(Duration::from_millis(1500));
        assert!(battement.exists(), "le petit-enfant doit battre");

        kill_tree(&mut child);

        std::thread::sleep(Duration::from_millis(800));
        let apres_mort = std::fs::metadata(&battement).unwrap().modified().unwrap();
        std::thread::sleep(Duration::from_millis(1200));
        let plus_tard = std::fs::metadata(&battement).unwrap().modified().unwrap();

        assert_eq!(
            apres_mort, plus_tard,
            "le cœur bat encore : le processus enfant a survécu au kill"
        );
        std::fs::remove_file(&battement).ok();
        std::fs::remove_file(&script).ok();
    }
}
