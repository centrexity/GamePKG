//! Shared, pinned native engine sources and reviewed adaptations.
use crate::verify::digest;
use anyhow::{ensure, Context, Result};
use std::{fs, path::Path};

pub(super) struct Plan {
    pub label: &'static str,
    pub sources: Vec<String>,
    pub includes: Vec<String>,
    pub definitions: &'static [&'static str],
    pub configuration: &'static str,
    pub wraps: &'static [&'static str],
}
const DOOM_WRAPS: &[&str] = &[
    "fopen",
    "fclose",
    "fread",
    "fwrite",
    "fseek",
    "ftell",
    "feof",
    "ferror",
    "fflush",
    "fgetc",
    "fputc",
    "fprintf",
    "fgets",
    "fscanf",
    "__isoc99_fscanf",
    "remove",
    "rename",
    "mkdir",
    "getenv",
    "access",
    "printf",
    "puts",
    "putchar",
];
const ZELDA_WRAPS: &[&str] = &[
    "printf", "puts", "putchar", "fprintf", "fopen", "fread", "fwrite", "fclose", "getenv",
];
pub(super) fn stage(stage: &Path, profile: &str) -> Result<Plan> {
    let doom = profile == crate::doom::PROFILE;
    ensure!(
        doom || profile == crate::zelda3::PROFILE,
        "unsupported native engine"
    );
    let (kit, label) = if doom {
        (crate::doom_sources::FILES, "doom")
    } else {
        (crate::zelda3_sources::FILES, "zelda3")
    };
    for (name, bytes) in kit {
        let p = stage.join(name);
        fs::create_dir_all(p.parent().unwrap())?;
        fs::write(p, bytes)?;
    }
    let lock: serde_json::Value = serde_json::from_slice(if doom {
        include_bytes!("../../engines/doom/upstream-lock.json")
    } else {
        include_bytes!("../../engines/zelda3/upstream-lock.json")
    })?;
    for (name, hash) in lock["files"].as_object().context("invalid engine lock")? {
        if name.ends_with(".py") {
            continue;
        }
        ensure!(
            digest(&fs::read(
                stage.join(format!("engines/{label}/upstream/{name}"))
            )?) == hash.as_str().unwrap_or(""),
            "pinned engine source changed: {name}"
        );
    }
    if doom {
        crate::doom::stage_desktop_sources(stage)?;
    }
    if !doom {
        let path = stage.join("engines/zelda3/upstream/src/zelda_rtl.c");
        let mut source = fs::read_to_string(&path)?;
        for name in ["ZeldaReadSram", "ZeldaWriteSram"] {
            let begin = source
                .find(&format!("void {name}() {{"))
                .context("missing pinned SRAM function")?;
            let brace = begin + source[begin..].find('{').unwrap();
            let mut depth = 0;
            let mut end = None;
            for (offset, byte) in source.as_bytes()[brace..].iter().enumerate() {
                match byte {
                    b'{' => depth += 1,
                    b'}' => depth -= 1,
                    _ => {}
                }
                if depth == 0 {
                    end = Some(brace + offset + 1);
                    break;
                }
            }
            source.replace_range(begin..end.context("invalid pinned SRAM function")?, "");
        }
        fs::write(path, crate::zelda3::stage_snapshot_bridge(&source)?)?;
        let ppu = stage.join("engines/zelda3/upstream/snes/ppu.c");
        fs::write(
            &ppu,
            crate::zelda3::adapt_ppu_state(&crate::zelda3::adapt_ppu(&fs::read_to_string(&ppu)?)?)?,
        )?;
        let dma = stage.join("engines/zelda3/upstream/snes/dma.c");
        fs::write(
            &dma,
            crate::zelda3::adapt_dma_state(&fs::read_to_string(&dma)?)?,
        )?;
        let dsp = stage.join("engines/zelda3/upstream/snes/dsp.c");
        fs::write(
            &dsp,
            crate::zelda3::adapt_dsp_state(&fs::read_to_string(&dsp)?)?,
        )?;
        let spc = stage.join("engines/zelda3/upstream/src/spc_player.c");
        fs::write(
            &spc,
            crate::zelda3::adapt_spc_playback(&crate::zelda3::adapt_spc_state(
                &fs::read_to_string(&spc)?,
            )?)?,
        )?;
    }
    let adapter = stage.join(format!("engines/{label}/adapters/engine.c"));
    let mut text = fs::read_to_string(&adapter)?;
    if doom {
        let a = text.find("__wasi_errno_t __wrap___wasi_fd_write").unwrap();
        let b = text.find("void DG_Init").unwrap();
        text.replace_range(a..b, "");
        text = text.replace("#include <wasi/api.h>", "").replace(
            "#define EXPORT(name) __attribute__((export_name(name)))",
            "#define EXPORT(name)",
        );
    } else {
        text = text.replace(
            "const char *file, int line, const char *function",
            "const char *file, unsigned int line, const char *function",
        );
        let a = text.find("#include <wasi/api.h>").unwrap();
        let b = text[a..]
            .find("__attribute__((export_name(\"reset\")))")
            .unwrap()
            + a;
        text.replace_range(a..b, "");
        for name in [
            "renderer_revision",
            "configure_video",
            "allocate",
            "initialize",
            "step",
            "pixels",
            "pixel_format",
            "audio",
            "audio_channels",
            "sram",
            "save_dirty",
            "reset",
            "status",
            "position",
            "controller",
        ] {
            text = text.replace(&format!("__attribute__((export_name(\"{name}\")))"), "");
        }
    }
    if doom {
        text.push_str("\nint __wrap_printf(const char *format,...){return 0;} int __wrap_puts(const char *text){return 0;} int __wrap_putchar(int c){return c;}\n");
    }
    if !doom {
        text.push_str(include_str!("zelda_stdio.c.txt"));
    }
    fs::write(adapter, text)?;
    let mut sources: Vec<String> = if doom {
        lock["sources"]
            .as_array()
            .context("missing sources")?
            .iter()
            .map(|n| format!("engines/doom/upstream/{}", n.as_str().unwrap()))
            .collect()
    } else {
        let mut v = Vec::new();
        for (name, _) in kit {
            if let Some(tail) = name.strip_prefix("engines/zelda3/upstream/src/") {
                if tail.ends_with(".c")
                    && !tail.contains('/')
                    && ![
                        "main.c",
                        "audio.c",
                        "config.c",
                        "glsl_shader.c",
                        "opengl.c",
                        "zelda_cpu_infra.c",
                    ]
                    .contains(&tail)
                {
                    v.push(name.to_string());
                }
            }
        }
        v.sort();
        for n in ["ppu.c", "dma.c", "dsp.c", "snes_other.c"] {
            v.push(format!("engines/zelda3/upstream/snes/{n}"));
        }
        v
    };
    sources.push(format!("engines/{label}/adapters/engine.c"));
    if doom {
        crate::doom::verify_music_sources(stage)?;
        crate::doom::verify_merge_sources(stage)?;
        sources.push("engines/doom/merge/upstream/w_merge.c".into());
        sources.extend(
            crate::doom::MUSIC_SOURCES
                .iter()
                .map(|name| format!("engines/doom/music/{name}")),
        );
    }
    let mut includes = vec![
        format!("engines/{label}/upstream"),
        format!("engines/{label}/adapters/include"),
    ];
    if doom {
        includes.extend([
            "engines/doom/music".into(),
            "engines/doom/music/upstream".into(),
        ]);
    }
    Ok(Plan {
        label,
        sources,
        includes,
        definitions: if doom {
            &[
                "DOOMGENERIC_RESX=320",
                "DOOMGENERIC_RESY=200",
                "FEATURE_SOUND",
                "FEATURE_WAD_MERGE",
                "NORMALUNIX",
                "_DEFAULT_SOURCE",
            ]
        } else {
            &[]
        },
        configuration: if doom {
            "gamepkg_doom"
        } else {
            "gamepkg_zelda"
        },
        wraps: if doom { DOOM_WRAPS } else { ZELDA_WRAPS },
    })
}
