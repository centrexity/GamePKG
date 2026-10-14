#[path = "support/adversarial.rs"]
mod helper;
use gamepkg::{archive::Package, verify::verify_embedded};

#[test]
fn host_width_byte_order_and_layout_cannot_choose_the_save_format() {
    for source in [
        "fn x(v:u32) { v.to_ne_bytes(); }",
        "fn x() { u32::from_ne_bytes([0;4]); }",
        "use byteorder_lite::NativeEndian;",
        "fn x(v:usize) { v.to_le_bytes(); }",
        "fn x(v:usize) { usize::to_be_bytes(v); }",
        "fn x() { usize::from_le_bytes([0;4]); }",
        "fn x(v:u32) { v.to_le_bytes(); }",
        "fn x(v:usize) { v.r#to_le_bytes(); }",
        "mod r#u32 { pub fn to_le_bytes(_:usize)->[u8;8] { [0;8] } }",
        "fn x() { core::arch::x86_64::_mm_setzero_si128(); }",
        "fn x(v:f64) { v.to_be_bytes(); }",
        "fn x() { core::mem::size_of::<usize>(); }",
        "fn x(v:u32) { core::mem::size_of_val(&v); }",
        "fn x() { core::mem::align_of::<u64>(); }",
        "fn x(v:u32) { core::mem::align_of_val(&v); }",
        "fn x() { core::mem::transmute::<u32,[u8;4]>(1); }",
        "fn x(v:&[u8]) { v.as_ptr(); }",
        "fn x(v:&mut[u8]) { v.as_mut_ptr(); }",
        "fn x() { let address=Box::into_raw(Box::new(1u32)) as usize; }",
        "fn x() { let address=core::ptr::null::<u32>() as usize; }",
        "fn x() { core::ptr::without_provenance::<u32>(1usize); }",
        "fn x() { core::ptr::NonNull::<u32>::dangling(); }",
        "fn x() { let mut p:core::sync::atomic::AtomicPtr<u8>=Default::default(); let address=*p.get_mut() as usize; }",
        "fn x() { core::mem::MaybeUninit::<u32>::zeroed(); }",
        "fn x(v:*const u32) {}",
        "fn x(v:*mut u32) {}",
        "fn x(v:isize) {}",
        "mod u32 { pub fn to_le_bytes(_:usize)->[u8;8] { [0;8] } }",
        "struct u32 { value: usize }",
        "enum u64 { Value }",
        "union u32 { value:usize }",
        "fn x(v:usize) { core::primitive::usize::to_le_bytes(v); }",
        "fn x(v:u32) { <u32>::to_le_bytes(v); }",
    ] {
        let temp = tempfile::tempdir().unwrap();
        let mut files = helper::fixture("examples/minimal");
        files.insert(
            "source/src/lib.rs".into(),
            format!("pub fn game_tick(_:u32,_:u64)->u64 {{0}}\n{source}").into_bytes(),
        );
        let root = temp.path().join("source");
        helper::write_fixture(&files, &root);
        let output = temp.path().join("rejected.gamepkg");
        assert!(
            gamepkg::pack::pack(&root, &output, &helper::SEED)
                .unwrap_err()
                .to_string()
                .contains("portable data policy"),
            "{source}"
        );
        assert!(!output.exists() && !output.with_extension("gamepkg.partial").exists());
        let signed = helper::sign(&files, temp.path());
        assert!(
            verify_embedded(&mut Package::open(&signed).unwrap())
                .unwrap_err()
                .to_string()
                .contains("portable data policy"),
            "{source}"
        );
    }
}

#[test]
fn fixed_width_encoding_and_usize_indexing_remain_valid() {
    for source in [
        "pub fn game_tick(_:u32,_:u64)->u64 { let bytes=u32::to_le_bytes(0x01020304); u32::from_le_bytes(bytes) as u64 }",
        "pub fn game_tick(_:u32,_:u64)->u64 { let bytes=u64::to_be_bytes(0x0102030405060708); u64::from_be_bytes(bytes) }",
        "pub fn game_tick(_:u32,_:u64)->u64 { let bytes=[1,2,3]; let index:usize=bytes.len()-1; bytes[index] as u64 }",
        "pub fn game_tick(_:u32,_:u64)->u64 { let bytes=core::primitive::i64::to_le_bytes(-1); core::primitive::i64::from_le_bytes(bytes) as u64 }",
        "pub fn game_tick(_:u32,_:u64)->u64 { let bytes=f32::to_be_bytes(1.5); f32::from_be_bytes(bytes) as u64 }",
    ] {
        let temp=tempfile::tempdir().unwrap();let mut files=helper::fixture("examples/minimal");
        files.insert("source/src/lib.rs".into(),source.as_bytes().to_vec());
        let signed=helper::sign(&files,temp.path());
        verify_embedded(&mut Package::open(&signed).unwrap()).unwrap();
    }
}

#[test]
fn wasm_guest_has_fixed_word_size_and_little_endian_memory_on_every_host() {
    let manifest = gamepkg::manifest::Manifest::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/minimal/manifest.toml"
    )))
    .unwrap();
    let bytes = wat::parse_str(
        r#"(module (memory (export "memory") 1)
        (func (export "gamepkg_tick") (param i32 i64) (result i64)
            i32.const 0 i64.const 0x0102030405060708 i64.store
            i32.const 0 i32.load8_u i32.const 8 i32.ne if unreachable end
            i32.const 7 i32.load8_u i32.const 1 i32.ne if unreachable end
            i32.const 0 i64.load i64.const 0x0102030405060708 i64.ne if unreachable end
            i64.const 0x100000001))"#,
    )
    .unwrap();
    let mut game = gamepkg::runtime::Game::from_bytes(&bytes, &manifest).unwrap();
    assert_eq!(game.step(0).unwrap(), 0x100000001);
    let memory64=wat::parse_str(r#"(module (memory i64 1) (func (export "gamepkg_tick") (param i32 i64) (result i64) i64.const 0))"#).unwrap();
    assert!(gamepkg::runtime::Game::from_bytes(&memory64, &manifest).is_err());
}

#[test]
fn floating_arithmetic_has_canonical_nan_bits_and_relaxed_simd_is_denied() {
    let manifest = gamepkg::manifest::Manifest::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/minimal/manifest.toml"
    )))
    .unwrap();
    let bytes = wat::parse_str(
        r#"(module
        (func (export "gamepkg_tick") (param i32 i64) (result i64)
            local.get 0 f32.convert_i32_u f32.const 0 f32.div i32.reinterpret_f32
            i32.const 0x7fc00000 i32.ne if unreachable end
            local.get 0 f64.convert_i32_u f64.const 0 f64.div i64.reinterpret_f64
            i64.const 0x7ff8000000000000 i64.ne if unreachable end
            i64.const 42))"#,
    )
    .unwrap();
    let mut game = gamepkg::runtime::Game::from_bytes(&bytes, &manifest).unwrap();
    assert_eq!(game.step(0).unwrap(), 42);
    let relaxed = wat::parse_str(
        r#"(module
        (func (export "gamepkg_tick") (param i32 i64) (result i64)
            v128.const f32x4 0 0 0 0 v128.const f32x4 0 0 0 0
            v128.const f32x4 0 0 0 0 f32x4.relaxed_madd drop i64.const 0))"#,
    )
    .unwrap();
    assert!(gamepkg::runtime::Game::from_bytes(&relaxed, &manifest).is_err());
}
