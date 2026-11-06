//! Loopback-only static file server. No application backend is required.
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    time::Duration,
};

fn file_for(root: &Path, url: &str) -> Option<PathBuf> {
    let raw = url.split('?').next()?;
    let mut decoded = Vec::new();
    let bytes = raw.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' {
            let hex = std::str::from_utf8(bytes.get(at + 1..at + 3)?).ok()?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            at += 3;
        } else {
            decoded.push(bytes[at]);
            at += 1;
        }
    }
    let path = String::from_utf8(decoded).ok()?;
    if !path.starts_with('/') || path.contains('\\') || path.contains('\0') {
        return None;
    }
    let relative = path.trim_start_matches('/');
    let file = root
        .join(if relative.is_empty() {
            "index.html"
        } else {
            relative
        })
        .canonicalize()
        .ok()?;
    (file.starts_with(root) && file.is_file()).then_some(file)
}
fn serve(mut stream: TcpStream, root: &Path) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(3)));
    let mut request = [0u8; 8192];
    let mut count = 0;
    loop {
        match stream.read(&mut request[count..]) {
            Ok(0) | Err(_) => return,
            Ok(n) => count += n,
        }
        if request[..count].windows(4).any(|s| s == b"\r\n\r\n") {
            break;
        }
        if count == request.len() {
            return;
        }
    }
    let text = String::from_utf8_lossy(&request[..count]);
    let mut parts = text.lines().next().unwrap_or("").split_whitespace();
    let method = parts.next().unwrap_or("");
    let url = parts.next().unwrap_or("");
    let (status, mime, body) = if method != "GET" && method != "HEAD" {
        (
            "405 Method Not Allowed",
            "text/plain",
            b"Read-only server".to_vec(),
        )
    } else if let Some(path) = file_for(root, url) {
        let mime = match path.extension().and_then(|s| s.to_str()).unwrap_or("") {
            "html" => "text/html; charset=utf-8",
            "css" => "text/css; charset=utf-8",
            "js" => "text/javascript; charset=utf-8",
            "wasm" => "application/wasm",
            "json" => "application/json",
            "jpg" | "jpeg" => "image/jpeg",
            "png" => "image/png",
            "svg" => "image/svg+xml",
            "rbs" => "application/octet-stream",
            _ => "application/octet-stream",
        };
        match fs::read(path) {
            Ok(body) => ("200 OK", mime, body),
            Err(_) => (
                "500 Internal Server Error",
                "text/plain",
                b"Read failed".to_vec(),
            ),
        }
    } else {
        ("404 Not Found", "text/plain", b"Not found".to_vec())
    };
    let header=format!("HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",body.len());
    if stream.write_all(header.as_bytes()).is_ok() && method != "HEAD" {
        let _ = stream.write_all(&body);
    }
}
fn main() -> std::io::Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../dist")
        .canonicalize()?;
    let port = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "3380".into())
        .parse::<u16>()
        .expect("port must be 1..65535");
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    println!("RB-338 Rust web app: http://127.0.0.1:{port} (Ctrl+C to stop)");
    for stream in listener.incoming() {
        let root = root.clone();
        if let Ok(stream) = stream {
            std::thread::spawn(move || serve(stream, &root));
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_serve_under_root() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .canonicalize()
            .unwrap();
        assert!(file_for(&root, "/Cargo.toml?q=1").is_some());
        for path in [
            "/../model/Cargo.toml",
            "/%2e%2e/model/Cargo.toml",
            "/..%5cmodel/Cargo.toml",
            "/missing",
            "/%00",
        ] {
            assert!(file_for(&root, path).is_none(), "{path}");
        }
    }
}
