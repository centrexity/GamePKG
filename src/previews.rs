//! Optional preview validation runs in a separate, network-isolated process.
use anyhow::{bail, Context, Result};
use std::{io::Write, process::Command};
pub const MAX_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_FRAMES: usize = 900;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    pub frames: usize,
    pub codec: String,
}
fn command(path: &std::path::Path, tool: &str, output: Option<&std::path::Path>) -> Command {
    let mut cmd = crate::compiler::command(48 * 1024 * 1024);
    cmd.args([
        "bwrap",
        "--unshare-all",
        "--die-with-parent",
        "--new-session",
        "--clearenv",
        "--ro-bind",
        "/usr",
        "/usr",
        "--ro-bind",
        "/lib",
        "/lib",
    ]);
    if std::path::Path::new("/lib64").exists() {
        cmd.args(["--ro-bind", "/lib64", "/lib64"]);
    }
    for dependency in ["/etc/alternatives", "/etc/ld.so.cache"] {
        if std::path::Path::new(dependency).exists() {
            cmd.args(["--ro-bind", dependency, dependency]);
        }
    }
    cmd.args([
        "--proc",
        "/proc",
        "--dev",
        "/dev",
        "--tmpfs",
        "/tmp",
        "--ro-bind",
    ])
    .arg(path)
    .arg("/preview");
    if let Some(output) = output {
        cmd.arg("--bind").arg(output).arg("/output");
    }
    cmd.args(["--", tool]);
    cmd
}
fn inspect(value: &serde_json::Value, expected: &str) -> Result<Info> {
    let streams = value["streams"]
        .as_array()
        .context("missing preview streams")?;
    if streams.len() != 1 {
        bail!("previews must contain exactly one video stream");
    }
    let stream = &streams[0];
    if stream["codec_type"] != "video" || stream["codec_name"] != expected {
        bail!("preview codec does not match its filename");
    }
    let width = stream["width"].as_u64().context("missing preview width")?;
    let height = stream["height"]
        .as_u64()
        .context("missing preview height")?;
    if width == 0 || height == 0 || width > 640 || height > 480 {
        bail!("preview dimensions must be at most 640×480");
    }
    let frames = value["frames"]
        .as_array()
        .context("missing decoded preview frames")?;
    if frames.is_empty() || frames.len() > MAX_FRAMES || frames[0]["key_frame"] != 1 {
        bail!("preview requires an initial keyframe and 1–900 frames");
    }
    let duration: f64 = value["format"]["duration"]
        .as_str()
        .context("missing preview duration")?
        .parse()?;
    if !duration.is_finite() || duration <= 0.0 || duration > 30.0 {
        bail!("preview duration must be at most 30 seconds");
    }
    Ok(Info {
        width: width as u32,
        height: height as u32,
        frames: frames.len(),
        codec: expected.into(),
    })
}
fn webm_header(bytes: &[u8]) -> Result<()> {
    fn vint(bytes: &[u8], size: bool) -> Result<(u64, usize)> {
        let first = *bytes.first().context("truncated EBML integer")?;
        let length = first.leading_zeros() as usize + 1;
        if length > if size { 8 } else { 4 } || bytes.len() < length {
            bail!("invalid EBML integer");
        }
        let mut value = u64::from(if size {
            first & ((0xffu16 >> length) as u8)
        } else {
            first
        });
        for byte in &bytes[1..length] {
            value = (value << 8) | u64::from(*byte);
        }
        Ok((value, length))
    }
    if !bytes.starts_with(b"\x1a\x45\xdf\xa3") {
        bail!("missing WebM EBML header");
    }
    let (size, prefix) = vint(&bytes[4..], true)?;
    if size > 4096 {
        bail!("EBML header too large");
    }
    let mut header = bytes
        .get(4 + prefix..4 + prefix + size as usize)
        .context("truncated EBML header")?;
    let mut doc_type = None;
    while !header.is_empty() {
        let (id, id_len) = vint(header, false)?;
        let (length, length_len) = vint(&header[id_len..], true)?;
        let start = id_len + length_len;
        let end = start
            .checked_add(usize::try_from(length)?)
            .context("EBML length overflow")?;
        let data = header.get(start..end).context("truncated EBML element")?;
        if id == 0x4282 && doc_type.replace(data).is_some() {
            bail!("duplicate WebM document type");
        }
        header = &header[end..];
    }
    if doc_type != Some(b"webm".as_slice()) {
        bail!("AV1 preview must use a WebM container");
    }
    Ok(())
}
pub fn validate(name: &str, bytes: &[u8]) -> Result<Info> {
    let expected = match name {
        "preview.av1.webm" => "av1",
        "preview.ogv" => "theora",
        _ => bail!("unknown preview filename"),
    };
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        bail!("preview must be 1 byte to 16 MiB");
    }
    if (expected == "av1" && !bytes.starts_with(b"\x1a\x45\xdf\xa3"))
        || (expected == "theora" && !bytes.starts_with(b"OggS"))
    {
        bail!("preview container does not match its filename");
    }
    if expected == "av1" {
        webm_header(bytes)?;
    }
    if !cfg!(target_os = "linux") {
        bail!("isolated preview validation currently requires Linux");
    }
    let mut input = tempfile::NamedTempFile::new()?;
    input.write_all(bytes)?;
    let mut probe = command(input.path(), "/usr/bin/ffprobe", None);
    probe.args([
        "-v",
        "error",
        "-threads",
        "1",
        "-protocol_whitelist",
        "file",
        "-show_streams",
        "-show_frames",
        "-show_format",
        "-show_entries",
        "stream=codec_type,codec_name,width,height:frame=key_frame:format=duration",
        "-of",
        "json",
        "/preview",
    ]);
    let output = crate::compiler::run(&mut probe)
        .context("preview validation requires ffprobe and bubblewrap")?;
    if !output.status.success() || !output.stderr.trim().is_empty() {
        bail!("preview decoding failed: {}", output.stderr);
    }
    inspect(
        &serde_json::from_str(&output.stdout).context("invalid preview metadata")?,
        expected,
    )
}

