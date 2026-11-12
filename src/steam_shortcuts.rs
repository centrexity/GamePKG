//! Host-owned non-Steam shortcuts. Steam launches GamePKG, so its normal game
//! session still records play history. No package source can write this file.
use anyhow::{bail, ensure, Context, Result};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    io::Write,
    ops::Range,
    path::{Path, PathBuf},
};

const MAX_VDF: u64 = 16 * 1024 * 1024;
const MAX_DEPTH: usize = 32;

#[derive(Clone, Debug)]
pub struct Account {
    pub label: String,
    pub shortcuts: PathBuf,
}

fn steam_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(custom) = std::env::var_os("GAMEPKG_STEAM_ROOT") {
        roots.push(PathBuf::from(custom));
    }
    #[cfg(target_os = "linux")]
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        roots.extend([
            home.join(".local/share/Steam"),
            home.join(".steam/steam"),
            home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
        ]);
    }
    #[cfg(target_os = "windows")]
    {
        for variable in ["PROGRAMFILES(X86)", "PROGRAMFILES", "LOCALAPPDATA"] {
            if let Some(root) = std::env::var_os(variable) {
                roots.push(PathBuf::from(root).join("Steam"));
            }
        }
    }
    roots
}

pub fn accounts() -> Result<Vec<Account>> {
    let mut seen = HashSet::new();
    let mut found = Vec::new();
    for root in steam_roots() {
        for account in accounts_at(&root)? {
            if seen.insert(account.shortcuts.clone()) {
                found.push(account);
            }
        }
    }
    found.sort_by(|a, b| a.label.cmp(&b.label));
    Ok(found)
}

/// A chosen Steam install may be outside its usual platform location.
pub fn accounts_at(root: &Path) -> Result<Vec<Account>> {
    let userdata = root.join("userdata");
    if !userdata.is_dir() {
        return Ok(Vec::new());
    }
    let mut accounts = Vec::new();
    for entry in fs::read_dir(&userdata).with_context(|| format!("read {}", userdata.display()))? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(id) = name.to_str() else { continue };
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) || !entry.file_type()?.is_dir()
        {
            continue;
        }
        let config = entry.path().join("config");
        if !config.is_dir() {
            continue;
        }
        let config = config.canonicalize()?;
        accounts.push(Account {
            label: format!("Steam account {id}"),
            shortcuts: config.join("shortcuts.vdf"),
        });
    }
    Ok(accounts)
}

pub fn current_launcher() -> Result<PathBuf> {
    #[cfg(target_os = "linux")]
    if let Some(appimage) = std::env::var_os("APPIMAGE") {
        let path = PathBuf::from(appimage);
        if path.is_absolute() && path.is_file() {
            return Ok(path);
        }
    }
    Ok(std::env::current_exe()?)
}

#[derive(Default)]
struct Fields {
    exe: Option<Vec<u8>>,
    options: Option<Vec<u8>>,
    appid: Option<u32>,
    exe_range: Option<Range<usize>>,
    options_range: Option<Range<usize>>,
    start_range: Option<Range<usize>>,
    title_range: Option<Range<usize>>,
}
struct Entry {
    index: u32,
    fields: Fields,
}
struct Parsed {
    entries: Vec<Entry>,
    root_end: usize,
}

