//! Load reviewed external-engine records. The bytes are compiled in from
//! `engines/*/engine.toml`. A package cannot replace them.
use super::{AudioFallback, Companion, Engine, Image, LaunchSpec, Target};
use crate::manifest::DataLayout;
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::collections::HashSet;

pub struct Record {
    pub manifest: &'static str,
    pub patches: &'static [(&'static str, &'static str)],
    pub files: &'static [(&'static str, &'static [u8])],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EngineFile {
    id: String,
    clone_url: String,
    commit: String,
    license_sha256: String,
    license_marker: String,
    memory: u64,
    hard_memory: u64,
    network_allowed: bool,
    launcher: String,
    required_paths: Vec<String>,
    forbidden_directories: Vec<String>,
    forbidden_suffixes: Vec<String>,
    cleanup: Vec<String>,
    #[serde(default)]
    patches: Vec<String>,
    layouts: Vec<DataLayout>,
    targets: Vec<TargetFile>,
    launch: LaunchFile,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetFile {
    id: String,
    os: String,
    arch: String,
    #[serde(default)]
    blocker: Option<String>,
    #[serde(default)]
    note: Option<String>,
    tools: Vec<String>,
    configure: Vec<String>,
    #[serde(default)]
    prepare: Option<String>,
    build_program: String,
    build_args: Vec<String>,
    binary: String,
    #[serde(default)]
    companions: Vec<CompanionFile>,
    image: ImageFile,
    #[serde(default)]
    audio: Option<AudioFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "lowercase")]
