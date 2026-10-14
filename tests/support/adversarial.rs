use ed25519_dalek::{Signer, SigningKey};
use gamepkg::{
    archive::Package,
    manifest::Manifest,
    verify::{digest, tree_hash, Envelope},
};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};
pub const SEED: [u8; 32] = [73; 32];

pub fn fixture(example: &str) -> BTreeMap<String, Vec<u8>> {
    #[cfg(target_os = "android")]
    {
        macro_rules! embedded {
            ($base:literal; $($name:literal),+ $(,)?) => {
                BTreeMap::from([$(($name.to_owned(), include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $base, "/", $name)).to_vec())),+])
            };
        }
        match example {
            "examples/minimal" => {
                embedded!("examples/minimal"; "manifest.toml", "CREDITS.md", "LICENSE", "NOTICE", "thumbnail.png", "source/Cargo.toml", "source/Cargo.lock", "source/src/lib.rs", "shaders/main.slang", "assets/hello.txt")
            }
            "examples/good/byteorder-game" => {
                embedded!("examples/good/byteorder-game"; "manifest.toml", "CREDITS.md", "LICENSE", "NOTICE", "thumbnail.png", "source/Cargo.toml", "source/Cargo.lock", "source/src/lib.rs", "shaders/main.slang", "assets/hello.txt", "assets/score.bin")
            }
            _ => panic!("embed the requested Android fixture: {example}"),
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        fn read(root: &Path, folder: &Path, result: &mut BTreeMap<String, Vec<u8>>) {
            for entry in fs::read_dir(folder).unwrap() {
                let entry = entry.unwrap();
                if entry.file_type().unwrap().is_dir() {
                    read(root, &entry.path(), result);
                } else {
                    result.insert(
                        entry
                            .path()
                            .strip_prefix(root)
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .replace('\\', "/"),
                        fs::read(entry.path()).unwrap(),
                    );
                }
            }
        }
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(example);
        let mut files = BTreeMap::new();
        read(&root, &root, &mut files);
        files
    }
}

pub fn write_fixture(files: &BTreeMap<String, Vec<u8>>, root: &Path) {
    for (name, bytes) in files {
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
}

// Sign deliberately hostile source without calling the production packer.
// Rejection must come from receiver policy, not a broken signature/lock hash.
pub fn sign(files: &BTreeMap<String, Vec<u8>>, root: &Path) -> PathBuf {
    let key = SigningKey::from_bytes(&SEED);
    let mut files = files.clone();
    let mut manifest = Manifest::parse(&files["manifest.toml"]).unwrap();
    manifest.creator_key = hex::encode(key.verifying_key().as_bytes());
    manifest.lockfile_sha256 = digest(&files["source/Cargo.lock"]);
    files.insert(
        "manifest.toml".into(),
        toml::to_string(&manifest).unwrap().into_bytes(),
    );
    let path = root.join("hostile.gamepkg");
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let mut writer = zip::ZipWriter::new(fs::File::create(&path).unwrap());
    for (name, bytes) in &files {
        writer.start_file(name, options).unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap();
    let hash = tree_hash(&mut Package::open(&path).unwrap()).unwrap();
    let envelope = Envelope {
        public_key: manifest.creator_key,
        signature: hex::encode(key.sign(&hash).to_bytes()),
    };
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    let mut writer = zip::ZipWriter::new_append(file).unwrap();
    writer.start_file("signature.toml", options).unwrap();
    writer
        .write_all(toml::to_string(&envelope).unwrap().as_bytes())
        .unwrap();
    writer.finish().unwrap();
    path
}