fn cstring<'a>(data: &'a [u8], at: &mut usize) -> Result<&'a [u8]> {
    let tail = data.get(*at..).context("truncated Steam shortcut string")?;
    let length = tail
        .iter()
        .position(|b| *b == 0)
        .context("unterminated Steam shortcut string")?;
    let value = &tail[..length];
    *at += length + 1;
    Ok(value)
}
fn bytes<'a>(data: &'a [u8], at: &mut usize, length: usize) -> Result<&'a [u8]> {
    let end = at
        .checked_add(length)
        .context("Steam shortcut extent overflow")?;
    let value = data
        .get(*at..end)
        .context("truncated Steam shortcut value")?;
    *at = end;
    Ok(value)
}
fn object(
    data: &[u8],
    at: &mut usize,
    depth: usize,
    mut fields: Option<&mut Fields>,
) -> Result<()> {
    ensure!(depth <= MAX_DEPTH, "Steam shortcut nesting is too deep");
    loop {
        let kind = *bytes(data, at, 1)?.first().unwrap();
        if kind == 8 {
            return Ok(());
        }
        let key = cstring(data, at)?;
        let value_start = *at;
        let value = match kind {
            0 => {
                object(data, at, depth + 1, None)?;
                None
            }
            1 => Some(cstring(data, at)?.to_vec()),
            2 | 3 | 4 | 6 => Some(bytes(data, at, 4)?.to_vec()),
            5 => {
                let start = *at;
                loop {
                    let unit = bytes(data, at, 2)?;
                    if unit == [0, 0] {
                        break;
                    }
                }
                Some(data[start..*at].to_vec())
            }
            7 => Some(bytes(data, at, 8)?.to_vec()),
            _ => bail!("unsupported Steam shortcut field type {kind}"),
        };
        if let Some(fields) = fields.as_deref_mut() {
            if key.eq_ignore_ascii_case(b"exe") && kind == 1 {
                ensure!(
                    fields.exe_range.is_none(),
                    "duplicate Steam shortcut executable field"
                );
                fields.exe = value;
                fields.exe_range = Some(value_start..*at - 1);
            } else if key.eq_ignore_ascii_case(b"launchoptions") && kind == 1 {
                ensure!(
                    fields.options_range.is_none(),
                    "duplicate Steam shortcut launch options field"
                );
                fields.options = value;
                fields.options_range = Some(value_start..*at - 1);
            } else if key.eq_ignore_ascii_case(b"startdir") && kind == 1 {
                ensure!(
                    fields.start_range.is_none(),
                    "duplicate Steam shortcut start folder field"
                );
                fields.start_range = Some(value_start..*at - 1);
            } else if key.eq_ignore_ascii_case(b"appname") && kind == 1 {
                ensure!(
                    fields.title_range.is_none(),
                    "duplicate Steam shortcut title field"
                );
                fields.title_range = Some(value_start..*at - 1);
            } else if key.eq_ignore_ascii_case(b"appid") && kind == 2 {
                ensure!(
                    fields.appid.is_none(),
                    "duplicate Steam shortcut app ID field"
                );
                fields.appid = value
                    .and_then(|v| v.try_into().ok())
                    .map(u32::from_le_bytes);
            }
        }
    }
}
fn parse(data: &[u8]) -> Result<Parsed> {
    ensure!(
        data.len() as u64 <= MAX_VDF,
        "Steam shortcut file is too large"
    );
    let mut at = 0;
    ensure!(
        bytes(data, &mut at, 1)? == [0],
        "invalid Steam shortcut root"
    );
    ensure!(
        cstring(data, &mut at)?.eq_ignore_ascii_case(b"shortcuts"),
        "invalid Steam shortcut root"
    );
    let mut entries = Vec::new();
    let mut indices = HashSet::new();
    loop {
        let root_end = at;
        let kind = *bytes(data, &mut at, 1)?.first().unwrap();
        if kind == 8 {
            ensure!(at == data.len(), "trailing Steam shortcut data");
            return Ok(Parsed { entries, root_end });
        }
        ensure!(kind == 0, "invalid Steam shortcut entry");
        let key = cstring(data, &mut at)?;
        let index = std::str::from_utf8(key)?
            .parse::<u32>()
            .context("invalid Steam shortcut index")?;
        ensure!(indices.insert(index), "duplicate Steam shortcut index");
        ensure!(entries.len() < 10000, "too many Steam shortcuts");
        let mut fields = Fields::default();
        object(data, &mut at, 1, Some(&mut fields))?;
        entries.push(Entry { index, fields });
    }
}

