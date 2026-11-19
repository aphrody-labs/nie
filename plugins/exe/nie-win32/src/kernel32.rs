//! kernel32 functions with a faithful single-process implementation.

use std::cell::Cell;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crate::cpu::Cpu;
use crate::imports::ImportImpl;

const HEAP_ZERO_MEMORY: u64 = 0x8;
const PROCESS_HEAP: u64 = 0x1;
const PSEUDO_PROCESS: u64 = u64::MAX;
const PSEUDO_THREAD: u64 = u64::MAX - 1;
/// QueryPerformanceFrequency: nanosecond ticks.
const QPC_HZ: u64 = 1_000_000_000;

thread_local! {
    static LAST_ERROR: Cell<u32> = const { Cell::new(0) };
}

fn epoch() -> Instant {
    static T0: OnceLock<Instant> = OnceLock::new();
    *T0.get_or_init(Instant::now)
}

fn get_last_error(cpu: &mut Cpu) {
    cpu.ret(u64::from(LAST_ERROR.with(Cell::get)));
}

fn set_last_error(cpu: &mut Cpu) {
    LAST_ERROR.with(|e| e.set(cpu.arg(0) as u32));
}

fn get_current_thread_id(cpu: &mut Cpu) {
    cpu.ret(u64::from(thread_id()));
}

#[cfg(unix)]
fn thread_id() -> u32 {
    // SAFETY: gettid has no arguments and cannot fail.
    unsafe { libc::syscall(libc::SYS_gettid) as u32 }
}

#[cfg(not(unix))]
fn thread_id() -> u32 {
    0
}

fn get_current_process_id(cpu: &mut Cpu) {
    cpu.ret(u64::from(std::process::id()));
}

fn get_current_process(cpu: &mut Cpu) {
    cpu.ret(PSEUDO_PROCESS);
}

fn get_current_thread(cpu: &mut Cpu) {
    cpu.ret(PSEUDO_THREAD);
}

fn get_tick_count64(cpu: &mut Cpu) {
    cpu.ret(epoch().elapsed().as_millis() as u64);
}

fn get_tick_count(cpu: &mut Cpu) {
    cpu.ret(u64::from(epoch().elapsed().as_millis() as u32));
}

fn query_performance_counter(cpu: &mut Cpu) {
    let ticks = epoch().elapsed().as_nanos() as u64;
    let out = cpu.arg(0) as *mut u64;
    if out.is_null() {
        cpu.ret(0);
        return;
    }
    // SAFETY: the guest passes a LARGE_INTEGER pointer, 8 bytes, in guest memory.
    unsafe { out.write_unaligned(ticks) };
    cpu.ret(1);
}

fn query_performance_frequency(cpu: &mut Cpu) {
    let out = cpu.arg(0) as *mut u64;
    if out.is_null() {
        cpu.ret(0);
        return;
    }
    // SAFETY: as above.
    unsafe { out.write_unaligned(QPC_HZ) };
    cpu.ret(1);
}

fn get_process_heap(cpu: &mut Cpu) {
    cpu.ret(PROCESS_HEAP);
}

fn heap_alloc(cpu: &mut Cpu) {
    let flags = cpu.arg(1);
    let size = cpu.arg(2) as usize;
    // SAFETY: plain libc allocation; the result is only handed to guest code.
    let p = unsafe {
        if flags & HEAP_ZERO_MEMORY != 0 {
            libc::calloc(1, size.max(1))
        } else {
            libc::malloc(size.max(1))
        }
    };
    cpu.ret(p as u64);
}

fn heap_free(cpu: &mut Cpu) {
    let p = cpu.arg(2) as *mut libc::c_void;
    // SAFETY: the guest only frees pointers it got from HeapAlloc/HeapReAlloc.
    unsafe { libc::free(p) };
    cpu.ret(1);
}

