//! Native preparation from compiled-in, reviewed data; never package-supplied commands.
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
};

type Inputs = &'static [(&'static str, &'static [u8])];
const LIMIT: u64 = 512 * 1024 * 1024;

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    version: u32,
    ninja: String,
    ninja_output: String,
    compiler_stamp: String,
    command_inputs: Vec<String>,
    command_outputs: Vec<String>,
    reset_cmake_dirs: Vec<String>,
    headers: Vec<Header>,
    resource_template: String,
    resource_output: String,
    resource_sha256: String,
    blobs: Vec<Blob>,
    downloads: Vec<Download>,
    skip_archive_paths: Vec<String>,
    commands: Vec<Step>,
    dependency_trees: Vec<Tree>,
    required_defines: Defines,
    link_objects: String,
    binary: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Header {
    input: String,
    output: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Blob {
    symbol: String,
    source: String,
    sha256: String,
    mode: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Download {
    urls: Vec<String>,
    sha256: String,
    format: String,
    output: String,
    strip: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Step {
    program: String,
    args: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Tree {
    root: String,
    files: Vec<TreeFile>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TreeFile {
    path: String,
    sha256: Option<String>,
    link: Option<String>,
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Defines {
    path: String,
    names: Vec<String>,
}

pub(super) struct LinkCheck {
    objects: PathBuf,
    binary: PathBuf,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PreparationReceipt {
    identity: String,
    outputs: BTreeMap<String, String>,
}

fn output_identities(root: &Path, paths: &[String]) -> Result<BTreeMap<String, String>> {
    let mut hashes = BTreeMap::new();
    for path in paths {
        let canonical = root.join(relative(path)?).canonicalize()?;
        ensure!(
            canonical.starts_with(root),
            "prepared output leaves source root"
        );
        let bytes = crate::files::read_regular(&canonical, 64 * 1024 * 1024, false)?;
        hashes.insert(path.clone(), crate::verify::digest(&bytes));
    }
    Ok(hashes)
}

fn relative(value: &str) -> Result<&Path> {
    let path = Path::new(value);
    ensure!(!value.contains(':'), "drive-qualified prepared path");
    ensure!(
        !value.is_empty()
            && value.len() <= 4096
            && !value.contains(['\\', '\0'])
            && path.components().all(|c| matches!(c, Component::Normal(_))),
        "invalid prepared path"
    );
    Ok(path)
}

fn input(files: Inputs, base: &Path, name: &str) -> Result<&'static [u8]> {
    let key = base
        .join(relative(name)?)
        .to_string_lossy()
        .replace('\\', "/");
    files
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, bytes)| *bytes)
        .with_context(|| format!("prepared input {key} is not in this app"))
}

fn destination(root: &Path, value: &str) -> Result<PathBuf> {
    let path = relative(value)?;
    let mut directory = root.to_path_buf();
    let components: Vec<_> = path.components().collect();
    for component in &components[..components.len() - 1] {
        directory.push(component);
        match fs::symlink_metadata(&directory) {
            Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => {}
            Ok(meta) if meta.file_type().is_symlink() => {
                directory = directory.canonicalize()?;
                ensure!(
                    directory.starts_with(root) && directory.is_dir(),
                    "prepared directory leaves its root"
                );
            }
            Ok(_) => bail!("prepared parent is not a directory"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&directory)?
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(directory.join(components.last().unwrap()))
}

fn generated(root: &Path, path: &str, bytes: &[u8]) -> Result<()> {
    let output = destination(root, path)?;
    if crate::files::read_regular(&output, LIMIT, false)
        .ok()
        .as_deref()
        == Some(bytes)
    {
        return Ok(());
    }
    let mut file = tempfile::NamedTempFile::new_in(output.parent().unwrap())?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    if output.symlink_metadata().is_ok() {
        fs::remove_file(&output)?;
    }
    file.persist(output).map_err(|error| error.error)?;
    Ok(())
}

fn source_bytes(root: &Path, path: &str, limit: u64) -> Result<Vec<u8>> {
    let path = root.join(relative(path)?);
    ensure!(
        path.canonicalize()?.starts_with(root),
        "prepared input leaves source root"
    );
    crate::files::read_regular(&path, limit, false)
}

fn resources(root: &Path, plan: &Plan, files: Inputs, base: &Path) -> Result<()> {
    let mut values = BTreeMap::new();
    for blob in &plan.blobs {
        ensure!(
            !blob.symbol.is_empty()
                && blob
                    .symbol
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_'),
            "invalid prepared symbol"
        );
        let mut bytes = source_bytes(root, &blob.source, 32 * 1024 * 1024)?;
        ensure!(
            crate::verify::digest(&bytes) == blob.sha256,
            "prepared resource changed: {}",
            blob.source
        );
        match blob.mode.as_str() {
            "bytes" => {}
            "ascii-nul" => {
                for byte in &mut bytes {
                    if *byte >= 128 {
                        *byte = b' ';
                    }
                }
                bytes.push(0);
            }
            _ => bail!("unknown prepared resource transform"),
        }
        ensure!(
            values.insert(blob.symbol.as_str(), bytes).is_none(),
            "duplicate prepared symbol"
        );
    }
    let template = std::str::from_utf8(input(files, base, &plan.resource_template)?)?;
    let mut output = String::new();
    for (index, part) in template.split("@@").enumerate() {
        if index % 2 == 0 {
            output.push_str(part);
            continue;
        }
        let bytes = values
            .remove(part)
            .context("unknown or repeated prepared resource placeholder")?;
        for (row, chunk) in bytes.chunks(32).enumerate() {
            if row != 0 {
                output.push('\n');
            }
            output.push('\t');
            for (index, word) in chunk.chunks(4).enumerate() {
                if index != 0 {
                    output.push(' ');
                }
                let mut padded = [0; 4];
                padded[..word.len()].copy_from_slice(word);
                use std::fmt::Write;
                write!(output, "0x{:08x},", u32::from_le_bytes(padded))?;
            }
        }
        ensure!(
            output.len() <= 128 * 1024 * 1024,
            "prepared resources exceed budget"
        );
    }
    ensure!(
        values.is_empty() && crate::verify::digest(output.as_bytes()) == plan.resource_sha256,
        "prepared resource output differs from the reviewed source"
    );
    generated(root, &plan.resource_output, output.as_bytes())
}

fn tree_matches(root: &Path, tree: &Tree) -> Result<bool> {
    let path = root.join(relative(&tree.root)?);
    if !path.is_dir() {
        return Ok(false);
    }
    ensure!(
        path.canonicalize()?.starts_with(root),
        "dependency root leaves source"
    );
    for entry in &tree.files {
        let file = path.join(relative(&entry.path)?);
        if let Some(link) = &entry.link {
            if fs::read_link(&file).ok().as_deref() != Some(Path::new(link)) {
                return Ok(false);
            }
        } else if let Some(hash) = &entry.sha256 {
            let bytes = match source_bytes(&path, &entry.path, 64 * 1024 * 1024) {
                Ok(bytes) => bytes,
                Err(_) => return Ok(false),
            };
            if crate::verify::digest(&bytes) != *hash {
                return Ok(false);
            }
        } else {
            bail!("dependency entry has no identity");
        }
    }
    Ok(true)
}

fn normalized(path: &Path) -> Result<PathBuf> {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => out.push(value),
            Component::CurDir => {}
            Component::ParentDir => ensure!(out.pop(), "archive link leaves destination"),
            _ => bail!("absolute archive path"),
        }
    }
    Ok(out)
}

fn archive_link(parent: &Path, link: &Path) -> Result<PathBuf> {
    // Dependency archives use POSIX paths even on a Windows host. In
    // particular, Windows considers /lib rooted but not fully absolute.
    let spelling = link.to_str().context("archive link is not UTF-8")?;
    ensure!(
        !spelling.is_empty() && spelling.len() <= 4096 && !spelling.contains(['\\', ':', '\0']),
        "invalid archive link"
    );
    let Some(target) = spelling.strip_prefix('/') else {
        normalized(&parent.join(link))?;
        return Ok(link.to_owned());
    };
    let target = normalized(Path::new(target))?;
    let a: Vec<_> = parent.components().collect();
    let b: Vec<_> = target.components().collect();
    let common = a.iter().zip(&b).take_while(|(a, b)| a == b).count();
    let mut relative = PathBuf::new();
    for _ in common..a.len() {
        relative.push("..");
    }
    for part in &b[common..] {
        relative.push(part);
    }
    Ok(relative)
}

fn unpack<R: Read>(reader: R, root: &Path, strip: usize, skip: &[String]) -> Result<()> {
    fs::create_dir_all(root)?;
    let root = root.canonicalize()?;
    let mut archive = tar::Archive::new(reader.take(LIMIT + 16 * 1024 * 1024));
    let mut total = 0u64;
    let mut links = Vec::new();
    for entry in archive.entries()? {
        let mut entry = entry?;
        let raw = entry.path()?.into_owned();
        ensure!(!raw.is_absolute(), "absolute archive member");
        let parts: Vec<_> = raw
            .components()
            .filter(|part| !matches!(part, Component::CurDir))
            .collect();
        ensure!(
            parts.iter().all(|p| matches!(p, Component::Normal(_))),
            "archive member traversal"
        );
        if parts.len() <= strip {
            continue;
        }
        let path: PathBuf = parts[strip..].iter().collect();
        if skip.iter().any(|prefix| path.starts_with(prefix)) {
            continue;
        }
        let text = path.to_str().context("archive path is not UTF-8")?;
        let output = destination(&root, text)?;
        let kind = entry.header().entry_type();
        if kind.is_dir() {
            if !output.exists() {
                fs::create_dir(&output)?;
            }
            ensure!(
                output.canonicalize()?.starts_with(&root) && output.is_dir(),
                "invalid archive directory"
            );
        } else if kind.is_file() {
            total = total
                .checked_add(entry.size())
                .context("archive size overflow")?;
            ensure!(total <= LIMIT, "dependency archive exceeds budget");
            if output.symlink_metadata().is_ok() {
                fs::remove_file(&output)?;
            }
            let mut file = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&output)?;
            std::io::copy(&mut entry, &mut file)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(
                    &output,
                    fs::Permissions::from_mode((entry.header().mode()? & 0o755) | 0o600),
                )?;
            }
        } else if kind.is_symlink() {
            let link = entry
                .link_name()?
                .context("missing archive link")?
                .into_owned();
            let link = archive_link(path.parent().unwrap_or(Path::new("")), &link)?;
            if output.symlink_metadata().is_ok() {
                fs::remove_file(&output)?;
            }
            #[cfg(unix)]
            std::os::unix::fs::symlink(link, &output)?;
            #[cfg(not(unix))]
            {
                let _ = link;
                bail!("this dependency needs native symbolic links");
            }
        } else if kind.is_hard_link() {
            let link = normalized(&entry.link_name()?.context("missing hard link")?)?;
            links.push((output, link));
        } else {
            bail!("unsupported dependency archive entry");
        }
    }
    // Tar ends at its zero blocks before the compressed stream necessarily
    // ends. Drain the bounded remainder to validate compression trailers and
    // avoid closing an xz pipe early with SIGPIPE.
    let mut remainder = archive.into_inner();
    std::io::copy(&mut remainder, &mut std::io::sink())?;
    ensure!(remainder.limit() > 0, "dependency stream exceeds budget");
    for (output, link) in links {
        let original = root.join(link);
        ensure!(
            original.canonicalize()?.starts_with(&root),
            "archive hard link leaves root"
        );
        if output.symlink_metadata().is_ok() {
            fs::remove_file(&output)?;
        }
        fs::copy(original, output)?;
    }
    Ok(())
}

fn deb_member(bytes: &[u8]) -> Result<(&str, &[u8])> {
    ensure!(
        bytes.starts_with(b"!<arch>\n"),
        "dependency is not an ar archive"
    );
    let mut offset = 8usize;
    while let Some(header) = bytes.get(offset..offset.saturating_add(60)) {
        ensure!(&header[58..] == b"`\n", "invalid ar member header");
        let name = std::str::from_utf8(&header[..16])?
            .trim()
            .trim_end_matches('/');
        let size: usize = std::str::from_utf8(&header[48..58])?.trim().parse()?;
        let start = offset.checked_add(60).context("ar offset overflow")?;
        let end = start.checked_add(size).context("ar extent overflow")?;
        let payload = bytes.get(start..end).context("truncated ar member")?;
        if name.starts_with("data.tar.") {
            return Ok((name, payload));
        }
        offset = end.checked_add(size & 1).context("ar padding overflow")?;
    }
    bail!("dependency has no data archive")
}

fn unpack_xz(
    bytes: &[u8],
    output: &Path,
    strip: usize,
    skip: &[String],
    stage: &Path,
) -> Result<()> {
    let mut packed = tempfile::NamedTempFile::new_in(stage)?;
    packed.write_all(bytes)?;
    let mut child = Command::new("xz")
        .args([
            "--decompress",
            "--stdout",
            "--memlimit-decompress=256MiB",
            "--",
        ])
        .arg(packed.path())
        .stdout(Stdio::piped())
        .spawn()
        .context("start native xz decompressor")?;
    let result = unpack(child.stdout.take().unwrap(), output, strip, skip);
    if result.is_err() {
        let _ = child.kill();
    }
    let status = child.wait()?;
    result?;
    ensure!(status.success(), "dependency decompression failed");
    Ok(())
}

fn dependencies(root: &Path, plan: &Plan) -> Result<()> {
    if plan
        .dependency_trees
        .iter()
        .map(|tree| tree_matches(root, tree))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .all(|v| v)
    {
        return Ok(());
    }
    // An unverified existing tree must never receive partially verified replacements.
    let staging = tempfile::tempdir_in(root)?;
    let stage = staging.path().canonicalize()?;
    for download in &plan.downloads {
        let mut verified = None;
        for url in &download.urls {
            ensure!(url.starts_with("https://"), "dependency URL must use HTTPS");
            if let Ok(file) = crate::registry::download_direct(url, 128 * 1024 * 1024) {
                let bytes = crate::files::read_regular(file.path(), 128 * 1024 * 1024, false)?;
                if crate::verify::digest(&bytes) == download.sha256 {
                    verified = Some(bytes);
                    break;
                }
            }
        }
        let bytes =
            verified.context("could not fetch the pinned dependency from its reviewed mirrors")?;
        let output = stage.join(relative(&download.output)?);
        match download.format.as_str() {
            "tar-xz" => unpack_xz(
                &bytes,
                &output,
                download.strip,
                &plan.skip_archive_paths,
                &stage,
            )?,
            "tar-gz" => unpack(
                flate2::read::GzDecoder::new(bytes.as_slice()),
                &output,
                download.strip,
                &plan.skip_archive_paths,
            )?,
            "deb" => {
                let (kind, payload) = deb_member(&bytes)?;
                if kind.ends_with(".gz") {
                    unpack(
                        flate2::read::GzDecoder::new(payload),
                        &output,
                        download.strip,
                        &plan.skip_archive_paths,
                    )?;
                } else if kind.ends_with(".xz") {
                    unpack_xz(
                        payload,
                        &output,
                        download.strip,
                        &plan.skip_archive_paths,
                        &stage,
                    )?;
                } else {
                    bail!("unsupported dependency compression");
                }
            }
            _ => bail!("unknown dependency archive format"),
        }
    }
    for tree in &plan.dependency_trees {
        ensure!(
            tree_matches(&stage, tree)?,
            "extracted dependency differs from its reviewed file manifest"
        );
    }
    for tree in &plan.dependency_trees {
        let to = destination(root, &tree.root)?;
        if to.symlink_metadata().is_ok() {
            ensure!(
                to.is_dir() && !to.is_symlink(),
                "dependency destination must be a real directory"
            );
            fs::remove_dir_all(&to)?;
        }
        fs::rename(stage.join(&tree.root), to)?;
    }
    Ok(())
}

pub(super) fn configure(root: &Path, name: &str, files: Inputs) -> Result<LinkCheck> {
    let root = root.canonicalize()?;
    let name = relative(name)?;
    let base = name.parent().unwrap_or(Path::new(""));
    let bytes = files
        .iter()
        .find(|(key, _)| Path::new(key) == name)
        .context("preparation plan is not compiled in")?
        .1;
    let plan: Plan = serde_json::from_slice(bytes)?;
    ensure!(plan.version == 1, "unsupported preparation plan");
    dependencies(&root, &plan)?;
    for header in &plan.headers {
        generated(&root, &header.output, input(files, base, &header.input)?)?;
    }
    resources(&root, &plan, files, base)?;
    let compiler = Command::new("clang").arg("--version").output()?;
    ensure!(
        compiler.status.success(),
        "cannot identify the native C compiler"
    );
    let mut context = bytes.to_vec();
    context.extend_from_slice(&compiler.stdout);
    context.extend_from_slice(crate::native::cpu_identity().as_bytes());
    for input in &plan.command_inputs {
        context.extend_from_slice(&source_bytes(&root, input, 4 * 1024 * 1024)?);
    }
    for name in [
        "CFLAGS",
        "CPPFLAGS",
        "LDFLAGS",
        "PKG_CONFIG_PATH",
        "PKG_CONFIG_SYSROOT_DIR",
    ] {
        context.extend_from_slice(format!("{name}={:?}\0", std::env::var_os(name)).as_bytes());
    }
    let identity = crate::verify::digest(&context);
    generated(&root, &plan.compiler_stamp, identity.as_bytes())?;
    let receipt_path = "build/gamepkg-preparation.json";
    let reusable = crate::files::read_regular(&root.join(receipt_path), 65536, false)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<PreparationReceipt>(&bytes).ok())
        .is_some_and(|receipt| {
            receipt.identity == identity
                && output_identities(&root, &plan.command_outputs)
                    .is_ok_and(|outputs| outputs == receipt.outputs)
        });
    if !reusable {
        for directory in &plan.reset_cmake_dirs {
            reset_cmake_cache(&root, directory)?;
        }
        for step in &plan.commands {
            ensure!(
                !step.program.contains(['/', '\\']) && !step.program.is_empty(),
                "invalid prepared tool"
            );
            let args: Vec<_> = step
                .args
                .iter()
                .map(|arg| arg.replace("{source}", &root.to_string_lossy()))
                .collect();
            let status = Command::new(&step.program)
                .args(&args)
                .current_dir(&root)
                .status()
                .with_context(|| format!("start prepared tool {}", step.program))?;
            ensure!(status.success(), "prepared tool {} failed", step.program);
        }
    }
    let config = source_bytes(&root, &plan.required_defines.path, 1024 * 1024)?;
    let text = std::str::from_utf8(&config)?;
    let defined: BTreeSet<_> = text
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            if parts.next() == Some("#define") {
                let name = parts.next()?;
                if parts.next().is_some() {
                    return Some(name);
                }
            }
            None
        })
        .collect();
    for name in &plan.required_defines.names {
        ensure!(
            defined.contains(name.as_str()),
            "native dependency is missing required driver {name}"
        );
    }
    let receipt = PreparationReceipt {
        identity,
        outputs: output_identities(&root, &plan.command_outputs)?,
    };
    generated(&root, receipt_path, &serde_json::to_vec(&receipt)?)?;
    generated(&root, &plan.ninja_output, input(files, base, &plan.ninja)?)?;
    Ok(LinkCheck {
        objects: root.join(relative(&plan.link_objects)?),
        binary: root.join(relative(&plan.binary)?),
    })
}

