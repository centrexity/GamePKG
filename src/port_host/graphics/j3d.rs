//! Host-owned J3D/GX state vocabulary.
//!
//! Games send only fixed-width state words. This module validates the exact
//! vocabulary, canonicalizes it and derives a stable identity from the signed
//! template identity plus the canonical bits. No game supplies a shader, a
//! native handle or an executable transform.

use super::{Error, Reader, Result};

/// Exact wire size: 16 header words plus eight four-word TEV stages.
pub const STATE_WORDS: usize = 48;
pub const STATE_BYTES: usize = STATE_WORDS * 4;

const FLAG_ALPHA_TEST: u32 = 1 << 0;
const FLAG_FOG: u32 = 1 << 1;
const FLAG_LIGHTING: u32 = 1 << 2;
const FLAG_NBT: u32 = 1 << 3;
const FLAG_INDIRECT: u32 = 1 << 4;
const FLAG_MULTITEX: u32 = 1 << 5;
const FLAG_LOGIC_BLEND2: u32 = 1 << 6;
const FLAG_CULL_ALL: u32 = 1 << 7;
const KNOWN_FLAGS: u32 = FLAG_ALPHA_TEST
    | FLAG_FOG
    | FLAG_LIGHTING
    | FLAG_NBT
    | FLAG_INDIRECT
    | FLAG_MULTITEX
    | FLAG_LOGIC_BLEND2
    | FLAG_CULL_ALL;

/// A validated state. Every field is data; no host object or code is embedded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct State {
    flags: u32,
    alpha_compare: u32,
    alpha_ref: u32,
    fog_type: u32,
    fog_start: u32,
    fog_end: u32,
    fog_color: u32,
    light_mask: u32,
    nbt_mode: u32,
    indirect_stages: u32,
    texture_slots: u32,
    lod_bias: u32,
    logic_blend_op: u32,
    tev_count: u32,
    tev: [[u32; 4]; 8],
}

fn finite_bits(bits: u32) -> bool {
    let value = f32::from_bits(bits);
    value.is_finite() && value.abs() <= 65536.0
}

impl State {
    pub(super) fn parse(r: &mut Reader<'_>) -> Result<Self> {
        let flags = r.u32()?;
        if flags & !KNOWN_FLAGS != 0 {
            return Err(Error::Invalid);
        }
        let alpha_compare = r.u32()?;
        let alpha_ref = r.u32()?;
        let fog_type = r.u32()?;
        let fog_start = r.u32()?;
        let fog_end = r.u32()?;
        let fog_color = r.u32()?;
        let light_mask = r.u32()?;
        let nbt_mode = r.u32()?;
        let indirect_stages = r.u32()?;
        let texture_slots = r.u32()?;
        let lod_bias = r.u32()?;
        let logic_blend_op = r.u32()?;
        let tev_count = r.u32()?;
        let reserved = r.u32()?;
        let reserved_tail = r.u32()?;
        if reserved != 0
            || reserved_tail != 0
            || alpha_compare > 7
            || alpha_ref > 255
            || fog_type > 3
            || !finite_bits(fog_start)
            || !finite_bits(fog_end)
            || light_mask > 0x0f
            || nbt_mode > 3
            || indirect_stages > 3
            || !(1..=8).contains(&texture_slots)
            || (lod_bias as i32) < -2048
            || (lod_bias as i32) > 2048
            || logic_blend_op > 3
            || tev_count > 8
        {
            return Err(Error::Invalid);
        }
        if flags & FLAG_INDIRECT == 0 && indirect_stages != 0 {
            return Err(Error::Invalid);
        }
        if flags & FLAG_NBT == 0 && nbt_mode != 0 {
            return Err(Error::Invalid);
        }
        if flags & FLAG_LIGHTING == 0 && light_mask != 0 {
            return Err(Error::Invalid);
        }
        if flags & FLAG_MULTITEX == 0 && texture_slots != 1 {
            return Err(Error::Invalid);
        }
        if flags & FLAG_LOGIC_BLEND2 == 0 && logic_blend_op != 0 {
            return Err(Error::Invalid);
        }
        if flags & FLAG_FOG == 0
            && (fog_type != 0 || fog_start != 0 || fog_end != 0 || fog_color != 0)
        {
            return Err(Error::Invalid);
        }
        if flags & FLAG_ALPHA_TEST == 0 && (alpha_compare != 0 || alpha_ref != 0) {
            return Err(Error::Invalid);
        }
        let mut tev = [[0u32; 4]; 8];
        for stage in &mut tev {
            *stage = [r.u32()?, r.u32()?, r.u32()?, r.u32()?];
        }
        // Disabled stages have one canonical encoding. This makes the stable
        // identity independent of unused game scratch words.
        for stage in tev.iter().skip(tev_count as usize) {
            if *stage != [0; 4] {
                return Err(Error::Invalid);
            }
        }
        for stage in tev.iter().take(tev_count as usize) {
            if stage[0] > 15 || stage[1] > 0xffff || stage[2] > 0xffff || stage[3] > 7 {
                return Err(Error::Invalid);
            }
        }
        r.end()?;
        Ok(Self {
            flags,
            alpha_compare,
            alpha_ref,
            fog_type,
            fog_start,
            fog_end,
            fog_color,
            light_mask,
            nbt_mode,
            indirect_stages,
            texture_slots,
            lod_bias,
            logic_blend_op,
            tev_count,
            tev,
        })
    }

