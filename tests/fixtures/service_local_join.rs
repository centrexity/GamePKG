// Ordinary dependency-free checked game source; evidence is minted only by SDK join.
pub fn joined_worker(value: u64) -> u64 {
    value
}
fn word(bytes: &[u8]) -> Result<u64, i32> {
    let b = bytes.get(..8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
fn verify() -> Result<(), i32> {
    let expected = 0xfedcba9876543210u64;
    let mut job = [0u8; 8];
    if gamepkg::local_call(
        10,
        &[
            u32::to_le_bytes(0).as_slice(),
            &u64::to_le_bytes(expected),
            &u32::to_le_bytes(1),
        ]
        .concat(),
        &mut job,
    )? != 8
    {
        return Err(-3);
    }
    let id = word(&job)?;
    if gamepkg::local_task_join(id).err() != Some(-2) || gamepkg::local_task_finished(id)? {
        return Err(-3);
    }
    let mut out = [0u8; 4];
    gamepkg::local_call(13, &job, &mut out)?;
    let start = gamepkg::local_performance_sample()?;
    while !gamepkg::local_task_finished(id)? {
        let now = gamepkg::local_performance_sample()?;
        if now.frequency != start.frequency || now.counter - start.counter > start.frequency * 3 {
            return Err(-2);
        }
    }
    let joined = gamepkg::local_task_join(id)?;
    if joined.task_id() != id
        || joined.result() != expected
        || gamepkg::local_task_join(id).err() != Some(-4)
        || gamepkg::local_task_finished(id) != Err(-4)
    {
        return Err(-3);
    }
    Ok(())
}
pub fn game_main() {
    let code = match verify() {
        Ok(()) => 0u64,
        Err(code) => code as i64 as u64,
    };
    let _ = gamepkg::diagnostic(1, 0x4a4f49ff, &[code]);
}
