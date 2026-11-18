//! Trusted Linux compatibility entry used by the checkout-backed builder.
#![allow(unexpected_cfgs)]
#[cfg(not(gamepkg_linked_runtime))]
fn main() -> anyhow::Result<()> {
    anyhow::bail!("native service runtime requires separately audited game objects")
}

#[cfg(gamepkg_linked_runtime)]
#[allow(dead_code)]
#[path = "../../engines/native-services/client.rs"]
mod client;
#[cfg(gamepkg_linked_runtime)]
#[allow(dead_code)]
#[path = "../../engines/native-services/local_bindings.rs"]
mod local;
#[cfg(gamepkg_linked_runtime)]
#[path = "../../engines/native-services/lockdown.rs"]
mod lockdown;
#[cfg(gamepkg_linked_runtime)]
include!(env!("GAMEPKG_LOCAL_RUNTIME_CONFIG"));

#[cfg(gamepkg_linked_runtime)]
fn main() -> anyhow::Result<()> {
    use anyhow::ensure;
    use clap::Parser;
    use gamepkg::launch_options::{AspectRatio, WindowMode};
    use gamepkg::port_host::{
        owner::{Grants, SaveGrant},
        player::{self, Options},
        process::NativeLaunch,
    };
    use std::io::Write;
    use std::os::{
        fd::{AsFd, AsRawFd},
        unix::fs::MetadataExt,
    };

    ensure!(
        gamepkg::port_host::build_identity() == SDK,
        "incompatible linked service SDK"
    );
    let manifest = gamepkg::manifest::Manifest::parse(MANIFEST.as_bytes())?;
    let args = NativeLaunch::parse();
    let launch = args.graphics.resolved(manifest.launch_options)?;
    let unit = std::env::var("GAMEPKG_MEMORY_SCOPE")?;
    let cgroup = std::fs::read_to_string("/proc/self/cgroup")?;
    ensure!(
        unit.starts_with("gamepkg-native-")
            && unit.ends_with(".scope")
            && cgroup
                .lines()
                .any(|line| line.rsplit('/').next() == Some(unit.as_str())),
        "native executable requires the GamePKG memory scope"
    );
    let parent_netns = std::env::var("GAMEPKG_PARENT_NETWORK_NS")?.parse::<u64>()?;
    let own_netns = std::fs::metadata("/proc/self/ns/net")?.ino();
    ensure!(
        (parent_netns == own_netns) == manifest.permissions.network,
        "native process network namespace differs from signed permissions"
    );
    ensure!(
        gamepkg::verify::digest(PIPELINES) == PIPELINE_PAYLOAD_HASH,
        "embedded pipeline payload differs from the linked build"
    );
    let pipelines = serde_json::from_slice(PIPELINES)?;
    let shader_hashes = SHADER_HASHES
        .iter()
        .map(|hash| (*hash).to_owned())
        .collect::<Vec<_>>();
    let mut grants = Grants::default();
    if std::env::var("GAMEPKG_ASSET_GRANT").as_deref() == Ok("1") {
        grants.assets = Some(cap_std::fs::Dir::open_ambient_dir(
            "/run/gamepkg-assets",
            cap_std::ambient_authority(),
        )?);
    }
    if manifest.native_tick && std::path::Path::new("/run/gamepkg-audio").is_file() {
        let mut package =
            gamepkg::archive::Package::open(std::path::Path::new("/run/gamepkg-audio"))?;
        gamepkg::verify::verify_embedded(&mut package)?;
        ensure!(
            hex::encode(gamepkg::verify::tree_hash(&mut package)?) == SOURCE_KIT,
            "recorded audio differs from the audited source kit"
        );
        grants.recorded_audio = Some(package);
    }
    if manifest.native_tick && std::env::var("GAMEPKG_FILE_PICKER").as_deref() == Ok("1") {
        grants.picker = Some(std::os::unix::net::UnixStream::from(
            std::io::stdin().as_fd().try_clone_to_owned()?,
        ));
    }
    if let Some(quota) = std::env::var_os("GAMEPKG_SAVE_QUOTA") {
        let quota = quota
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("invalid save quota"))?
            .parse()?;
        grants.saves = Some(SaveGrant::open_native_mount(&manifest, quota)?);
    }
    match manifest.required_rom_count() {
        0 => {}
        1 => {
            let file = std::fs::File::from(std::io::stdin().as_fd().try_clone_to_owned()?);
            let flags = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFL) };
            ensure!(
                flags >= 0 && flags & (libc::O_ACCMODE | libc::O_PATH) == libc::O_RDONLY,
                "ROM descriptor must be read-only"
            );
            grants.roms.push(file);
        }
        _ => anyhow::bail!("this bootstrap supports one inherited ROM descriptor"),
    }
    let rom_descriptor = grants.roms.first().map(AsRawFd::as_raw_fd);
    let options = Options {
        title: manifest
            .title
            .clone()
            .unwrap_or_else(|| manifest.id.clone()),
        visible: args.visible,
        fullscreen: launch.window_mode == Some(WindowMode::Fullscreen),
        width: if launch.aspect_ratio == Some(AspectRatio::Standard) {
            960
        } else {
            1280
        },
        mute_audio: args.mute_audio,
        volume: args.volume,
        samples: SAMPLES,
        process_memory: PROCESS_MEMORY,
        launch,
        ..Options::default()
    };
    // Claim fixed address windows before the graphics driver starts.
    let runtime =
        local_runtime(PROCESS_MEMORY).map_err(|e| anyhow::anyhow!("local runtime: {e:?}"))?;
    let report = player::run_local(
        manifest,
        pipelines,
        &shader_hashes,
        grants,
        options,
        move || {
            let mut output = std::io::stdout().lock();
            writeln!(output, "GamePKG presented {unit}")?;
            output.flush()?;
            Ok(())
        },
        move |connection| {
            lockdown::install_local_worker()?;
            if let Some(fd) = rom_descriptor {
                let status = unsafe { libc::fcntl(fd, libc::F_GETFD) };
                ensure!(
                    status == -1
                        && std::io::Error::last_os_error().raw_os_error() == Some(libc::EBADF),
                    "trusted ROM descriptor remained open on the game worker"
                );
            }
            client::connect_local(move |op, bytes, limit| {
                connection
                    .call(op, bytes, limit)
                    .map_err(|error| error as i32)
            })?;
            let result = (|| {
                local::install(runtime).map_err(|e| anyhow::anyhow!("local binding: {e:?}"))?;
                unsafe { game_service_entry() };
                Ok(())
            })();
            local::shutdown();
            client::disconnect();
            result
        },
    )?;
    println!("{report:?}");
    Ok(())
}
