//! Lancer un projet installé, selon ce que « lancer » veut dire pour lui.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use crate::catalog::{Launch, Project};
use crate::error::{Error, Result};
use crate::platform::Platform;
use crate::process;
use crate::server;
use crate::state::State;

/// Ce qui tourne après un lancement. Tant que la session est vivante, le
/// projet l'est aussi ; la laisser tomber revient à l'arrêter.
pub struct Session {
    pub project_id: String,
    /// Adresse ouverte dans le navigateur, s'il y en a une.
    pub url: Option<String>,
    child: Option<Child>,
    stop_flag: Option<Arc<AtomicBool>>,
    /// Vrai quand le projet vit dans sa propre fenêtre : le hub ne le suit plus.
    detached: bool,
}

impl Session {
    fn detached(project_id: &str) -> Self {
        Session {
            project_id: project_id.to_string(),
            url: None,
            child: None,
            stop_flag: None,
            detached: true,
        }
    }

    pub fn is_detached(&self) -> bool {
        self.detached
    }

    /// Attend la fin du projet (Ctrl-C pour un serveur).
    pub fn wait(&mut self) -> Result<()> {
        if let Some(child) = self.child.as_mut() {
            child.wait()?;
        } else if let Some(flag) = self.stop_flag.as_ref() {
            while !flag.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(200));
            }
        }
        Ok(())
    }

    /// Arrête ce qui a été lancé.
    pub fn stop(&mut self) {
        if let Some(flag) = self.stop_flag.take() {
            flag.store(true, Ordering::Relaxed);
        }
        if let Some(mut child) = self.child.take() {
            process::kill_tree(&mut child);
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Lance un projet. Il doit avoir été installé : c'est l'état du hub qui dit
/// où sont ses fichiers.
pub fn run(project: &Project, on_step: &mut dyn FnMut(&str)) -> Result<Session> {
    let state = State::load()?;
    let installed = state
        .get(&project.id)
        .ok_or_else(|| Error::NotInstalled(project.id.clone()))?;
    let root = installed.path.clone();
    let platform = Platform::current();

    // La fiche peut ne rien dire : on regarde alors les fichiers installés.
    let launch = project
        .launch_in(&root)
        .ok_or_else(|| Error::NoLaunch(project.id.clone()))?;

    match &launch {
        Launch::Process { exec, args, cwd } => {
            let exec = pick(exec.get(platform), project, platform)?;
            let dir = working_dir(&root, cwd.as_deref());
            let binary = resolve_exec(&root, exec);
            on_step(&format!("lancement de {}", binary.display()));
            process::spawn_exec(&binary, args, &dir)?;
            Ok(Session::detached(&project.id))
        }

        Launch::Terminal { command, cwd } => {
            let command = pick(command.get(platform), project, platform)?;
            let dir = working_dir(&root, cwd.as_deref());
            on_step(&format!("terminal : {command}"));
            process::open_terminal(command, &dir)?;
            Ok(Session::detached(&project.id))
        }

        Launch::Static { dir, port } => {
            let served = working_dir(&root, dir.as_deref());
            let (url, flag) = server::serve(&served, *port)?;
            on_step(&format!("dossier servi sur {url}"));
            process::open_url(&url)?;
            Ok(Session {
                project_id: project.id.clone(),
                url: Some(url),
                child: None,
                stop_flag: Some(flag),
                detached: false,
            })
        }

        Launch::Server {
            command,
            cwd,
            url,
            ready,
        } => {
            let command = pick(command.get(platform), project, platform)?;
            let dir = working_dir(&root, cwd.as_deref());
            on_step(&format!("démarrage : {command}"));
            let mut child = process::spawn(command, &dir)?;

            wait_until_ready(&mut child, ready.as_deref());
            on_step(&format!("ouverture de {url}"));
            process::open_url(url)?;

            Ok(Session {
                project_id: project.id.clone(),
                url: Some(url.clone()),
                child: Some(child),
                stop_flag: None,
                detached: false,
            })
        }
    }
}

/// Attend que le serveur soit prêt : soit son motif apparaît dans sa sortie,
/// soit on laisse passer un délai raisonnable. Dans les deux cas la sortie du
/// serveur continue d'être affichée.
fn wait_until_ready(child: &mut Child, ready: Option<&str>) {
    let Some(stdout) = child.stdout.take() else {
        std::thread::sleep(Duration::from_millis(1500));
        return;
    };

    let (tx, rx) = mpsc::channel();
    let needle = ready.map(str::to_string);

    std::thread::spawn(move || {
        let mut signaled = false;
        for line in BufReader::new(stdout).lines().map_while(std::io::Result::ok) {
            println!("{line}");
            if !signaled {
                let hit = match &needle {
                    Some(n) => line.contains(n.as_str()),
                    None => false,
                };
                if hit {
                    signaled = true;
                    tx.send(()).ok();
                }
            }
        }
    });

    // Sans motif, on ne peut que patienter ; avec, on n'attend pas plus de 30 s.
    let timeout = if ready.is_some() {
        Duration::from_secs(30)
    } else {
        Duration::from_millis(1500)
    };
    rx.recv_timeout(timeout).ok();
}

fn pick<'a>(value: Option<&'a str>, project: &Project, platform: Platform) -> Result<&'a str> {
    value.ok_or_else(|| Error::UnsupportedPlatform {
        project: project.id.clone(),
        platform: platform.as_str(),
    })
}

fn working_dir(root: &Path, cwd: Option<&str>) -> PathBuf {
    match cwd {
        Some(sub) => root.join(sub),
        None => root.to_path_buf(),
    }
}

/// Un exécutable du dossier du projet, ou un nom à chercher dans le PATH.
fn resolve_exec(root: &Path, exec: &str) -> PathBuf {
    let candidate = root.join(exec);
    if candidate.exists() {
        candidate
    } else {
        PathBuf::from(exec)
    }
}
