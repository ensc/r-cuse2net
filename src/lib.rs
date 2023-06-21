#![allow(clippy::redundant_field_names)]
#![allow(dead_code)]

#[macro_use]
extern crate tracing;

mod error;
pub mod virtdev;
pub mod realdev;
pub mod proto;

use ensc_cuse_ffi::CuseDevice;
pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;

pub type CuseFileDevice = CuseDevice<std::fs::File>;

pub fn deadlock_detect() {
    use std::thread;
    use std::time::Duration;
    use parking_lot::deadlock;

    thread::spawn(move || {
	loop {
            thread::sleep(Duration::from_secs(10));
            let deadlocks = deadlock::check_deadlock();
            if deadlocks.is_empty() {
		continue;
            }

            println!("{} deadlocks detected", deadlocks.len());
            for (i, threads) in deadlocks.iter().enumerate() {
		println!("Deadlock #{}", i);
		for t in threads {
                    println!("Thread Id {:#?}", t.thread_id());
                    println!("{:#?}", t.backtrace());
		}
            }
	}
    });
}
