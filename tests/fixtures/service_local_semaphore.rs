// Ordinary dependency-free game source. Only host-generated service bindings.
fn need(value: bool) -> Result<(), i32> {
    if value {
        Ok(())
    } else {
        Err(-3)
    }
}
fn word(bytes: &[u8]) -> Result<u64, i32> {
    let b = bytes.get(..8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
fn spawn(entry: u32, argument: u64) -> Result<u64, i32> {
    let mut result = [0u8; 8];
    let request = [
        u32::to_le_bytes(entry).as_slice(),
        &u64::to_le_bytes(argument),
        &u32::to_le_bytes(0),
    ]
    .concat();
    need(gamepkg::local_call(10, &request, &mut result)? == 8)?;
    word(&result)
}
fn joined(task: u64) -> Result<u64, i32> {
    let start = gamepkg::local_performance_sample()?;
    while !gamepkg::local_task_finished(task)? {
        let now = gamepkg::local_performance_sample()?;
        if now.frequency != start.frequency || now.counter - start.counter > start.frequency * 3 {
            return Err(-2);
        }
    }
    Ok(gamepkg::local_task_join(task)?.result())
}
fn close_sync(id: u64) -> Result<(), i32> {
    need(gamepkg::local_call(25, &u64::to_le_bytes(id), &mut [])? == 0)
}
pub fn semaphore_release_worker(id: u64) -> u64 {
    match gamepkg::local_semaphore_release(id, 1) {
        Ok(previous) => u64::from(previous),
        Err(code) => code as i64 as u64,
    }
}
pub fn semaphore_take_worker(id: u64) -> u64 {
    match gamepkg::local_semaphore_wait(id, 0) {
        Ok(()) => 1,
        Err(-2) => 0,
        Err(code) => code as i64 as u64,
    }
}
fn cancelled(request: u64) -> Result<(), i32> {
    let mut bytes = [0u8; 16];
    need(gamepkg::local_mailbox_receive(request, 1000, &mut bytes)? == 16)?;
    let id = word(&bytes)?;
    let ready = word(bytes.get(8..).ok_or(-1)?)?;
    gamepkg::local_mailbox_send(ready, &[1])?;
    need(gamepkg::local_semaphore_wait(id, 1000) == Err(-4))?;
    need(gamepkg::local_semaphore_state(id) == Err(-4))?;
    need(gamepkg::local_semaphore_release(id, 1) == Err(-4))?;
    need(gamepkg::local_semaphore_create(0, 1) == Err(-4))
}
pub fn semaphore_cancel_worker(request: u64) -> u64 {
    match cancelled(request) {
        Ok(()) => 55,
        Err(code) => code as i64 as u64,
    }
}
fn verify() -> Result<(), i32> {
    need(gamepkg::local_semaphore_create(0, 0) == Err(-1))?;
    need(gamepkg::local_semaphore_create(0, 0x80000000) == Err(-1))?;
    need(gamepkg::local_semaphore_create(2, 1) == Err(-1))?;
    let id = gamepkg::local_semaphore_create(0, 16)?;
    let state = gamepkg::local_semaphore_state(id)?;
    need(state.count == 0 && state.maximum == 16)?;
    need(gamepkg::local_semaphore_state(0) == Err(-4))?;
    need(gamepkg::local_semaphore_state(u64::MAX) == Err(-4))?;
    need(gamepkg::local_semaphore_wait(id, 1001) == Err(-1))?;
    need(gamepkg::local_semaphore_release(id, 0) == Err(-1))?;
    need(gamepkg::local_semaphore_release(id, 17) == Err(-1))?;
    need(gamepkg::local_semaphore_release(id, u32::MAX) == Err(-1))?;
    need(gamepkg::local_semaphore_state(id)?.count == 0)?;
    let mut out = [0u8; 8];
    need(gamepkg::local_call(43, &[0u8; 7], &mut out) == Err(-1))?;
    need(
        gamepkg::local_call(
            43,
            &[u32::to_le_bytes(0), u32::to_le_bytes(1)].concat(),
            out.get_mut(..7).ok_or(-1)?,
        ) == Err(-1),
    )?;
    let release = [u64::to_le_bytes(id).as_slice(), &u32::to_le_bytes(1)].concat();
    need(gamepkg::local_call(44, &release, out.get_mut(..3).ok_or(-1)?) == Err(-1))?;
    need(gamepkg::local_call(44, &[release.as_slice(), &[0u8]].concat(), &mut out) == Err(-1))?;
    need(gamepkg::local_semaphore_state(id)?.count == 0)?;
    need(gamepkg::local_event_state(id) == Err(-1))?;
    need(gamepkg::local_event_wait(id, 0) == Err(-1))?;
    need(gamepkg::local_mutex_lock(id, 0) == Err(-1))?;
    need(
        gamepkg::local_call(
            23,
            &[u64::to_le_bytes(id).as_slice(), &u32::to_le_bytes(0)].concat(),
            &mut [],
        ) == Err(-1),
    )?;
    for op in [20u32, 21] {
        let request = if op == 20 {
            [u32::to_le_bytes(0), u32::to_le_bytes(1)].concat()
        } else {
            std::vec::Vec::new()
        };
        need(gamepkg::local_call(op, &request, &mut out)? == 8)?;
        let wrong = word(&out)?;
        need(gamepkg::local_semaphore_state(wrong) == Err(-1))?;
        need(gamepkg::local_semaphore_release(wrong, 1) == Err(-1))?;
        need(gamepkg::local_semaphore_wait(wrong, 0) == Err(-1))?;
        if op == 20 {
            need(gamepkg::local_event_state(wrong)?.signaled)?;
        } else {
            gamepkg::local_mutex_lock(wrong, 0)?;
            gamepkg::local_call(24, &u64::to_le_bytes(wrong), &mut [])?;
        }
        close_sync(wrong)?;
    }
    let mut jobs = std::vec::Vec::with_capacity(16);
    for _ in 0..16u32 {
        jobs.push(spawn(0, id)?);
    }
    let mut previous = 0u32;
    for task in jobs {
        let value = joined(task)?;
        need(value < 16)?;
        let bit = 1u32 << (value as u32);
        need(previous & bit == 0)?;
        previous |= bit;
    }
    need(previous == 65535 && gamepkg::local_semaphore_state(id)?.count == 16)?;
    need(gamepkg::local_semaphore_release(id, 1) == Err(-1))?;
    for _ in 0..11u32 {
        gamepkg::local_semaphore_wait(id, 0)?;
    }
    let mut jobs = std::vec::Vec::with_capacity(16);
    for _ in 0..16u32 {
        jobs.push(spawn(1, id)?);
    }
    let mut acquired = 0u64;
    for task in jobs {
        let value = joined(task)?;
        need(value < 2)?;
        acquired += value;
    }
    need(acquired == 5 && gamepkg::local_semaphore_state(id)?.count == 0)?;
    let request = gamepkg::local_mailbox_create(16, 1)?;
    let ready = gamepkg::local_mailbox_create(1, 1)?;
    gamepkg::local_mailbox_send(
        request,
        &[u64::to_le_bytes(id), u64::to_le_bytes(ready)].concat(),
    )?;
    let task = spawn(2, request)?;
    let mut signal = [0u8; 1];
    need(gamepkg::local_mailbox_receive(ready, 1000, &mut signal)? == 1 && signal[0] == 1)?;
    gamepkg::local_call(12, &u64::to_le_bytes(task), &mut [])?;
    need(gamepkg::local_semaphore_release(id, 1)? == 0)?;
    need(joined(task)? == 55 && gamepkg::local_semaphore_state(id)?.count == 1)?;
    gamepkg::local_mailbox_release(request)?;
    gamepkg::local_mailbox_release(ready)?;
    close_sync(id)?;
    need(gamepkg::local_semaphore_state(id) == Err(-4))?;
    need(gamepkg::local_semaphore_release(id, 1) == Err(-4))?;
    need(gamepkg::local_semaphore_wait(id, 0) == Err(-4))?;
    let large = gamepkg::local_semaphore_create(0x7fffffff, 0x7fffffff)?;
    need(gamepkg::local_semaphore_release(large, 1) == Err(-1))?;
    gamepkg::local_semaphore_wait(large, 0)?;
    need(gamepkg::local_semaphore_release(large, 1)? == 0x7ffffffe)?;
    close_sync(large)?;
    let mut objects = std::vec::Vec::with_capacity(128);
    for _ in 0..128u32 {
        objects.push(gamepkg::local_semaphore_create(0, 1)?);
    }
    need(gamepkg::local_semaphore_create(0, 1) == Err(-2))?;
    for object in objects {
        close_sync(object)?;
    }
    Ok(())
}
pub fn game_main() {
    let code = match verify() {
        Ok(()) => 0u64,
        Err(code) => code as i64 as u64,
    };
    let _ = gamepkg::diagnostic(1, 0x53454dff, &[code]);
}
