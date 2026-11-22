//! NVI1 canonical DATA contract for signed, bounded shader templates. This is
//! not an NV2A decoder/interpreter or evidence that a template is faithful.
pub const BYTES: usize = 13_072;
pub const VERTEX_SLOTS: usize = 136;
pub const COMBINER_STAGES: usize = 8;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rejection {
    pub byte_offset: u32,
    pub reason: &'static str,
    pub unsupported: bool,
}
fn reject(word: usize, reason: &'static str) -> Rejection {
    Rejection {
        byte_offset: (word * 4) as u32,
        reason,
        unsupported: false,
    }
}
fn unsupported(word: usize, reason: &'static str) -> Rejection {
    Rejection {
        unsupported: true,
        ..reject(word, reason)
    }
}
fn general(register: u32) -> bool {
    register <= 5 || (8..=13).contains(&register)
}
fn input(value: u32, final_input: bool, special: bool) -> bool {
    value <= 255
        && (general(value & 15) || (special && value & 15 >= 14))
        && (!final_input || (value >> 5) <= 1)
}
fn destination(value: u32) -> bool {
    let temp = value & 15;
    let mask = (value >> 4) & 15;
    let output = (value >> 8) & 255;
    let out_mask = (value >> 16) & 15;
    value >> 20 == 0
        && if mask == 0 { temp == 15 } else { temp <= 12 }
        && if out_mask == 0 {
            output == 255
        } else {
            matches!(output, 0 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12)
        }
}
fn writes(value: u32) -> bool {
    value & 0xf00f0 != 0
}
/// `cube[slot]` is derived from the installed verified pipeline layout, never
/// from a game claim. Unknown/reserved encodings fail, rather than becoming NOP.
pub fn validate(bytes: &[u8], cube: [bool; 4]) -> Result<(), Rejection> {
    if bytes.len() != BYTES {
        return Err(reject(0, "NVI1 block length"));
    }
    let w: Vec<u32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .copied()
        .map(u32::from_le_bytes)
        .collect();
    if w[..3] != [0x3149564e, 1, BYTES as u32] {
        return Err(reject(0, "NVI1 header/version"));
    }
    if w[3] & !0xe0 != 0 {
        return Err(reject(3, "reserved final flags"));
    }
    let vertices = w[4] as usize;
    let stages = w[5] as usize;
    let mode = w[7] & 3;
    if w[7] & !7 != 0 || mode == 3 || vertices > VERTEX_SLOTS || (mode == 2) != (vertices == 0) {
        return Err(reject(4, "vertex count/mode"));
    }
    if stages > COMBINER_STAGES || (w[7] & 4 != 0 && stages != 0) {
        return Err(reject(5, "combiner count/mode"));
    }
    if w[6] > 65535 {
        return Err(reject(6, "input mask"));
    }
    for at in [8usize, 9, 10, 11, 14, 15] {
        if w[at] > 1 {
            return Err(reject(at, "boolean control"));
        }
    }
    let alpha = f32::from_bits(w[12]);
    if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) || !(1..=8).contains(&w[13]) {
        return Err(reject(12, "alpha comparison/reference"));
    }
    let mut inputs = 0u32;
    for slot in 0..VERTEX_SLOTS {
        let at = 804 + slot * 16;
        let s = &w[at..at + 16];
        if slot >= vertices {
            if s.iter().any(|x| *x != 0) {
                return Err(reject(at, "nonzero inactive vertex record"));
            }
            continue;
        }
        let mac = s[0];
        let ilu = s[1];
        if mac > 13 || ilu > 7 {
            return Err(reject(at, "reserved vertex opcode"));
        }
        for dest in [2usize, 3] {
            if !destination(s[dest]) {
                return Err(reject(at + dest, "vertex destination"));
            }
        }
        if (mac == 0 || mac == 13) && writes(s[2]) || ilu == 0 && writes(s[3]) {
            return Err(reject(at + 2, "inactive/address-only destination"));
        }
        if mac != 0 && ilu != 0 && (s[3] >> 4) & 15 != 0 && s[3] & 15 != 1 {
            return Err(reject(at + 3, "paired ILU destination must be R1"));
        }
        let used = [
            mac != 0,
            matches!(mac, 2 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12),
            matches!(mac, 3 | 4) || ilu != 0,
        ];
        for (operand, used) in used.into_iter().enumerate() {
            let pos = 4 + operand * 4;
            let bank = s[pos];
            let index = s[pos + 1];
            let flags = s[pos + 3];
            if (!used || bank == 0) && s[pos..pos + 4].iter().any(|v| *v != 0) {
                return Err(reject(at + pos, "noncanonical unused/zero operand"));
            }
            if bank > 3
                || s[pos + 2] > 255
                || flags > 3
                || (flags & 2 != 0 && bank != 3)
                || index
                    > match bank {
                        0 => 0,
                        1 => 12,
                        2 => 15,
                        _ => 191,
                    }
            {
                return Err(reject(at + pos, "vertex operand"));
            }
            if used && bank == 2 {
                inputs |= 1 << index;
            }
        }
        if (s[2] >> 16) & 15 != 0 && (s[3] >> 16) & 15 != 0 {
            return Err(reject(at + 2, "both units write explicit outputs"));
        }
    }
    if mode == 2 {
        inputs = 0x1e19;
    }
    if w[6] != inputs {
        return Err(reject(6, "input mask differs from active operands"));
    }
    for stage in 0..COMBINER_STAGES {
        let at = 2980 + stage * 16;
        let s = &w[at..at + 16];
        if stage >= stages {
            if s.iter().any(|x| *x != 0) {
                return Err(reject(at, "nonzero inactive combiner record"));
            }
            continue;
        }
        for (i, v) in s[..8].iter().enumerate() {
            if !input(*v, false, false) {
                return Err(reject(at + i, "combiner input"));
            }
        }
        for part in 0..2 {
            let pos = 8 + part * 4;
            if s[pos..pos + 3].iter().any(|v| !general(*v)) {
                return Err(reject(at + pos, "combiner destination"));
            }
            let flags = s[pos + 3];
            let map = (flags >> 8) & 7;
            let allowed = if part == 0 { 0x71f } else { 0x704 };
            if flags & !allowed != 0 || !matches!(map, 0 | 1 | 2 | 3 | 4 | 6) {
                return Err(reject(at + pos + 3, "combiner output mapping/flags"));
            }
        }
    }
    for i in 0..7 {
        if !input(w[3108 + i], true, i != 4 && i != 5) {
            return Err(reject(3108 + i, "final combiner input"));
        }
    }
    if w[3115] != 0 {
        return Err(reject(3115, "final padding"));
    }
    for slot in 0..4 {
        let mode = w[3192 + slot];
        if mode > 7 {
            return Err(reject(3192 + slot, "texture mode"));
        }
        if mode == 2 {
            return Err(unsupported(3192 + slot, "volume template not qualified"));
        }
        if (mode == 3 && !cube[slot]) || (matches!(mode, 1 | 6 | 7) && cube[slot]) {
            return Err(reject(3192 + slot, "texture/template dimensionality"));
        }
        if w[3196 + slot] > 15 || w[3200 + slot] > 3 || w[3204 + slot] > 15 || w[3208 + slot] > 1 {
            return Err(reject(3196 + slot, "texture flags"));
        }
        if mode != 5 && w[3204 + slot] != 0 {
            return Err(reject(3204 + slot, "inactive clipplane mask"));
        }
        if matches!(mode, 6 | 7) {
            if slot == 0 {
                return Err(unsupported(3192 + slot, "stage-zero dependent texture"));
            }
            if w[3200 + slot] >= slot as u32 {
                return Err(reject(
                    3200 + slot,
                    "dependent stage must precede current stage",
                ));
            }
        }
    }
    for at in (16..804).chain(3116..3192) {
        if f32::from_bits(w[at]).is_nan() {
            return Err(reject(at, "supplied NaN constant"));
        }
    }
    // Dynamic shader constants retain IEEE bits; only template control/texture
    // parameters require finite values. Shader exceptional arithmetic still
    // needs the coordinated template policy and fidelity qualification.
    for (at, value) in w.iter().enumerate().take(3264).skip(3212) {
        if !f32::from_bits(*value).is_finite() {
            return Err(reject(at, "nonfinite texture parameter"));
        }
    }
    if w[3264..] != [1f32.to_bits(), 1f32.to_bits(), 1f32.to_bits(), 0] {
        return Err(unsupported(3264, "HDR template not qualified"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn block() -> Vec<u32> {
        let mut w = vec![0; BYTES / 4];
        w[..16].copy_from_slice(&[
            0x3149564e,
            1,
            BYTES as u32,
            0,
            0,
            0,
            0,
            6,
            0,
            0,
            0,
            0,
            0,
            8,
            0,
            0,
        ]);
        w[3264..].copy_from_slice(&[1f32.to_bits(), 1f32.to_bits(), 1f32.to_bits(), 0]);
        w[6] = 0x1e19;
        w
    }
    fn bytes(w: &[u32]) -> Vec<u8> {
        w.iter().flat_map(|x| x.to_le_bytes()).collect()
    }
    #[test]
    fn fixed_and_programmed_layouts_validate_but_reserved_data_never_becomes_nop() {
        let w = block();
        assert_eq!(validate(&bytes(&w), [false; 4]), Ok(()));
        let mut active = w.clone();
        active[4] = 1;
        active[6] = 1;
        active[7] = 4;
        active[804..820].copy_from_slice(&[
            1, 0, 0x000f000f, 0x0000ff0f, 2, 0, 0x1b, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]);
        // MOV to oPos, source input register zero, identity swizzle.
        assert_eq!(validate(&bytes(&active), [false; 4]), Ok(()));
        for (at, value) in [
            (804, 14),
            (805, 8),
            (809, 16),
            (811, 2),
            (806, 0x100000),
            (6, 2),
            (4, 137),
            (5, 9),
            (819, 4),
            (3, 1),
            (3115, 1),
            (3208, 2),
            (3204, 16),
        ] {
            let mut bad = active.clone();
            bad[at] = value;
            assert!(validate(&bytes(&bad), [false; 4]).is_err(), "word {at}");
        }
        for size in [0, 1, BYTES - 1, BYTES + 1] {
            assert!(validate(&vec![0; size], [false; 4]).is_err());
        }
    }
    #[test]
    fn combiner_reserved_mappings_registers_dimensions_and_suffixes_fail() {
        let mut w = block();
        w[7] = 2;
        w[5] = 1;
        assert_eq!(validate(&bytes(&w), [false; 4]), Ok(()));
        for (at, value) in [
            (2980, 6),
            (2981, 7),
            (2982, 14),
            (2991, 5 << 8),
            (2995, 1),
            (3108 + 4, 14),
            (2980 + 16, 1),
            (3192, 8),
            (3200 + 1, 3),
        ] {
            let mut bad = w.clone();
            bad[at] = value;
            if at == 3201 {
                bad[3193] = 6;
            }
            assert!(validate(&bytes(&bad), [false; 4]).is_err(), "word {at}");
        }
        w[3192] = 3;
        assert!(validate(&bytes(&w), [false; 4]).is_err());
        assert_eq!(validate(&bytes(&w), [true, false, false, false]), Ok(()));
        w[3192] = 2;
        assert!(validate(&bytes(&w), [false; 4]).unwrap_err().unsupported);
        w[3192] = 0;
        w[3264] = 0;
        assert!(validate(&bytes(&w), [false; 4]).unwrap_err().unsupported);
    }
}
