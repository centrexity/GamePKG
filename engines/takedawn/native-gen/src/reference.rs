//! Bounded maintainer oracle inventory, not player-side function discovery.
use anyhow::{ensure, Result};
use gamepkg_takedawn_media::{Budget, Lease};
use serde::{
    de::{self, SeqAccess, Visitor},
    Deserialize, Deserializer,
};
use std::{fmt, io::Read};
const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_FUNCTIONS: usize = 32768;
#[derive(Deserialize)]
pub struct Function {
    #[serde(deserialize_with = "hex_address")]
    pub start: u32,
    #[serde(deserialize_with = "hex_address")]
    pub end: u32,
}
fn hex_address<'de, D: Deserializer<'de>>(decoder: D) -> std::result::Result<u32, D::Error> {
    struct Address;
    impl Visitor<'_> for Address {
        type Value = u32;
        fn expecting(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
            out.write_str("a 0x-prefixed 32-bit address")
        }
        fn visit_str<E: de::Error>(self, text: &str) -> std::result::Result<u32, E> {
            if !(3..=10).contains(&text.len()) || !text.starts_with("0x") {
                return Err(E::custom("invalid reference address width/prefix"));
            }
            u32::from_str_radix(&text[2..], 16).map_err(E::custom)
        }
    }
    decoder.deserialize_str(Address)
}
struct Inventory(Vec<Function>);
impl<'de> Deserialize<'de> for Inventory {
    fn deserialize<D: Deserializer<'de>>(decoder: D) -> std::result::Result<Self, D::Error> {
        struct Functions;
        impl<'de> Visitor<'de> for Functions {
            type Value = Inventory;
            fn expecting(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
                out.write_str("a bounded function inventory")
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> std::result::Result<Inventory, A::Error> {
                let mut functions = Vec::new();
                while let Some(function) = seq.next_element::<Function>()? {
                    if functions.len() == MAX_FUNCTIONS {
                        return Err(de::Error::custom("reference function count limit exceeded"));
                    }
                    if function.end <= function.start {
                        return Err(de::Error::custom("invalid reference function extent"));
                    }
                    functions.push(function);
                }
                if functions.is_empty() {
                    return Err(de::Error::custom("reference inventory is empty"));
                }
                Ok(Inventory(functions))
            }
        }
        decoder.deserialize_seq(Functions)
    }
}
pub struct References {
    functions: Vec<Function>,
    _memory: Lease,
}
impl References {
    pub fn read(reader: impl Read, budget: &Budget) -> Result<Self> {
        // Capped file + escaped-string scratch + fixed-width candidate vector.
        let memory = budget.reserve(48 * 1024 * 1024)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(MAX_BYTES + 1)?;
        reader
            .take((MAX_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() <= MAX_BYTES,
            "reference metadata byte limit exceeded"
        );
        let Inventory(functions) = serde_json::from_slice(&bytes)?;
        Ok(Self {
            functions,
            _memory: memory,
        })
    }
    pub fn functions(&self) -> &[Function] {
        &self.functions
    }
}
