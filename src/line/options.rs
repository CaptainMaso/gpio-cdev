use crate::uapi;

pub mod builder {
    pub use super::super::option_builder::*;
}

pub trait AsLineOptions {
    fn build_v2(self) -> uapi::v2::LineFlags;
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum EventClock {
    #[default]
    Default,
    HardwareTimestampEngine,
    RealTime,
}
