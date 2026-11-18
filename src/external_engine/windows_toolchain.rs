//! Select one coherent installed MSVC/Windows SDK for reviewed i686 builds.
//! SDK files stay on the host; no proprietary headers or libraries are bundled.
use anyhow::{ensure, Context, Result};
use std::{
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Default)]
pub(super) struct Environment {
    values: Vec<(OsString, OsString)>,
}
impl Environment {
    pub(super) fn command(&self, tool: &OsStr) -> Command {
        let mut command = Command::new(tool);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        command.envs(self.values.iter().cloned());
        command
    }
    pub(super) fn identity(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        for (key, value) in &self.values {
            bytes.extend_from_slice(format!("{key:?}={value:?}\0").as_bytes());
        }
        bytes
    }
}
fn version(name: &OsStr) -> Option<Vec<u32>> {
    let name = name.to_str()?;
    if name.is_empty() || name.len() > 64 {
        return None;
    }
    let parts: Option<Vec<_>> = name
        .split('.')
        .map(|p| {
            if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            p.parse::<u32>().ok()
        })
        .collect();
    parts.filter(|p| (2..=6).contains(&p.len()))
}
fn versions(root: &Path) -> Vec<PathBuf> {
    let mut rows: Vec<_> = fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| Some((version(&entry.file_name())?, entry.path())))
        .filter(|(_, path)| path.is_dir())
        .collect();
    rows.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    rows.into_iter().map(|(_, path)| path).collect()
}
fn file(directory: &Path, name: &str) -> Result<PathBuf> {
    let mut matches: Vec<_> = fs::read_dir(directory)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|s| s.eq_ignore_ascii_case(name))
        })
        .map(|entry| entry.path())
        .collect();
    ensure!(matches.len() == 1, "missing or ambiguous SDK file {name}");
    let path = matches.pop().unwrap().canonicalize()?;
    ensure!(path.is_file(), "SDK input must be a regular file");
    Ok(path)
}
fn canonical_directory(root: &Path) -> Result<PathBuf> {
    ensure!(root.is_absolute(), "compiler SDK paths must be absolute");
    let root = root.canonicalize()?;
    ensure!(root.is_dir(), "compiler SDK path is not a directory");
    ensure!(
        !root.as_os_str().to_string_lossy().contains(';'),
        "SDK paths cannot contain an INCLUDE/LIB separator"
    );
    // Clang's MSVC environment search does not accept the verbatim prefix
    // returned by canonicalize. Keep executable paths unchanged; normalize
    // only these compiler include/library search arguments.
    #[cfg(windows)]
    let root = crate::takedawn_windows_sdk::compiler_path(&root)?;
    Ok(root)
}
fn joined(paths: &[PathBuf]) -> OsString {
    let mut value = OsString::new();
    for (index, path) in paths.iter().enumerate() {
        if index != 0 {
            value.push(";");
        }
        value.push(path);
    }
    value
}
fn coherent_sdk(root: &Path, requested: Option<&OsStr>) -> Result<(PathBuf, PathBuf)> {
    let candidates = if let Some(requested) = requested {
        let text = requested
            .to_str()
            .context("invalid Windows SDK version")?
            .trim_end_matches(['/', '\\']);
        ensure!(
            version(OsStr::new(text)).is_some(),
            "invalid Windows SDK version"
        );
        vec![root.join("Include").join(text)]
    } else {
        versions(&root.join("Include"))
    };
    for include in candidates {
        let library = root
            .join("Lib")
            .join(include.file_name().context("SDK version")?);
        let complete = file(&include.join("ucrt"), "stdio.h").is_ok()
            && file(&include.join("um"), "windows.h").is_ok()
            && file(&include.join("shared"), "minwindef.h").is_ok()
            && file(&library.join("ucrt/x86"), "libucrt.lib").is_ok()
            && file(&library.join("um/x86"), "kernel32.lib").is_ok();
        if complete {
            return Ok((include, library));
        }
    }
    anyhow::bail!("Install a complete Windows 10/11 SDK with x86 headers and libraries; the selected SDK is incomplete")
}
fn roots(vc: &Path, sdk: &Path, requested: Option<&OsStr>) -> Result<Environment> {
    let vc = canonical_directory(vc)?;
    let sdk = canonical_directory(sdk)?;
    let vc_header =
        file(&vc.join("include"), "vcruntime.h").context("MSVC C/C++ headers are missing")?;
    let vc_library = file(&vc.join("lib/x86"), "libcmt.lib")
        .context("MSVC x86 runtime libraries are missing")?;
    let (include, library) = coherent_sdk(&sdk, requested)?;
    let mut includes = vec![
        vc.join("include"),
        include.join("ucrt"),
        include.join("um"),
        include.join("shared"),
    ];
    for name in ["winrt", "cppwinrt"] {
        let path = include.join(name);
        if path.is_dir() {
            includes.push(path);
        }
    }
    let libraries = vec![
        vc.join("lib/x86"),
        library.join("ucrt/x86"),
        library.join("um/x86"),
    ];
    let sdk_version = include.file_name().unwrap().to_os_string();
    let mut values = vec![
        ("INCLUDE".into(), joined(&includes)),
        ("LIB".into(), joined(&libraries)),
        ("VCToolsInstallDir".into(), vc.into_os_string()),
        ("WindowsSdkDir".into(), sdk.into_os_string()),
        ("WindowsSDKVersion".into(), sdk_version),
        ("VSCMD_ARG_TGT_ARCH".into(), "x86".into()),
    ];
    // Record the selected compiler-runtime identity as well as its path. Ninja
    // tracks the rest of the included SDK headers through ordinary depfiles.
    for (key, path) in [
        ("GAMEPKG_MSVC_HEADER_ID", vc_header),
        ("GAMEPKG_MSVC_RUNTIME_ID", vc_library),
    ] {
        let bytes = crate::files::read_regular(&path, 64 * 1024 * 1024, false)?;
        values.push((key.into(), crate::verify::digest(&bytes).into()));
    }
    Ok(Environment { values })
}
fn installed_vc(program_files: &[PathBuf]) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    for base in program_files {
        let visual = base.join("Microsoft Visual Studio");
        for year in fs::read_dir(visual).into_iter().flatten().flatten() {
            for edition in fs::read_dir(year.path()).into_iter().flatten().flatten() {
                candidates.extend(versions(&edition.path().join("VC/Tools/MSVC")));
            }
        }
    }
    candidates.sort_by(|a, b| {
        version(b.file_name().unwrap())
            .cmp(&version(a.file_name().unwrap()))
            .then_with(|| a.cmp(b))
    });
    candidates.into_iter().find(|p| {
        file(&p.join("include"), "vcruntime.h").is_ok()
            && file(&p.join("lib/x86"), "libcmt.lib").is_ok()
    })
}
pub(super) fn resolve() -> Result<Environment> {
    let program_files: Vec<_> = ["ProgramFiles(x86)", "ProgramFiles"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .collect();
    let vc=std::env::var_os("GAMEPKG_MSVC_TOOLS").or_else(||std::env::var_os("VCToolsInstallDir"))
        .map(PathBuf::from).or_else(||installed_vc(&program_files))
        .context("Install Microsoft C++ Build Tools, or set GAMEPKG_MSVC_TOOLS to its versioned VC/Tools/MSVC directory")?;
    let sdk=std::env::var_os("GAMEPKG_WINDOWS_SDK").or_else(||std::env::var_os("WindowsSdkDir"))
        .map(PathBuf::from).or_else(||program_files.iter().map(|p|p.join("Windows Kits/10")).find(|p|p.is_dir()))
        .context("Install a Windows 10/11 SDK, or set GAMEPKG_WINDOWS_SDK to its Windows Kits/10 directory")?;
    roots(&vc, &sdk, std::env::var_os("WindowsSDKVersion").as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn put(root: &Path, name: &str) {
        let p = root.join(name);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, b"synthetic compiler input").unwrap();
    }
    fn sdk(root: &Path, v: &str) {
        for f in ["ucrt/stdio.h", "um/windows.h", "shared/minwindef.h"] {
            put(root, &format!("Include/{v}/{f}"));
        }
        for f in ["ucrt/x86/libucrt.lib", "um/x86/kernel32.lib"] {
            put(root, &format!("Lib/{v}/{f}"));
        }
    }
    #[test]
    fn selects_complete_numeric_sdk_without_mixing_versions_or_architectures() {
        let temp = tempfile::tempdir().unwrap();
        let vc = temp.path().join("VC with spaces");
        let kit = temp.path().join("SDK with spaces");
        put(&vc, "include/vcruntime.h");
        put(&vc, "lib/x86/LIBCMT.lib");
        sdk(&kit, "10.0.9");
        sdk(&kit, "10.0.10");
        put(&kit, "Include/10.0.11/ucrt/stdio.h");
        let env = roots(&vc, &kit, None).unwrap();
        let includes = env
            .values
            .iter()
            .find(|(k, _)| k == "INCLUDE")
            .unwrap()
            .1
            .to_string_lossy();
        assert!(includes.contains("10.0.10"));
        assert!(!includes.contains("10.0.9"));
        assert!(!includes.contains("10.0.11"));
        assert!(roots(&vc, &kit, Some(OsStr::new("10.0.11"))).is_err());
        assert!(roots(&vc, &kit, Some(OsStr::new("../10.0.10"))).is_err());
        assert!(roots(&vc, &kit, Some(OsStr::new("10.0.9\\"))).is_ok());
        fs::remove_file(vc.join("lib/x86/LIBCMT.lib")).unwrap();
        put(&vc, "lib/x64/LIBCMT.lib");
        assert!(roots(&vc, &kit, None).is_err());
    }
    #[test]
    fn runtime_content_and_sdk_version_change_preparation_identity() {
        let temp = tempfile::tempdir().unwrap();
        let vc = temp.path().join("VC");
        let kit = temp.path().join("SDK");
        put(&vc, "include/vcruntime.h");
        put(&vc, "lib/x86/libcmt.lib");
        sdk(&kit, "10.0.1");
        let a = roots(&vc, &kit, None).unwrap().identity();
        fs::write(vc.join("lib/x86/libcmt.lib"), b"updated runtime").unwrap();
        let b = roots(&vc, &kit, None).unwrap().identity();
        assert_ne!(a, b);
        sdk(&kit, "10.0.2");
        assert_ne!(b, roots(&vc, &kit, None).unwrap().identity());
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "Requires an installed Windows SDK, MSVC runtime and native clang/lld"]
    fn installed_sdk_compiles_links_and_runs_i686() {
        let environment = resolve().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("native-sdk-check.c");
        let executable = temp.path().join("native-sdk-check.exe");
        fs::write(&source, b"#include <stdint.h>\n#include <stdio.h>\n#include <windows.h>\n_Static_assert(sizeof(void*)==4,\"i686 ABI\");\n_Static_assert(sizeof(wchar_t)==2,\"Windows wchar ABI\");\nint main(void){printf(\"PASS native i686 SDK and runtime %lu\\n\",GetCurrentProcessId());return 0;}\n").unwrap();
        let output = environment
            .command(OsStr::new("clang"))
            .args([
                "--no-default-config",
                "--target=i686-pc-windows-msvc",
                "-fuse-ld=lld",
                "-std=c11",
            ])
            .arg(&source)
            .args(["-lkernel32", "-o"])
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = environment
            .command(executable.as_os_str())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("PASS native i686 SDK and runtime")
        );
    }
}
