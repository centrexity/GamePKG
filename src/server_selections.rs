//! Explicit host-user networking choices for a signed reviewed engine selector.
use crate::manifest::{Manifest, Permissions};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, io::Write, path::Path};
const MAX_BYTES: u64 = 1048576;
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    #[serde(default)]
    pub server: String,
    #[serde(default)]
    pub master: String,
    #[serde(default)]
    pub listen: String,
    #[serde(default)]
    pub allow_local: bool,
}
impl Selection {
    fn validated(&self) -> Result<Permissions> {
        ensure!(
            self.server.len() <= 256
                && self.listen.len() <= 256
                && self.master.len() <= 256
                && !self
                    .server
                    .chars()
                    .chain(self.listen.chars())
                    .chain(self.master.chars())
                    .any(char::is_control),
            "invalid multiplayer address"
        );
        let mut policy =
            crate::network::selected_multiplayer(&self.server, &self.listen, self.allow_local)?;
        if !self.master.is_empty() {
            crate::network::selected_multiplayer(&self.master, "", self.allow_local)?;
            policy.network = "multiplayer".into();
            policy.network_allow_local = self.allow_local;
        }
        Ok(policy)
    }
    fn normalized(&self) -> Result<Self> {
        ensure!(
            !self
                .server
                .chars()
                .chain(self.listen.chars())
                .chain(self.master.chars())
                .any(char::is_control),
            "invalid multiplayer address"
        );
        let selection = Self {
            server: self.server.trim().into(),
            master: self.master.trim().into(),
            listen: self.listen.trim().into(),
            allow_local: self.allow_local,
        };
        let p = selection.validated()?;
        let master = if selection.master.is_empty() {
            String::new()
        } else {
            crate::network::selected_multiplayer(&selection.master, "", selection.allow_local)?
                .network_hosts[0]
                .clone()
        };
        Ok(Self {
            server: p.network_hosts.first().cloned().unwrap_or_default(),
            master,
            listen: p.network_listen.first().cloned().unwrap_or_default(),
            allow_local: p.network_allow_local,
        })
    }
    /// Commands contain only canonical DNS/IPv4 hostnames and decimal ports.
    /// They are passed directly as an engine argument, never through a shell.
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn commands(&self) -> Result<String> {
        let normalized = self.normalized()?;
        let mut commands = String::new();
        if !normalized.listen.is_empty() {
            let (host, port) = normalized.listen.rsplit_once(':').unwrap();
            commands = format!("; serverip \"{host}\"; serverport {port}; startlistenserver 0");
        }
        if !normalized.server.is_empty() {
            let (host, port) = normalized.server.rsplit_once(':').unwrap();
            commands = format!("; connect \"{host}\" {port}");
        }
        if !normalized.master.is_empty() {
            let (host, port) = normalized.master.rsplit_once(':').unwrap();
            commands = format!("; mastername \"{host}\"; masterport {port}{commands}");
        }
        Ok(commands)
    }
}

