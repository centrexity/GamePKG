// Ordinary checked game source. The first remote request is the final report.
fn word(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let value = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        value[0], value[1], value[2], value[3], value[4], value[5], value[6], value[7],
    ]))
}
pub fn deadline_worker(mailbox: u64) -> u64 {
    match deadline_actor(mailbox) {
        Ok(value) => value,
        Err(_) => 0,
    }
}
fn deadline_actor(mailbox: u64) -> Result<u64, i32> {
    let state = gamepkg::local_clock_state()?;
    let mut message = std::vec::Vec::new();
    message.extend(u64::to_le_bytes(state.sample.counter));
    message.extend(u64::to_le_bytes(state.sample.frequency));
    message.extend(u64::to_le_bytes(state.generation));
    gamepkg::local_mailbox_send(mailbox, &message)?;
    let result = gamepkg::local_clock_wait(
        state.sample.counter + state.sample.frequency,
        state.sample.frequency,
        state.generation,
    )?;
    if result.reason != 1 || result.state.generation != state.generation + 1 {
        return Err(-3);
    }
    Ok(result.state.generation)
}
fn verify() -> Result<(), i32> {
    verify_events()?;
    let initial = gamepkg::local_clock_state()?;
    if initial.generation != 0 || initial.sample.frequency == 0 {
        return Err(-3);
    }
    let raw = gamepkg::local_performance_sample()?;
    if raw.frequency != initial.sample.frequency || raw.counter < initial.sample.counter {
        return Err(-3);
    }
    if gamepkg::local_clock_wait(0, raw.frequency, 1) != Err(-1)
        || gamepkg::local_clock_wait(0, 0, 0) != Err(-1)
        || gamepkg::local_clock_wait(raw.counter + raw.frequency * 3, raw.frequency, 0) != Err(-1)
        || gamepkg::local_clock_wake()? != 1
    {
        return Err(-3);
    }
    let early = gamepkg::local_clock_wait(0, raw.frequency, 0)?;
    if early.reason != 1 || early.state.generation != 1 {
        return Err(-3);
    }
    let state = gamepkg::local_clock_state()?;
    let deadline = state.sample.counter + state.sample.frequency.div_ceil(500);
    let expiry = gamepkg::local_clock_wait(deadline, state.sample.frequency, 1)?;
    if expiry.reason != 0 || expiry.state.sample.counter < deadline {
        return Err(-3);
    }
    let mailbox = gamepkg::local_mailbox_create(24, 1)?;
    let mut request = std::vec::Vec::new();
    request.extend(u32::to_le_bytes(0));
    request.extend(u64::to_le_bytes(mailbox));
    request.extend(u32::to_le_bytes(0));
    let mut task = [0u8; 8];
    if gamepkg::local_call(10, &request, &mut task)? != 8 {
        return Err(-3);
    }
    let mut message = [0u8; 24];
    if gamepkg::local_mailbox_receive(mailbox, 1000, &mut message)? != 24 {
        return Err(-3);
    }
    let counter = word(&message, 0)?;
    let frequency = word(&message, 8)?;
    let generation = word(&message, 16)?;
    loop {
        match gamepkg::local_clock_wait(0, frequency, generation) {
            Err(-2) => break,
            Ok(value) => {
                if value.reason != 0 || value.state.sample.counter - counter > frequency / 2 {
                    return Err(-3);
                }
            }
            Err(code) => return Err(code),
        }
    }
    if gamepkg::local_clock_wake()? != generation + 1 {
        return Err(-3);
    }
    let mut result = [0u8; 8];
    loop {
        match gamepkg::local_call(11, &task, &mut result) {
            Ok(8) => break,
            Err(-2) => {
                if gamepkg::local_clock_state()?.sample.counter - counter > frequency {
                    return Err(-3);
                }
            }
            _ => return Err(-3),
        }
    }
    if u64::from_le_bytes(result) != generation + 1 {
        return Err(-3);
    }
    gamepkg::local_mailbox_release(mailbox)?;
    if gamepkg::local_clock_state()?.generation != 2 {
        return Err(-3);
    }
    Ok(())
}

