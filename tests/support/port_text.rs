// The native SDK validates bytes in its trusted provider. These host-side
// source fixtures use Rust's equivalent checks without a game transport.
#[allow(dead_code)]
pub fn text_from_utf8(bytes: &[u8]) -> Result<&str, i32> {
    std::str::from_utf8(bytes).map_err(|_| -1)
}

#[allow(dead_code)]
pub fn text_from_utf8_owned(bytes: Vec<u8>) -> Result<String, Vec<u8>> {
    String::from_utf8(bytes).map_err(|error| error.into_bytes())
}