fn reset_cmake_cache(root: &Path, directory: &str) -> Result<()> {
    relative(directory)?;
    for name in ["CMakeCache.txt", "CMakeFiles"] {
        let path = destination(root, &format!("{directory}/{name}"))?;
        let metadata = match path.symlink_metadata() {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        ensure!(
            !metadata.is_symlink(),
            "configuration cache cannot be a link"
        );
        if metadata.is_dir() {
            ensure!(name == "CMakeFiles", "invalid configuration cache type");
            fs::remove_dir_all(path)?;
        } else {
            ensure!(metadata.is_file(), "invalid configuration cache type");
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn response_words(text: &str) -> Result<Vec<String>> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut escape = false;
    for c in text.chars() {
        if escape {
            word.push(c);
            escape = false;
        } else if c == '\\' && quote != Some('\'') {
            escape = true;
        } else if Some(c) == quote {
            quote = None;
        } else if quote.is_some() {
            word.push(c);
        } else if c == '\'' || c == '"' {
            quote = Some(c);
        } else if c.is_whitespace() {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
        } else {
            word.push(c);
        }
    }
    ensure!(
        quote.is_none() && !escape,
        "invalid native response quoting"
    );
    if !word.is_empty() {
        words.push(word);
    }
    Ok(words)
}

impl LinkCheck {
    pub(super) fn finish(self, root: &Path) -> Result<()> {
        let words = response_words(&fs::read_to_string(self.objects)?)?;
        ensure!(
            !words.is_empty() && words.len() <= 4096,
            "invalid native link inputs"
        );
        for path in &words {
            relative(path)?;
        }
        let output = Command::new("llvm-nm")
            .arg("--format=posix")
            .args(words.iter().map(|path| root.join(path)))
            .current_dir(root)
            .output()
            .context("inspect native link symbols")?;
        ensure!(
            output.status.success(),
            "native symbol inspection failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut symbols = BTreeMap::<String, BTreeSet<char>>::new();
        for line in std::str::from_utf8(&output.stdout)?.lines() {
            let fields: Vec<_> = line.split_whitespace().collect();
            if fields.len() >= 2 && !fields[0].ends_with(':') && fields[1].len() == 1 {
                symbols
                    .entry(fields[0].into())
                    .or_default()
                    .insert(fields[1].chars().next().unwrap());
            }
        }
        let runtime = [
            "__gmon_start__",
            "_ITM_deregisterTMCloneTable",
            "_ITM_registerTMCloneTable",
            "__cxa_finalize",
            "_Jv_RegisterClasses",
            "__pthread_key_create",
            "pthread_cancel",
        ];
        for (name, kinds) in symbols {
            ensure!(
                !kinds.iter().any(|c| "wv".contains(*c))
                    || kinds.iter().any(|c| "TDBRCVW".contains(*c))
                    || runtime.contains(&name.as_str()),
                "undefined weak native symbol: {name}"
            );
        }
        let imports = Command::new("llvm-nm")
            .arg("--undefined-only")
            .arg(self.binary)
            .output()?;
        ensure!(imports.status.success(), "native import inspection failed");
        for token in std::str::from_utf8(&imports.stdout)?.split_whitespace() {
            let name = token.split('@').next().unwrap_or(token);
            ensure!(
                !["wcs", "wmem", "towlower", "towupper", "isw"]
                    .iter()
                    .any(|p| name.starts_with(p))
                    && !name.contains("wprintf"),
                "incompatible native wide-character import: {name}"
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const TEMPLATE: &str = "const unsigned int sample[] = {\n@@sample@@\n};\n";
    const EXPECTED: &str = "const unsigned int sample[] = {\n\t0x04030201, 0x00ff8005,\n};\n";
    const INPUTS: Inputs = &[("test/template.c", TEMPLATE.as_bytes())];

    fn plan() -> Plan {
        Plan {
            version: 1,
            resource_template: "template.c".into(),
            resource_output: "generated/assets.c".into(),
            resource_sha256: crate::verify::digest(EXPECTED.as_bytes()),
            blobs: vec![Blob {
                symbol: "sample".into(),
                source: "input.bin".into(),
                sha256: crate::verify::digest(&[1, 2, 3, 4, 5, 128, 255]),
                mode: "bytes".into(),
            }],
            ..Plan::default()
        }
    }

    #[test]
    fn resource_bytes_have_reviewed_endianness_padding_and_atomic_failure() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        fs::write(root.join("input.bin"), [1, 2, 3, 4, 5, 128, 255]).unwrap();
        let mut plan = plan();
        resources(&root, &plan, INPUTS, Path::new("test")).unwrap();
        assert_eq!(
            fs::read_to_string(root.join("generated/assets.c")).unwrap(),
            EXPECTED
        );
        fs::write(root.join("input.bin"), b"changed").unwrap();
        assert!(resources(&root, &plan, INPUTS, Path::new("test")).is_err());
        assert_eq!(
            fs::read_to_string(root.join("generated/assets.c")).unwrap(),
            EXPECTED
        );
        plan.blobs[0].sha256 = crate::verify::digest(b"changed");
        assert!(
            resources(&root, &plan, INPUTS, Path::new("test")).is_err(),
            "final source identity must also match"
        );
        assert_eq!(
            fs::read_to_string(root.join("generated/assets.c")).unwrap(),
            EXPECTED
        );
    }

    #[cfg(unix)]
    #[test]
    fn resource_inputs_and_output_parents_cannot_follow_links_outside_source() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let target = outside.path().join("file.bin");
        fs::write(&target, [1, 2, 3, 4, 5, 128, 255]).unwrap();
        std::os::unix::fs::symlink(&target, root.join("input.bin")).unwrap();
        assert!(resources(&root, &plan(), INPUTS, Path::new("test")).is_err());
        fs::remove_file(root.join("input.bin")).unwrap();
        fs::write(root.join("input.bin"), [1, 2, 3, 4, 5, 128, 255]).unwrap();
        std::os::unix::fs::symlink(outside.path(), root.join("generated")).unwrap();
        assert!(resources(&root, &plan(), INPUTS, Path::new("test")).is_err());
        assert!(!outside.path().join("assets.c").exists());
        assert_eq!(fs::read(target).unwrap(), [1, 2, 3, 4, 5, 128, 255]);
    }

    #[test]
    fn native_paths_and_response_arguments_reject_escapes_and_keep_spaces() {
        for path in ["", "../out", "a/../../out", "/absolute", "C:/out", "a\\out"] {
            assert!(relative(path).is_err(), "{path}");
        }
        assert_eq!(
            response_words("one.o 'saved games/two.o' three\\ four.o\n").unwrap(),
            ["one.o", "saved games/two.o", "three four.o"]
        );
        assert!(response_words("'unclosed").is_err());
        for link in [
            "",
            "../../outside",
            "C:/outside",
            "C:outside",
            r"\outside",
            r"\\server\share",
            "//server/share",
            "/../outside",
            "bad\0name",
        ] {
            assert!(
                archive_link(Path::new("lib"), Path::new(link)).is_err(),
                "{link:?}"
            );
        }
        assert_eq!(
            archive_link(
                Path::new("lib/abi"),
                Path::new("../saved games/object.so.1")
            )
            .unwrap(),
            Path::new("../saved games/object.so.1")
        );
        assert_eq!(
            archive_link(Path::new("lib/abi"), Path::new("/lib/abi/object.so.1")).unwrap(),
            Path::new("object.so.1")
        );
    }

    #[test]
    fn fresh_configuration_removes_only_its_cache_and_rejects_external_links() {
        let root = tempfile::tempdir().unwrap();
        let build = root.path().join("build/dependency");
        fs::create_dir_all(build.join("CMakeFiles")).unwrap();
        fs::write(build.join("CMakeCache.txt"), b"old options").unwrap();
        fs::write(build.join("CMakeFiles/internal"), b"old compiler").unwrap();
        fs::write(build.join("keep.o"), b"unchanged object").unwrap();
        reset_cmake_cache(root.path(), "build/dependency").unwrap();
        assert!(!build.join("CMakeCache.txt").exists());
        assert!(!build.join("CMakeFiles").exists());
        assert_eq!(fs::read(build.join("keep.o")).unwrap(), b"unchanged object");
        assert!(reset_cmake_cache(root.path(), "../outside").is_err());
        #[cfg(unix)]
        {
            let outside = tempfile::tempdir().unwrap();
            fs::write(outside.path().join("CMakeCache.txt"), b"preserve").unwrap();
            std::os::unix::fs::symlink(outside.path(), root.path().join("redirect")).unwrap();
            assert!(reset_cmake_cache(root.path(), "redirect").is_err());
            assert_eq!(
                fs::read(outside.path().join("CMakeCache.txt")).unwrap(),
                b"preserve"
            );
        }
    }

    #[test]
    fn dependency_stream_is_drained_and_its_compression_trailer_checked() {
        let mut bytes = Vec::new();
        tar::Builder::new(&mut bytes).finish().unwrap();
        bytes.extend_from_slice(&[0; 65536]);
        let mut input = bytes.as_slice();
        let root = tempfile::tempdir().unwrap();
        unpack(&mut input, root.path(), 0, &[]).unwrap();
        assert!(input.is_empty());

        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(&bytes).unwrap();
        let mut compressed = encoder.finish().unwrap();
        let trailer = compressed.len() - 8;
        compressed[trailer] ^= 1;
        assert!(unpack(
            flate2::read::GzDecoder::new(compressed.as_slice()),
            root.path(),
            0,
            &[]
        )
        .is_err());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn xz_dependency_stream_checks_payload_trailer_and_cleanup() {
        let stage = tempfile::tempdir().unwrap();
        let mut archive = Vec::new();
        {
            let mut tar = tar::Builder::new(&mut archive);
            let mut header = tar::Header::new_gnu();
            header.set_size(7);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, "release/input.txt", b"fixture".as_slice())
                .unwrap();
            tar.finish().unwrap();
        }
        let plain = stage.path().join("input.tar");
        fs::write(&plain, archive).unwrap();
        let compressed = Command::new("xz")
            .args(["--compress", "--stdout", "--"])
            .arg(&plain)
            .output()
            .unwrap();
        assert!(compressed.status.success());
        let destination = stage.path().join("valid");
        unpack_xz(&compressed.stdout, &destination, 1, &[], stage.path()).unwrap();
        assert_eq!(fs::read(destination.join("input.txt")).unwrap(), b"fixture");
        let before = fs::read_dir(stage.path()).unwrap().count();
        for (i, length) in [0, 8, compressed.stdout.len() - 1].into_iter().enumerate() {
            assert!(unpack_xz(
                &compressed.stdout[..length],
                &stage.path().join(format!("truncated-{i}")),
                1,
                &[],
                stage.path(),
            )
            .is_err());
        }
        let mut corrupt = compressed.stdout;
        let last = corrupt.len() - 1;
        corrupt[last] ^= 1;
        assert!(unpack_xz(
            &corrupt,
            &stage.path().join("corrupt"),
            1,
            &[],
            stage.path(),
        )
        .is_err());
        assert_eq!(fs::read_dir(stage.path()).unwrap().count(), before + 4);
    }

    #[test]
    fn malformed_debian_member_extents_fail_without_panics() {
        for bytes in [b"".as_slice(), b"!<arch>\n", b"not an archive"] {
            assert!(deb_member(bytes).is_err());
        }
        let mut bytes = b"!<arch>\n".to_vec();
        let mut header = [b' '; 60];
        header[..12].copy_from_slice(b"data.tar.xz/");
        header[48..58].copy_from_slice(b"9999999999");
        header[58..].copy_from_slice(b"`\n");
        bytes.extend_from_slice(&header);
        assert!(deb_member(&bytes).is_err());
    }

    #[test]
    fn archive_symlinks_cannot_redirect_extraction_outside_the_destination() {
        let mut bytes = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut bytes);
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_mode(0o777);
            header.set_size(0);
            header.set_link_name("../outside").unwrap();
            header.set_cksum();
            builder
                .append_data(&mut header, "redirect", std::io::empty())
                .unwrap();
            builder.finish().unwrap();
        }
        let root = tempfile::tempdir().unwrap();
        assert!(unpack(bytes.as_slice(), root.path(), 0, &[]).is_err());
        assert!(!root.path().join("redirect").exists());
    }
}
