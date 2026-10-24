//! Host-only snapshot ownership and fixed reservations inside the game ceiling.
use crate::{
    manifest::Manifest,
    memory_budget::{Budget, Lease},
};
use anyhow::{ensure, Context as _, Result};
mod slots;
pub(crate) use slots::slot_names;
pub(crate) const SIZE: usize = 275497;
pub(crate) const CHUNK: usize = 65536;
pub(crate) const HOST_MEMORY: u64 = 1048576;
const SCRATCH: u64 = (2 * CHUNK + 4096) as u64;
const METADATA: u64 = 512;
/// Owned host snapshot. Games cannot access this type through the injected SDK.
pub struct Snapshot {
    pub(crate) bytes: Vec<u8>,
    pub(crate) state: u64,
    owner: [u8; 32],
    _lease: Lease,
}
impl Snapshot {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn state(&self) -> u64 {
        self.state
    }
}
pub(crate) struct Context {
    budget: Budget,
    owner: [u8; 32],
    _metadata: Lease,
    schema: u32,
}
pub(crate) fn guest_memory(manifest: &Manifest, memory: u64) -> Result<u64> {
    if manifest.builtin_engine.as_deref() != Some(crate::zelda3::PROFILE)
        || !manifest.permissions.save
    {
        return Ok(memory);
    }
    memory
        .checked_sub(HOST_MEMORY)
        .filter(|n| *n >= 65536)
        .context("Zelda snapshot reservation exceeds the game memory ceiling")
}
impl Context {
    pub(crate) fn new(manifest: &Manifest) -> Result<Option<Self>> {
        if manifest.builtin_engine.as_deref() != Some(crate::zelda3::PROFILE)
            || !manifest.permissions.save
        {
            return Ok(None);
        }
        use sha2::{Digest, Sha256};
        let key =
            hex::decode(&manifest.creator_key).context("invalid snapshot creator identity")?;
        ensure!(key.len() == 32, "invalid snapshot creator identity");
        ensure!(manifest.save_schema > 0, "invalid snapshot save schema");
        let mut hash = Sha256::new();
        hash.update((manifest.id.len() as u64).to_le_bytes());
        hash.update(manifest.id.as_bytes());
        hash.update(key);
        hash.update(manifest.save_schema.to_le_bytes());
        let owner = hash.finalize().into();
        let budget = Budget::new(HOST_MEMORY);
        let metadata = budget.reserve(METADATA)?;
        Ok(Some(Self {
            budget,
            owner,
            _metadata: metadata,
            schema: manifest.save_schema,
        }))
    }
    pub(crate) fn allocate(&self, state: u64) -> Result<Snapshot> {
        let lease = self
            .budget
            .reserve((SIZE + std::mem::size_of::<Snapshot>()) as u64)
            .context("snapshot memory budget exceeded; release an earlier snapshot")?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(SIZE)
            .context("unable to allocate snapshot buffer")?;
        bytes.resize(SIZE, 0);
        Ok(Snapshot {
            bytes,
            state,
            owner: self.owner,
            _lease: lease,
        })
    }
    pub(crate) fn scratch(&self) -> Result<Lease> {
        self.budget.reserve(SCRATCH)
    }
    pub(crate) fn incoming(&self, snapshot: &Snapshot) -> Result<Option<Lease>> {
        ensure!(
            snapshot.owner == self.owner,
            "snapshot belongs to a different game, creator or save schema"
        );
        ensure!(snapshot.bytes.len() == SIZE, "invalid Zelda snapshot size");
        if self.budget.owns(&snapshot._lease) {
            Ok(None)
        } else {
            self.budget
                .reserve((SIZE + std::mem::size_of::<Snapshot>()) as u64)
                .map(Some)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn manifest(key: u8) -> Manifest {
        let mut m = Manifest::parse(include_bytes!("../ports/zelda3/manifest.toml")).unwrap();
        m.creator_key = hex::encode([key; 32]);
        m
    }
    #[test]
    fn snapshots_and_transfer_scratch_share_the_reserved_ceiling_and_release() {
        let m = manifest(1);
        let context = Context::new(&m).unwrap().unwrap();
        assert_eq!(guest_memory(&m, 32 * 1048576).unwrap(), 31 * 1048576);
        assert!(guest_memory(&m, HOST_MEMORY).is_err());
        let a = context.allocate(7).unwrap();
        let b = context.allocate(8).unwrap();
        let c = context.allocate(9).unwrap();
        let scratch = context.scratch().unwrap();
        assert!(context.allocate(10).is_err());
        assert_eq!(a.bytes().len(), SIZE);
        assert_eq!(a.state(), 7);
        drop(b);
        assert!(context.allocate(10).is_ok());
        drop(scratch);
        drop(a);
        drop(c);
        assert_eq!(context.budget.used(), METADATA);
    }
    #[test]
    fn creator_game_identity_and_cross_session_borrows_are_accounted() {
        let a = Context::new(&manifest(1)).unwrap().unwrap();
        let snapshot = a.allocate(19).unwrap();
        assert!(a.incoming(&snapshot).unwrap().is_none());
        let b = Context::new(&manifest(1)).unwrap().unwrap();
        let before = b.budget.used();
        let borrow = b.incoming(&snapshot).unwrap().unwrap();
        assert!(b.budget.used() > before);
        drop(borrow);
        assert_eq!(b.budget.used(), before);
        let different = Context::new(&manifest(2)).unwrap().unwrap();
        assert!(different.incoming(&snapshot).is_err());
        let mut schema = manifest(1);
        schema.save_schema = 2;
        assert!(Context::new(&schema)
            .unwrap()
            .unwrap()
            .incoming(&snapshot)
            .is_err());
        let mut m = manifest(1);
        m.id = "com.gamepkg.other".into();
        assert!(Context::new(&m)
            .unwrap()
            .unwrap()
            .incoming(&snapshot)
            .is_err());
        let root = a.budget.clone();
        drop(a);
        assert!(root.used() > 0);
        drop(snapshot);
        assert_eq!(root.used(), 0);
        m.builtin_engine = None;
        assert!(Context::new(&m).unwrap().is_none());
        assert_eq!(guest_memory(&m, 1234).unwrap(), 1234);
    }
    #[test]
    fn save_permission_identity_and_incoming_extent_are_checked_before_reserving() {
        let mut m = manifest(1);
        m.permissions.save = false;
        assert!(Context::new(&m).unwrap().is_none());
        assert_eq!(guest_memory(&m, 32 * 1048576).unwrap(), 32 * 1048576);
        m.permissions.save = true;
        m.creator_key = "invalid".into();
        assert!(Context::new(&m).is_err());
        m.creator_key = "ab".repeat(31);
        assert!(Context::new(&m).is_err());
        let context = Context::new(&manifest(1)).unwrap().unwrap();
        let mut snapshot = context.allocate(1).unwrap();
        snapshot.bytes.pop();
        let before = context.budget.used();
        assert!(context.incoming(&snapshot).is_err());
        assert_eq!(context.budget.used(), before);
    }
    #[test]
    #[ignore = "requires the signed built Zelda3 fixture and player-selected ROM"]
    fn actual_corrupt_snapshots_preserve_running_game_and_wrapper_state() {
        let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_ZELDA3_TEST_ROOT").unwrap());
        let rom = std::path::PathBuf::from(std::env::var_os("GAMEPKG_ZELDA3_TEST_ROM").unwrap());
        let native = match std::env::var("GAMEPKG_ZELDA3_TEST_TARGET")
            .unwrap()
            .as_str()
        {
            "native" => true,
            "wasm" => false,
            _ => panic!("invalid test runtime"),
        };
        let mut game =
            crate::runtime::Game::load_target(&root, "com.gamepkg.zelda3", &[rom], Some(native))
                .unwrap();
        for _ in 0..200 {
            game.step(0).unwrap();
        }
        let good = game.capture_zelda_snapshot().unwrap();
        let mut bad = game.capture_zelda_snapshot().unwrap();
        let diagnostic = game.builtin_state().unwrap();
        for offset in [0, 8, 16, 24, 32 + 27] {
            bad.bytes[offset] ^= 0x80;
            assert!(
                game.restore_zelda_snapshot(&bad).is_err(),
                "accepted corrupt offset {offset}"
            );
            bad.bytes[offset] ^= 0x80;
            assert_eq!(game.state, good.state());
            assert_eq!(game.builtin_state().unwrap(), diagnostic);
            assert_eq!(game.capture_zelda_snapshot().unwrap().bytes(), good.bytes());
        }
        bad.owner[0] ^= 1;
        assert!(game.restore_zelda_snapshot(&bad).is_err());
        bad.owner[0] ^= 1;
        bad.bytes.pop();
        assert!(game.restore_zelda_snapshot(&bad).is_err());
        assert_eq!(game.state, good.state());
        game.step(0).unwrap();
        assert_eq!(game.state, 201);
        game.finish().unwrap();
    }
}