fn heap_realloc(cpu: &mut Cpu) {
    let flags = cpu.arg(1);
    let p = cpu.arg(2) as *mut libc::c_void;
    let size = cpu.arg(3) as usize;
    // SAFETY: as in heap_free; realloc preserves the prefix, zeroing follows HEAP_ZERO_MEMORY.
    let q = unsafe {
        let old = if p.is_null() { 0 } else { libc::malloc_usable_size(p) };
        let q = libc::realloc(p, size.max(1));
        if !q.is_null() && flags & HEAP_ZERO_MEMORY != 0 && size > old {
            libc::memset(q.cast::<u8>().add(old).cast(), 0, size - old);
        }
        q
    };
    cpu.ret(q as u64);
}

fn heap_size(cpu: &mut Cpu) {
    let p = cpu.arg(2) as *mut libc::c_void;
    // SAFETY: pointer from HeapAlloc.
    let n = if p.is_null() { u64::MAX } else { unsafe { libc::malloc_usable_size(p) as u64 } };
    cpu.ret(n);
}

fn is_processor_feature_present(cpu: &mut Cpu) {
    // PF_XMMI64_INSTRUCTIONS_AVAILABLE (10) and PF_FLOATING_POINT_EMULATED (1, absent) only.
    cpu.ret(u64::from(cpu.arg(0) == 10));
}

fn sleep(cpu: &mut Cpu) {
    std::thread::sleep(Duration::from_millis(cpu.arg(0) as u32 as u64));
}

fn switch_to_thread(cpu: &mut Cpu) {
    std::thread::yield_now();
    cpu.ret(1);
}

fn exit_process(cpu: &mut Cpu) {
    std::process::exit(cpu.arg(0) as u32 as i32);
}

macro_rules! k32 {
    ($($name:literal => $f:ident),* $(,)?) => {
        &[$(ImportImpl { dll: "kernel32.dll", name: $name, f: $f }),*]
    };
}

pub(crate) static TABLE: &[ImportImpl] = k32! {
    "ExitProcess" => exit_process,
    "GetCurrentProcess" => get_current_process,
    "GetCurrentProcessId" => get_current_process_id,
    "GetCurrentThread" => get_current_thread,
    "GetCurrentThreadId" => get_current_thread_id,
    "GetLastError" => get_last_error,
    "GetProcessHeap" => get_process_heap,
    "GetTickCount" => get_tick_count,
    "GetTickCount64" => get_tick_count64,
    "HeapAlloc" => heap_alloc,
    "HeapFree" => heap_free,
    "HeapReAlloc" => heap_realloc,
    "HeapSize" => heap_size,
    "IsProcessorFeaturePresent" => is_processor_feature_present,
    "QueryPerformanceCounter" => query_performance_counter,
    "QueryPerformanceFrequency" => query_performance_frequency,
    "SetLastError" => set_last_error,
    "Sleep" => sleep,
    "SwitchToThread" => switch_to_thread,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted_and_unique() {
        let names: Vec<_> = TABLE.iter().map(|i| i.name).collect();
        let mut s = names.clone();
        s.sort_unstable();
        s.dedup();
        assert_eq!(names, s);
    }

    #[test]
    fn last_error_roundtrip() {
        let mut cpu = Cpu::new();
        cpu.gpr[crate::reg::RCX] = 87;
        set_last_error(&mut cpu);
        get_last_error(&mut cpu);
        assert_eq!(cpu.gpr[crate::reg::RAX], 87);
    }

    #[test]
    fn heap_zero_alloc_free() {
        let mut cpu = Cpu::new();
        cpu.gpr[crate::reg::RDX] = HEAP_ZERO_MEMORY;
        cpu.gpr[crate::reg::R8] = 64;
        heap_alloc(&mut cpu);
        let p = cpu.gpr[crate::reg::RAX] as *const u8;
        assert!(!p.is_null());
        // SAFETY: 64 zeroed bytes were just allocated.
        assert!((0..64).all(|i| unsafe { *p.add(i) } == 0));
        cpu.gpr[crate::reg::R8] = p as u64;
        heap_free(&mut cpu);
    }
}