fn string(out: &mut Vec<u8>, key: &str, value: &str) {
    out.push(1);
    out.extend(key.as_bytes());
    out.push(0);
    out.extend(value.as_bytes());
    out.push(0);
}
fn integer(out: &mut Vec<u8>, key: &str, value: u32) {
    out.push(2);
    out.extend(key.as_bytes());
    out.push(0);
    out.extend(value.to_le_bytes());
}
fn checked_text(value: &str, label: &str) -> Result<()> {
    ensure!(
        !value.is_empty() && !value.chars().any(char::is_control),
        "invalid {label} for Steam shortcut"
    );
    Ok(())
}
fn shortcut_title(display: &str, id: &str) -> String {
    let mut clean = String::new();
    for character in display.chars().filter(|character| !character.is_control()) {
        if clean.len() + character.len_utf8() > 220 {
            break;
        }
        clean.push(character);
    }
    let title = clean.trim();
    format!("{} (GamePKG)", if title.is_empty() { id } else { title })
}
fn quoted_path(path: &Path) -> Result<String> {
    let text = path.to_str().context("Steam shortcut path is not UTF-8")?;
    checked_text(text, "path")?;
    ensure!(
        !text.contains('"'),
        "Steam shortcut paths cannot contain quotation marks"
    );
    Ok(format!("\"{text}\""))
}
fn launch_options(
    library: &Path,
    id: &str,
    target: &str,
    configuration: Option<&Path>,
) -> Result<String> {
    ensure!(
        id.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b)),
        "invalid game ID"
    );
    ensure!(matches!(target, "native" | "wasm"), "invalid game target");
    let mut options = format!(
        "--library {} --play {id} --target {target}",
        quoted_path(library)?
    );
    if let Some(configuration) = configuration {
        options.push_str(" --config ");
        options.push_str(&quoted_path(configuration)?);
    }
    Ok(options)
}

// Match the game's identity across a target/settings change, including legacy
// shortcuts without --config. Accept only the options this host generates;
// arbitrary extra arguments must not identify an unrelated shortcut as ours.
fn matching_options(options: &[u8], base: &str) -> bool {
    let Ok(options) = std::str::from_utf8(options) else {
        return false;
    };
    if options == base {
        return true;
    }
    let Some(configuration) = options
        .strip_prefix(base)
        .and_then(|s| s.strip_prefix(" --config "))
    else {
        return false;
    };
    let Some(path) = configuration
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
    else {
        return false;
    };
    !path.is_empty() && !path.chars().any(|c| c == '"' || c.is_control())
}
fn entry(index: u32, appid: u32, title: &str, exe: &Path, options: &str) -> Result<Vec<u8>> {
    let start = quoted_path(
        exe.parent()
            .context("GamePKG executable has no parent folder")?,
    )?;
    let exe = quoted_path(exe)?;
    let mut out = Vec::new();
    out.push(0);
    out.extend(index.to_string().as_bytes());
    out.push(0);
    integer(&mut out, "appid", appid);
    string(&mut out, "AppName", title);
    string(&mut out, "Exe", &exe);
    string(&mut out, "StartDir", &start);
    string(&mut out, "icon", "");
    string(&mut out, "ShortcutPath", "");
    string(&mut out, "LaunchOptions", options);
    for (key, value) in [
        ("IsHidden", 0),
        ("AllowDesktopConfig", 1),
        ("AllowOverlay", 1),
        ("OpenVR", 0),
        ("Devkit", 0),
        ("LastPlayTime", 0),
    ] {
        integer(&mut out, key, value);
    }
    string(&mut out, "DevkitGameID", "");
    out.extend([0, b't', b'a', b'g', b's', 0, 8, 8]);
    Ok(out)
}
fn appid(options: &str, used: &HashSet<u32>) -> u32 {
    let digest = Sha256::digest(options.as_bytes());
    let mut id = u32::from_le_bytes(digest[..4].try_into().unwrap()) | 0x8000_0000;
    while used.contains(&id) {
        id = id.wrapping_add(1) | 0x8000_0000;
    }
    id
}
fn add_bytes(
    existing: &[u8],
    title: &str,
    exe: &Path,
    library: &Path,
    id: &str,
    target: &str,
    configuration: Option<&Path>,
) -> Result<(Vec<u8>, bool)> {
    checked_text(title, "game title")?;
    ensure!(title.len() <= 250, "game title is too long for Steam");
    let options = launch_options(library, id, target, configuration)?;
    let legacy = launch_options(library, id, target, None)?;
    let alternate = launch_options(
        library,
        id,
        if target == "native" { "wasm" } else { "native" },
        None,
    )?;
    let parsed = parse(existing)?;
    let matches =
        parsed
            .entries
            .iter()
            .filter(|entry| {
                entry.fields.options.as_deref().is_some_and(|o| {
                    matching_options(o, &legacy) || matching_options(o, &alternate)
                })
            })
            .collect::<Vec<_>>();
    ensure!(
        matches.len() <= 1,
        "multiple matching GamePKG shortcuts exist in Steam; remove duplicates there first"
    );
    if let Some(found) = matches.first() {
        let current_exe = quoted_path(exe)?;
        let start = quoted_path(
            exe.parent()
                .context("GamePKG executable has no parent folder")?,
        )?;
        let fields = &found.fields;
        let changes = [
            (
                fields
                    .exe_range
                    .clone()
                    .context("Steam shortcut has no executable field")?,
                current_exe,
            ),
            (
                fields
                    .options_range
                    .clone()
                    .context("Steam shortcut has no launch options field")?,
                options,
            ),
            (
                fields
                    .start_range
                    .clone()
                    .context("Steam shortcut has no start folder field")?,
                start,
            ),
            (
                fields
                    .title_range
                    .clone()
                    .context("Steam shortcut has no title field")?,
                title.to_owned(),
            ),
        ];
        let changed = changes
            .iter()
            .any(|(range, value)| &existing[range.clone()] != value.as_bytes());
        if !changed {
            return Ok((existing.to_vec(), false));
        }
        let mut sorted = changes;
        sorted.sort_by_key(|(range, _)| range.start);
        let mut output = Vec::with_capacity(existing.len() + 1024);
        let mut cursor = 0;
        for (range, value) in sorted {
            ensure!(range.start >= cursor, "overlapping Steam shortcut fields");
            output.extend(&existing[cursor..range.start]);
            output.extend(value.as_bytes());
            cursor = range.end;
        }
        output.extend(&existing[cursor..]);
        ensure!(
            output.len() as u64 <= MAX_VDF,
            "Steam shortcut file would be too large"
        );
        parse(&output)?;
        return Ok((output, true));
    }
    let next = parsed
        .entries
        .iter()
        .map(|e| e.index)
        .max()
        .map_or(0, |n| n.saturating_add(1));
    ensure!(
        !parsed.entries.iter().any(|e| e.index == next),
        "Steam shortcut indices are exhausted"
    );
    let used = parsed
        .entries
        .iter()
        .filter_map(|e| e.fields.appid)
        .collect();
    let shortcut = entry(next, appid(&options, &used), title, exe, &options)?;
    ensure!(
        (existing.len() + shortcut.len()) as u64 <= MAX_VDF,
        "Steam shortcut file would be too large"
    );
    let mut output = Vec::with_capacity(existing.len() + shortcut.len());
    output.extend(&existing[..parsed.root_end]);
    output.extend(shortcut);
    output.extend(&existing[parsed.root_end..]);
    parse(&output)?;
    Ok((output, true))
}