enum ImageFile {
    Elf { class: u8, machine: u16 },
    Pe { machine: u16, magic: u16 },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CompanionFile {
    relative: String,
    limit: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AudioFile {
    libraries: Vec<String>,
    variable: String,
    value: String,
    warning: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LaunchFile {
    #[serde(default)]
    data_root: Option<String>,
    #[serde(default)]
    data_file: Option<String>,
    #[serde(default)]
    scratch: Vec<super::Scratch>,
    #[serde(default)]
    save_root: Option<String>,
    #[serde(default)]
    fixed: Vec<[String; 2]>,
    #[serde(default)]
    passthrough: Vec<String>,
}

pub fn load(records: &[Record]) -> Result<Vec<Engine>> {
    let mut engines = Vec::new();
    let mut ids = HashSet::new();
    for record in records {
        let file: EngineFile = toml::from_str(record.manifest).context("external engine record")?;
        let engine = convert(file, record)?;
        ensure!(
            ids.insert(engine.id.clone()),
            "duplicate external engine id"
        );
        engines.push(engine);
    }
    Ok(engines)
}

fn convert(file: EngineFile, record: &Record) -> Result<Engine> {
    ensure!(
        record.files.len() <= 64
            && record
                .files
                .iter()
                .all(|(name, bytes)| relative_path(name) && bytes.len() <= 4 * 1024 * 1024),
        "invalid prepared engine inputs"
    );
    ensure!(
        valid_id(&file.id),
        "external engine id must be a short token"
    );
    ensure!(
        file.clone_url.starts_with("https://")
            && !file.clone_url.chars().any(char::is_control)
            && file.clone_url.len() <= 512,
        "external engine clone URL must be https"
    );
    ensure!(
        file.commit.len() == 40 && file.commit.bytes().all(|b| b.is_ascii_hexdigit()),
        "external engine commit must be 40 hex digits"
    );
    ensure!(
        file.license_sha256.len() == 64
            && file.license_sha256.bytes().all(|b| b.is_ascii_hexdigit()),
        "external engine license digest must be 64 hex digits"
    );
    ensure!(
        !file.license_marker.is_empty()
            && file.license_marker.len() <= 128
            && !file.license_marker.chars().any(char::is_control),
        "external engine license marker is invalid"
    );
    ensure!(
        file.memory > 0
            && file.hard_memory >= file.memory
            && file.hard_memory <= 8 * 1024 * 1024 * 1024,
        "external engine memory bounds are invalid"
    );
    ensure!(
        !file.launcher.is_empty() && file.launcher.len() <= 4096 && !file.launcher.contains('\0'),
        "external engine launcher is invalid"
    );
    for path in file
        .required_paths
        .iter()
        .chain(&file.forbidden_directories)
        .chain(&file.cleanup)
    {
        ensure!(relative_token(path), "external engine path is not relative");
    }
    for suffix in &file.forbidden_suffixes {
        ensure!(
            suffix.starts_with('.')
                && suffix.len() <= 16
                && suffix.bytes().all(|b| b.is_ascii_lowercase() || b == b'.'),
            "external engine forbidden suffix is invalid"
        );
    }
    crate::game_data::validate_layouts(&file.layouts)?;
    let mut patch_bodies = Vec::new();
    let mut seen_patches = HashSet::new();
    for name in &file.patches {
        ensure!(
            seen_patches.insert(name.clone()) && valid_patch_name(name),
            "external engine patch name is invalid or repeated"
        );
        let body = record
            .patches
            .iter()
            .find(|(embedded, _)| embedded == name)
            .map(|(_, body)| (*body).to_owned())
            .with_context(|| format!("external engine patch {name} is not in the build"))?;
        ensure!(
            body.ends_with('\n'),
            "external engine patch must end in a newline"
        );
        patch_bodies.push((name.clone(), body));
    }
    ensure!(
        record.patches.len() == file.patches.len(),
        "external engine record lists a different patch set than the build embedded"
    );
    let mut targets = Vec::new();
    let mut target_ids = HashSet::new();
    let mut platforms = HashSet::new();
    for target in file.targets {
        ensure!(
            target_ids.insert(target.id.clone()) && valid_id(&target.id),
            "duplicate or invalid engine target id"
        );
        ensure!(
            matches!(target.os.as_str(), "linux" | "windows"),
            "engine target os must be linux or windows"
        );
        ensure!(
            matches!(target.arch.as_str(), "x86_64" | "i686"),
            "engine target arch must be x86_64 or i686"
        );
        ensure!(
            platforms.insert((target.os.clone(), target.arch.clone())),
            "duplicate engine target platform"
        );
        if let Some(blocker) = &target.blocker {
            ensure!(
                !blocker.trim().is_empty()
                    && blocker.len() <= 2000
                    && !blocker.chars().any(char::is_control),
                "engine target blocker is invalid"
            );
        }
        if let Some(note) = &target.note {
            ensure!(
                note.len() <= 2000 && !note.chars().any(char::is_control),
                "engine target note is invalid"
            );
        }
        for tool in &target.tools {
            ensure!(valid_id(tool), "engine target tool name is invalid");
        }
        for arg in target.configure.iter().chain(&target.build_args) {
            ensure!(
                !arg.is_empty() && arg.len() <= 512 && !arg.chars().any(char::is_control),
                "engine target argument is invalid"
            );
        }
        ensure!(
            target.blocker.is_some() || !target.configure.is_empty() || target.prepare.is_some(),
            "a ready engine target needs a configure command"
        );
        if let Some(path) = &target.prepare {
            ensure!(
                relative_path(path) && record.files.iter().any(|(name, _)| *name == path),
                "engine preparation plan is not a compiled reviewed input"
            );
        }
        ensure!(
            valid_id(&target.build_program) && relative_path(&target.binary),
            "engine target build program or binary path is invalid"
        );
        let mut companions = Vec::new();
        for companion in target.companions {
            ensure!(
                relative_path(&companion.relative)
                    && companion.limit > 0
                    && companion.limit <= 64 * 1024 * 1024,
                "engine companion path or limit is invalid"
            );
            companions.push(Companion {
                relative: companion.relative,
                limit: companion.limit,
            });
        }
        let image = match target.image {
            ImageFile::Elf { class, machine } => {
                ensure!(matches!(class, 1 | 2), "ELF class must be 1 or 2");
                Image::Elf { class, machine }
            }
            ImageFile::Pe { machine, magic } => {
                ensure!(matches!(magic, 0x10b | 0x20b), "PE magic is invalid");
                Image::Pe { machine, magic }
            }
        };
        let audio = match target.audio {
            None => None,
            Some(audio) => {
                ensure!(
                    !audio.libraries.is_empty()
                        && audio.libraries.iter().all(|path| {
                            path.starts_with('/')
                                && path.len() <= 256
                                && !path.chars().any(char::is_control)
                                && !path.contains('\0')
                        })
                        && env_name(&audio.variable)
                        && !audio.value.is_empty()
                        && audio.value.len() <= 64
                        && !audio.value.chars().any(char::is_control)
                        && !audio.warning.is_empty()
                        && audio.warning.len() <= 256
                        && !audio.warning.chars().any(char::is_control),
                    "engine audio fallback is invalid"
                );
                Some(AudioFallback {
                    libraries: audio.libraries,
                    variable: audio.variable,
                    value: audio.value,
                    warning: audio.warning,
                })
            }
        };
        targets.push(Target {
            id: target.id,
            os: target.os,
            arch: target.arch,
            blocker: target.blocker,
            note: target.note,
            tools: target.tools,
            configure: target.configure,
            prepare: target.prepare,
            build_program: target.build_program,
            build_args: target.build_args,
            binary: target.binary,
            companions,
            image,
            audio,
        });
    }
    ensure!(!targets.is_empty(), "external engine needs a target");
    let mut names = HashSet::new();
    let fixed = file
        .launch
        .fixed
        .into_iter()
        .map(|pair| {
            ensure!(
                env_name(&pair[0]) && names.insert(pair[0].clone()),
                "bad launch variable"
            );
            ensure!(
                pair[1].len() <= 256 && !pair[1].chars().any(char::is_control),
                "bad launch value"
            );
            Ok((pair[0].clone(), pair[1].clone()))
        })
        .collect::<Result<Vec<_>>>()?;
    for name in file
        .launch
        .data_root
        .iter()
        .chain(&file.launch.data_file)
        .chain(&file.launch.save_root)
        .chain(&file.launch.passthrough)
    {
        ensure!(
            env_name(name) && names.insert(name.clone()),
            "bad launch variable"
        );
    }
    let mut scratch_total = 0u64;
    let mut scratch_paths = Vec::<&str>::new();
    for scratch in &file.launch.scratch {
        ensure!(
            relative_path(&scratch.path) && scratch.bytes >= 1024 * 1024,
            "invalid private scratch directory"
        );
        ensure!(
            !scratch_paths.iter().any(|other| scratch.path == *other
                || scratch.path.starts_with(&format!("{other}/"))
                || other.starts_with(&format!("{}/", scratch.path))),
            "overlapping private scratch directories"
        );
        scratch_paths.push(&scratch.path);
        scratch_total = scratch_total
            .checked_add(scratch.bytes)
            .context("scratch budget overflow")?;
    }
    ensure!(
        file.launch.scratch.len() <= 8 && scratch_total < file.memory,
        "private scratch exceeds the engine memory budget"
    );
    let source_revision = record_revision(record);
    let revision = if targets.iter().any(|target| target.prepare.is_some()) {
        crate::verify::digest(
            format!(
                "GamePKG native prepared v1\0{source_revision}\0{}",
                crate::native::cpu_identity()
            )
            .as_bytes(),
        )
    } else {
        source_revision
    };
    Ok(Engine {
        id: file.id,
        revision,
        prepared: record.files,
        clone_url: file.clone_url,
        commit: file.commit,
        license_sha256: file.license_sha256,
        license_marker: file.license_marker,
        memory: file.memory,
        hard_memory: file.hard_memory,
        network_allowed: file.network_allowed,
        launcher: file.launcher,
        required_paths: file.required_paths,
        forbidden_directories: file.forbidden_directories,
        forbidden_suffixes: file.forbidden_suffixes,
        cleanup: file.cleanup,
        patches: patch_bodies,
        layouts: file.layouts,
        targets,
        launch: LaunchSpec {
            data_root: file.launch.data_root,
            data_file: file.launch.data_file,
            scratch: file.launch.scratch,
            save_root: file.launch.save_root,
            fixed,
            passthrough: file.launch.passthrough,
        },
    })
}

fn record_revision(record: &Record) -> String {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(b"GamePKG reviewed engine v1\0");
    for bytes in std::iter::once(record.manifest.as_bytes()).chain(
        record
            .patches
            .iter()
            .flat_map(|(name, body)| [name.as_bytes(), body.as_bytes()]),
    ) {
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    for (name, bytes) in record.files {
        hash.update((name.len() as u64).to_le_bytes());
        hash.update(name.as_bytes());
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    hex::encode(hash.finalize())
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn valid_patch_name(value: &str) -> bool {
    value.len() <= 128
        && value.ends_with(".patch")
        && !value.contains('/')
        && !value.contains('\\')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn relative_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !value.contains('\\')
        && !value.starts_with('/')
        && !value
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        && !value.chars().any(char::is_control)
}

fn relative_path(value: &str) -> bool {
    relative_token(value) && value.len() <= 256
}

fn env_name(value: &str) -> bool {
    let mut bytes = value.bytes();
    match bytes.next() {
        Some(first) if first.is_ascii_alphabetic() || first == b'_' => {
            value.len() <= 64 && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        }
        _ => false,
    }
}
