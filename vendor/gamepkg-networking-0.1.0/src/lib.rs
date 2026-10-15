//! Networking is denied by the GamePKG runtime policy. No sockets or DNS.
#![no_std]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error { PermissionDenied }
pub fn request(_url: &str, _output: &mut [u8]) -> Result<u32, Error> {
    Err(Error::PermissionDenied)
}
