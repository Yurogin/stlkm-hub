//! Un serveur de fichiers minimal, pour les projets qui ne sont qu'un dossier
//! de HTML. Le hub les sert lui-même : pas de `npm`, pas de dépendance, et le
//! projet s'ouvre dans le navigateur comme les autres.
//!
//! Il n'écoute que sur `127.0.0.1` : rien n'est exposé au réseau.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::error::Result;

/// Démarre le serveur en tâche de fond et retourne son adresse, plus le
/// drapeau qui l'arrête.
pub fn serve(dir: &Path, port: Option<u16>) -> Result<(String, Arc<AtomicBool>)> {
    let listener = TcpListener::bind(("127.0.0.1", port.unwrap_or(0)))?;
    let url = format!("http://{}", listener.local_addr()?);
    listener.set_nonblocking(true)?;

    let stop = Arc::new(AtomicBool::new(false));
    let root = dir.to_path_buf();
    let flag = Arc::clone(&stop);

    std::thread::spawn(move || {
        while !flag.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((stream, _)) => {
                    let root = root.clone();
                    std::thread::spawn(move || {
                        stream.set_nonblocking(false).ok();
                        handle(stream, &root).ok();
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(_) => break,
            }
        }
    });

    Ok((url, stop))
}

fn handle(mut stream: TcpStream, root: &Path) -> std::io::Result<()> {
    let mut request = String::new();
    BufReader::new(&stream).read_line(&mut request)?;

    let Some(target) = request.split_whitespace().nth(1) else {
        return respond(&mut stream, 400, "text/plain", b"Requete invalide");
    };

    match resolve(root, target) {
        Some(file) => {
            let mut content = Vec::new();
            std::fs::File::open(&file)?.read_to_end(&mut content)?;
            respond(&mut stream, 200, mime_of(&file), &content)
        }
        None => respond(&mut stream, 404, "text/plain", b"Introuvable"),
    }
}

/// Traduit une cible HTTP en chemin de fichier, en refusant toute sortie du
/// dossier servi.
fn resolve(root: &Path, target: &str) -> Option<PathBuf> {
    let path = target.split(['?', '#']).next().unwrap_or("/");
    let decoded = percent_decode(path);
    let relative = decoded.trim_start_matches('/');

    let mut file = root.to_path_buf();
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(part) => file.push(part),
            // `..`, `/`, `C:` : une tentative de sortir du dossier.
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
            Component::CurDir => {}
        }
    }

    if file.is_dir() {
        file = file.join("index.html");
    }
    file.is_file().then_some(file)
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&input[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn mime_of(file: &Path) -> &'static str {
    match file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

fn respond(
    stream: &mut TcpStream,
    code: u16,
    mime: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let reason = if code == 200 { "OK" } else { "Error" };
    write!(
        stream,
        "HTTP/1.1 {code} {reason}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_les_pourcents() {
        assert_eq!(percent_decode("/mon%20fichier.html"), "/mon fichier.html");
        assert_eq!(percent_decode("/simple.css"), "/simple.css");
    }

    #[test]
    fn refuse_de_sortir_du_dossier() {
        let root = Path::new(".");
        assert!(resolve(root, "/../../etc/passwd").is_none());
        assert!(resolve(root, "/..%2fsecret").is_none());
    }

    #[test]
    fn devine_le_type_de_contenu() {
        assert_eq!(mime_of(Path::new("a/b.html")), "text/html; charset=utf-8");
        assert_eq!(mime_of(Path::new("style.CSS")), "text/css; charset=utf-8");
        assert_eq!(mime_of(Path::new("truc.inconnu")), "application/octet-stream");
    }
}
