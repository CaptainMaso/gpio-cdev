use crate::uapi;

pub struct Timestamp(u64);

impl Timestamp {
    pub fn now() -> Self {
        let mut timespec = std::mem::MaybeUninit::<nix::libc::timespec>::zeroed();
        let res =
            unsafe { nix::libc::clock_gettime(nix::libc::CLOCK_MONOTONIC, timespec.as_mut_ptr()) };

        if res == -1 {
            let err = nix::errno::Errno::last();
            match err {
                nix::errno::Errno::EINVAL => {
                    let res = unsafe {
                        nix::libc::clock_gettime(nix::libc::CLOCK_REALTIME, timespec.as_mut_ptr())
                    };

                    if res == -1 {
                        let err = nix::errno::Errno::last();
                        panic!("No clocks valid on system: {err:#?}");
                    }
                }
                _ => unreachable!(),
            }
        }

        let timespec = unsafe { timespec.assume_init() };

        let t = timespec.tv_sec as u64 * 1_000_000_000 + timespec.tv_nsec as u64;

        Self(t)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    Rising,
    Falling,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineEvent {
    timestamp: std::time::SystemTime,
    kind: EventKind,
    offset: u32,
    sequence: u32,
    line_sequence: u32,
}

impl LineEvent {
    pub(crate) const fn from_v2(event: uapi::v2::gpio_line_event) -> Self {
        let timestamp = Timestamp(event.timestamp_ns);

        let kind = match event.id {
            uapi::v2::LineEventId::FALLING_EDGE => event::EventKind::Falling,
            uapi::v2::LineEventId::RISING_EDGE => event::EventKind::Rising,
        };

        let data = LineEvent {
            timestamp,
            kind,
            offset: event.offset,
            sequence: event.seqno,
            line_sequence: event.line_seqno,
        };
    }
}