/// Decode a silent, bounded 320×240 preview at five frames per second.
pub fn decode(name: &str, bytes: &[u8]) -> Result<Vec<Vec<u8>>> {
    validate(name, bytes)?;
    let mut input = tempfile::NamedTempFile::new()?;
    input.write_all(bytes)?;
    let output = tempfile::tempdir()?;
    let mut mounted = command(input.path(), "/usr/bin/ffmpeg", Some(output.path()));
    mounted.args([
        "-nostdin",
        "-v",
        "error",
        "-threads",
        "1",
        "-filter_threads",
        "1",
        "-protocol_whitelist",
        "file",
        "-i",
        "/preview",
        "-an",
        "-vf",
        "fps=5,scale=320:240:force_original_aspect_ratio=decrease,pad=320:240:(ow-iw)/2:(oh-ih)/2",
        "-t",
        "30",
        "-frames:v",
        "150",
        "-pix_fmt",
        "rgb24",
        "-f",
        "rawvideo",
        "/output/frames.rgb",
    ]);
    let result = crate::compiler::run(&mut mounted)?;
    if !result.status.success() {
        bail!("preview playback decoding failed: {}", result.stderr);
    }
    let bytes = std::fs::read(output.path().join("frames.rgb"))?;
    const FRAME: usize = 320 * 240 * 3;
    if bytes.is_empty() || bytes.len() > FRAME * 150 || bytes.len() % FRAME != 0 {
        bail!("invalid decoded preview frame data");
    }
    Ok(bytes
        .chunks_exact(FRAME)
        .map(|frame| frame.to_vec())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn metadata() -> serde_json::Value {
        serde_json::json!({"streams":[{"codec_type":"video","codec_name":"av1","width":64,"height":64}],"frames":[{"key_frame":1}],"format":{"duration":"1.0"}})
    }
    #[test]
    fn webm_document_type_and_truncation_are_checked() {
        let good = b"\x1a\x45\xdf\xa3\x87\x42\x82\x84webm";
        webm_header(good).unwrap();
        for length in 0..good.len() {
            assert!(webm_header(&good[..length]).is_err());
        }
        assert!(webm_header(b"\x1a\x45\xdf\xa3\x8b\x42\x82\x88matroska").is_err());
        assert!(webm_header(b"\x1a\x45\xdf\xa3\x01\x00\x00\x00\x00\x00\x00\x00").is_err());
    }
    #[test]
    fn codec_dimensions_duration_and_initial_keyframe_are_required() {
        let good = metadata();
        assert_eq!(inspect(&good, "av1").unwrap().frames, 1);
        assert!(inspect(&good, "theora").is_err());
        for (field, value) in [("width", 0), ("width", 641), ("height", 481)] {
            let mut bad = good.clone();
            bad["streams"][0][field] = value.into();
            assert!(inspect(&bad, "av1").is_err());
        }
        let mut bad = good.clone();
        bad["frames"][0]["key_frame"] = 0.into();
        assert!(inspect(&bad, "av1").is_err());
        for duration in ["0", "31", "NaN", "inf"] {
            let mut bad = good.clone();
            bad["format"]["duration"] = duration.into();
            assert!(inspect(&bad, "av1").is_err());
        }
        let mut bad = good.clone();
        bad["streams"] = serde_json::json!([]);
        assert!(inspect(&bad, "av1").is_err());
        let mut bad = good;
        bad["frames"] = serde_json::json!([]);
        assert!(inspect(&bad, "av1").is_err());
        assert!(validate("preview.ogv", b"invalid").is_err());
        assert!(validate("preview.av1.webm", &vec![0; MAX_BYTES + 1]).is_err());
    }
}