pub fn identity(manifest: &Manifest) -> Result<String> {
    ensure!(
        manifest.uses_selected_servers(),
        "this signed game does not allow user-selected servers"
    );
    crate::network::validate_user_selected(&manifest.permissions)?;
    Ok(crate::verify::digest(&serde_json::to_vec(&(
        &manifest.id,
        &manifest.creator_key,
        &manifest.builtin_engine,
        &manifest.game_data,
        &manifest.permissions,
    ))?))
}
fn read(root: &Path) -> Result<BTreeMap<String, Selection>> {
    let path = root.join("network-selections.json");
    let bytes = match crate::files::read_regular(&path, MAX_BYTES, false) {
        Ok(bytes) => bytes,
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
        {
            return Ok(BTreeMap::new())
        }
        Err(error) => return Err(error),
    };
    let entries: BTreeMap<String, Selection> = serde_json::from_slice(&bytes)?;
    ensure!(entries.len() <= 1024, "too many remembered network choices");
    for (key, selection) in &entries {
        ensure!(
            key.len() == 64 && key.bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid network choice identity"
        );
        selection.validated()?;
    }
    Ok(entries)
}
pub fn load(root: &Path, manifest: &Manifest) -> Result<Selection> {
    let key = identity(manifest)?;
    Ok(read(root)?.remove(&key).unwrap_or_default())
}
pub fn remember(root: &Path, manifest: &Manifest, selection: &Selection) -> Result<Selection> {
    let key = identity(manifest)?;
    let selection = selection.normalized()?;
    let mut entries = read(root)?;
    if selection.server.is_empty() && selection.listen.is_empty() && selection.master.is_empty() {
        entries.remove(&key);
    } else {
        entries.insert(key, selection.clone());
    }
    ensure!(entries.len() <= 1024, "too many remembered network choices");
    let bytes = serde_json::to_vec_pretty(&entries)?;
    ensure!(
        bytes.len() as u64 <= MAX_BYTES,
        "network choices exceed their quota"
    );
    let metadata = root.symlink_metadata()?;
    ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "library must be a real directory"
    );
    let mut temp = tempfile::NamedTempFile::new_in(root)?;
    temp.write_all(&bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(root.join("network-selections.json"))
        .map_err(|error| error.error)?;
    Ok(selection)
}
pub fn remember_for_game(root: &Path, id: &str, selection: &Selection) -> Result<()> {
    let _change = crate::registry::LibraryChange::acquire(root)?;
    let manifest = crate::registry::installed_manifest(root, id)?;
    remember(root, &manifest, selection)?;
    Ok(())
}
#[cfg(any(target_os = "linux", test))]
pub(crate) fn effective(root: &Path, manifest: &Manifest) -> Result<(Permissions, String)> {
    if !manifest.uses_selected_servers() {
        return Ok((manifest.permissions.clone(), String::new()));
    }
    let selection = load(root, manifest)?;
    let mut permissions = selection.validated()?;
    permissions.input = manifest.permissions.input;
    permissions.audio = manifest.permissions.audio;
    permissions.save = manifest.permissions.save;
    Ok((permissions, selection.commands()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn manifest() -> Manifest {
        Manifest::parse(include_bytes!("../ports/cube2/manifest.toml")).unwrap()
    }
    fn join() -> Selection {
        Selection {
            server: "127.0.0.1:28785".into(),
            master: String::new(),
            listen: String::new(),
            allow_local: true,
        }
    }
    #[test]
    fn master_choices_are_explicit_scoped_and_do_not_become_static_udp_grants() {
        let root = tempfile::tempdir().unwrap();
        let m = manifest();
        let choice = Selection {
            master: "Master.Example:28787".into(),
            ..Default::default()
        };
        let saved = remember(root.path(), &m, &choice).unwrap();
        assert_eq!(saved.master, "master.example:28787");
        let (p, commands) = effective(root.path(), &m).unwrap();
        assert_eq!(p.network, "multiplayer");
        assert!(p.network_hosts.is_empty() && p.network_listen.is_empty());
        assert_eq!(
            commands,
            "; mastername \"master.example\"; masterport 28787"
        );
        for master in [
            "127.0.0.1:28787",
            "0.0.0.0:28787",
            "x:0",
            "x:65535",
            "x:1;quit",
            "x:1\n",
            "x:1/path",
            "*.example:1",
        ] {
            assert!(
                remember(
                    root.path(),
                    &m,
                    &Selection {
                        master: master.into(),
                        ..Default::default()
                    }
                )
                .is_err(),
                "accepted {master}"
            );
            assert_eq!(load(root.path(), &m).unwrap(), saved);
        }
        let local = Selection {
            master: "127.0.0.1:28787".into(),
            allow_local: true,
            ..Default::default()
        };
        remember(root.path(), &m, &local).unwrap();
        assert!(effective(root.path(), &m).unwrap().0.network_allow_local);
        remember(root.path(), &m, &Selection::default()).unwrap();
        assert_eq!(load(root.path(), &m).unwrap(), Selection::default());
    }
    #[test]
    fn offline_is_default_and_selected_ports_are_explicit() {
        let root = tempfile::tempdir().unwrap();
        let m = manifest();
        let (p, commands) = effective(root.path(), &m).unwrap();
        assert_eq!(p.network, "none");
        assert!(p.network_hosts.is_empty());
        assert!(commands.is_empty());
        remember(root.path(), &m, &join()).unwrap();
        let (p, commands) = effective(root.path(), &m).unwrap();
        assert_eq!(p.network, "multiplayer");
        assert_eq!(p.network_hosts, ["127.0.0.1:28785", "127.0.0.1:28786"]);
        assert!(p.network_listen.is_empty());
        assert_eq!(commands, "; connect \"127.0.0.1\" 28785");
        assert!(p.input && p.audio && p.save);
        let hosting = Selection {
            server: String::new(),
            master: String::new(),
            listen: "127.0.0.1:28785".into(),
            allow_local: true,
        };
        remember(root.path(), &m, &hosting).unwrap();
        let (p, commands) = effective(root.path(), &m).unwrap();
        assert!(p.network_hosts.is_empty());
        assert_eq!(p.network_listen, ["127.0.0.1:28785", "127.0.0.1:28786"]);
        assert_eq!(
            commands,
            "; serverip \"127.0.0.1\"; serverport 28785; startlistenserver 0"
        );
        remember(root.path(), &m, &Selection::default()).unwrap();
        assert_eq!(load(root.path(), &m).unwrap(), Selection::default());
    }
    #[test]
    fn choices_are_scoped_to_library_game_creator_and_signed_declaration() {
        let root = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let m = manifest();
        remember(root.path(), &m, &join()).unwrap();
        assert_eq!(load(root.path(), &m).unwrap(), join());
        assert_eq!(load(other.path(), &m).unwrap(), Selection::default());
        for field in ["id", "creator", "declaration"] {
            let mut changed = m.clone();
            match field {
                "id" => changed.id = "com.gamepkg.other".into(),
                "creator" => changed.creator_key = "other".into(),
                _ => changed.game_data.as_mut().unwrap().title = "Different declaration".into(),
            };
            assert_eq!(load(root.path(), &changed).unwrap(), Selection::default());
        }
        let mut changed = m.clone();
        changed.version = "0.1.1".into();
        assert_eq!(load(root.path(), &changed).unwrap(), join());
        changed.game_data.as_mut().unwrap().user_selected_servers = false;
        changed.permissions.network = "none".into();
        assert!(remember(root.path(), &changed, &join()).is_err());
        assert_eq!(effective(root.path(), &changed).unwrap().0.network, "none");
    }
    #[test]
    fn address_attacks_and_unauthorized_local_access_do_not_replace_good_choices() {
        let root = tempfile::tempdir().unwrap();
        let m = manifest();
        remember(root.path(), &m, &join()).unwrap();
        let path = root.path().join("network-selections.json");
        let before = std::fs::read(&path).unwrap();
        for address in [
            "127.0.0.1",
            "127.0.0.1:0",
            "127.0.0.1:65535",
            "127.0.0.1:65536",
            "u@host:28785",
            "host:28785/path",
            "host:28785?x=1",
            "host:28785#x",
            "host:28785;quit",
            "host:28785\n",
            "host:28785\0",
            "\";quit;\":28785",
            "*.example:28785",
            "[::1]:28785",
            "0.0.0.0:28785",
            "224.0.0.1:28785",
            "255.255.255.255:28785",
            "$(touch x):28785",
            "`x`:28785",
        ] {
            assert!(
                remember(
                    root.path(),
                    &m,
                    &Selection {
                        server: address.into(),
                        master: String::new(),
                        listen: String::new(),
                        allow_local: true
                    }
                )
                .is_err(),
                "accepted {address:?}"
            );
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }
        let mut denied = join();
        denied.allow_local = false;
        assert!(remember(root.path(), &m, &denied).is_err());
        denied = join();
        denied.listen = denied.server.clone();
        assert!(remember(root.path(), &m, &denied).is_err());
        denied = Selection {
            server: String::new(),
            master: String::new(),
            listen: "example.com:28785".into(),
            allow_local: false,
        };
        assert!(remember(root.path(), &m, &denied).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
    #[test]
    fn corrupt_oversized_and_linked_settings_fail_closed() {
        let root = tempfile::tempdir().unwrap();
        let m = manifest();
        let path = root.path().join("network-selections.json");
        for bytes in [
            b"not-json".to_vec(),
            b"{\"bad\":{\"server\":\"x:1\",\"extra\":true}}".to_vec(),
            vec![b' '; MAX_BYTES as usize + 1],
        ] {
            std::fs::write(&path, &bytes).unwrap();
            assert!(load(root.path(), &m).is_err());
            assert!(remember(root.path(), &m, &join()).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }
        #[cfg(unix)]
        {
            let outside = root.path().join("private.json");
            std::fs::write(&outside, b"{}").unwrap();
            std::fs::remove_file(&path).unwrap();
            std::os::unix::fs::symlink(&outside, &path).unwrap();
            assert!(load(root.path(), &m).is_err());
            assert!(remember(root.path(), &m, &join()).is_err());
            assert_eq!(std::fs::read(outside).unwrap(), b"{}");
        }
    }
}
