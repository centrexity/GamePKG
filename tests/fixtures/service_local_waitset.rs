// Ordinary dependency-free checked game; host bindings are injected after admission.
fn need(value: bool) -> Result<(), i32> {
    if value {
        Ok(())
    } else {
        Err(-3)
    }
}
fn word(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let b = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
fn item(id: u64, kind: u32) -> gamepkg::LocalWaitObject {
    gamepkg::LocalWaitObject { id, kind }
}
fn event(signaled: bool) -> Result<u64, i32> {
    let mut out = [0u8; 8];
    need(
        gamepkg::local_call(
            20,
            &[u32::to_le_bytes(0), u32::to_le_bytes(u32::from(signaled))].concat(),
            &mut out,
        )? == 8,
    )?;
    word(&out, 0)
}
fn signal(id: u64) -> Result<(), i32> {
    gamepkg::local_call(
        22,
        &[u64::to_le_bytes(id).as_slice(), &u32::to_le_bytes(1)].concat(),
        &mut [],
    )?;
    Ok(())
}
fn spawn(entry: u32, argument: u64) -> Result<u64, i32> {
    let mut out = [0u8; 8];
    need(
        gamepkg::local_call(
            10,
            &[
                u32::to_le_bytes(entry).as_slice(),
                &u64::to_le_bytes(argument),
                &u32::to_le_bytes(0),
            ]
            .concat(),
            &mut out,
        )? == 8,
    )?;
    word(&out, 0)
}
fn release(id: u64, kind: u32) -> Result<(), i32> {
    gamepkg::local_sync_release(id, kind)?;
    Ok(())
}
fn run() -> Result<(), i32> {
    let automatic = event(true)?;
    let semaphore = gamepkg::local_semaphore_create(0, 4)?;
    let pair = [item(automatic, 1), item(semaphore, 3)];
    need(gamepkg::local_wait_many(&pair, true, 0) == Err(-2))?;
    need(gamepkg::local_event_state(automatic)?.signaled)?;
    let task = spawn(0, semaphore)?;
    let combined = [item(automatic, 1), item(semaphore, 3), item(task, 4)];
    let result = gamepkg::local_wait_many(&combined, true, 1000)?;
    need(result.index == 0 && !result.abandoned)?;
    need(
        gamepkg::local_task_finished(task)?
            && gamepkg::local_semaphore_state(semaphore)?.count == 3
            && !gamepkg::local_event_state(automatic)?.signaled,
    )?;
    need(gamepkg::local_task_join(task)?.result() == 41)?;
    signal(automatic)?;
    need(gamepkg::local_wait_many(&combined, true, 0) == Err(-4))?;
    need(
        gamepkg::local_event_state(automatic)?.signaled
            && gamepkg::local_semaphore_state(semaphore)?.count == 3,
    )?;
    need(gamepkg::local_wait_many(&[item(automatic, 1), item(semaphore, 2)], false, 0) == Err(-1))?;
    need(gamepkg::local_wait_many(&[item(automatic, 1); 2], false, 0) == Err(-1))?;
    need(gamepkg::local_wait_many(&[item(automatic, 1); 65], true, 0) == Err(-1))?;
    need(gamepkg::local_wait_many(&pair, true, 1001) == Err(-1))?;
    let request = [
        u32::to_le_bytes(2).as_slice(),
        &u32::to_le_bytes(1),
        &u32::to_le_bytes(0),
        &u64::to_le_bytes(automatic),
        &u32::to_le_bytes(1),
        &u64::to_le_bytes(semaphore),
        &u32::to_le_bytes(3),
    ]
    .concat();
    for end in 0..request.len() {
        let mut out = [0u8; 8];
        need(gamepkg::local_call(47, &request[..end], &mut out) == Err(-1))?;
    }
    for capacity in 0..8usize {
        let mut out = [0u8; 8];
        need(gamepkg::local_call(47, &request, &mut out[..capacity]) == Err(-1))?;
    }
    let mut out = [0u8; 8];
    need(gamepkg::local_call(21, &[], &mut out)? == 8)?;
    let mutex = word(&out, 0)?;
    let initial = gamepkg::local_mutex_state(mutex)?;
    need(!initial.locked && !initial.owned_by_caller && !initial.abandoned && initial.depth == 0)?;
    for end in 0..8usize {
        let mut query = [0u8; 16];
        need(gamepkg::local_call(48, &u64::to_le_bytes(mutex)[..end], &mut query) == Err(-1))?;
    }
    need(gamepkg::local_sync_release(mutex, 1) == Err(-1))?;
    let task = spawn(1, mutex)?;
    gamepkg::local_wait_many(&[item(task, 4)], true, 1000)?;
    let abandoned = gamepkg::local_mutex_state(mutex)?;
    need(
        !abandoned.locked
            && !abandoned.owned_by_caller
            && abandoned.abandoned
            && abandoned.depth == 0,
    )?;
    need(gamepkg::local_mutex_lock(mutex, 0) == Err(-4))?;
    let result = gamepkg::local_wait_many(&[item(semaphore, 3), item(mutex, 2)], true, 0)?;
    need(result.abandoned && result.index == 1)?;
    let acquired = gamepkg::local_mutex_state(mutex)?;
    need(
        acquired.locked && acquired.owned_by_caller && !acquired.abandoned && acquired.depth == 1,
    )?;
    need(gamepkg::local_semaphore_state(semaphore)?.count == 2)?;
    gamepkg::local_call(24, &u64::to_le_bytes(mutex), &mut [])?;
    let unlocked = gamepkg::local_mutex_state(mutex)?;
    need(
        !unlocked.locked && !unlocked.owned_by_caller && !unlocked.abandoned && unlocked.depth == 0,
    )?;
    release(mutex, 2)?;
    need(gamepkg::local_mutex_state(mutex) == Err(-4))?;
    need(gamepkg::local_task_join(task)?.result() == 42)?;
    gamepkg::local_semaphore_wait(semaphore, 0)?;
    gamepkg::local_semaphore_wait(semaphore, 0)?;
    let ready = event(false)?;
    let mailbox = gamepkg::local_mailbox_create(16, 1)?;
    gamepkg::local_mailbox_send(
        mailbox,
        &[u64::to_le_bytes(semaphore), u64::to_le_bytes(ready)].concat(),
    )?;
    let task = spawn(2, mailbox)?;
    gamepkg::local_event_wait(ready, 1000)?;
    gamepkg::local_call(12, &u64::to_le_bytes(task), &mut [])?;
    gamepkg::local_semaphore_release(semaphore, 1)?;
    gamepkg::local_wait_many(&[item(task, 4)], true, 1000)?;
    need(gamepkg::local_task_join(task)?.result() == 43)?;
    need(gamepkg::local_semaphore_state(semaphore)?.count == 1)?;
    gamepkg::local_mailbox_release(mailbox)?;
    release(ready, 1)?;
    let mut many = std::vec::Vec::new();
    for _ in 0..64u32 {
        many.push(item(event(true)?, 1));
    }
    let result = gamepkg::local_wait_many(&many, true, 0)?;
    need(result.index == 0 && !result.abandoned)?;
    for object in many {
        release(object.id, 1)?;
    }
    release(automatic, 1)?;
    release(semaphore, 3)?;
    Ok(())
}
fn take_permits(argument: u64) -> Result<u64, i32> {
    need(gamepkg::local_semaphore_release(argument, 4)? == 0)?;
    Ok(41)
}
fn abandon(argument: u64) -> Result<u64, i32> {
    gamepkg::local_mutex_lock(argument, 0)?;
    gamepkg::local_mutex_lock(argument, 0)?;
    let held = gamepkg::local_mutex_state(argument)?;
    need(held.locked && held.owned_by_caller && !held.abandoned && held.depth == 2)?;
    Ok(42)
}
fn cancelled(argument: u64) -> Result<u64, i32> {
    let mut bytes = [0u8; 16];
    need(gamepkg::local_mailbox_receive(argument, 1000, &mut bytes)? == 16)?;
    let semaphore = word(&bytes, 0)?;
    signal(word(&bytes, 8)?)?;
    need(gamepkg::local_wait_many(&[item(semaphore, 3)], false, 1000) == Err(-4))?;
    need(gamepkg::local_mutex_state(semaphore) == Err(-4))?;
    Ok(43)
}
pub fn waitset_release_worker(argument: u64) -> u64 {
    match take_permits(argument) {
        Ok(v) => v,
        Err(code) => code as i64 as u64,
    }
}
pub fn waitset_abandon_worker(argument: u64) -> u64 {
    match abandon(argument) {
        Ok(v) => v,
        Err(code) => code as i64 as u64,
    }
}
pub fn waitset_cancel_worker(argument: u64) -> u64 {
    match cancelled(argument) {
        Ok(v) => v,
        Err(code) => code as i64 as u64,
    }
}
pub fn game_main() -> u32 {
    let code = match run() {
        Ok(()) => 0u64,
        Err(code) => code as i64 as u64,
    };
    match gamepkg::diagnostic(1, 0x575354ff, &[code]) {
        Ok(_) => 0,
        Err(_) => 1,
    }
}
