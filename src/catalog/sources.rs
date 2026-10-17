//! Remember list subscriptions independently of installed packages and creator trust.
use super::{Catalog, Entry};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, io::Write, path::Path};
const STATE_FILE: &str = "catalog-sources.json";
const STATE_LIMIT: u64 = 16 * 1024 * 1024;
const MAX_LISTS: usize = 64;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedList {
    url: String,
    games: Vec<Entry>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    format_version: u32,
    local_games: Vec<Entry>,
    lists: Vec<SavedList>,
}
#[derive(Clone)]
pub struct List {
    pub url: String,
    pub games: usize,
}
fn canonical(value: &str) -> Result<String> {
    crate::registry::validate_url(value)?;
    Ok(url::Url::parse(value)?.to_string())
}
fn validate_games(games: &[Entry]) -> Result<()> {
    Catalog::parse(
        toml::to_string(&Catalog {
            format_version: 1,
            games: games.to_vec(),
        })?
        .as_bytes(),
    )?;
    Ok(())
}
fn merge(state: &State) -> Result<Vec<Entry>> {
    // Local imports take precedence, then the first subscribed list. Metadata
    // never replaces installed signed manifests or changes creator approval.
    let mut games = BTreeMap::new();
    for entry in state
        .local_games
        .iter()
        .chain(state.lists.iter().flat_map(|s| &s.games))
    {
        games
            .entry(entry.id.clone())
            .or_insert_with(|| entry.clone());
    }
    let games: Vec<_> = games.into_values().collect();
    validate_games(&games)?;
    Ok(games)
}
fn load(root: &Path) -> Result<State> {
    let path = root.join(STATE_FILE);
    if !path.try_exists()? {
        return Ok(State {
            format_version: 1,
            local_games: super::legacy_entries(root)?,
            lists: Vec::new(),
        });
    }
    let bytes = crate::files::read_regular(&path, STATE_LIMIT, false)?;
    let state: State = serde_json::from_slice(&bytes).context("invalid saved game lists")?;
    anyhow::ensure!(state.format_version == 1, "unsupported saved list format");
    anyhow::ensure!(state.lists.len() <= MAX_LISTS, "too many saved lists");
    validate_games(&state.local_games)?;
    let mut seen = std::collections::HashSet::new();
    for list in &state.lists {
        anyhow::ensure!(
            canonical(&list.url)? == list.url && seen.insert(&list.url),
            "invalid or duplicate saved list URL"
        );
        validate_games(&list.games)?;
    }
    merge(&state)?;
    Ok(state)
}
struct Lock {
    path: std::path::PathBuf,
    file: Option<fs::File>,
}
impl Drop for Lock {
    fn drop(&mut self) {
        self.file.take();
        let _ = fs::remove_file(&self.path);
    }
}
fn lock(root: &Path) -> Result<Lock> {
    fs::create_dir_all(root)?;
    let path = root.join(".catalog-lock");
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .context("another catalog change is active")?;
    Ok(Lock {
        path,
        file: Some(file),
    })
}
fn write(root: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    let mut staged = tempfile::NamedTempFile::new_in(root)?;
    staged.write_all(bytes)?;
    staged.as_file().sync_all()?;
    staged.persist(root.join(name)).map_err(|e| e.error)?;
    Ok(())
}
fn save(root: &Path, state: &State) -> Result<()> {
    merge(state)?;
    let bytes = serde_json::to_vec(state)?;
    anyhow::ensure!(
        bytes.len() as u64 <= STATE_LIMIT,
        "saved lists exceed 16 MiB"
    );
    write(root, STATE_FILE, &bytes)
}
pub(super) fn entries(root: &Path) -> Result<Vec<Entry>> {
    merge(&load(root)?)
}
pub fn lists(root: &Path) -> Result<Vec<List>> {
    Ok(load(root)?
        .lists
        .into_iter()
        .map(|s| List {
            url: s.url,
            games: s.games.len(),
        })
        .collect())
}
pub(super) fn import_local(root: &Path, incoming: Catalog) -> Result<usize> {
    let _lock = lock(root)?;
    let mut state = load(root)?;
    let mut all: BTreeMap<_, _> = merge(&state)?
        .into_iter()
        .map(|e| (e.id.clone(), e))
        .collect();
    let mut added = 0;
    for entry in incoming.games {
        match all.get(&entry.id) {
            Some(existing) if existing != &entry => {
                bail!("catalog entry conflicts with existing game: {}", entry.id)
            }
            Some(_) => {}
            None => {
                all.insert(entry.id.clone(), entry.clone());
                added += 1;
            }
        }
        if !state.local_games.iter().any(|e| e.id == entry.id) {
            state.local_games.push(entry);
        }
    }
    validate_games(&state.local_games)?;
    if root.join(STATE_FILE).try_exists()? {
        save(root, &state)?;
    } else {
        let bytes = toml::to_string(&Catalog {
            format_version: 1,
            games: state.local_games,
        })?;
        write(root, "catalog.gamepkglist", bytes.as_bytes())?;
    }
    Ok(added)
}
fn apply(root: &Path, value: &str, incoming: Catalog, allow_new: bool) -> Result<usize> {
    validate_games(&incoming.games)?;
    let url = canonical(value)?;
    let _lock = lock(root)?;
    let mut state = load(root)?;
    let before: std::collections::HashSet<_> = merge(&state)?.into_iter().map(|e| e.id).collect();
    if allow_new && !state.lists.iter().any(|s| s.url == url) {
        // Explicitly subscribing assigns matching old imported entries to the
        // URL; unrelated local metadata remains independent.
        state
            .local_games
            .retain(|e| !incoming.games.iter().any(|item| item.id == e.id));
    }
    if let Some(list) = state.lists.iter_mut().find(|s| s.url == url) {
        list.games = incoming.games;
    } else {
        anyhow::ensure!(allow_new, "list was removed while updating");
        anyhow::ensure!(
            state.lists.len() < MAX_LISTS,
            "at most 64 saved lists are supported"
        );
        state.lists.push(SavedList {
            url,
            games: incoming.games,
        });
    }
    let added = merge(&state)?
        .iter()
        .filter(|e| !before.contains(&e.id))
        .count();
    save(root, &state)?;
    Ok(added)
}
/// Remote imports remember the original URL, including GitHub project URLs.
pub fn import_source(root: &Path, value: &str) -> Result<usize> {
    if !value.contains("://") {
        return super::import(root, Path::new(value));
    }
    let url = canonical(value)?;
    let file = crate::registry::download_catalog(&url)?;
    apply(root, &url, Catalog::read(file.path())?, true)
}
pub fn update(root: &Path, value: &str) -> Result<usize> {
    let url = canonical(value)?;
    anyhow::ensure!(
        lists(root)?.iter().any(|s| s.url == url),
        "list is not saved"
    );
    let file = crate::registry::download_catalog(&url)?;
    apply(root, &url, Catalog::read(file.path())?, false)
}
pub fn remove(root: &Path, value: &str) -> Result<()> {
    let url = canonical(value)?;
    let _lock = lock(root)?;
    let mut state = load(root)?;
    state.lists.retain(|s| s.url != url);
    save(root, &state)
}
/// Each failed list keeps its previous snapshot; other lists still update.
pub fn refresh(root: &Path) -> Result<String> {
    let lists = lists(root)?;
    let (mut updated, mut added) = (0, 0);
    let mut failures = Vec::new();
    for list in lists {
        match update(root, &list.url) {
            Ok(count) => {
                updated += 1;
                added += count;
            }
            Err(error) => failures.push(format!("{}: {error:#}", list.url)),
        }
    }
    let mut message = format!("Library refreshed. Updated {updated} lists; added {added} games. No game packages downloaded.");
    if !failures.is_empty() {
        message.push_str(&format!(
            " {} lists could not update; their saved entries were kept. {}",
            failures.len(),
            failures.join("; ")
        ));
    }
    Ok(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn catalog(ids: &[&str], version: &str) -> Catalog {
        Catalog {
            format_version: 1,
            games: ids
                .iter()
                .map(|id| Entry {
                    id: id.to_string(),
                    title: id.to_string(),
                    description: None,
                    download_url: format!("https://games.invalid/{id}.gamepkg"),
                    version: Some(version.into()),
                    requires_rom: Some(false),
                })
                .collect(),
        }
    }
    #[test]
    fn remembered_updates_are_atomic_and_removal_preserves_other_lists_and_installed_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        apply(
            root,
            "https://one.invalid/list",
            catalog(&["shared", "one"], "1.0.0"),
            true,
        )
        .unwrap();
        apply(
            root,
            "https://two.invalid/list",
            catalog(&["shared", "two"], "2.0.0"),
            true,
        )
        .unwrap();
        assert_eq!(lists(root).unwrap().len(), 2);
        assert_eq!(
            entries(root)
                .unwrap()
                .iter()
                .find(|e| e.id == "shared")
                .unwrap()
                .version
                .as_deref(),
            Some("1.0.0")
        );
        apply(
            root,
            "https://one.invalid/list",
            catalog(&["shared", "one", "new"], "3.0.0"),
            false,
        )
        .unwrap();
        assert_eq!(entries(root).unwrap().len(), 4);
        assert_eq!(lists(root).unwrap()[0].games, 3);
        let before = fs::read(root.join(STATE_FILE)).unwrap();
        let mut bad = catalog(&["one"], "1.0.0");
        bad.games[0].download_url = "file:///private".into();
        assert!(apply(root, "https://one.invalid/list", bad, false).is_err());
        assert_eq!(fs::read(root.join(STATE_FILE)).unwrap(), before);
        fs::write(
            root.join("one.gamepkg"),
            b"installed signed archive sentinel",
        )
        .unwrap();
        remove(root, "https://one.invalid/list").unwrap();
        assert_eq!(
            entries(root)
                .unwrap()
                .iter()
                .map(|e| e.id.as_str())
                .collect::<Vec<_>>(),
            vec!["shared", "two"]
        );
        assert_eq!(
            fs::read(root.join("one.gamepkg")).unwrap(),
            b"installed signed archive sentinel"
        );
        assert!(apply(
            root,
            "https://one.invalid/list",
            catalog(&["resurrected"], "1.0.0"),
            false
        )
        .is_err());
        assert_eq!(lists(root).unwrap().len(), 1);
    }
    #[test]
    fn reimport_migrates_matching_legacy_entries_and_remembers_only_one_canonical_url() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        import_local(root, catalog(&["old", "local"], "1.0.0")).unwrap();
        apply(
            root,
            "https://ONE.invalid:443/list",
            catalog(&["old"], "2.0.0"),
            true,
        )
        .unwrap();
        apply(
            root,
            "https://one.invalid/list",
            catalog(&["old", "new"], "2.0.0"),
            true,
        )
        .unwrap();
        assert_eq!(lists(root).unwrap().len(), 1);
        assert_eq!(
            entries(root)
                .unwrap()
                .iter()
                .find(|e| e.id == "old")
                .unwrap()
                .version
                .as_deref(),
            Some("2.0.0")
        );
        remove(root, "https://one.invalid/list").unwrap();
        assert_eq!(
            entries(root)
                .unwrap()
                .iter()
                .map(|e| e.id.as_str())
                .collect::<Vec<_>>(),
            vec!["local"]
        );
        assert!(!root.join(".catalog-lock").exists());
    }
    #[test]
    fn local_import_after_subscription_remains_offline_and_conflicts_preserve_state() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        apply(
            root,
            "https://one.invalid/list",
            catalog(&["one"], "1.0.0"),
            true,
        )
        .unwrap();
        import_local(root, catalog(&["local"], "1.0.0")).unwrap();
        let before = fs::read(root.join(STATE_FILE)).unwrap();
        assert!(import_local(root, catalog(&["local", "new"], "2.0.0")).is_err());
        assert_eq!(fs::read(root.join(STATE_FILE)).unwrap(), before);
        remove(root, "https://one.invalid/list").unwrap();
        assert_eq!(entries(root).unwrap().len(), 1);
        fs::write(root.join(STATE_FILE), b"invalid").unwrap();
        assert!(entries(root).is_err());
    }
}
