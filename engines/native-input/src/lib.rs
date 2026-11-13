//! Shared native input state. Backend adapters own OS event pumping; this
//! service owns device generations, player slots, focus and bounded delivery.
#![deny(unsafe_op_in_unsafe_fn)]
use std::collections::{BTreeMap, VecDeque};
use std::sync::{LazyLock, Mutex};

pub const MAX_DEVICES: usize = 16;
pub const MAX_PLAYERS: usize = 4;
pub const MAX_EVENTS: usize = 2048;
pub const MAX_TOUCHES: usize = 16;
pub const KEYBOARD: u32 = 1;
pub const MOUSE: u32 = 2;
pub const GAMEPAD: u32 = 3;
pub const TOUCH: u32 = 4;
pub const BUTTON_MASK: u32 = 0x1f_ffff;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Event {
    pub device: u64,
    pub timestamp_ns: u64,
    pub kind: u32,
    pub code: u32,
    pub x: f32,
    pub y: f32,
    pub contact: u64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Gamepad {
    pub device: u64,
    pub timestamp_ns: u64,
    pub buttons: u32,
    pub left_x: i16,
    pub left_y: i16,
    pub right_x: i16,
    pub right_y: i16,
    pub left_trigger: u16,
    pub right_trigger: u16,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pointer {
    pub device: u64,
    pub timestamp_ns: u64,
    pub x: f32,
    pub y: f32,
    pub buttons: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Contact {
    pub device: u64,
    pub timestamp_ns: u64,
    pub contact: u64,
    pub x: f32,
    pub y: f32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DeviceInfo {
    pub device: u64,
    pub backend_token: u64,
    pub kind: u32,
    pub backend: u32,
    pub player: i32,
    pub reserved: u32,
}
const _: () = {
    assert!(std::mem::size_of::<Event>() == 40);
    assert!(std::mem::size_of::<Gamepad>() == 32);
    assert!(std::mem::size_of::<Pointer>() == 32);
    assert!(std::mem::size_of::<Contact>() == 32);
    assert!(std::mem::size_of::<DeviceInfo>() == 32);
    assert!(std::mem::offset_of!(Event, contact) == 32);
    assert!(std::mem::offset_of!(Gamepad, left_x) == 20);
};
struct Device {
    kind: u32,
    backend: u32,
    token: u64,
    timestamp: u64,
    keys: [u64; 8],
    blocked_keys: [u64; 8],
    pad: Gamepad,
    blocked_buttons: u32,
    sample_after_reset: bool,
    pointer: Pointer,
    contacts: [Contact; MAX_TOUCHES],
}
impl Device {
    fn new(kind: u32, backend: u32, token: u64) -> Self {
        Self {
            kind,
            backend,
            token,
            timestamp: 0,
            keys: [0; 8],
            blocked_keys: [0; 8],
            pad: Gamepad::default(),
            blocked_buttons: 0,
            sample_after_reset: false,
            pointer: Pointer::default(),
            contacts: [Contact::default(); MAX_TOUCHES],
        }
    }
    fn suppress_held(&mut self) {
        for (blocked, held) in self.blocked_keys.iter_mut().zip(self.keys) {
            *blocked |= held;
        }
        self.blocked_buttons |= self.pad.buttons | self.pointer.buttons;
        self.sample_after_reset = true;
        self.contacts.fill(Contact::default());
    }
    fn accepts_time(&self, time: u64) -> bool {
        time == 0 || self.timestamp <= time
    }
    fn note_time(&mut self, time: u64) {
        self.timestamp = self.timestamp.max(time);
    }
}
pub struct Service {
    devices: BTreeMap<u64, Device>,
    next: u64,
    players: [u64; MAX_PLAYERS],
    focused: bool,
    events: VecDeque<Event>,
}
impl Default for Service {
    fn default() -> Self {
        Self::new()
    }
}
impl Service {
    pub fn new() -> Self {
        Self {
            devices: BTreeMap::new(),
            next: 0,
            players: [0; MAX_PLAYERS],
            focused: false,
            events: VecDeque::with_capacity(MAX_EVENTS),
        }
    }
    fn emit(&mut self, event: Event) -> bool {
        if self.events.len() == MAX_EVENTS {
            // Snapshot consumers must keep held acceleration/steering even
            // when no event consumer is present. Event consumers resample
            // authoritative state after RESET without inventing new presses.
            self.events.clear();
            self.events.push_back(Event::default());
            return false;
        }
        self.events.push_back(event);
        true
    }
    fn reset_held(&mut self) {
        for device in self.devices.values_mut() {
            device.suppress_held();
        }
        self.events.clear();
        self.events.push_back(Event::default());
    }
    pub fn focus(&mut self, focused: bool) {
        if self.focused == focused {
            return;
        }
        self.focused = focused;
        if !focused {
            self.reset_held();
        }
        self.emit(Event {
            kind: 11,
            code: u32::from(focused),
            ..Event::default()
        });
    }
    pub fn connect(&mut self, kind: u32, backend: u32, token: u64) -> Option<u64> {
        if !(KEYBOARD..=TOUCH).contains(&kind) || backend == 0 {
            return None;
        }
        if let Some((&id, _)) = self
            .devices
            .iter()
            .find(|(_, d)| d.kind == kind && d.backend == backend && d.token == token)
        {
            return Some(id);
        }
        if self.devices.len() == MAX_DEVICES {
            return None;
        }
        let id = self.next.checked_add(1)?;
        self.next = id;
        let mut device = Device::new(kind, backend, token);
        device.sample_after_reset = !self.focused;
        self.devices.insert(id, device);
        self.emit(Event {
            device: id,
            kind: 12,
            code: kind,
            ..Event::default()
        });
        Some(id)
    }
    pub fn disconnect(&mut self, id: u64) -> bool {
        let Some(device) = self.devices.remove(&id) else {
            return false;
        };
        for player in &mut self.players {
            if *player == id {
                *player = 0;
            }
        }
        self.emit(Event {
            device: id,
            kind: 13,
            code: device.kind,
            ..Event::default()
        });
        true
    }
    pub fn devices(&self) -> Vec<DeviceInfo> {
        self.devices
            .iter()
            .map(|(&id, d)| DeviceInfo {
                device: id,
                backend_token: d.token,
                kind: d.kind,
                backend: d.backend,
                player: self
                    .players
                    .iter()
                    .position(|&p| p == id)
                    .map_or(-1, |i| i as i32),
                reserved: 0,
            })
            .collect()
    }
    pub fn gamepad(&self, player: usize) -> Option<Gamepad> {
        let d = self.devices.get(self.players.get(player)?)?;
        let mut state = d.pad;
        if self.focused {
            state.buttons &= !d.blocked_buttons;
        } else {
            state = Gamepad {
                device: state.device,
                timestamp_ns: state.timestamp_ns,
                ..Gamepad::default()
            };
        }
        Some(state)
    }
    pub fn key(&self, id: u64, code: u32) -> Option<bool> {
        if !(1..512).contains(&code) {
            return None;
        }
        let d = self.devices.get(&id)?;
        if d.kind != KEYBOARD {
            return None;
        }
        let bit = 1u64 << (code % 64);
        Some(
            self.focused
                && d.keys[code as usize / 64] & !d.blocked_keys[code as usize / 64] & bit != 0,
        )
    }
    pub fn pointer(&self, id: u64) -> Option<Pointer> {
        let d = self.devices.get(&id)?;
        if d.kind != MOUSE {
            return None;
        }
        let mut state = d.pointer;
        state.device = id;
        state.buttons = if self.focused {
            state.buttons & !d.blocked_buttons
        } else {
            0
        };
        Some(state)
    }
    pub fn contacts(&self) -> Vec<Contact> {
        self.devices
            .values()
            .flat_map(|d| d.contacts.iter().copied())
            .filter(|c| c.device != 0)
            .collect()
    }
    pub fn update_gamepad(&mut self, state: Gamepad) -> bool {
        let Some(d) = self.devices.get_mut(&state.device) else {
            return false;
        };
        if d.kind != GAMEPAD
            || state.buttons & !BUTTON_MASK != 0
            || !d.accepts_time(state.timestamp_ns)
        {
            return false;
        }
        let old = d.pad;
        let old_buttons = if self.focused {
            old.buttons & !d.blocked_buttons
        } else {
            0
        };
        d.blocked_buttons &= state.buttons;
        if self.focused && d.sample_after_reset {
            d.blocked_buttons |= state.buttons;
            d.sample_after_reset = false;
        }
        if !self.focused {
            d.blocked_buttons |= state.buttons;
        }
        d.pad = state;
        d.note_time(state.timestamp_ns);
        let buttons = if self.focused {
            state.buttons & !d.blocked_buttons
        } else {
            0
        };
        if !self.focused {
            return true;
        }
        if buttons & !old.buttons != 0 && !self.players.contains(&state.device) {
            if let Some(player) = self.players.iter().position(|&p| p == 0) {
                self.players[player] = state.device;
                if !self.emit(Event {
                    device: state.device,
                    timestamp_ns: state.timestamp_ns,
                    kind: 18,
                    code: player as u32,
                    ..Event::default()
                }) {
                    return true;
                }
            }
        }
        for bit in 0..21 {
            let mask = 1 << bit;
            if (old_buttons ^ buttons) & mask != 0
                && !self.emit(Event {
                    device: state.device,
                    timestamp_ns: state.timestamp_ns,
                    kind: if buttons & mask != 0 { 14 } else { 15 },
                    code: bit,
                    ..Event::default()
                })
            {
                return true;
            }
        }
        let axes = [
            state.left_x as f32,
            state.left_y as f32,
            state.right_x as f32,
            state.right_y as f32,
            state.left_trigger as f32,
            state.right_trigger as f32,
        ];
        let old_axes = [
            old.left_x as f32,
            old.left_y as f32,
            old.right_x as f32,
            old.right_y as f32,
            old.left_trigger as f32,
            old.right_trigger as f32,
        ];
        for (axis, (&value, &before)) in axes.iter().zip(&old_axes).enumerate() {
            if value != before
                && !self.emit(Event {
                    device: state.device,
                    timestamp_ns: state.timestamp_ns,
                    kind: 16,
                    code: axis as u32,
                    x: value,
                    y: 0.0,
                    contact: 0,
                })
            {
                return true;
            }
        }
        true
    }
    pub fn submit(&mut self, event: Event) -> bool {
        if !event.x.is_finite()
            || !event.y.is_finite()
            || (!(8..=10).contains(&event.kind) && event.contact != 0)
        {
            return false;
        }
        let Some(d) = self.devices.get_mut(&event.device) else {
            return false;
        };
        if !d.accepts_time(event.timestamp_ns) {
            return false;
        }
        let deliver = match (d.kind, event.kind) {
            (KEYBOARD, 1 | 2)
                if (1..512).contains(&event.code) && event.x == 0.0 && event.y == 0.0 =>
            {
                let word = event.code as usize / 64;
                let bit = 1u64 << (event.code % 64);
                let before = d.keys[word] & !d.blocked_keys[word] & bit != 0;
                if event.kind == 1 {
                    d.keys[word] |= bit;
                    if !self.focused {
                        d.blocked_keys[word] |= bit;
                    }
                } else {
                    d.keys[word] &= !bit;
                    d.blocked_keys[word] &= !bit;
                }
                let after = d.keys[word] & !d.blocked_keys[word] & bit != 0;
                self.focused && before != after
            }
            (KEYBOARD, 17)
                if event.code != 0
                    && char::from_u32(event.code).is_some()
                    && event.x == 0.0
                    && event.y == 0.0 =>
            {
                self.focused
            }
            (MOUSE, 3 | 4 | 7) if event.code == 0 => {
                if event.kind == 3 {
                    d.pointer.x = event.x;
                    d.pointer.y = event.y;
                }
                d.pointer.timestamp_ns = event.timestamp_ns;
                self.focused
            }
            (MOUSE, 5 | 6) if (1..=5).contains(&event.code) && event.x == 0.0 && event.y == 0.0 => {
                let bit = 1 << (event.code - 1);
                let before = d.pointer.buttons & !d.blocked_buttons & bit != 0;
                if event.kind == 5 {
                    d.pointer.buttons |= bit;
                    if !self.focused {
                        d.blocked_buttons |= bit;
                    }
                } else {
                    d.pointer.buttons &= !bit;
                    d.blocked_buttons &= !bit;
                }
                d.pointer.timestamp_ns = event.timestamp_ns;
                self.focused && before != (d.pointer.buttons & !d.blocked_buttons & bit != 0)
            }
            (TOUCH, 8..=10)
                if event.code == 0
                    && (0.0..=1.0).contains(&event.x)
                    && (0.0..=1.0).contains(&event.y) =>
            {
                if !self.focused {
                    return true;
                }
                let existing = d
                    .contacts
                    .iter()
                    .position(|c| c.device != 0 && c.contact == event.contact);
                let index = if event.kind == 8 {
                    if existing.is_some() {
                        return false;
                    }
                    let Some(free) = d.contacts.iter().position(|c| c.device == 0) else {
                        return false;
                    };
                    free
                } else {
                    let Some(index) = existing else {
                        return false;
                    };
                    index
                };
                d.contacts[index] = if event.kind == 10 {
                    Contact::default()
                } else {
                    Contact {
                        device: event.device,
                        timestamp_ns: event.timestamp_ns,
                        contact: event.contact,
                        x: event.x,
                        y: event.y,
                    }
                };
                true
            }
            _ => return false,
        };
        d.note_time(event.timestamp_ns);
        if deliver {
            self.emit(event);
        }
        true
    }
    pub fn drain(&mut self, capacity: usize) -> Vec<Event> {
        self.events
            .drain(..capacity.min(self.events.len()))
            .collect()
    }
}

static INPUT: LazyLock<Mutex<Service>> = LazyLock::new(|| Mutex::new(Service::new()));
fn lock() -> std::sync::MutexGuard<'static, Service> {
    INPUT.lock().unwrap_or_else(|p| p.into_inner())
}
#[no_mangle]
pub extern "C" fn gamepkg_input_connect(kind: u32, backend: u32, token: u64) -> u64 {
    lock().connect(kind, backend, token).unwrap_or(0)
}
#[no_mangle]
pub extern "C" fn gamepkg_input_disconnect(device: u64) -> i32 {
    i32::from(lock().disconnect(device))
}
#[no_mangle]
pub extern "C" fn gamepkg_input_focus(focused: u32) -> i32 {
    if focused > 1 {
        return -1;
    }
    lock().focus(focused != 0);
    1
}
#[no_mangle]
pub extern "C" fn gamepkg_input_key_state(device: u64, code: u32) -> i32 {
    lock().key(device, code).map_or(-1, i32::from)
}
/// # Safety
/// `state` must reference one readable Gamepad. Unaligned storage is supported.
#[no_mangle]
pub unsafe extern "C" fn gamepkg_input_update_gamepad(state: *const Gamepad) -> i32 {
    if state.is_null() {
        return -1;
    }
    let state = unsafe { state.read_unaligned() };
    i32::from(lock().update_gamepad(state))
}
/// # Safety
/// `event` must reference one readable Event. Unaligned storage is supported.
#[no_mangle]
pub unsafe extern "C" fn gamepkg_input_submit(event: *const Event) -> i32 {
    if event.is_null() {
        return -1;
    }
    let event = unsafe { event.read_unaligned() };
    i32::from(lock().submit(event))
}
/// # Safety
/// `out` must reference one writable Gamepad. Invalid player indices leave it unchanged.
#[no_mangle]
pub unsafe extern "C" fn gamepkg_input_gamepad_state(player: u32, out: *mut Gamepad) -> i32 {
    if out.is_null() || player as usize >= MAX_PLAYERS {
        return -1;
    }
    let state = lock().gamepad(player as usize);
    unsafe { out.write_unaligned(state.unwrap_or_default()) };
    i32::from(state.is_some())
}
/// # Safety
/// `out` must reference one writable Pointer. Invalid device handles leave it unchanged.
#[no_mangle]
pub unsafe extern "C" fn gamepkg_input_pointer_state(device: u64, out: *mut Pointer) -> i32 {
    if out.is_null() {
        return -1;
    }
    let Some(state) = lock().pointer(device) else {
        return -1;
    };
    unsafe { out.write_unaligned(state) };
    1
}
/// # Safety
/// When capacity is nonzero, out must reference capacity writable Event records.
#[no_mangle]
pub unsafe extern "C" fn gamepkg_input_read(out: *mut Event, capacity: u32) -> i32 {
    if capacity as usize > MAX_EVENTS || (out.is_null() && capacity != 0) {
        return -1;
    }
    let mut input = lock();
    let count = (capacity as usize).min(input.events.len());
    for index in 0..count {
        let event = input.events.pop_front().unwrap();
        unsafe { out.add(index).write_unaligned(event) };
    }
    count as i32
}
/// # Safety
/// When capacity is nonzero, out must reference capacity writable DeviceInfo records.
/// A zero capacity queries the count. Insufficient capacity writes nothing and returns -1.
#[no_mangle]
pub unsafe extern "C" fn gamepkg_input_devices(out: *mut DeviceInfo, capacity: u32) -> i32 {
    let input = lock();
    let count = input.devices.len();
    if capacity == 0 {
        return count as i32;
    }
    if out.is_null() || capacity as usize > MAX_DEVICES || (capacity as usize) < count {
        return -1;
    }
    for (index, info) in input.devices().into_iter().enumerate() {
        unsafe { out.add(index).write_unaligned(info) };
    }
    count as i32
}
/// # Safety
/// When capacity is nonzero, out must reference capacity writable Contact records.
/// A zero capacity queries the count. Insufficient capacity writes nothing and returns -1.
#[no_mangle]
pub unsafe extern "C" fn gamepkg_input_contacts(out: *mut Contact, capacity: u32) -> i32 {
    let input = lock();
    let contacts = input.contacts();
    if capacity == 0 {
        return contacts.len() as i32;
    }
    if out.is_null()
        || capacity as usize > MAX_DEVICES * MAX_TOUCHES
        || (capacity as usize) < contacts.len()
    {
        return -1;
    }
    for (index, contact) in contacts.iter().enumerate() {
        unsafe { out.add(index).write_unaligned(*contact) };
    }
    contacts.len() as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event(device: u64, kind: u32, code: u32) -> Event {
        Event {
            device,
            kind,
            code: if (8..=10).contains(&kind) { 0 } else { code },
            contact: if (8..=10).contains(&kind) {
                u64::from(code)
            } else {
                0
            },
            ..Event::default()
        }
    }
    fn active() -> Service {
        let mut s = Service::new();
        s.focus(true);
        s.drain(MAX_EVENTS);
        s
    }
    #[test]
    fn physical_order_does_not_choose_player_and_hotplug_never_reuses_handles() {
        let mut s = active();
        let idle = s.connect(GAMEPAD, 3, 0).unwrap();
        let used = s.connect(GAMEPAD, 3, 3).unwrap();
        assert!(s.update_gamepad(Gamepad {
            device: idle,
            left_x: i16::MAX,
            ..Gamepad::default()
        }));
        assert!(s.gamepad(0).is_none());
        assert!(s.update_gamepad(Gamepad {
            device: used,
            buttons: 1 << 12,
            ..Gamepad::default()
        }));
        assert_eq!(s.gamepad(0).unwrap().device, used);
        assert!(s.update_gamepad(Gamepad {
            device: idle,
            buttons: 1 << 13,
            ..Gamepad::default()
        }));
        assert_eq!(s.gamepad(1).unwrap().device, idle);
        assert!(s.disconnect(used));
        assert!(s.gamepad(0).is_none());
        let replacement = s.connect(GAMEPAD, 3, 3).unwrap();
        assert_ne!(replacement, used);
        assert!(!s.update_gamepad(Gamepad {
            device: used,
            buttons: 1,
            ..Gamepad::default()
        }));
        assert!(s.update_gamepad(Gamepad {
            device: replacement,
            buttons: 1,
            ..Gamepad::default()
        }));
        assert_eq!(s.gamepad(0).unwrap().device, replacement);
        assert_eq!(s.gamepad(1).unwrap().device, idle);
        assert_eq!(s.connect(GAMEPAD, 3, 3), Some(replacement));
    }
    #[test]
    fn every_raw_axis_and_trigger_value_survives_without_a_second_deadzone() {
        let mut s = active();
        let id = s.connect(GAMEPAD, 1, 0).unwrap();
        s.update_gamepad(Gamepad {
            device: id,
            buttons: 1,
            ..Gamepad::default()
        });
        s.drain(MAX_EVENTS);
        for raw in 0..=u16::MAX {
            let expected = Gamepad {
                device: id,
                timestamp_ns: u64::from(raw) + 1,
                buttons: 0,
                left_x: raw as i16,
                left_y: (raw ^ 0x8000) as i16,
                right_x: raw.wrapping_add(1) as i16,
                right_y: raw.wrapping_sub(1) as i16,
                left_trigger: raw,
                right_trigger: !raw,
            };
            assert!(s.update_gamepad(expected));
            assert_eq!(s.gamepad(0), Some(expected));
            s.drain(MAX_EVENTS);
        }
    }
    #[test]
    fn focus_loss_releases_all_devices_and_held_buttons_need_a_real_release() {
        let mut s = active();
        let key = s.connect(KEYBOARD, 1, 0).unwrap();
        let mouse = s.connect(MOUSE, 1, 0).unwrap();
        let pad = s.connect(GAMEPAD, 1, 0).unwrap();
        let touch = s.connect(TOUCH, 1, 0).unwrap();
        s.submit(event(key, 1, 4));
        s.submit(event(mouse, 5, 1));
        s.submit(Event {
            x: 0.5,
            y: 0.25,
            ..event(touch, 8, 17)
        });
        s.update_gamepad(Gamepad {
            device: pad,
            buttons: 1,
            left_x: 123,
            ..Gamepad::default()
        });
        s.focus(false);
        assert_eq!(s.key(key, 4), Some(false));
        assert_eq!(s.pointer(mouse).unwrap().buttons, 0);
        assert_eq!(s.gamepad(0).unwrap().buttons, 0);
        assert_eq!(s.gamepad(0).unwrap().left_x, 0);
        assert!(s.contacts().is_empty());
        assert_eq!(
            s.drain(MAX_EVENTS)
                .iter()
                .map(|e| e.kind)
                .collect::<Vec<_>>(),
            vec![0, 11]
        );
        s.focus(true);
        s.submit(event(key, 1, 4));
        s.submit(event(mouse, 5, 1));
        s.update_gamepad(Gamepad {
            device: pad,
            buttons: 1,
            left_x: 123,
            ..Gamepad::default()
        });
        assert_eq!(s.key(key, 4), Some(false));
        assert_eq!(s.pointer(mouse).unwrap().buttons, 0);
        assert_eq!(s.gamepad(0).unwrap().buttons, 0);
        assert_eq!(s.gamepad(0).unwrap().left_x, 123);
        assert!(!s.submit(Event {
            x: 0.5,
            y: 0.25,
            ..event(touch, 9, 17)
        }));
        s.submit(event(key, 2, 4));
        s.submit(event(key, 1, 4));
        s.submit(event(mouse, 6, 1));
        s.submit(event(mouse, 5, 1));
        s.update_gamepad(Gamepad {
            device: pad,
            ..Gamepad::default()
        });
        s.update_gamepad(Gamepad {
            device: pad,
            buttons: 1,
            ..Gamepad::default()
        });
        assert_eq!(s.key(key, 4), Some(true));
        assert_eq!(s.pointer(mouse).unwrap().buttons, 1);
        assert_eq!(s.gamepad(0).unwrap().buttons, 1);
    }
    #[test]
    fn background_presses_never_claim_a_player_or_replay_after_focus() {
        let mut s = Service::new();
        let id = s.connect(GAMEPAD, 1, 0).unwrap();
        s.update_gamepad(Gamepad {
            device: id,
            buttons: 1,
            ..Gamepad::default()
        });
        s.focus(true);
        s.update_gamepad(Gamepad {
            device: id,
            buttons: 1,
            ..Gamepad::default()
        });
        assert!(s.gamepad(0).is_none());
        s.update_gamepad(Gamepad {
            device: id,
            ..Gamepad::default()
        });
        s.update_gamepad(Gamepad {
            device: id,
            buttons: 1,
            ..Gamepad::default()
        });
        assert_eq!(s.gamepad(0).unwrap().device, id);
    }
    #[test]
    fn ordered_edges_unicode_relative_motion_and_multiple_contacts_are_kept() {
        let mut s = active();
        let key = s.connect(KEYBOARD, 1, 0).unwrap();
        let mouse = s.connect(MOUSE, 1, 0).unwrap();
        let touch = s.connect(TOUCH, 1, 0).unwrap();
        s.drain(MAX_EVENTS);
        let down = event(key, 1, 4);
        let up = event(key, 2, 4);
        assert!(s.submit(down));
        assert!(s.submit(down));
        assert!(s.submit(up));
        assert!(s.submit(event(key, 17, 0x1f600)));
        let motion = Event {
            x: -13.5,
            y: 123456.0,
            ..event(mouse, 4, 0)
        };
        assert!(s.submit(motion));
        let a = Event {
            contact: 0x1_0000_0009,
            x: 0.25,
            y: 0.75,
            ..event(touch, 8, 9)
        };
        let b = Event {
            contact: 0x2_0000_0009,
            x: 1.0,
            y: 0.0,
            ..event(touch, 8, 10)
        };
        assert!(s.submit(a));
        assert!(s.submit(b));
        assert_eq!(s.contacts().len(), 2);
        assert!(!s.submit(a));
        assert!(s.submit(Event { kind: 10, ..a }));
        assert_eq!(s.contacts()[0].contact, 0x2_0000_0009);
        let delivered = s.drain(MAX_EVENTS);
        assert_eq!(
            &delivered[..5],
            &[down, up, event(key, 17, 0x1f600), motion, a]
        );
        assert_eq!(s.drain(MAX_EVENTS), vec![]);
    }
    #[test]
    fn overflow_preserves_snapshot_controls_and_requires_event_resampling() {
        let mut s = active();
        let key = s.connect(KEYBOARD, 1, 0).unwrap();
        let mouse = s.connect(MOUSE, 1, 0).unwrap();
        let pad = s.connect(GAMEPAD, 1, 0).unwrap();
        s.drain(MAX_EVENTS);
        s.submit(event(key, 1, 4));
        s.update_gamepad(Gamepad {
            device: pad,
            buttons: 1,
            ..Gamepad::default()
        });
        s.drain(MAX_EVENTS);
        for _ in 0..=MAX_EVENTS {
            assert!(s.submit(Event {
                x: 1.0,
                ..event(mouse, 4, 0)
            }));
        }
        assert_eq!(s.drain(MAX_EVENTS), vec![Event::default()]);
        assert_eq!(s.key(key, 4), Some(true));
        assert_eq!(s.gamepad(0).unwrap().device, pad);
        assert_eq!(s.gamepad(0).unwrap().buttons, 1);
        s.submit(event(key, 2, 4));
        s.submit(event(key, 1, 4));
        assert_eq!(s.key(key, 4), Some(true));
    }
    #[test]
    fn first_focused_sample_cannot_replay_an_unobserved_background_press() {
        let mut s = active();
        let pad = s.connect(GAMEPAD, 1, 0).unwrap();
        s.focus(false);
        s.focus(true);
        s.update_gamepad(Gamepad {
            device: pad,
            buttons: 1,
            ..Gamepad::default()
        });
        assert!(s.gamepad(0).is_none());
        s.update_gamepad(Gamepad {
            device: pad,
            ..Gamepad::default()
        });
        s.update_gamepad(Gamepad {
            device: pad,
            buttons: 1,
            ..Gamepad::default()
        });
        assert_eq!(s.gamepad(0).unwrap().buttons, 1);
        let mut startup = Service::new();
        let pad = startup.connect(GAMEPAD, 1, 0).unwrap();
        startup.focus(true);
        startup.update_gamepad(Gamepad {
            device: pad,
            buttons: 1,
            ..Gamepad::default()
        });
        assert!(startup.gamepad(0).is_none());
        startup.update_gamepad(Gamepad {
            device: pad,
            ..Gamepad::default()
        });
        startup.update_gamepad(Gamepad {
            device: pad,
            buttons: 1,
            ..Gamepad::default()
        });
        assert_eq!(startup.gamepad(0).unwrap().buttons, 1);
    }
    #[test]
    fn snapshot_only_analog_polling_keeps_held_acceleration_after_many_overflows() {
        let mut s = active();
        let pad = s.connect(GAMEPAD, 1, 0).unwrap();
        for tick in 1..10000i16 {
            let state = Gamepad {
                device: pad,
                timestamp_ns: tick as u64,
                buttons: 1,
                left_x: tick,
                right_trigger: u16::MAX,
                ..Gamepad::default()
            };
            assert!(s.update_gamepad(state));
            assert_eq!(s.gamepad(0), Some(state));
        }
        let events = s.drain(MAX_EVENTS);
        assert!(events.iter().any(|e| e.kind == 0));
    }
    #[test]
    fn malformed_wrong_device_and_stale_updates_do_not_change_state() {
        let mut s = active();
        let key = s.connect(KEYBOARD, 1, 0).unwrap();
        let pad = s.connect(GAMEPAD, 1, 0).unwrap();
        let touch = s.connect(TOUCH, 1, 0).unwrap();
        s.drain(MAX_EVENTS);
        for e in [
            event(key, 0, 0),
            event(key, 1, 0),
            event(key, 1, 512),
            event(key, 17, 0xd800),
            event(key, 17, 0),
            event(pad, 1, 4),
            event(999, 1, 4),
            Event {
                x: f32::NAN,
                ..event(key, 1, 4)
            },
            Event {
                x: 1.1,
                ..event(touch, 8, 0)
            },
        ] {
            assert!(!s.submit(e));
        }
        assert!(!s.update_gamepad(Gamepad {
            device: pad,
            buttons: !BUTTON_MASK,
            ..Gamepad::default()
        }));
        assert!(!s.update_gamepad(Gamepad {
            device: key,
            ..Gamepad::default()
        }));
        assert!(s.submit(Event {
            timestamp_ns: 100,
            ..event(key, 1, 4)
        }));
        assert!(!s.submit(Event {
            timestamp_ns: 99,
            ..event(key, 2, 4)
        }));
        assert_eq!(s.key(key, 4), Some(true));
        assert!(s.submit(Event {
            timestamp_ns: 101,
            ..event(key, 2, 4)
        }));
        assert_eq!(s.key(key, 4), Some(false));
    }
    #[test]
    fn capacity_limits_do_not_evict_existing_devices_or_touch_contacts() {
        let mut s = active();
        for token in 0..MAX_DEVICES {
            assert!(s.connect(GAMEPAD, 1, token as u64).is_some());
        }
        assert!(s.connect(GAMEPAD, 1, MAX_DEVICES as u64).is_none());
        assert_eq!(s.devices().len(), MAX_DEVICES);
        assert!(s.connect(0, 1, 0).is_none());
        assert!(s.connect(KEYBOARD, 0, 0).is_none());
        let mut s = active();
        let id = s.connect(TOUCH, 1, 0).unwrap();
        for contact in 0..MAX_TOUCHES {
            assert!(s.submit(event(id, 8, contact as u32)));
        }
        assert!(!s.submit(event(id, 8, MAX_TOUCHES as u32)));
        assert_eq!(s.contacts().len(), MAX_TOUCHES);
    }
    #[test]
    fn ffi_records_have_exact_offsets_and_null_buffers_do_not_consume_events() {
        use std::mem::{offset_of, size_of};
        assert_eq!(size_of::<Event>(), 40);
        assert_eq!(size_of::<Gamepad>(), 32);
        assert_eq!(size_of::<Pointer>(), 32);
        assert_eq!(size_of::<Contact>(), 32);
        assert_eq!(size_of::<DeviceInfo>(), 32);
        assert_eq!(offset_of!(Event, kind), 16);
        assert_eq!(offset_of!(Event, x), 24);
        assert_eq!(offset_of!(Gamepad, left_x), 20);
        assert_eq!(offset_of!(Gamepad, right_trigger), 30);
        gamepkg_input_focus(1);
        let id = gamepkg_input_connect(GAMEPAD, 0x7fff, 0);
        assert_ne!(id, 0);
        assert_eq!(gamepkg_input_focus(2), -1);
        gamepkg_input_focus(1);
        let report = Gamepad {
            device: id,
            buttons: 1,
            left_x: i16::MIN,
            right_trigger: u16::MAX,
            ..Gamepad::default()
        };
        let mut bytes = [0u8; 33];
        unsafe {
            bytes
                .as_mut_ptr()
                .add(1)
                .cast::<Gamepad>()
                .write_unaligned(report);
            assert_eq!(
                gamepkg_input_update_gamepad(bytes.as_ptr().add(1).cast()),
                1
            );
        }
        assert_eq!(unsafe { gamepkg_input_read(std::ptr::null_mut(), 1) }, -1);
        assert_eq!(unsafe { gamepkg_input_read(std::ptr::null_mut(), 0) }, 0);
        let mut events = [Event::default(); MAX_EVENTS];
        assert!(unsafe { gamepkg_input_read(events.as_mut_ptr(), MAX_EVENTS as u32) } > 0);
        assert_eq!(
            unsafe { gamepkg_input_gamepad_state(0, bytes.as_mut_ptr().add(1).cast()) },
            1
        );
        assert_eq!(
            unsafe { bytes.as_ptr().add(1).cast::<Gamepad>().read_unaligned() },
            report
        );
        assert_eq!(
            unsafe { gamepkg_input_gamepad_state(4, bytes.as_mut_ptr().add(1).cast()) },
            -1
        );
        let mut info = DeviceInfo::default();
        assert_eq!(unsafe { gamepkg_input_devices(&mut info, 1) }, 1);
        assert_eq!(info.device, id);
        assert_eq!(
            unsafe { gamepkg_input_devices(std::ptr::null_mut(), 1) },
            -1
        );
        assert_eq!(
            unsafe { gamepkg_input_update_gamepad(std::ptr::null()) },
            -1
        );
        // Exercise actual FFI producers/consumers, not a mock callback. Every
        // snapshot must be complete even while another thread updates it.
        let producer = std::thread::spawn(move || {
            for i in 1..10000i16 {
                let state = Gamepad {
                    device: id,
                    timestamp_ns: i as u64,
                    left_x: i,
                    right_x: i,
                    ..Gamepad::default()
                };
                assert_eq!(unsafe { gamepkg_input_update_gamepad(&state) }, 1);
            }
        });
        for _ in 0..10000 {
            let mut state = Gamepad::default();
            assert_eq!(unsafe { gamepkg_input_gamepad_state(0, &mut state) }, 1);
            if state.timestamp_ns != 0 {
                assert_eq!(state.left_x, state.right_x);
                assert_eq!(state.timestamp_ns, state.left_x as u64);
            }
        }
        producer.join().unwrap();
        assert_eq!(gamepkg_input_disconnect(id), 1);
    }
}
