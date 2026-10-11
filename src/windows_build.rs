//! Opt-in WSL 2 transport for the verified private compiler exchange.
#[cfg(windows)]
use anyhow::Context;
use anyhow::{bail, Result};

fn require_wsl2(list: &str, distribution: &str) -> Result<()> {
    let clean = list.replace('\0', "");
    let matches: Vec<_> = clean
        .lines()
        .filter_map(|line| {
            let line = line.trim().trim_start_matches('*').trim();
            let suffix = line.strip_prefix(distribution)?;
            if !suffix.starts_with(char::is_whitespace) {
                return None;
            }
            suffix.split_whitespace().last()
        })
        .collect();
    if matches != ["2"] {
        bail!("selected compiler distribution must exist exactly once and use WSL 2");
    }
    Ok(())
}

#[cfg(windows)]
pub(crate) fn compile(
    root: &std::path::Path,
    id: &str,
    held: Option<&crate::registry::LibraryChange>,
) -> Result<()> {
    use std::{
        path::{Path, PathBuf},
        process::Command,
        time::Duration,
    };
    let settings = crate::settings::load()?;
    settings.validate()?;
    if settings.windows_compiler_distribution.is_empty() {
        bail!("Windows builds require an explicitly configured WSL 2 compiler; isolated builds currently require Linux and bubblewrap");
    }
    let owned = if held.is_none() {
        Some(crate::registry::LibraryChange::acquire(root)?)
    } else {
        None
    };
    let guard = held
        .or(owned.as_ref())
        .context("compiler requires a library change guard")?;
    let wsl = PathBuf::from(
        std::env::var_os("SystemRoot").context("Windows system folder is unavailable")?,
    )
    .join("System32/wsl.exe");
    let invoke = |arguments: &[&str], seconds| -> Result<String> {
        let mut command = Command::new(&wsl);
        command.args(arguments);
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
        let output = crate::compiler::run_timed(&mut command, Duration::from_secs(seconds))
            .context("WSL compiler launch failed")?;
        if !output.status.success() {
            bail!("WSL compiler failed: {}{}", output.stdout, output.stderr);
        }
        Ok(output.stdout)
    };
    require_wsl2(
        &invoke(&["--list", "--verbose"], 30)?,
        &settings.windows_compiler_distribution,
    )?;
    let distro = settings.windows_compiler_distribution.as_str();
    let executable = settings.windows_compiler_executable.as_str();
    let version = invoke(
        &["--distribution", distro, "--exec", executable, "--version"],
        30,
    )?;
    if version.trim() != format!("gamepkg {}", env!("CARGO_PKG_VERSION")) {
        bail!("WSL compiler must use the same GamePKG version as the native host");
    }
    let workspace = tempfile::tempdir()?;
    let request = workspace.path().join("request.gamepkgstore");
    let response = workspace.path().join("response");
    crate::build_worker::prepare_with_guard(root, id, &request, guard)?;
    let translate = |path: &Path| -> Result<String> {
        let windows = path
            .to_str()
            .context("compiler workspace path is not Unicode")?;
        let result = invoke(
            &[
                "--distribution",
                distro,
                "--exec",
                "wslpath",
                "-a",
                "-u",
                windows,
            ],
            30,
        )?;
        let path = result.trim();
        if !path.starts_with('/') || path.chars().any(char::is_control) {
            bail!("WSL returned an invalid workspace path");
        }
        Ok(path.to_owned())
    };
    let linux_request = translate(&request)?;
    let linux_response = translate(&response)?;
    let mut arguments = vec![
        "--distribution",
        distro,
        "--exec",
        "timeout",
        "180",
        executable,
        "build-worker",
        &linux_request,
        &linux_response,
        "--shader-backend",
        &settings.shader_backend,
        "--copy-output",
    ];
    if !settings.optimize_builds {
        arguments.push("--no-optimize");
    }
    if settings.approved_crates.is_empty() {
        arguments.push("--no-approved-crates");
    }
    invoke(&arguments, 210)?;
    if crate::settings::load()? != settings {
        bail!("settings changed during compilation; compiler response was not published");
    }
    crate::build_worker::accept_with_guard(root, id, &response, guard)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_distribution_requires_an_unambiguous_version_two_row() {
        require_wsl2(
            " NAME STATE VERSION\n* Ubuntu Running 2\n Other Stopped 1",
            "Ubuntu",
        )
        .unwrap();
        require_wsl2(
            &"Ubuntu Running 2"
                .chars()
                .flat_map(|c| [c, '\0'])
                .collect::<String>(),
            "Ubuntu",
        )
        .unwrap();
        for listing in [
            "Ubuntu Running 1",
            "Ubuntu-other Running 2",
            "Ubuntu Running 2\nUbuntu Stopped 2",
            "Ubuntu Running",
            "",
        ] {
            assert!(require_wsl2(listing, "Ubuntu").is_err());
        }
    }
}
