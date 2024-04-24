use std::time::Duration;

use crate::uapi;

pub mod builder {
    pub use super::super::option_builder::*;
}

pub trait AsLineOptions {
    fn build_v2(self) -> uapi::v2::LineFlags;
}

impl AsLineOptions for () {
    fn build_v2(self) -> uapi::v2::LineFlags {
        uapi::v2::LineFlags::empty()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum LineOptions {
    Input {
        bias: Bias,
        active: Active,
        edge: Option<EdgeDetect>,
        clock: EventClock,
    },
    DrivenOutput {
        active: Active,
    },
    OpenOutput {
        drive: Drive,
        bias: Bias,
        active: Active,
        edge: Option<EdgeDetect>,
        clock: EventClock,
    },
}

impl LineOptions {
    pub const OUTPUT: Self = Self::DrivenOutput {
        active: Active::High,
    };

    pub const INPUT: Self = Self::Input {
        active: Active::High,
        bias: Bias::Disabled,
        edge: None,
        clock: EventClock::Default,
    };

    pub const fn build() -> builder::LineOptionBuilder<()> {
        builder::LineOptionBuilder::new()
    }

    pub(crate) const fn build_v2(self) -> uapi::v2::LineFlags {
        match self {
            LineOptions::Input {
                active,
                bias,
                edge,
                clock,
            } => builder::LineOptionBuilder {
                active: Some(active),
                edge,
                bias: Some(bias),
                clock: Some(clock),
                ..Self::build().input()
            }
            .build_v2(),
            LineOptions::DrivenOutput { active } => builder::LineOptionBuilder {
                active: Some(active),
                ..Self::build().output()
            }
            .build_v2(),
            LineOptions::OpenOutput {
                drive,
                bias,
                active,
                edge,
                clock,
            } => builder::LineOptionBuilder {
                active: Some(active),
                edge,
                bias: Some(bias),
                clock: Some(clock),
                ..Self::build().output().with_drive_open(drive)
            }
            .build_v2(),
        }
    }
}

impl AsLineOptions for LineOptions {
    #[inline(always)]
    fn build_v2(self) -> uapi::v2::LineFlags {
        Self::build_v2(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Input,
    Output,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Active {
    #[default]
    High,
    Low,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum EdgeDetect {
    Rising,
    Falling,
    #[default]
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drive {
    OpenDrain,
    OpenSource,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Bias {
    #[default]
    Disabled,
    PullUp,
    PullDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Debounce {
    d: u32,
}

impl Debounce {
    pub fn new(d: Duration) -> std::io::Result<Self> {
        let d = d.as_micros().try_into().map_err(|_e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Debounce period must be at most 4294 seconds",
            )
        })?;
        Ok(Self { d })
    }

    pub const fn new_micros(micros: u32) -> Self {
        Self { d: micros }
    }

    /// # Safety
    ///
    /// - The caller must ensure that duration is not greater than 2^32 microseconds (1.19 hours)
    pub const unsafe fn new_unchecked(d: Duration) -> Self {
        Self {
            d: d.as_micros() as u32,
        }
    }

    pub const fn as_duration(&self) -> Duration {
        Duration::from_micros(self.d as u64)
    }

    pub const fn as_micros(&self) -> u32 {
        self.d
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum EventClock {
    #[default]
    Default,
    HardwareTimestampEngine,
    RealTime,
}
