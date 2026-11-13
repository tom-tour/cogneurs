//! Home hosting: opens a free Cloudflare quick tunnel to this server and
//! publishes its current https address in a GitHub gist, where the page on
//! GitHub Pages looks for it. When the server stops, the gist is set back to
//! offline.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

pub const GIST_FILE: &str = "cogneurs-server.json";

pub struct Tunnel {
    child: Child,
    gist: Option<String>,
    pub url: String,
}

fn find_program(name: &str, extra: &[&str]) -> Option<PathBuf> {
    let exe = if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let p = dir.join(&exe);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    extra.iter().map(PathBuf::from).find(|p| p.is_file())
}

fn cloudflared() -> Option<PathBuf> {
    find_program(
        "cloudflared",
        &[
            r"C:\Program Files (x86)\cloudflared\cloudflared.exe",
            r"C:\Program Files\cloudflared\cloudflared.exe",
        ],
    )
}

fn gh() -> Option<PathBuf> {
    find_program("gh", &[r"C:\Program Files\GitHub CLI\gh.exe"])
}

/// Writes `{"url": ..., "since": ...}` into the gist.
pub async fn publish(gist: &str, url: Option<&str>) -> Result<(), String> {
    let gh = gh().ok_or("GitHub CLI (gh) introuvable")?;
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let content = serde_json::json!({ "url": url, "since": since }).to_string();
    let body = serde_json::json!({ "files": { GIST_FILE: { "content": content } } }).to_string();
    let mut child = Command::new(gh)
        .args(["api", "--method", "PATCH", &format!("gists/{gist}"), "--input", "-", "--silent"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    child.stdin.take().unwrap().write_all(body.as_bytes()).await.map_err(|e| e.to_string())?;
    let out = child.wait_with_output().await.map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

pub async fn start(port: u16, gist: Option<String>) -> Result<Tunnel, String> {
    let exe = cloudflared().ok_or(
        "cloudflared introuvable. Installe-le avec : winget install --id Cloudflare.cloudflared",
    )?;
    let mut child = Command::new(exe)
        .args(["tunnel", "--no-autoupdate", "--url", &format!("http://localhost:{port}")])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("cloudflared : {e}"))?;

    // cloudflared prints the address of the quick tunnel in its log.
    let mut lines = BufReader::new(child.stderr.take().unwrap()).lines();
    let find = async {
        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(start) = line.find("https://") {
                let url: String = line[start..].chars().take_while(|c| !c.is_whitespace() && *c != '|').collect();
                if url.contains(".trycloudflare.com") {
                    return Some(url);
                }
            }
        }
        None
    };
    let url = tokio::time::timeout(Duration::from_secs(60), find)
        .await
        .ok()
        .flatten()
        .ok_or("cloudflared n'a pas donné d'adresse (connexion internet ?)")?;
    // Keep draining the log so cloudflared never blocks on a full pipe.
    tokio::spawn(async move { while let Ok(Some(_)) = lines.next_line().await {} });

    if let Some(g) = &gist {
        // The tunnel needs a few seconds before it answers.
        tokio::time::sleep(Duration::from_secs(3)).await;
        publish(g, Some(&url)).await.map_err(|e| format!("gist : {e}"))?;
    }
    Ok(Tunnel { child, gist, url })
}

impl Tunnel {
    pub async fn stop(mut self) {
        if let Some(g) = &self.gist {
            match publish(g, None).await {
                Ok(()) => println!("Serveur marqué hors ligne."),
                Err(e) => eprintln!("gist : {e}"),
            }
        }
        let _ = self.child.kill().await;
    }
}