pub fn event_worker(request: u64) -> u64 {
    match event_actor(request) {
        Ok(value) => value,
        Err(code) => code as i64 as u64,
    }
}
fn event_actor(request: u64) -> Result<u64, i32> {
    let mut b = [0u8; 16];
    if gamepkg::local_mailbox_receive(request, 1000, &mut b)? != 16 {
        return Err(-3);
    }
    let event = word(&b, 0)?;
    let ready = word(&b, 8)?;
    gamepkg::local_mailbox_send(ready, &[1u8])?;
    if gamepkg::local_event_wait(event, 1000) != Err(-4)
        || gamepkg::local_event_state(event) != Err(-4)
    {
        return Err(-3);
    }
    Ok(123)
}
fn verify_events() -> Result<(), i32> {
    let mut b = [0u8; 8];
    if gamepkg::local_call(21, &[], &mut b)? != 8 {
        return Err(-3);
    }
    let mutex = u64::from_le_bytes(b);
    if gamepkg::local_event_state(mutex) != Err(-1)
        || gamepkg::local_event_wait(mutex, 0) != Err(-1)
    {
        return Err(-3);
    }
    gamepkg::local_mutex_lock(mutex, 0)?;
    gamepkg::local_call(24, &b, &mut [])?;
    gamepkg::local_call(25, &b, &mut [])?;
    for manual in [0u32, 1] {
        if gamepkg::local_call(
            20,
            &[u32::to_le_bytes(manual), u32::to_le_bytes(1)].concat(),
            &mut b,
        )? != 8
        {
            return Err(-3);
        }
        let event = u64::from_le_bytes(b);
        for _ in 0..2u32 {
            let state = gamepkg::local_event_state(event)?;
            if state.manual != (manual != 0) || !state.signaled {
                return Err(-3);
            }
        }
        gamepkg::local_event_wait(event, 0)?;
        if gamepkg::local_event_state(event)?.signaled != (manual != 0) {
            return Err(-3);
        }
        gamepkg::local_call(25, &b, &mut [])?;
        if gamepkg::local_event_state(event) != Err(-4)
            || gamepkg::local_event_wait(event, 0) != Err(-4)
        {
            return Err(-3);
        }
    }
    if gamepkg::local_call(20, &[0u8; 8], &mut b)? != 8 {
        return Err(-3);
    }
    let event = u64::from_le_bytes(b);
    let request = gamepkg::local_mailbox_create(16, 1)?;
    let ready = gamepkg::local_mailbox_create(1, 1)?;
    gamepkg::local_mailbox_send(
        request,
        &[u64::to_le_bytes(event), u64::to_le_bytes(ready)].concat(),
    )?;
    if gamepkg::local_call(
        10,
        &[
            u32::to_le_bytes(1).as_slice(),
            &u64::to_le_bytes(request),
            &u32::to_le_bytes(0),
        ]
        .concat(),
        &mut b,
    )? != 8
    {
        return Err(-3);
    }
    let task = u64::from_le_bytes(b);
    let mut signal = [0u8; 1];
    if gamepkg::local_mailbox_receive(ready, 1000, &mut signal)? != 1 || signal != [1] {
        return Err(-3);
    }
    gamepkg::local_call(12, &u64::to_le_bytes(task), &mut [])?;
    for _ in 0..100000u32 {
        if gamepkg::local_task_finished(task)? {
            break;
        }
        let _ = gamepkg::local_performance_sample()?;
    }
    if gamepkg::local_call(11, &u64::to_le_bytes(task), &mut b)? != 8
        || u64::from_le_bytes(b) != 123
    {
        return Err(-3);
    }
    gamepkg::local_call(25, &u64::to_le_bytes(event), &mut [])?;
    gamepkg::local_mailbox_release(request)?;
    gamepkg::local_mailbox_release(ready)?;
    Ok(())
}
pub fn game_main() {
    let status = match verify() {
        Ok(()) => 0u64,
        Err(code) => code as i64 as u64,
    };
    let _ = gamepkg::diagnostic(1, 0x434c4bff, &[status]);
}
