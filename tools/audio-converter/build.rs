fn main() {
    if std::env::var("TARGET").as_deref() == Ok("wasm32-unknown-unknown") {
        println!("cargo:rustc-link-arg=-zstack-size=131072");
        println!("cargo:rustc-link-arg=--max-memory=262144");
    }
}
