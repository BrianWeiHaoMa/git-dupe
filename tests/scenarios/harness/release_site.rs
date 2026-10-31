//! A stand-in for GitHub's releases pages, so that a check of an install script asks
//! nothing of another machine (`Composition/The checks`): an HTTP/1.1 server on
//! `127.0.0.1`, on a port the system chose, answering what the script asks of the base
//! `base()` names, its `GIT_DUPE_RELEASES_URL`, as GitHub answers it for a repository's
//! releases, and serving the script itself where a run fetches it first:
//!
//! - `<base>/latest`: a 302 to `<base>/tag/<tag>`, the latest release's page;
//! - `<base>/tag/<tag>`: 200;
//! - `<base>/download/<tag>/<name>`: 200 with the bytes of the file `name` it serves;
//! - `/install.ps1`, once `with_script` gave it, with or without a release: 200 with its
//!   bytes;
//! - every other path, and every path where it serves no release: 404.
//!
//! It answers `GET` and `HEAD` alike, without a body for `HEAD`, every answer with its
//! `Content-Length` and `Connection: close`; any other method is a 404. It records each
//! request line before it answers it, so that once a client has its answer, `asked`
//! holds the request. One thread answers one connection after another, which is all a
//! script asking one request at a time needs, until the site is dropped.

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// The path of the releases pages on the site.
const BASE: &str = "/releases";

/// The path of the script on the site.
const SCRIPT: &str = "/install.ps1";

/// How long the site waits for a request's head before it drops the connection.
const PATIENCE: Duration = Duration::from_secs(30);

/// The one release the site serves, the latest.
struct Release {
    tag: String,
    files: Vec<(String, Vec<u8>)>,
}

pub struct ReleaseSite {
    address: SocketAddr,
    asked: Arc<Mutex<Vec<String>>>,
    script: Arc<Mutex<Option<Vec<u8>>>>,
    stopping: Arc<AtomicBool>,
    answering: Option<JoinHandle<()>>,
}

impl ReleaseSite {
    /// A site whose latest release is `tag`, its files each a name and its bytes.
    pub fn serving(tag: &str, files: Vec<(String, Vec<u8>)>) -> ReleaseSite {
        ReleaseSite::start(Some(Release {
            tag: tag.to_owned(),
            files,
        }))
    }

    /// A site with no release, as a repository has before its first.
    pub fn without_a_release() -> ReleaseSite {
        ReleaseSite::start(None)
    }

    fn start(release: Option<Release>) -> ReleaseSite {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .unwrap_or_else(|cause| panic!("cannot listen on 127.0.0.1: {cause}"));
        let address = listener.local_addr().unwrap();
        let asked = Arc::new(Mutex::new(Vec::new()));
        let script = Arc::new(Mutex::new(None));
        let stopping = Arc::new(AtomicBool::new(false));
        let answering = {
            let (asked, script) = (Arc::clone(&asked), Arc::clone(&script));
            let stopping = Arc::clone(&stopping);
            std::thread::spawn(move || {
                for connection in listener.incoming() {
                    if stopping.load(Ordering::SeqCst) {
                        break;
                    }
                    if let Ok(connection) = connection {
                        answer(connection, address, release.as_ref(), &script, &asked);
                    }
                }
            })
        };
        ReleaseSite {
            address,
            asked,
            script,
            stopping,
            answering: Some(answering),
        }
    }

    /// The site serving `bytes` at `script_url`, whether or not it has a release.
    pub fn with_script(self, bytes: Vec<u8>) -> ReleaseSite {
        *self.script.lock().unwrap() = Some(bytes);
        self
    }

    /// The base of the releases pages, as the install scripts take it.
    pub fn base(&self) -> String {
        format!("http://{}{BASE}", self.address)
    }

    /// The address of the script, for the line pasted in the place of GitHub's.
    pub fn script_url(&self) -> String {
        format!("http://{}{SCRIPT}", self.address)
    }

    /// Every request line the site was asked, in order, as `GET /releases/latest
    /// HTTP/1.1`.
    pub fn asked(&self) -> Vec<String> {
        self.asked.lock().unwrap().clone()
    }
}

impl Drop for ReleaseSite {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::SeqCst);
        // A connection of its own ends the wait for the next one; without it the thread
        // would wait for ever, so it is joined only once the connection was made.
        if TcpStream::connect(self.address).is_ok()
            && let Some(answering) = self.answering.take()
        {
            let _ = answering.join();
        }
    }
}

/// Reads one request's head from `connection` and answers it. A client that sends no
/// whole request line in time, or goes away, gets no answer.
fn answer(
    mut connection: TcpStream,
    address: SocketAddr,
    release: Option<&Release>,
    script: &Mutex<Option<Vec<u8>>>,
    asked: &Mutex<Vec<String>>,
) {
    let _ = connection.set_read_timeout(Some(PATIENCE));
    let Some(request) = request_line(&connection) else {
        return;
    };
    asked.lock().unwrap().push(request.clone());
    let mut words = request.split(' ');
    let (method, target) = (words.next().unwrap_or(""), words.next().unwrap_or(""));
    let served_script = script.lock().unwrap().clone().filter(|_| target == SCRIPT);
    let (status, location, body) = match (method, served_script, release) {
        ("GET" | "HEAD", Some(script), _) => ("200 OK", None, script),
        ("GET" | "HEAD", None, Some(release)) => route(target, address, release),
        _ => not_found(),
    };
    let mut head = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    if let Some(location) = location {
        head.push_str(&format!("Location: {location}\r\n"));
    }
    head.push_str("\r\n");
    let _ = connection.write_all(head.as_bytes());
    if method != "HEAD" {
        let _ = connection.write_all(&body);
    }
    let _ = connection.shutdown(Shutdown::Write);
}

/// The request line, without its line end, once the whole head has been read; the
/// headers are read and not kept.
fn request_line(connection: &TcpStream) -> Option<String> {
    let mut reader = BufReader::new(connection);
    let mut line = String::new();
    if reader.read_line(&mut line).ok()? == 0 || !line.ends_with('\n') {
        return None;
    }
    loop {
        let mut header = String::new();
        match reader.read_line(&mut header) {
            Ok(0) | Err(_) => return None,
            Ok(_) if header.trim_end().is_empty() => break,
            Ok(_) => {}
        }
    }
    Some(line.trim_end().to_owned())
}

/// The status, the redirect's location, and the body for `target` on a site serving
/// `release`.
fn route(
    target: &str,
    address: SocketAddr,
    release: &Release,
) -> (&'static str, Option<String>, Vec<u8>) {
    let tag = &release.tag;
    let Some(page) = target.strip_prefix(BASE) else {
        return not_found();
    };
    if page == "/latest" {
        let location = format!("http://{address}{BASE}/tag/{tag}");
        return ("302 Found", Some(location), Vec::new());
    }
    if page == format!("/tag/{tag}") {
        return ("200 OK", None, format!("the release {tag}\n").into_bytes());
    }
    let served = page
        .strip_prefix(&format!("/download/{tag}/"))
        .and_then(|name| release.files.iter().find(|(served, _)| served == name));
    match served {
        Some((_, bytes)) => ("200 OK", None, bytes.clone()),
        None => not_found(),
    }
}

fn not_found() -> (&'static str, Option<String>, Vec<u8>) {
    ("404 Not Found", None, b"not found\n".to_vec())
}
