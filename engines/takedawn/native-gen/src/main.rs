use anyhow::{ensure, Context, Result};
use gamepkg_takedawn_media::{Budget, VerifiedDisc};
use gamepkg_takedawn_native_gen::{
    decode::{blocks, Decoder},
    dispatch::write_streamed as write_dispatch_streamed,
    dispatch::write_streamed_controlled as write_controlled_dispatch_streamed,
    lower::dispatch::block as dispatch_block,
    lower::dispatch::controlled_block as controlled_dispatch_block,
    lower::{data_instruction, flow, instruction as lower_instruction, integer_instruction},
    program::{closed_entries, write_streamed},
    reference::References,
    xbe::{Image, MAX_XBE},
};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::Path,
};

struct HashWriter(Sha256);
impl Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn canonical(value: &serde_json::Value, out: &mut impl Write) -> Result<()> {
    match value {
        serde_json::Value::Object(map) => {
            out.write_all(b"{")?;
            let mut fields: Vec<_> = map.iter().collect();
            fields.sort_by_key(|(key, _)| *key);
            for (i, (key, value)) in fields.into_iter().enumerate() {
                if i != 0 {
                    out.write_all(b",")?;
                }
                serde_json::to_writer(&mut *out, key)?;
                out.write_all(b":")?;
                canonical(value, out)?;
            }
            out.write_all(b"}")?;
        }
        serde_json::Value::Array(values) => {
            out.write_all(b"[")?;
            for (i, value) in values.iter().enumerate() {
                if i != 0 {
                    out.write_all(b",")?;
                }
                canonical(value, out)?;
            }
            out.write_all(b"]")?;
        }
        _ => serde_json::to_writer(out, value)?,
    }
    Ok(())
}
fn digest(value: &impl serde::Serialize) -> Result<String> {
    let mut out = HashWriter(Sha256::new());
    canonical(&serde_json::to_value(value)?, &mut out)?;
    Ok(hex::encode(out.0.finalize()))
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    ensure!((2..=5).contains(&args.len()),
        "usage: gamepkg-takedawn-native-gen inspect DISC | reference-linear|reference-cfg|reference-data-coverage|reference-integer-coverage|reference-instruction-coverage|reference-leaf-coverage|reference-program-coverage DISC PRIVATE-FUNCTIONS.json | reference-source|reference-dispatch-source|reference-controlled-dispatch-source DISC PRIVATE-FUNCTIONS.json PRIVATE-TEMP-OUTPUT.c | reference-controlled-dispatch-tables-source DISC PRIVATE-FUNCTIONS.json PRIVATE-TEMP-OUTPUT.c REVIEWED-TABLES.json; full native generation remains in development");
    let command = args[0].to_str().context("invalid command")?;
    ensure!(
        (command == "inspect" && args.len() == 2)
            || (matches!(
                command,
                "reference-linear"
                    | "reference-cfg"
                    | "reference-data-coverage"
                    | "reference-integer-coverage"
                    | "reference-instruction-coverage"
                    | "reference-leaf-coverage"
                    | "reference-program-coverage"
            ) && args.len() == 3)
            || (matches!(
                command,
                "reference-source"
                    | "reference-dispatch-source"
                    | "reference-controlled-dispatch-source"
            ) && args.len() == 4)
            || (command == "reference-controlled-dispatch-tables-source" && args.len() == 5),
        "unknown/incomplete native generator command"
    );
    // The whole-graph maintainer diagnostic additionally owns bounded node,
    // edge and reverse-worklist metadata. This is not a game runtime allowance.
    let table_source = command == "reference-controlled-dispatch-tables-source";
    let controlled_source = command == "reference-controlled-dispatch-source" || table_source;
    let dispatch_source = command == "reference-dispatch-source" || controlled_source;
    let source = command == "reference-source" || dispatch_source;
    let program = command == "reference-program-coverage" || source;
    let source_output = if source {
        let output = Path::new(&args[3]);
        let parent = output
            .parent()
            .context("missing private output directory")?
            .canonicalize()?;
        let temporary_root = std::env::temp_dir().canonicalize()?;
        ensure!(
            parent.starts_with(&temporary_root) && parent != temporary_root,
            "executable-derived source must stay in a private temporary subdirectory"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            ensure!(
                fs::metadata(&parent)?.permissions().mode() & 0o077 == 0,
                "private source directory must not allow group or other access"
            );
        }
        let output = parent.join(output.file_name().context("missing output filename")?);
        ensure!(
            !output.try_exists()?,
            "private source output already exists"
        );
        Some(output)
    } else {
        None
    };
    let budget = Budget::new(if program {
        192 * 1024 * 1024
    } else {
        128 * 1024 * 1024
    });
    let media = VerifiedDisc::open(Path::new(&args[1]), &budget)?;
    let mut stream = media.open_stream("default.xbe")?;
    ensure!(
        !stream.is_empty() && stream.len() <= MAX_XBE as u64,
        "unsupported executable size"
    );
    let _executable_memory = budget.reserve(stream.len())?;
    let mut executable = Vec::new();
    executable.try_reserve_exact(stream.len() as usize)?;
    stream.read_to_end(&mut executable)?;
    drop(stream);
    ensure!(
        hex::encode(Sha256::digest(&executable))
            == "9f497acd82adb8edfb72cc31aeeb35010b45d32b13a3f1fc6540717ff24d13f9",
        "unsupported executable hash"
    );
    let _layout_memory = budget.reserve(65536)?;
    let image = Image::parse(&executable)?;
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    if command == "inspect" {
        serde_json::to_writer_pretty(
            &mut out,
            &serde_json::json!({
                "base_address": image.base, "image_size": image.image_size, "entry_point": image.entry,
                "kernel_thunk": image.kernel_thunk, "sections": image.sections,
                "manifest_id": media.report().manifest_id, "native_generation_complete": false,
            }),
        )?;
        writeln!(out)?;
    } else {
        // This is an explicit maintainer differential input, never a player
        // discovery shortcut. Private reference addresses are not shipped.
        let functions = References::read(fs::File::open(&args[2])?, &budget)?;
        let decoder = Decoder::new(&budget)?;
        let table_profiles = if table_source {
            let p = gamepkg_takedawn_native_gen::jump_table::Function::read(
                fs::File::open(&args[4])?,
                &budget,
            )?;
            ensure!(
                functions
                    .functions()
                    .iter()
                    .any(|f| f.start == p.start && f.end == p.end),
                "jump-table profile absent from function inventory"
            );
            BTreeMap::from([(p.start, p)])
        } else {
            BTreeMap::new()
        };
        // The explicit private source diagnostic also streams generated C.
        // Ordinary coverage commands only retain counts and graph metadata.
        let _coverage_memory = budget.reserve(if program {
            8 * 1024 * 1024
        } else {
            1024 * 1024
        })?;
        let targets = if program && !dispatch_source {
            functions.functions().iter().map(|f| f.start).collect()
        } else {
            BTreeSet::new()
        };
        let mut dispatch_ranges = Vec::new();
        dispatch_ranges.try_reserve_exact(if dispatch_source {
            functions.functions().len()
        } else {
            0
        })?;
        let mut dependencies = BTreeMap::new();
        let mut sizes = BTreeMap::new();
        let mut dependency_edges = 0usize;
        let integer = command == "reference-integer-coverage";
        let all_instructions = command == "reference-instruction-coverage";
        let leaf = command == "reference-leaf-coverage";
        let coverage = command == "reference-data-coverage" || integer || all_instructions || leaf;
        let mut generated_functions = 0u64;
        let mut rejected_functions = 0u64;
        let mut generated_source_bytes = 0u64;
        let mut lowered = BTreeMap::<String, u64>::new();
        let mut unsupported = BTreeMap::<String, u64>::new();
        let mut backed_functions = 0u64;
        let mut rejected_extents = 0u64;
        serde_json::to_writer(
            &mut out,
            &serde_json::json!({
                "kind": "image", "base_address": image.base, "image_size": image.image_size,
                "entry_point": image.entry, "kernel_thunk": image.kernel_thunk, "sections": image.sections,
            }),
        )?;
        writeln!(out)?;
        let stops = BTreeSet::new();
        for function in functions.functions() {
            let start = function.start;
            let end = function.end;
            let size = end
                .checked_sub(start)
                .filter(|n| *n > 0)
                .context("invalid reference function extent")?;
            let Ok(bytes) = image.code_bytes(start, size) else {
                rejected_extents += 1;
                // Account for every reference candidate, including upstream
                // spans extending into BSS or unrelated raw sections. Never
                // reinterpret those extra file bytes as this function's code.
                serde_json::to_writer(
                    &mut out,
                    &serde_json::json!({
                        "kind": "rejected_extent", "start": start, "end": end,
                        "reason": "not_backed_executable_span",
                    }),
                )?;
                writeln!(out)?;
                continue;
            };
            let decoded = if let Some(profile) = table_profiles.get(&start) {
                profile.decode(&decoder, bytes, &budget)
            } else if command != "reference-linear" {
                decoder.cfg(bytes, start, &[start], &BTreeSet::new(), &stops)
            } else {
                decoder.linear(bytes, start, &[])
            }
            .with_context(|| format!("decode {start:#010x}..{end:#010x}"))?;
            backed_functions += 1;
            if dispatch_source {
                let lower = if controlled_source {
                    controlled_dispatch_block
                } else {
                    dispatch_block
                };
                match lower(&decoded, start, end, &budget) {
                    Ok(block) => {
                        generated_functions += 1;
                        generated_source_bytes += block.source().len() as u64;
                        dispatch_ranges.push((start, end));
                    }
                    Err(_) => rejected_functions += 1,
                }
                continue;
            }
            if leaf || program {
                let generated = if program {
                    flow::direct_function(&decoded, start, end, &targets, &budget)
                } else {
                    flow::leaf(&decoded, start, end, &budget)
                };
                match generated {
                    Ok(generated) => {
                        generated_functions += 1;
                        generated_source_bytes += generated.source().len() as u64;
                        if program {
                            dependency_edges += generated.dependencies().len();
                            ensure!(
                                dependency_edges <= 262144,
                                "native program diagnostic edge limit"
                            );
                            dependencies.insert(start, generated.dependencies().to_vec());
                            sizes.insert(start, generated.source().len() as u64);
                        }
                    }
                    Err(_) => rejected_functions += 1,
                }
                continue;
            }
            if coverage {
                for instruction in decoded.instructions() {
                    let emitted = if all_instructions {
                        lower_instruction(instruction)
                    } else if integer {
                        integer_instruction(instruction)
                    } else {
                        data_instruction(instruction)
                    };
                    let counts = if emitted.is_ok() {
                        &mut lowered
                    } else {
                        &mut unsupported
                    };
                    ensure!(
                        counts.len() < 4096 || counts.contains_key(&instruction.mnemonic),
                        "coverage mnemonic limit exceeded"
                    );
                    *counts.entry(instruction.mnemonic.clone()).or_default() += 1;
                }
                continue;
            }
            let blocks = blocks(&decoded, start, end, &[], &stops)?;
            serde_json::to_writer(
                &mut out,
                &serde_json::json!({
                    "kind": "function",
                    "start": start, "end": end, "instructions": decoded.instructions().len(), "blocks": blocks.len(),
                    "instruction_sha256": digest(&decoded.instructions())?, "block_sha256": digest(&blocks)?,
                }),
            )?;
            writeln!(out)?;
        }
        if dispatch_source {
            let destination = source_output.context("missing private dispatch output")?;
            let mut staged = tempfile::NamedTempFile::new_in(
                destination.parent().context("missing output directory")?,
            )?;
            staged.write_all(b"#include \"recomp_types.h\"\n")?;
            let write_dispatch = if controlled_source {
                write_controlled_dispatch_streamed
            } else {
                write_dispatch_streamed
            };
            let summary = if table_source {
                gamepkg_takedawn_native_gen::dispatch::write_streamed_controlled_tables(
                    &dispatch_ranges,
                    |start, end| image.code_bytes(start, end - start),
                    &decoder,
                    &budget,
                    512 * 1024 * 1024,
                    &mut staged,
                    &table_profiles,
                )?
            } else {
                write_dispatch(
                    &dispatch_ranges,
                    |start, end| image.code_bytes(start, end - start),
                    &decoder,
                    &budget,
                    512 * 1024 * 1024,
                    &mut staged,
                )?
            };
            ensure!(
                summary.functions as u64 == generated_functions,
                "dispatch selection count changed"
            );
            staged.as_file().sync_all()?;
            staged
                .persist_noclobber(&destination)
                .context("publish private dispatch source")?;
            serde_json::to_writer(
                &mut out,
                &serde_json::json!({"kind":if controlled_source {"private_streamed_controlled_dispatch_source"} else {"private_streamed_dispatch_source"},"functions":summary.functions,"instruction_entries":summary.instruction_entries,"shared_tail_entries":summary.shared_tail_entries,"overlapping_units":summary.overlapping_units,"body_bytes":summary.bytes,"body_sha256":summary.source_sha256,"map_sha256":summary.map_sha256,"backed_functions":backed_functions,"rejected_extents":rejected_extents,"rejected_functions":rejected_functions,"block_source_bytes":generated_source_bytes,"native_generation_complete":false,"program_ready":false}),
            )?;
            writeln!(out)?;
        } else if program {
            let closed = closed_entries(&dependencies, &budget)?;
            let closed_bytes: u64 = closed.entries().iter().map(|entry| sizes[entry]).sum();
            if let Some(destination) = source_output {
                let mut ends = BTreeMap::new();
                for function in functions.functions() {
                    ensure!(
                        ends.insert(function.start, function.end).is_none(),
                        "duplicate reference entry requires recovery"
                    );
                }
                let ranges: Vec<_> = closed
                    .entries()
                    .iter()
                    .map(|entry| (*entry, ends[entry]))
                    .collect();
                let mut staged = tempfile::NamedTempFile::new_in(
                    destination.parent().context("missing output directory")?,
                )?;
                staged.write_all(b"#include \"recomp_types.h\"\n")?;
                let summary = write_streamed(
                    &ranges,
                    |start, end| image.code_bytes(start, end - start),
                    &decoder,
                    &budget,
                    512 * 1024 * 1024,
                    &mut staged,
                )?;
                ensure!(
                    summary.bytes == closed_bytes + ranges.len() as u64 * 25,
                    "native source planning size mismatch"
                );
                staged.as_file().sync_all()?;
                staged
                    .persist_noclobber(&destination)
                    .context("publish private diagnostic source")?;
                serde_json::to_writer(
                    &mut out,
                    &serde_json::json!({
                        "kind":"private_streamed_source", "functions":summary.functions,
                    "shared_tail_entries":summary.shared_tail_entries,
                        "body_bytes":summary.bytes,"body_sha256":summary.source_sha256,
                        "native_generation_complete":false,"program_ready":false,
                    }),
                )?;
                writeln!(out)?;
            }
            serde_json::to_writer(
                &mut out,
                &serde_json::json!({
                    "kind": "native_dependency_coverage", "native_generation_complete":false, "program_ready":false,
                    "backed_functions":backed_functions, "rejected_extents":rejected_extents,
                    "provisional_functions":generated_functions, "rejected_functions":rejected_functions,
                    "dependency_edges":dependency_edges, "dependency_closed_functions":closed.entries().len(),
                    "dependency_rejected_functions":generated_functions-closed.entries().len() as u64,
                    "dependency_closed_source_bytes":closed_bytes,
                }),
            )?;
            writeln!(out)?;
        } else if leaf {
            serde_json::to_writer(
                &mut out,
                &serde_json::json!({
                    "kind": "closed_leaf_coverage", "native_generation_complete": false,
                    "backed_functions": backed_functions, "rejected_extents": rejected_extents,
                    "generated_functions": generated_functions, "rejected_functions": rejected_functions,
                    "generated_source_bytes": generated_source_bytes,
                }),
            )?;
            writeln!(out)?;
        } else if coverage {
            serde_json::to_writer(
                &mut out,
                &serde_json::json!({
                    "kind": if all_instructions { "instruction_lowering_coverage" } else if integer { "integer_lowering_coverage" } else { "data_lowering_coverage" }, "native_generation_complete": false,
                    "backed_functions": backed_functions, "rejected_extents": rejected_extents,
                    "lowered_instructions": lowered.values().sum::<u64>(),
                    "unsupported_instructions": unsupported.values().sum::<u64>(),
                    "lowered_by_mnemonic": lowered, "unsupported_by_mnemonic": unsupported,
                }),
            )?;
            writeln!(out)?;
        }
    }
    out.flush()?;
    Ok(())
}
