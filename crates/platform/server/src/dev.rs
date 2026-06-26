//! Minimal Rust dev server for OpenIE.
//! Serves the `web/` directory with correct MIME types for WASM.
//! No dependencies beyond std — zero bloat.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};

const PORT: u16 = 3718;

fn main() {
    let web_dir = find_web_dir();
    println!("[openie-dev] Serving {} on http://localhost:{PORT}", web_dir.display());
    println!("[openie-dev] Open in a WebGPU-enabled browser (Chrome, Edge, Firefox Nightly)");

    let listener = TcpListener::bind(format!("0.0.0.0:{PORT}")).unwrap_or_else(|e| {
        eprintln!("[openie-dev] Failed to bind port {PORT}: {e}");
        std::process::exit(1);
    });

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => handle_connection(stream, &web_dir),
            Err(e) => eprintln!("[openie-dev] Connection error: {e}"),
        }
    }
}

fn handle_connection(mut stream: std::net::TcpStream, web_dir: &Path) {
    let buf_reader = BufReader::new(&stream);
    let request_line = match buf_reader.lines().next() {
        Some(Ok(line)) => line,
        _ => return,
    };

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }

    let method = parts[0];
    let raw_path = parts[1];

    if method != "GET" {
        let response = "HTTP/1.1 405 Method Not Allowed\r\n\r\n";
        let _ = stream.write_all(response.as_bytes());
        return;
    }

    // Resolve path
    let request_path = if raw_path == "/" { "/index.html" } else { raw_path };
    let file_path = web_dir.join(&request_path[1..]); // strip leading /

    // Security: prevent path traversal
    let canonical = match fs::canonicalize(&file_path) {
        Ok(p) => p,
        Err(_) => {
            send_404(&mut stream);
            return;
        }
    };
    let web_canonical = match fs::canonicalize(web_dir) {
        Ok(p) => p,
        Err(_) => {
            send_404(&mut stream);
            return;
        }
    };
    if !canonical.starts_with(&web_canonical) {
        send_404(&mut stream);
        return;
    }

    match fs::read(&canonical) {
        Ok(contents) => {
            let mime = mime_type(&canonical);
            let headers = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCross-Origin-Opener-Policy: same-origin\r\nCross-Origin-Embedder-Policy: require-corp\r\n\r\n",
                contents.len()
            );
            let _ = stream.write_all(headers.as_bytes());
            let _ = stream.write_all(&contents);
        }
        Err(_) => send_404(&mut stream),
    }
}

fn send_404(stream: &mut std::net::TcpStream) {
    let body = "404 Not Found";
    let response = format!(
        "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

fn mime_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "application/javascript",
        Some("wasm") => "application/wasm",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

fn find_web_dir() -> PathBuf {
    // Try relative to CWD first, then relative to executable
    for candidate in &["web", "../../../web", "../../web"] {
        let p = PathBuf::from(candidate);
        if p.join("index.html").exists() {
            return p;
        }
    }
    // Fallback: walk up from executable
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent().map(|p| p.to_path_buf());
        for _ in 0..5 {
            if let Some(ref d) = dir {
                let web = d.join("web");
                if web.join("index.html").exists() {
                    return web;
                }
                dir = d.parent().map(|p| p.to_path_buf());
            }
        }
    }
    eprintln!("[openie-dev] Could not find web/ directory. Run from the project root.");
    std::process::exit(1);
}
