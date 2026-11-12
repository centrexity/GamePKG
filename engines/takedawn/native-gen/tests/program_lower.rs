use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{
    decode::Decoder,
    program::{Program, Unit},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Write},
};

#[test]
fn dependency_closure_preserves_cycles_and_removes_missing_transitive_calls() {
    let budget = Budget::new(8 * 1024 * 1024);
    let dependencies = BTreeMap::from([
        (1, vec![2]),
        (2, vec![1]),
        (3, vec![4]),
        (4, vec![5]),
        (6, vec![3]),
        (7, vec![]),
    ]);
    let closed =
        gamepkg_takedawn_native_gen::program::closed_entries(&dependencies, &budget).unwrap();
    assert_eq!(closed.entries(), &BTreeSet::from([1, 2, 7]));
    assert!(budget.used() > 0);
    drop(closed);
    assert_eq!(budget.used(), 0);
    let chain = (0..4096).map(|n| (n, vec![n + 1])).collect();
    let closed = gamepkg_takedawn_native_gen::program::closed_entries(&chain, &budget).unwrap();
    assert!(closed.entries().is_empty());
    drop(closed);
    assert_eq!(budget.used(), 0);
    let excessive = BTreeMap::from([(1, vec![1; 262145])]);
    assert!(gamepkg_takedawn_native_gen::program::closed_entries(&excessive, &budget).is_err());
    assert!(
        gamepkg_takedawn_native_gen::program::closed_entries(&dependencies, &Budget::new(1))
            .is_err()
    );
    assert_eq!(budget.used(), 0);
}