    pub(super) fn canonical(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(STATE_BYTES);
        let mut push = |value: u32| out.extend_from_slice(&value.to_le_bytes());
        push(self.flags);
        push(self.alpha_compare);
        push(self.alpha_ref);
        push(self.fog_type);
        push(self.fog_start);
        push(self.fog_end);
        push(self.fog_color);
        push(self.light_mask);
        push(self.nbt_mode);
        push(self.indirect_stages);
        push(self.texture_slots);
        push(self.lod_bias);
        push(self.logic_blend_op);
        push(self.tev_count);
        push(0);
        push(0);
        for stage in self.tev {
            for word in stage {
                push(word);
            }
        }
        out
    }

    pub(super) fn variant(&self, template_identity: &str, samples: u32) -> Result<[u8; 32]> {
        let mut message = b"GamePKG J3D/GX state v1|".to_vec();
        message.extend_from_slice(template_identity.as_bytes());
        message.extend_from_slice(format!("|samples={samples}|").as_bytes());
        message.extend_from_slice(&self.canonical());
        hex::decode(crate::verify::digest(&message))
            .map_err(|_| Error::Failed)?
            .try_into()
            .map_err(|_| Error::Failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn words() -> Vec<u8> {
        let mut values = vec![0u32; STATE_WORDS];
        values[10] = 1; // one texture slot
        values.into_iter().flat_map(u32::to_le_bytes).collect()
    }
    #[test]
    fn j3d_state_is_exact_canonical_and_stable() {
        let bytes = words();
        let mut reader = Reader(&bytes);
        let state = State::parse(&mut reader).unwrap();
        assert_eq!(state.canonical(), bytes);
        assert_eq!(
            state.variant("template-a", 1).unwrap(),
            state.variant("template-a", 1).unwrap()
        );
        assert_ne!(
            state.variant("template-a", 1).unwrap(),
            state.variant("template-b", 1).unwrap()
        );
        assert_ne!(
            state.variant("template-a", 1).unwrap(),
            state.variant("template-a", 2).unwrap()
        );
        assert!(State::parse(&mut Reader(&bytes[..STATE_BYTES - 1])).is_err());
        assert!(State::parse(&mut Reader(&[bytes, vec![0]].concat())).is_err());
    }
    #[test]
    fn j3d_vocabulary_rejects_unreviewed_fields_and_scratch() {
        let cases: [(usize, u32); 15] = [
            (0, 0x100), // unknown flag
            (1, 8),     // alpha compare
            (2, 256),   // alpha reference
            (3, 4),     // fog type
            (4, f32::INFINITY.to_bits()),
            (5, u32::MAX), // huge finite value
            (6, 1),        // fog color without the flag
            (7, 16),       // light mask
            (8, 4),        // NBT mode
            (9, 4),        // indirect stages without the flag
            (10, 0),       // no texture slot
            (11, 4096),    // LOD bias out of range
            (12, 4),       // logic blend op
            (13, 9),       // TEV stage count
            (15, 1),       // trailing reserved word
        ];
        for (index, value) in cases {
            let mut bytes = words();
            bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
            assert!(State::parse(&mut Reader(&bytes)).is_err(), "word {index}");
        }
        let mut bytes = words();
        bytes[64..68].copy_from_slice(&1u32.to_le_bytes());
        bytes[68..72].copy_from_slice(&0x10000u32.to_le_bytes());
        assert!(State::parse(&mut Reader(&bytes)).is_err());
    }
    #[test]
    fn j3d_full_state_accepts_tev_fog_lighting_and_cull_all() {
        let mut values = vec![0u32; STATE_WORDS];
        values[0] = 0xff;
        values[1] = 3;
        values[2] = 128;
        values[3] = 2;
        values[4] = f32::to_bits(10.0);
        values[5] = f32::to_bits(90.0);
        values[6] = 0x11223344;
        values[7] = 0x0f;
        values[8] = 2;
        values[9] = 3;
        values[10] = 2;
        values[11] = (-16i32) as u32;
        values[12] = 3;
        values[13] = 2;
        values[16] = 1;
        values[17] = 0x1234;
        values[18] = 0x5678;
        values[19] = 2;
        values[20] = 3;
        values[21] = 0xabcd;
        values[22] = 0xdcba;
        values[23] = 1;
        let bytes: Vec<u8> = values.into_iter().flat_map(u32::to_le_bytes).collect();
        let mut reader = Reader(&bytes);
        let state = State::parse(&mut reader).unwrap();
        assert_eq!(state.canonical(), bytes);
    }
}