#[cfg(target_os = "linux")]
fn steam_running() -> Result<bool> {
    for process in fs::read_dir("/proc")? {
        let process = process?;
        if !process
            .file_name()
            .to_string_lossy()
            .bytes()
            .all(|b| b.is_ascii_digit())
        {
            continue;
        }
        if let Ok(name) = fs::read_to_string(process.path().join("comm")) {
            if matches!(name.trim(), "steam" | "steamwebhelper" | "steam-runtime") {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
#[cfg(target_os = "windows")]
fn steam_running() -> Result<bool> {
    let output = std::process::Command::new("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .output()?;
    ensure!(
        output.status.success(),
        "unable to check whether Steam is running"
    );
    Ok(String::from_utf8_lossy(&output.stdout).lines().any(|line| {
        let lower = line.to_ascii_lowercase();
        lower.starts_with("\"steam.exe\",") || lower.starts_with("\"steamwebhelper.exe\",")
    }))
}
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn steam_running() -> Result<bool> {
    bail!("Steam shortcuts are supported on Linux and Windows")
}

fn persist(account: &Account, existing: &[u8], output: &[u8]) -> Result<()> {
    let config = account
        .shortcuts
        .parent()
        .context("invalid Steam account path")?;
    let mut temporary = tempfile::NamedTempFile::new_in(config)?;
    temporary.write_all(output)?;
    temporary.as_file().sync_all()?;
    // Avoid replacing a concurrent Steam or tool edit between our read and write.
    let latest = if account.shortcuts.exists() {
        fs::read(&account.shortcuts)?
    } else {
        b"\0shortcuts\0\x08".to_vec()
    };
    ensure!(
        latest == existing,
        "Steam shortcuts changed while adding this game; try again"
    );
    temporary
        .persist(&account.shortcuts)
        .context("save Steam shortcuts")?;
    Ok(())
}

pub fn add(
    account: &Account,
    manifest: &crate::manifest::Manifest,
    library: &Path,
    exe: &Path,
    target: &str,
) -> Result<String> {
    ensure!(
        !steam_running()?,
        "Close Steam before adding the shortcut, then open it again afterward"
    );
    let installed = crate::registry::installed_manifest(library, &manifest.id)?;
    ensure!(
        installed.creator_key == manifest.creator_key && installed.id == manifest.id,
        "the installed game changed; refresh the library"
    );
    ensure!(
        exe.is_absolute() && exe.is_file(),
        "GamePKG executable is unavailable"
    );
    ensure!(
        library.is_absolute() && library.is_dir(),
        "GamePKG library is unavailable"
    );
    let config = account
        .shortcuts
        .parent()
        .context("invalid Steam account path")?;
    ensure!(
        config.is_dir(),
        "Steam account configuration is unavailable"
    );
    let existing = if account.shortcuts.exists() {
        ensure!(
            !fs::symlink_metadata(&account.shortcuts)?
                .file_type()
                .is_symlink(),
            "Steam shortcut file is a symbolic link; manage it in Steam"
        );
        ensure!(
            fs::metadata(&account.shortcuts)?.len() <= MAX_VDF,
            "Steam shortcut file is too large"
        );
        let data = fs::read(&account.shortcuts)?;
        ensure!(
            data.len() as u64 <= MAX_VDF,
            "Steam shortcut file is too large"
        );
        data
    } else {
        b"\0shortcuts\0\x08".to_vec()
    };
    let title = shortcut_title(installed.display_title(), &installed.id);
    let configuration = std::path::absolute(crate::settings::path()?)?;
    let (output, changed) = add_bytes(
        &existing,
        &title,
        exe,
        library,
        &manifest.id,
        target,
        Some(&configuration),
    )?;
    if !changed {
        return Ok(format!("{title} is already in {}.", account.label));
    }
    ensure!(
        !steam_running()?,
        "Steam started while the shortcut was being prepared; close Steam and try again"
    );
    persist(account, &existing, &output)?;
    Ok(format!(
        "Steam shortcut for {title} is ready in {}. Open Steam to see it.",
        account.label
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY: &[u8] = b"\0shortcuts\0\x08";
    #[test]
    fn retains_settings_and_migrates_old_shortcuts_without_changing_appid() {
        let exe = Path::new("/opt/Game PKG/gamepkg-gui");
        let library = Path::new("/home/player/My Games");
        let first_config = Path::new("/home/player/Preview settings.toml");
        let second_config = Path::new("/home/player/Other settings.toml");
        let (legacy, _) =
            add_bytes(EMPTY, "Game", exe, library, "one.game", "native", None).unwrap();
        let old_id = parse(&legacy).unwrap().entries[0].fields.appid;
        let (migrated, changed) = add_bytes(
            &legacy,
            "Game",
            exe,
            library,
            "one.game",
            "native",
            Some(first_config),
        )
        .unwrap();
        assert!(changed);
        let parsed = parse(&migrated).unwrap();
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].fields.appid, old_id);
        assert_eq!(parsed.entries[0].fields.options.as_deref(), Some(b"--library \"/home/player/My Games\" --play one.game --target native --config \"/home/player/Preview settings.toml\"".as_slice()));
        let (same, changed) = add_bytes(
            &migrated,
            "Game",
            exe,
            library,
            "one.game",
            "native",
            Some(first_config),
        )
        .unwrap();
        assert!(!changed);
        assert_eq!(same, migrated);
        let (updated, changed) = add_bytes(
            &migrated,
            "Game",
            exe,
            library,
            "one.game",
            "wasm",
            Some(second_config),
        )
        .unwrap();
        assert!(changed);
        let parsed = parse(&updated).unwrap();
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].fields.appid, old_id);
        assert_eq!(parsed.entries[0].fields.options.as_deref(), Some(b"--library \"/home/player/My Games\" --play one.game --target wasm --config \"/home/player/Other settings.toml\"".as_slice()));
    }

    #[test]
    fn settings_identity_rejects_extra_arguments_and_unsafe_paths() {
        let base = "--library \"/home/player/games\" --play one.game --target native";
        assert!(matching_options(base.as_bytes(), base));
        assert!(matching_options(
            format!("{base} --config \"C:\\Users\\Player\\My Settings.toml\"").as_bytes(),
            base
        ));
        for suffix in [
            " --config \"\"",
            " --config \"one\" --support",
            " --config \"one\" --config \"two\"",
            " --config \"line\nbreak\"",
            " --config unquoted",
            " --unknown",
            "junk",
        ] {
            assert!(
                !matching_options(format!("{base}{suffix}").as_bytes(), base),
                "{suffix:?}"
            );
        }
        assert!(!matching_options(b"\xff", base));
        for config in [
            "/tmp/quote\".toml",
            "/tmp/line\nbreak.toml",
            "/tmp/zero\0.toml",
        ] {
            assert!(launch_options(
                Path::new("/games"),
                "game",
                "native",
                Some(Path::new(config))
            )
            .is_err());
        }
    }

    #[test]
    fn preserves_extra_argument_shortcuts_and_rejects_ambiguous_settings_duplicates() {
        let exe = Path::new("/opt/gamepkg-gui");
        let library = Path::new("/home/player/games");
        let config = Path::new("/home/player/settings.toml");
        let base = launch_options(library, "one.game", "native", None).unwrap();
        let foreign_options = format!("{base} --config \"/other/settings.toml\" --support");
        let mut foreign = b"\0shortcuts\0".to_vec();
        foreign.extend(entry(0, 123, "Different launch", exe, &foreign_options).unwrap());
        foreign.push(8);
        let (output, changed) = add_bytes(
            &foreign,
            "Game",
            exe,
            library,
            "one.game",
            "native",
            Some(config),
        )
        .unwrap();
        assert!(changed);
        let parsed = parse(&output).unwrap();
        assert_eq!(parsed.entries.len(), 2);
        assert_eq!(
            parsed.entries[0].fields.options.as_deref(),
            Some(foreign_options.as_bytes())
        );
        assert_eq!(parsed.entries[0].fields.appid, Some(123));

        let mut duplicates = b"\0shortcuts\0".to_vec();
        for (index, path) in ["/first.toml", "/second.toml"].iter().enumerate() {
            let options =
                launch_options(library, "one.game", "native", Some(Path::new(path))).unwrap();
            duplicates
                .extend(entry(index as u32, 123 + index as u32, "Game", exe, &options).unwrap());
        }
        duplicates.push(8);
        let before = duplicates.clone();
        let error = add_bytes(
            &duplicates,
            "Game",
            exe,
            library,
            "one.game",
            "native",
            Some(config),
        )
        .unwrap_err();
        assert!(error.to_string().contains("multiple matching"));
        assert_eq!(duplicates, before);
    }

    #[test]
    fn adds_idempotently_and_updates_existing_game_without_erasing_other_shortcuts() {
        let old = Path::new("/opt/Game PKG/bin/gamepkg-gui");
        let new = Path::new("/opt/GamePKG 2/bin/gamepkg-gui");
        let library = Path::new("/home/player/My Games");
        let (one, changed) = add_bytes(
            EMPTY,
            "One (GamePKG)",
            old,
            library,
            "one.game",
            "native",
            None,
        )
        .unwrap();
        assert!(changed);
        let first = parse(&one).unwrap();
        assert_eq!(first.entries.len(), 1);
        assert_eq!(
            first.entries[0].fields.exe.as_deref(),
            Some(b"\"/opt/Game PKG/bin/gamepkg-gui\"".as_slice())
        );
        assert_eq!(
            first.entries[0].fields.options.as_deref(),
            Some(b"--library \"/home/player/My Games\" --play one.game --target native".as_slice())
        );
        let (same, changed) = add_bytes(
            &one,
            "One (GamePKG)",
            old,
            library,
            "one.game",
            "native",
            None,
        )
        .unwrap();
        assert!(!changed);
        assert_eq!(same, one);
        let (two, changed) = add_bytes(
            &one,
            "Two (GamePKG)",
            old,
            library,
            "two.game",
            "native",
            None,
        )
        .unwrap();
        assert!(changed);
        assert_eq!(parse(&two).unwrap().entries.len(), 2);
        let (updated, changed) = add_bytes(
            &two,
            "One Revised (GamePKG)",
            new,
            library,
            "one.game",
            "wasm",
            None,
        )
        .unwrap();
        assert!(changed);
        let parsed = parse(&updated).unwrap();
        assert_eq!(parsed.entries.len(), 2);
        assert_eq!(
            parsed.entries[0].fields.exe.as_deref(),
            Some(b"\"/opt/GamePKG 2/bin/gamepkg-gui\"".as_slice())
        );
        assert_eq!(
            parsed.entries[0].fields.options.as_deref(),
            Some(b"--library \"/home/player/My Games\" --play one.game --target wasm".as_slice())
        );
        assert_eq!(
            parsed.entries[1].fields.options,
            parse(&two).unwrap().entries[1].fields.options
        );
        assert_eq!(
            parsed.entries[0].fields.appid,
            first.entries[0].fields.appid
        );
    }
    #[test]
    fn rejects_corrupt_or_unsafe_vdf_without_replacing_anything() {
        let exe = Path::new("/opt/gamepkg-gui");
        let library = Path::new("/home/player/games");
        for malformed in [
            b"".as_slice(),
            b"\0shortcuts\0",
            b"\0shortcuts\0\0x\0\x08\x08",
            b"\0shortcuts\0\x09a\0\x08",
            b"\0shortcuts\0\x08junk",
        ] {
            assert!(add_bytes(malformed, "Game", exe, library, "game", "native", None).is_err());
        }
        assert!(add_bytes(EMPTY, "Game\0Bad", exe, library, "game", "native", None).is_err());
        assert!(add_bytes(
            EMPTY,
            "Game",
            Path::new("/tmp/quote\"name"),
            library,
            "game",
            "native",
            None,
        )
        .is_err());
        assert!(add_bytes(EMPTY, "Game", exe, library, "game;rm", "native", None).is_err());
        assert!(add_bytes(EMPTY, "Game", exe, library, "game", "other", None).is_err());
    }
    #[test]
    fn only_numeric_local_account_folders_are_offered() {
        let temporary = tempfile::tempdir().unwrap();
        let userdata = temporary.path().join("userdata");
        fs::create_dir_all(userdata.join("123/config")).unwrap();
        fs::create_dir_all(userdata.join("not-an-account/config")).unwrap();
        fs::create_dir_all(userdata.join("456/other")).unwrap();
        let accounts = accounts_at(temporary.path()).unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].label, "Steam account 123");
    }
    #[test]
    fn long_unicode_titles_remain_valid_steam_strings() {
        let title = shortcut_title(&"🎮".repeat(200), "fallback.game");
        assert!(title.len() <= 250);
        assert!(title.ends_with(" (GamePKG)"));
        assert_eq!(
            shortcut_title("\n\t", "fallback.game"),
            "fallback.game (GamePKG)"
        );
    }
    #[test]
    fn atomic_save_preserves_existing_file_when_it_changes_before_publication() {
        let temporary = tempfile::tempdir().unwrap();
        let config = temporary.path().join("userdata/123/config");
        fs::create_dir_all(&config).unwrap();
        let account = Account {
            label: "Steam account 123".into(),
            shortcuts: config.join("shortcuts.vdf"),
        };
        let (updated, _) = add_bytes(
            EMPTY,
            "Game (GamePKG)",
            Path::new("/opt/gamepkg-gui"),
            Path::new("/home/player/games"),
            "game",
            "native",
            None,
        )
        .unwrap();
        persist(&account, EMPTY, &updated).unwrap();
        assert_eq!(fs::read(&account.shortcuts).unwrap(), updated);
        let (replaced, _) = add_bytes(
            &updated,
            "Game Revised (GamePKG)",
            Path::new("/opt/gamepkg-gui-v2"),
            Path::new("/home/player/games"),
            "game",
            "wasm",
            None,
        )
        .unwrap();
        persist(&account, &updated, &replaced).unwrap();
        assert_eq!(fs::read(&account.shortcuts).unwrap(), replaced);
        fs::write(&account.shortcuts, EMPTY).unwrap();
        assert!(persist(&account, &replaced, &replaced).is_err());
        assert_eq!(fs::read(&account.shortcuts).unwrap(), EMPTY);
    }
}
