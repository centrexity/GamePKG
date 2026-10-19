use gamepkg::{
    audio_decode::{Factory, Output},
    memory_budget::Budget,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Write, path::PathBuf};
#[test]
#[ignore = "requires private input/reference paths from tools/check_browser_audio_decoder.py"]
fn generate_browser_decoder_reference_and_fault_fixtures() {
    let input = PathBuf::from(std::env::var_os("GAMEPKG_BROWSER_DECODER_INPUT").unwrap());
    let output = PathBuf::from(std::env::var_os("GAMEPKG_BROWSER_DECODER_FIXTURE").unwrap());
    let factory =
        Factory::new(&fs::read(std::env::var_os("GAMEPKG_AUDIO_DECODER_WASM").unwrap()).unwrap())
            .unwrap();
    assert!(factory.maximum_memory_bytes() <= 6 * 1048576);
    let golden: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-music-worker-reference.json")).unwrap();
    let mut records = Vec::new();
    for case in golden["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let path = input.join(name);
        let encoded = fs::read(&path).unwrap();
        assert_eq!(
            hex::encode(Sha256::digest(&encoded)),
            case["input_sha256"].as_str().unwrap()
        );
        let budget = Budget::new(6 * 1048576);
        let mut file = std::fs::File::open(&path).unwrap();
        let mut stream = factory
            .open_budgeted(
                &mut file,
                encoded.len(),
                Output::OriginalMixer(44100),
                &budget,
                6 * 1048576,
            )
            .unwrap();
        let mut hash = Sha256::new();
        let mut samples = 0;
        let mut step = 0;
        let mut block = [0i16; 1600];
        loop {
            let size = [2, 226, 14, 1598, 1600][step % 5];
            step += 1;
            let count = stream.read(&mut block[..size]).unwrap();
            if count == 0 {
                break;
            }
            for sample in &block[..count] {
                hash.update(sample.to_le_bytes());
            }
            samples += count;
        }
        drop(stream);
        assert_eq!(budget.used(), 0);
        records.push(serde_json::json!({"name":name,"samples":samples,"sha256":hex::encode(hash.finalize()),"input_sha256":case["input_sha256"]}));
    }
    fs::write(
        output.join("reference.json"),
        serde_json::to_vec(&records).unwrap(),
    )
    .unwrap();
    for (name, read, pointer, work, open, rewind) in [
        ("valid", 2, 4096, 0, 0, 0),
        ("negative", -1, 4096, 0, 0, 0),
        ("odd", 1, 4096, 0, 0, 0),
        ("oversize", 1602, 4096, 0, 0, 0),
        ("range", 2, 65535, 0, 0, 0),
        ("work", 2, 4096, 524289, 0, 0),
        ("open", 2, 4096, 0, -1, 0),
        ("rewind", 2, 4096, 0, 0, -1),
        ("signature", 2, 4096, 0, 0, 0),
        ("import", 2, 4096, 0, 0, 0),
        ("table", 2, 4096, 0, 0, 0),
        ("unbounded", 2, 4096, 0, 0, 0),
        ("memory-max", 2, 4096, 0, 0, 0),
        ("start", 2, 4096, 0, 0, 0),
        ("multi-memory", 2, 4096, 0, 0, 0),
        ("memory-export", 2, 4096, 0, 0, 0),
        ("hang-read", 2, 4096, 0, 0, 0),
        ("hang-open", 2, 4096, 0, 0, 0),
    ] {
        let module = format!(
            r#"(module(memory(export "memory")1 96)(global $read(mut i32)(i32.const 0))
          (data(i32.const 4096)"\34\12\dc\fe")
          (func(export "decoder_reserve")(param i32)(result i32)i32.const 1024)
          (func(export "decoder_open")(result i32)i32.const {open})
          (func(export "decoder_read")(param i32)(result i32)i32.const 1 global.set $read i32.const {read})
          (func(export "decoder_output")(result i32)i32.const {pointer})
          (func(export "decoder_rate")(result i32)i32.const 44100)
          (func(export "decoder_channels")(result i32)i32.const 2)
          (func(export "decoder_work")(result i32)global.get $read i32.const {work} i32.mul)
          (func(export "decoder_rewind")(result i32)i32.const {rewind})
          (func(export "decoder_set_output_rate")(param i32)(result i32)i32.const 0)
          (func(export "decoder_set_mixer_rate")(param i32)(result i32)i32.const 0))"#
        );
        let module = match name {
            "signature" => module.replace(
                "(func(export \"decoder_read\")(param i32)",
                "(func(export \"decoder_read\")(param i64)",
            ),
            "import" => module.replacen("(module", "(module(import \"host\" \"cap\" (func))", 1),
            "table" => module.replacen("(module", "(module(table 4097 4097 funcref)", 1),
            "unbounded" => module.replace(
                "(memory(export \"memory\")1 96)",
                "(memory(export \"memory\")1)",
            ),
            "memory-max" => module.replace(
                "(memory(export \"memory\")1 96)",
                "(memory(export \"memory\")1 97)",
            ),
            "start" => module.replacen("(module", "(module(func $start)(start $start)", 1),
            "multi-memory" => module.replacen("(module", "(module(memory 1 1)", 1),
            "memory-export" => module.replace(
                "(memory(export \"memory\")",
                "(memory(export \"different\")",
            ),
            "hang-read" => module.replace("(func(export \"decoder_read\")(param i32)(result i32)i32.const 1 global.set $read i32.const 2)", "(func(export \"decoder_read\")(param i32)(result i32)(loop $forever br $forever)i32.const 0)"),
            "hang-open" => module.replace("(func(export \"decoder_open\")(result i32)i32.const 0)", "(func(export \"decoder_open\")(result i32)(loop $forever br $forever)i32.const 0)"),
            _ => module,
        };
        fs::File::create(output.join(format!("{name}.wasm")))
            .unwrap()
            .write_all(&wat::parse_str(module).unwrap())
            .unwrap();
    }
    println!("Native reference hashes for {} complete pinned tracks and decoder fault fixtures generated",records.len());
}
