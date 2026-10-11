//! Local browser metadata library backed by the user's Hindsight IndexedDB package.
use anyhow::{bail, Context, Result};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    time::Duration,
};
const PACKAGE_SHA: &str = "602b55aa4610aa47b3216f2c2299191730ce6bc847db1b2a81ef183a3b25129b";
fn icon_identity(m: &crate::manifest::Manifest) -> String {
    crate::verify::digest(format!("{}\0{}\0{}", m.id, m.version, m.creator_key).as_bytes())
}
fn icon(root: &Path, id: &str) -> Result<Vec<u8>> {
    crate::registry::installed_manifest(root, id)?;
    let mut package = crate::archive::Package::open(&root.join(format!("{id}.gamepkg")))?;
    let manifest = crate::verify::verify_embedded(&mut package)?;
    if manifest.id != id {
        bail!("icon package ID differs");
    }
    if package
        .entries
        .get("thumbnail.png")
        .copied()
        .unwrap_or(u64::MAX)
        > crate::drawing::image::MAX_PNG_BYTES as u64
    {
        bail!("thumbnail exceeds icon size limit");
    }
    let decoded = crate::drawing::image::Image::from_png(&package.read("thumbnail.png")?)
        .context("invalid thumbnail")?;
    let size = decoded.size();
    let pixels =
        image::RgbaImage::from_raw(size[0] as u32, size[1] as u32, decoded.pixels().to_vec())
            .context("invalid thumbnail pixels")?;
    let small = image::imageops::resize(&pixels, 40, 40, image::imageops::FilterType::Triangle);
    Ok(serde_json::to_vec(
        &serde_json::json!({"identity":icon_identity(&manifest),"width":40,"height":40,"rgba_hex":hex::encode(small.as_raw())}),
    )?)
}
fn snapshot(root: &Path) -> Result<Vec<u8>> {
    let report = crate::registry::inspect_library(root)?;
    let games=report.games.into_iter().map(|m| {
        let history=crate::progress::load(root,&m.id,&m.creator_key).ok();
        let usage=crate::storage::usage(root,&m).ok();
        serde_json::json!({"icon_identity":icon_identity(&m),"built":crate::build::is_built(root,&m.id).ok(),"manifest":m,"history":history,"used_bytes":usage.as_ref().map(|u|u.used_bytes()),"expected_bytes":usage.as_ref().and_then(|u|u.estimated_bytes),"storage_breakdown":usage.map(|u|u.breakdown())})
    }).collect::<Vec<_>>();
    let data = serde_json::to_vec(
        &serde_json::json!({"format_version":1,"scope":crate::verify::digest(fs::canonicalize(root)?.to_string_lossy().as_bytes()),"games":games,"catalog":crate::catalog::entries(root)?,"settings":crate::settings::load()?,"rejected":report.rejected,"library_used_bytes":crate::storage::total(root).ok()}),
    )?;
    if data.len() > 8 * 1024 * 1024 {
        bail!("browser library metadata exceeds 8 MiB");
    }
    Ok(data)
}
fn respond(stream: &mut TcpStream, status: &str, kind: &str, body: &[u8]) -> Result<()> {
    write!(stream,"HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nCross-Origin-Opener-Policy: same-origin\r\nCross-Origin-Embedder-Policy: require-corp\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'self'; script-src 'self'; worker-src 'self' blob:; connect-src 'self'; style-src 'self'; frame-ancestors 'none'; base-uri 'none'\r\n\r\n",body.len())?;
    stream.write_all(body)?;
    Ok(())
}
fn request(
    stream: &mut TcpStream,
    root: &Path,
    host: &str,
    token: &str,
    package: &[u8],
) -> Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 1024];
    while !bytes.windows(4).any(|w| w == b"\r\n\r\n") {
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            return Ok(());
        }
        bytes.extend_from_slice(&chunk[..count]);
        if bytes.len() > 16384 {
            return respond(
                stream,
                "431 Request Header Fields Too Large",
                "text/plain",
                b"Request too large",
            );
        }
    }
    let text = std::str::from_utf8(&bytes)?;
    let mut lines = text.split("\r\n");
    let first = lines.next().context("missing request")?;
    let parts = first.split_whitespace().collect::<Vec<_>>();
    if parts.len() != 3 || parts[0] != "GET" || parts[2] != "HTTP/1.1" {
        return respond(stream, "405 Method Not Allowed", "text/plain", b"GET only");
    }
    let headers = lines
        .filter(|l| !l.is_empty())
        .map(|line| line.split_once(':').context("invalid header"))
        .collect::<Result<Vec<_>>>()?;
    let values = |key: &str| {
        headers
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.trim())
            .collect::<Vec<_>>()
    };
    if values("host") != vec![host]
        || values("origin")
            .iter()
            .any(|v| *v != format!("http://{host}"))
    {
        return respond(stream, "403 Forbidden", "text/plain", b"Wrong origin");
    }
    match parts[1] {
        "/" => respond(
            stream,
            "200 OK",
            "text/html; charset=utf-8",
            include_bytes!("../html/library/index.html"),
        ),
        "/storage.js" => respond(
            stream,
            "200 OK",
            "text/javascript",
            include_bytes!("../html/library/storage.js"),
        ),
        "/library.js" => respond(
            stream,
            "200 OK",
            "text/javascript",
            include_bytes!("../html/library/library.js"),
        ),
        "/style.css" => respond(
            stream,
            "200 OK",
            "text/css",
            include_bytes!("../html/library/style.css"),
        ),
        "/hindsight.js" => respond(stream, "200 OK", "text/javascript", package),
        "/api/library" => {
            if values("authorization") != vec![format!("Bearer {token}").as_str()] {
                return respond(
                    stream,
                    "403 Forbidden",
                    "text/plain",
                    b"Session authorization required",
                );
            }
            match snapshot(root) {
                Ok(data) => respond(stream, "200 OK", "application/json", &data),
                Err(error) => respond(
                    stream,
                    "500 Internal Server Error",
                    "text/plain",
                    error.to_string().as_bytes(),
                ),
            }
        }
        path if path.starts_with("/api/icon/") => {
            if values("authorization") != vec![format!("Bearer {token}").as_str()] {
                return respond(
                    stream,
                    "403 Forbidden",
                    "text/plain",
                    b"Session authorization required",
                );
            }
            match icon(root, path.trim_start_matches("/api/icon/")) {
                Ok(data) => respond(stream, "200 OK", "application/json", &data),
                Err(error) => respond(
                    stream,
                    "404 Not Found",
                    "text/plain",
                    error.to_string().as_bytes(),
                ),
            }
        }
        _ => respond(stream, "404 Not Found", "text/plain", b"Not found"),
    }
}
pub fn serve(root: &Path, package: &Path, port: u16) -> Result<()> {
    let path = package.join("code_langs/js.js");
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.is_file() || metadata.len() > 512 * 1024 {
        bail!("invalid Hindsight IndexedDB package source");
    }
    let source = fs::read(path)?;
    if crate::verify::digest(&source) != PACKAGE_SHA {
        bail!("Hindsight IndexedDB source differs from the verified package revision");
    }
    fs::create_dir_all(root)?;
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    let host = listener.local_addr()?.to_string();
    let mut secret = [0u8; 32];
    getrandom::fill(&mut secret)
        .map_err(|e| anyhow::anyhow!("session randomness unavailable: {e}"))?;
    let token = hex::encode(secret);
    let scope = crate::verify::digest(fs::canonicalize(root)?.to_string_lossy().as_bytes());
    println!("Open http://{host}/#{token}:{scope}");
    std::io::stdout().flush()?;
    for stream in listener.incoming() {
        let mut stream = stream?;
        if let Err(error) = request(&mut stream, root, &host, &token, &source) {
            eprintln!("Browser request failed: {error}");
        }
    }
    Ok(())
}