fn source(functions: &[(u32, Vec<u8>)]) -> String {
    let budget = Budget::new(32 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    let decoded: Vec<_> = functions
        .iter()
        .map(|(entry, bytes)| {
            decoder
                .cfg(bytes, *entry, &[*entry], &BTreeSet::new(), &BTreeSet::new())
                .unwrap()
        })
        .collect();
    let units: Vec<_> = functions
        .iter()
        .zip(&decoded)
        .map(|((entry, bytes), decoded)| Unit {
            decoded,
            entry: *entry,
            end: *entry + bytes.len() as u32,
        })
        .collect();
    let program = Program::build(&units, &budget).unwrap();
    assert_eq!(program.function_count(), functions.len());
    let mut out = Vec::new();
    program.write_source(&mut out).unwrap();
    let ranges: Vec<_> = functions
        .iter()
        .map(|(entry, bytes)| (*entry, *entry + bytes.len() as u32))
        .collect();
    let mut streamed = Vec::new();
    let summary = gamepkg_takedawn_native_gen::program::write_streamed(
        &ranges,
        |entry, _| {
            Ok(functions
                .iter()
                .find(|(address, _)| *address == entry)
                .unwrap()
                .1
                .as_slice())
        },
        &decoder,
        &budget,
        out.len() as u64,
        &mut streamed,
    )
    .unwrap();
    assert_eq!(streamed, out);
    assert_eq!(summary.functions, functions.len());
    assert_eq!(summary.bytes, out.len() as u64);
    use sha2::Digest;
    assert_eq!(
        summary.source_sha256,
        hex::encode(sha2::Sha256::digest(&out))
    );
    String::from_utf8(out).unwrap()
}

#[test]
fn streaming_is_bounded_and_rejects_invalid_programs_before_output() {
    use gamepkg_takedawn_native_gen::program::write_streamed;
    let budget = Budget::new(8 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    let baseline = budget.used();
    let ranges: Vec<_> = (0..256)
        .map(|n| (0x10000 + n * 16, 0x10001 + n * 16))
        .collect();
    struct BoundedWriter<'a> {
        budget: &'a Budget,
        peak: u64,
        bytes: u64,
    }
    impl Write for BoundedWriter<'_> {
        fn write(&mut self, data: &[u8]) -> io::Result<usize> {
            self.peak = self.peak.max(self.budget.used());
            self.bytes += data.len() as u64;
            Ok(data.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut writer = BoundedWriter {
        budget: &budget,
        peak: 0,
        bytes: 0,
    };
    let summary = write_streamed(
        &ranges,
        |_, _| Ok(&[0xc3]),
        &decoder,
        &budget,
        1024 * 1024,
        &mut writer,
    )
    .unwrap();
    assert_eq!(summary.functions, 256);
    assert_eq!(summary.bytes, writer.bytes);
    assert!(writer.peak < 6 * 1024 * 1024);
    assert_eq!(budget.used(), baseline);
    for ranges in [
        vec![],
        vec![(1, 1)],
        vec![(1, 2), (1, 2)],
        vec![(1, 3), (2, 3)],
        vec![(1, 20000)],
    ] {
        let mut out = Vec::new();
        assert!(write_streamed(
            &ranges,
            |_, _| Ok(&[0xc3]),
            &decoder,
            &budget,
            u64::MAX,
            &mut out
        )
        .is_err());
        assert!(out.is_empty());
    }
    for (input, cap) in [
        (&[0xc3][..], 0),
        (&[][..], u64::MAX),
        (&[0xff][..], u64::MAX),
        (&[0x90][..], u64::MAX),
    ] {
        let mut out = Vec::new();
        assert!(write_streamed(
            &[(1, 2)],
            |_, _| Ok(input),
            &decoder,
            &budget,
            cap,
            &mut out
        )
        .is_err());
        assert!(out.is_empty());
    }
    let mut out = Vec::new();
    assert!(write_streamed(
        &[(1, 2)],
        |_, _| anyhow::bail!("missing function"),
        &decoder,
        &budget,
        u64::MAX,
        &mut out
    )
    .is_err());
    assert!(out.is_empty());
    assert!(write_streamed(
        &[(1, 2)],
        |_, _| Ok(&[0xc3]),
        &decoder,
        &Budget::new(1),
        u64::MAX,
        &mut out
    )
    .is_err());
    assert!(out.is_empty());
    let calls = std::cell::Cell::new(0);
    assert!(write_streamed(
        &[(1, 2)],
        |_, _| {
            calls.set(calls.get() + 1);
            Ok(if calls.get() == 1 { &[0xc3] } else { &[0x90] })
        },
        &decoder,
        &budget,
        u64::MAX,
        &mut out
    )
    .is_err());
    assert!(String::from_utf8(out)
        .unwrap()
        .starts_with("void sub_00000001(void);"));
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("broken writer"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    assert!(write_streamed(
        &[(1, 2)],
        |_, _| Ok(&[0xc3]),
        &decoder,
        &budget,
        u64::MAX,
        &mut Broken
    )
    .is_err());
    assert_eq!(budget.used(), baseline);
}

#[test]
fn streamed_shared_tails_require_identical_bytes_and_instruction_boundaries() {
    use gamepkg_takedawn_native_gen::program::write_streamed;
    let budget = Budget::new(8 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    let bytes = [0xb8, 1, 0, 0, 0, 0xc3];
    let mut out = Vec::new();
    let summary = write_streamed(
        &[(1, 7), (6, 7)],
        |start, _| Ok(&bytes[(start - 1) as usize..]),
        &decoder,
        &budget,
        10000,
        &mut out,
    )
    .unwrap();
    assert_eq!(summary.shared_tail_entries, 1);
    assert_eq!(summary.functions, 2);
    // Starting inside the first MOV can independently decode valid ADDs, but
    // cannot be treated as a consistent shared-tail instruction boundary.
    let interior = [0xb8, 0x90, 0x90, 0x90, 0x90, 0xc3];
    for (ranges, changed) in [
        (vec![(1, 7), (2, 7)], false),
        (vec![(1, 7), (6, 7)], true),
        (vec![(1, 7), (6, 8)], false),
    ] {
        let mut out = Vec::new();
        assert!(write_streamed(
            &ranges,
            |start, _| {
                Ok(if changed && start == 6 {
                    &[0x90][..]
                } else {
                    &interior[(start - 1) as usize..]
                })
            },
            &decoder,
            &budget,
            10000,
            &mut out
        )
        .is_err());
        assert!(out.is_empty());
    }
}
fn transfer(bytes: &mut Vec<u8>, entry: u32, opcode: u8, target: u32) {
    bytes.push(opcode);
    let next = entry + bytes.len() as u32 + 4;
    bytes.extend(target.wrapping_sub(next).to_le_bytes());
}
fn functions() -> Vec<(u32, Vec<u8>)> {
    let mut return_reader = vec![0xbd, 0x44, 0x33, 0x22, 0x11];
    transfer(&mut return_reader, 0x10000, 0xe8, 0x10100);
    return_reader.extend([0x89, 0xeb, 0xc3]);
    let mut args = vec![0x50, 0x52];
    transfer(&mut args, 0x11000, 0xe8, 0x11100);
    args.push(0xc3);
    let mut tail = vec![0xb8, 42, 0, 0, 0];
    transfer(&mut tail, 0x12000, 0xe9, 0x12100);
    let mut conditional = vec![0x83, 0xf8, 0, 0x0f];
    transfer(&mut conditional, 0x12200, 0x84, 0x12300);
    conditional.extend([0xba, 0x55, 0, 0, 0, 0xc3]);
    let recursive = vec![
        0x85, 0xc9, 0x74, 0x0b, 0x51, 0x49, 0xe8, 0xf5, 0xff, 0xff, 0xff, 0x5a, 0x01, 0xd0, 0xc3,
        0x31, 0xc0, 0xc3,
    ];
    let mut carry = vec![0xb8, 0xff, 0xff, 0xff, 0xff, 0x83, 0xc0, 1];
    transfer(&mut carry, 0x14000, 0xe8, 0x14100);
    carry.extend([0x83, 0xd0, 0, 0xc3]);
    let mut preserve = Vec::new();
    transfer(&mut preserve, 0x15000, 0xe8, 0x15100);
    preserve.push(0xc3);
    vec![
        (0x10000, return_reader),
        (
            0x10100,
            vec![
                0x8b, 0x04, 0x24, 0x89, 0xea, 0xbd, 0x88, 0x77, 0x66, 0x55, 0xc3,
            ],
        ),
        (0x11000, args),
        (
            0x11100,
            vec![0x8b, 0x44, 0x24, 4, 0x03, 0x44, 0x24, 8, 0xc2, 8, 0],
        ),
        (0x12000, tail),
        (0x12100, vec![0x83, 0xc0, 1, 0xc2, 4, 0]),
        (0x12200, conditional),
        (0x12300, vec![0xba, 0xaa, 0, 0, 0, 0xc3]),
        (0x13000, recursive),
        (0x14000, carry),
        (0x14100, vec![0xc3]),
        (0x15000, preserve),
        (
            0x15100,
            vec![
                0x53, 0x56, 0x57, 0x55, 0xbb, 1, 0, 0, 0, 0xbe, 2, 0, 0, 0, 0xbf, 3, 0, 0, 0, 0xbd,
                4, 0, 0, 0, 0x5d, 0x5f, 0x5e, 0x5b, 0xc3,
            ],
        ),
    ]
}

#[test]
fn closed_program_rejects_missing_invalid_duplicate_and_overlapping_units() {
    let budget = Budget::new(16 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    let mut caller = Vec::new();
    transfer(&mut caller, 0x10000, 0xe8, 0x10100);
    caller.push(0xc3);
    let a = decoder
        .cfg(
            &caller,
            0x10000,
            &[0x10000],
            &BTreeSet::new(),
            &BTreeSet::new(),
        )
        .unwrap();
    // Indirect CALL needs recovered dispatch metadata; it is not a closed
    // direct-call unit. IDIV is now supported and is no longer a negative case.
    let bad = decoder.linear(&[0xff, 0xd0, 0xc3], 0x10100, &[]).unwrap();
    let good = decoder.linear(&[0xc3], 0x10100, &[]).unwrap();
    let held = budget.used();
    assert!(Program::build(&[], &budget).is_err());
    assert!(Program::build(
        &[Unit {
            decoded: &a,
            entry: 0x10000,
            end: 0x10006
        }],
        &budget
    )
    .is_err());
    assert!(Program::build(
        &[
            Unit {
                decoded: &a,
                entry: 0x10000,
                end: 0x10006
            },
            Unit {
                decoded: &bad,
                entry: 0x10100,
                end: 0x10103
            }
        ],
        &budget
    )
    .is_err());
    assert!(Program::build(
        &[
            Unit {
                decoded: &good,
                entry: 0x10100,
                end: 0x10101
            },
            Unit {
                decoded: &good,
                entry: 0x10100,
                end: 0x10101
            }
        ],
        &budget
    )
    .is_err());
    assert!(Program::build(
        &[
            Unit {
                decoded: &a,
                entry: 0x10000,
                end: 0x10101
            },
            Unit {
                decoded: &good,
                entry: 0x10100,
                end: 0x10101
            }
        ],
        &budget
    )
    .is_err());
    assert_eq!(budget.used(), held);
    let units = [
        Unit {
            decoded: &good,
            entry: 0x10100,
            end: 0x10101,
        },
        Unit {
            decoded: &a,
            entry: 0x10000,
            end: 0x10006,
        },
    ];
    assert!(Program::build(&units, &Budget::new(1)).is_err());
    let program = Program::build(&units, &budget).unwrap();
    assert!(budget.used() > held);
    struct FailedWriter;
    impl Write for FailedWriter {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("test output quota"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    assert!(program.write_source(&mut FailedWriter).is_err());
    drop(a);
    drop(bad);
    drop(good);
    drop(decoder);
    assert!(budget.used() > 0);
    drop(program);
    assert_eq!(budget.used(), 0);
}

fn fixture() -> String {
    let mut c = String::from(
        "#include <stdint.h>\n#include <stdio.h>\n#include <assert.h>\n#include <setjmp.h>\n",
    );
    if let Some(header) = std::env::var_os("GAMEPKG_TAKEDAWN_RUNTIME_HEADER") {
        let header = header.to_str().unwrap();
        assert!(!header.contains(['"', '\n', '\r', '\\']));
        c.push_str(&format!("#include \"{header}\"\n"));
    } else {
        c.push_str("#define RECOMP_TLS _Thread_local\nuint32_t xbox_MmioLoad(uint32_t,int);\nvoid xbox_MmioStore(uint32_t,uint32_t,int);\n");
    }
    c.push_str(r#"
RECOMP_TLS uint32_t g_eax,g_ebx,g_ecx,g_edx,g_esi,g_edi,g_esp,g_ebp,g_fs_base,g_eflags;
static unsigned char memory[16384];
static int fail_store;
static unsigned hardware_checks;
static jmp_buf write_failure;
uint32_t xbox_MmioLoad(uint32_t address,int width) {
    assert(width==1 || width==2 || width==4);assert(address<=sizeof(memory)-(unsigned)width);
    uint32_t result=0;
    for(int i=0;i<width;++i)result|=(uint32_t)memory[address+(unsigned)i]<<(8u*(unsigned)i);
    return result;
}
void xbox_MmioStore(uint32_t address,uint32_t value,int width) {
    assert(width==1 || width==2 || width==4);assert(address<=sizeof(memory)-(unsigned)width);
    if(fail_store)longjmp(write_failure,1);
    for(int i=0;i<width;++i)memory[address+(unsigned)i]=(unsigned char)(value>>(8u*(unsigned)i));
}
#if defined(__i386__)
/* Original synthetic x86 call/return sequences execute on the native stack.
 * They preserve C's callee-saved registers and restore ESP before returning. */
__asm__(".text\n"
"gamepkg_test_hw_args:\nmovl 4(%esp),%eax\naddl 8(%esp),%eax\nret $8\n"
"gamepkg_test_hw_recursive:\ntestl %ecx,%ecx\njz 1f\npushl %ecx\ndecl %ecx\ncall gamepkg_test_hw_recursive\npopl %edx\naddl %edx,%eax\nret\n1:\nxorl %eax,%eax\nret\n"
"gamepkg_test_hw_tail:\nmovl $42,%eax\njmp gamepkg_test_hw_tail2\n"
"gamepkg_test_hw_tail2:\naddl $1,%eax\nret $4\n"
"gamepkg_test_hw_return:\nret\n");
#endif
"#);
    c.push_str(&source(&functions()));
    c.push_str(r#"
int main(void) {
    g_esp=0x1000;g_ebp=0;g_eflags=0x200ad7;
    sub_00010000();
    assert(g_eax==0x1000a && g_edx==0x11223344 && g_ebx==0x55667788 && g_ebp==0x55667788 && g_esp==0x1004 && g_eflags==0x200ad7);
    assert(xbox_MmioLoad(0xffc,4)==0x1000a);
    uint32_t edge[]={0,1,15,16,0x7fffffff,0x80000000,0xffffffff,0xffff0000};
    for(unsigned i=0;i<8;++i)for(unsigned j=0;j<8;++j) {
        uint32_t x=edge[i],y=edge[j];
        g_eax=x;g_edx=y;g_esp=0x1000;g_ebp=0x43211234;g_eflags=0x200202;
        sub_00011000();
        assert(g_eax==x+y && g_edx==y && g_esp==0x1004 && g_ebp==0x43211234);
#if defined(__i386__)
        uint32_t real=x,flags,before,after;
        __asm__ volatile("movl %%esp,%[before]\n\tpushl %[result]\n\tpushl %[y]\n\tcall gamepkg_test_hw_args\n\tmovl %%esp,%[after]\n\tpushf\n\tpop %[flags]"
            : [result] "+&a"(real),[flags] "=&r"(flags),[before] "=&r"(before),[after] "=&r"(after)
            : [y] "r"(y) : "cc","memory");
        assert(real==g_eax && before==after && ((flags^g_eflags)&0x8d5u)==0u);++hardware_checks;
#endif
    }
    for(uint32_t n=0;n<64;++n) {
        g_ecx=n;g_esp=0x1000;g_ebp=0x43211234;g_edx=0xf00d;g_eflags=0x200202;
        sub_00013000();
        assert(g_eax==n*(n+1u)/2u && g_ecx==0 && g_edx==(n==0?0xf00d:n) && g_esp==0x1004 && g_ebp==0x43211234);
#if defined(__i386__)
        uint32_t real,count=n,data=0xf00d,flags;
        __asm__ volatile("call gamepkg_test_hw_recursive\n\tpushf\n\tpop %[flags]"
            : [result] "=&a"(real),[count] "+&c"(count),[data] "+&d"(data),[flags] "=&r"(flags)
            : : "cc","memory");
        assert(real==g_eax && count==g_ecx && data==g_edx && ((flags^g_eflags)&(n==0?0x8c5u:0x8d5u))==0u);++hardware_checks;
#endif
    }
    g_esp=0x1000;g_eflags=0x200202;sub_00012000();assert(g_eax==43 && g_esp==0x1008);
#if defined(__i386__)
    uint32_t real,flags;
    __asm__ volatile("pushl $0\n\tcall gamepkg_test_hw_tail\n\tpushf\n\tpop %[flags]"
        : [result] "=&a"(real),[flags] "=&r"(flags) : : "cc","memory");
    assert(real==g_eax && ((flags^g_eflags)&0x8d5u)==0u);++hardware_checks;
#endif
    g_esp=0x1000;g_eflags=0x200202;sub_00014000();assert(g_eax==1 && g_esp==0x1004);
#if defined(__i386__)
    real=0xffffffffu;
    __asm__ volatile("addl $1,%[result]\n\tcall gamepkg_test_hw_return\n\tadcl $0,%[result]\n\tpushf\n\tpop %[flags]"
        : [result] "+&a"(real),[flags] "=&r"(flags) : : "cc","memory");
    assert(real==g_eax && ((flags^g_eflags)&0x8d5u)==0u);++hardware_checks;
#endif
    for(uint32_t value=0;value<2;++value) {
        g_eax=value;g_esp=0x1000;sub_00012200();assert(g_edx==(value==0?0xaa:0x55) && g_esp==0x1004);
    }
    g_esp=0x1000;g_ebx=0x12345678;g_esi=0x11223344;g_edi=0x55667788;g_ebp=0xaabbccdd;sub_00015000();
    assert(g_esp==0x1004 && g_ebx==0x12345678 && g_esi==0x11223344 && g_edi==0x55667788 && g_ebp==0xaabbccdd);
    g_esp=0x1000;g_ebp=0x11223344;g_eax=0x87654321;g_eflags=0x200ad7;
    uint32_t saved=xbox_MmioLoad(0xffc,4);fail_store=1;
    if(setjmp(write_failure)==0) {sub_00015000();assert(!"CALL store should fail");}
    fail_store=0;
    assert(g_esp==0x1000 && g_ebp==0x11223344 && g_eax==0x87654321 && g_eflags==0x200ad7 && xbox_MmioLoad(0xffc,4)==saved);
    printf("native program: calls, return addresses, cleanup, recursion, tails, flags, register saves and failed CALL store passed; %u original x86 call-stack comparisons\n",hardware_checks);
    return 0;
}
"#);
    c
}

#[test]
fn synthetic_closed_program_fixture_can_be_cross_compiled() {
    let c = fixture();
    if let Some(path) = std::env::var_os("GAMEPKG_TAKEDAWN_PROGRAM_FIXTURE") {
        std::fs::write(path, c).unwrap();
    }
}
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn generated_closed_program_executes_calls_and_returns() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("program.c");
    let binary = dir.path().join("program");
    std::fs::write(&source, fixture()).unwrap();
    let output = std::process::Command::new("cc")
        .args([
            "-std=c11",
            "-O3",
            "-mno-red-zone",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-fsanitize=undefined",
            "-fno-sanitize-recover=all",
        ])
        .arg(source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = std::process::Command::new(binary).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    print!("{}", String::from_utf8_lossy(&output.stdout));
}
