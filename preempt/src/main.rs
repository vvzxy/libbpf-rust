use anyhow::{bail, Result};
use libbpf_rs::RingBufferBuilder;
use plain::Plain;
use std::time::Duration;
use libc;
use std::thread::sleep;

mod preempt {
    include!(concat!(env!("OUT_DIR"), "/preempt.skel.rs"));
}
use preempt::PreemptSkelBuilder;  // 导入 PreemptSkelBuilder

// 手动定义 preempt_event 结构体
#[repr(C)]
#[derive(Default, Debug)]
struct PreemptEvent {
    prev_pid: i32,
    next_pid: i32,
    duration: u64,
    comm: [u8; 16],
}

fn bump_memlock_rlimit() -> Result<()> {
    let rlimit = libc::rlimit {
        rlim_cur: libc::RLIM_INFINITY,
        rlim_max: libc::RLIM_INFINITY,
    };

    if unsafe { libc::setrlimit(libc::RLIMIT_MEMLOCK, &rlimit) } != 0 {
        bail!("Failed to increase rlimit");
    }

    Ok(())
}

static mut EXITING: bool = false;

extern "C" fn sig_handler(_sig: libc::c_int) {
    unsafe {
        EXITING = true;
    }
}

unsafe impl Plain for PreemptEvent {}

fn handle_event(data: &[u8]) -> i32 {
    let event = plain::from_bytes::<PreemptEvent>(data).expect("Data conversion failed");
    println!(
        "{:<16} {:<7} {:<7} {:<11}",
        String::from_utf8_lossy(&event.comm),
        event.prev_pid,
        event.next_pid,
        event.duration
    );
    0
}

fn main() -> Result<()> {
    bump_memlock_rlimit()?;

    unsafe {
        libc::signal(libc::SIGINT, sig_handler as libc::sighandler_t);
        libc::signal(libc::SIGTERM, sig_handler as libc::sighandler_t);
    }

    let skel_builder = PreemptSkelBuilder::default();
    let open_skel = skel_builder.open()?;
    let mut skel = open_skel.load()?;
    skel.attach()?;

    let mut ring_buffer_builder = RingBufferBuilder::new();
    ring_buffer_builder.add(skel.maps().rb(), handle_event)?;
    let ring_buffer = ring_buffer_builder.build()?;

    println!("{:<12} {:<8} {:<8} {:<11}", "COMM", "prev_pid", "next_pid", "duration_ns");

    while !unsafe { EXITING } {
        ring_buffer.poll(Duration::from_millis(100))?;
        sleep(Duration::from_millis(100));
    }

    Ok(())
}
