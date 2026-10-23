//! Explicit initial context for GamePKG's virtual single-processor kernel.
//! Known Xbox offsets follow the public Cxbx/OpenXDK declarations; no native
//! pointers/struct packing are used. This is not a complete KPRCB/KTHREAD or a
//! scheduler. Unmodeled bytes are deliberately absent from the field maps.
use anyhow::{ensure, Context, Result};
pub const PCR_BYTES: u32 = 0x280;
pub const THREAD_BYTES: u32 = 0x140;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeLimitInitialization {
    /// Explicit GamePKG virtual compatibility contract: the observed zero word
    /// at PRCB+250 runs the title's full INIT/CS path. Its original Xbox semantic
    /// identity is NOT qualified. A nonzero bypass is not an available profile.
    RunTitlePath,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stack {
    pub low: u32,
    pub high: u32,
    pub tls: u32,
    pub tls_bytes: u32,
    pub esp: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Field<'a> {
    pub address: u32,
    pub writable: bool,
    pub bytes: &'a [u8],
}
pub struct Processor {
    base: u32,
    header: [u8; 12],
    next_idle: [u8; 8],
    dpc: [u8; 12],
    init: [u8; 4],
}
pub struct Thread {
    pcr: u32,
    address: u32,
    stack: Stack,
    tib: [u8; 28],
    current: [u8; 4],
    stack_image: [u8; 16],
}
fn word(b: &mut [u8], at: usize, n: u32) {
    b[at..at + 4].copy_from_slice(&n.to_le_bytes());
}
fn end(base: u32, bytes: u32) -> Result<u64> {
    let e = u64::from(base) + u64::from(bytes);
    ensure!(
        base >= 0x1000 && base & 3 == 0 && e <= u64::from(u32::MAX) + 1,
        "invalid processor/thread range"
    );
    Ok(e)
}
fn disjoint(a: u32, an: u32, b: u32, bn: u32) -> bool {
    u64::from(a) + u64::from(an) <= u64::from(b) || u64::from(b) + u64::from(bn) <= u64::from(a)
}
impl Processor {
    /// Cold bootstrap only: no next/idle thread, active DPC or raised IRQL is
    /// invented. One logical processor may construct several Thread images.
    pub fn new(base: u32, initialization: CodeLimitInitialization) -> Result<Self> {
        end(base, PCR_BYTES)?;
        let mut result = Self {
            base,
            header: [0; 12],
            next_idle: [0; 8],
            dpc: [0; 12],
            init: [0; 4],
        };
        word(&mut result.header, 0, base);
        word(&mut result.header, 4, base + 0x28);
        word(&mut result.dpc, 0, base + 0x50);
        word(&mut result.dpc, 4, base + 0x50);
        match initialization {
            CodeLimitInitialization::RunTitlePath => word(&mut result.init, 0, 0),
        }
        Ok(result)
    }
    pub fn address(&self) -> u32 {
        self.base
    }
    pub fn thread(&self, address: u32, stack: Stack) -> Result<Thread> {
        end(address, THREAD_BYTES)?;
        ensure!(
            stack.low >= 0x1000
                && stack.low < stack.high
                && stack.low & 3 == 0
                && stack.high & 3 == 0,
            "invalid stack bounds"
        );
        ensure!(
            stack.tls & 15 == 0 && stack.tls >= stack.low,
            "invalid TLS base"
        );
        let tls_end = stack
            .tls
            .checked_add(stack.tls_bytes)
            .context("TLS wraps")?;
        ensure!(tls_end <= stack.high, "TLS exceeds stack reservation");
        ensure!(
            stack.esp & 3 == 0
                && stack.esp >= stack.low
                && u64::from(stack.esp) + 12 <= u64::from(stack.tls),
            "entry frame overlaps TLS or stack boundary"
        );
        ensure!(
            disjoint(self.base, PCR_BYTES, address, THREAD_BYTES)
                && disjoint(self.base, PCR_BYTES, stack.low, stack.high - stack.low)
                && disjoint(address, THREAD_BYTES, stack.low, stack.high - stack.low),
            "processor/thread/stack objects overlap"
        );
        let mut result = Thread {
            pcr: self.base,
            address,
            stack,
            tib: [0; 28],
            current: address.to_le_bytes(),
            stack_image: [0; 16],
        };
        for (at, n) in [(0, u32::MAX), (4, tls_end), (8, stack.low), (24, self.base)] {
            word(&mut result.tib, at, n);
        }
        for (at, n) in [
            (0, stack.high),
            (4, stack.low),
            (8, stack.esp),
            (12, stack.tls),
        ] {
            word(&mut result.stack_image, at, n);
        }
        Ok(result)
    }
    /// Map these initial CPU-global fields once. The opaque init word is a
    /// read-only profile input, not an undocumented writable CPU feature bit.
    pub fn initial_fields(&self) -> [Field<'_>; 4] {
        [
            Field {
                address: self.base + 0x1c,
                writable: true,
                bytes: &self.header,
            },
            Field {
                address: self.base + 0x2c,
                writable: true,
                bytes: &self.next_idle,
            },
            Field {
                address: self.base + 0x50,
                writable: true,
                bytes: &self.dpc,
            },
            Field {
                address: self.base + 0x278,
                writable: false,
                bytes: &self.init,
            },
        ]
    }
    /// Initial thread projection. Context switching must SAVE guest-mutated
    /// NT_TIB bytes and kernel-stack state; reapplying an initial image would
    /// erase SEH/TLS changes. CPU ownership, raw/hidden banks and scheduler
    /// activation/retirement are separate responsibilities of the runtime.
    pub fn thread_fields<'a>(&self, thread: &'a Thread) -> Result<[Field<'a>; 3]> {
        ensure!(
            thread.pcr == self.base,
            "thread belongs to another processor"
        );
        Ok([
            Field {
                address: self.base,
                writable: true,
                bytes: &thread.tib,
            },
            Field {
                address: self.base + 0x28,
                writable: true,
                bytes: &thread.current,
            },
            Field {
                address: thread.address + 0x1c,
                writable: true,
                bytes: &thread.stack_image,
            },
        ])
    }
    /// Loader-only atomic installation into exclusively borrowed RAM. Map only
    /// the returned field spans, carving both RESERVED objects out of broad
    /// arenas; unknown gaps must not become implicit readable zero state.
    /// Run once before entering this initial thread. This neither grants RAM
    /// capabilities nor activates a CPU/FS bank or creates a kernel thread.
    pub fn install_initial(&self, thread: &Thread, ram_base: u32, ram: &mut [u8]) -> Result<()> {
        ensure!(
            u64::from(ram_base) + ram.len() as u64 <= u64::from(u32::MAX) + 1,
            "RAM wraps"
        );
        let cpu = self.initial_fields();
        let fields = self.thread_fields(thread)?;
        for f in cpu.iter().chain(fields.iter()) {
            let at = f
                .address
                .checked_sub(ram_base)
                .context("field before RAM")? as usize;
            let e = at
                .checked_add(f.bytes.len())
                .context("field offset wraps")?;
            ensure!(ram.get(at..e).is_some(), "processor/thread field unbacked");
        }
        for f in cpu.iter().chain(fields.iter()) {
            let at = (f.address - ram_base) as usize;
            ram[at..at + f.bytes.len()].copy_from_slice(f.bytes);
        }
        Ok(())
    }
}
impl Thread {
    pub fn address(&self) -> u32 {
        self.address
    }
    pub fn stack(&self) -> Stack {
        self.stack
    }
}
