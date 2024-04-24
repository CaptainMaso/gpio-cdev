use std::marker::PhantomData;

use crate::uapi;

use super::options::*;

pub struct HasInput;
pub struct HasOpenOutput;

pub struct HasDrivenOutput;

pub struct HasEvent;

pub struct LineOptionBuilder<Dir> {
    pub(super) d: PhantomData<Dir>,
    pub(super) active: Option<Active>,
    pub(super) edge: Option<EdgeDetect>,
    pub(super) bias: Option<Bias>,
    pub(super) drive: Option<Drive>,
    pub(super) clock: Option<EventClock>,
}

impl<D> LineOptionBuilder<D> {
    pub(super) const fn conv<O>(self) -> LineOptionBuilder<O> {
        let Self {
            d: _,
            active,
            edge,
            bias,
            drive,
            clock,
        } = self;
        LineOptionBuilder {
            d: PhantomData,
            active,
            edge,
            bias,
            drive,
            clock,
        }
    }
}

impl LineOptionBuilder<()> {
    pub const fn new() -> Self {
        Self {
            d: PhantomData,
            active: None,
            edge: None,
            bias: None,
            drive: None,
            clock: None,
        }
    }

    pub const fn input(self) -> LineOptionBuilder<HasInput> {
        self.conv()
    }

    pub const fn output(self) -> LineOptionBuilder<HasDrivenOutput> {
        self.conv()
    }
}

impl LineOptionBuilder<HasInput> {
    pub const fn with_active(self, active: Active) -> Self {
        Self {
            active: Some(active),
            ..self
        }
    }

    pub const fn with_bias(self, bias: Bias) -> Self {
        Self {
            bias: Some(bias),
            ..self
        }
    }

    pub const fn with_edge_detect(self, edge_detect: EdgeDetect) -> Self {
        Self {
            edge: Some(edge_detect),
            ..self
        }
    }

    pub const fn with_clock_source(self, clock: EventClock) -> Self {
        Self {
            clock: Some(clock),
            ..self
        }
    }

    pub(crate) const fn build_v2(self) -> uapi::v2::LineFlags {
        use uapi::v2::LineFlags;

        let flags = LineFlags::INPUT;

        let flags = match self.active {
            Some(Active::Low) => flags.union(LineFlags::ACTIVE_LOW),
            Some(Active::High) | None => flags,
        };

        let flags = match self.bias {
            Some(Bias::PullDown) => flags.union(LineFlags::BIAS_PULL_DOWN),
            Some(Bias::PullUp) => flags.union(LineFlags::BIAS_PULL_UP),
            Some(Bias::Disabled) | None => flags.union(LineFlags::BIAS_DISABLED),
        };

        let flags = match self.edge {
            Some(EdgeDetect::Both) => flags
                .union(LineFlags::EDGE_RISING)
                .union(LineFlags::EDGE_FALLING),
            Some(EdgeDetect::Rising) => flags.union(LineFlags::EDGE_RISING),
            Some(EdgeDetect::Falling) => flags.union(LineFlags::EDGE_FALLING),
            None => flags,
        };

        if self.edge.is_some() {
            match self.clock {
                Some(EventClock::HardwareTimestampEngine) => {
                    flags.union(LineFlags::EVENT_CLOCK_HTE)
                }
                Some(EventClock::RealTime) => flags.union(LineFlags::EVENT_CLOCK_REALTIME),
                Some(EventClock::Default) | None => flags,
            }
        } else {
            flags
        }
    }
}

impl LineOptionBuilder<HasOpenOutput> {
    pub const fn with_active(self, active: Active) -> Self {
        Self {
            active: Some(active),
            ..self
        }
    }

    pub const fn with_drive(self, drive: Drive) -> Self {
        Self {
            drive: Some(drive),
            ..self
        }
    }

    pub const fn with_bias(self, bias: Bias) -> Self {
        Self {
            bias: Some(bias),
            ..self
        }
    }

    pub const fn with_edge_detect(
        self,
        edge_detect: EdgeDetect,
    ) -> LineOptionBuilder<HasOpenOutput> {
        Self {
            edge: Some(edge_detect),
            ..self
        }
    }

    pub const fn with_clock_source(self, clock: EventClock) -> Self {
        Self {
            clock: Some(clock),
            ..self
        }
    }

    pub(crate) const fn build_v2(self) -> uapi::v2::LineFlags {
        use uapi::v2::LineFlags;

        let flags = LineFlags::OUTPUT;

        let flags = match self.active {
            Some(Active::Low) => flags.union(LineFlags::ACTIVE_LOW),
            Some(Active::High) | None => flags,
        };

        let flags = match self.bias {
            Some(Bias::PullDown) => flags.union(LineFlags::BIAS_PULL_DOWN),
            Some(Bias::PullUp) => flags.union(LineFlags::BIAS_PULL_UP),
            Some(Bias::Disabled) | None => flags.union(LineFlags::BIAS_DISABLED),
        };

        let flags = match self.edge {
            Some(EdgeDetect::Both) => flags
                .union(LineFlags::EDGE_RISING)
                .union(LineFlags::EDGE_FALLING),
            Some(EdgeDetect::Rising) => flags.union(LineFlags::EDGE_RISING),
            Some(EdgeDetect::Falling) => flags.union(LineFlags::EDGE_FALLING),
            None => flags,
        };

        let flags = match self.drive {
            Some(Drive::OpenDrain) => flags.union(LineFlags::OPEN_DRAIN),
            Some(Drive::OpenSource) => flags.union(LineFlags::OPEN_SOURCE),
            None => flags,
        };

        if self.edge.is_some() {
            match self.clock {
                Some(EventClock::HardwareTimestampEngine) => {
                    flags.union(LineFlags::EVENT_CLOCK_HTE)
                }
                Some(EventClock::RealTime) => flags.union(LineFlags::EVENT_CLOCK_REALTIME),
                Some(EventClock::Default) | None => flags,
            }
        } else {
            flags
        }
    }
}

impl LineOptionBuilder<HasDrivenOutput> {
    pub const fn with_active(self, active: Active) -> Self {
        Self {
            active: Some(active),
            ..self
        }
    }

    pub const fn with_drive_open(self, drive: Drive) -> LineOptionBuilder<HasOpenOutput> {
        Self {
            drive: Some(drive),
            ..self
        }
        .conv()
    }

    pub(crate) const fn build_v2(self) -> uapi::v2::LineFlags {
        use uapi::v2::LineFlags;

        let flags = LineFlags::OUTPUT;

        match self.active {
            Some(Active::Low) => flags.union(LineFlags::ACTIVE_LOW),
            Some(Active::High) | None => flags,
        }
    }
}

impl Default for LineOptionBuilder<()> {
    fn default() -> Self {
        Self::new()
    }
}

impl AsLineOptions for LineOptionBuilder<HasInput> {
    #[inline(always)]
    fn build_v2(self) -> uapi::v2::LineFlags {
        Self::build_v2(self)
    }
}

impl AsLineOptions for LineOptionBuilder<HasDrivenOutput> {
    #[inline(always)]
    fn build_v2(self) -> uapi::v2::LineFlags {
        Self::build_v2(self)
    }
}

impl AsLineOptions for LineOptionBuilder<HasOpenOutput> {
    #[inline(always)]
    fn build_v2(self) -> uapi::v2::LineFlags {
        Self::build_v2(self)
    }
}

#[cfg(test)]
mod test {
    use crate::uapi::v2::LineFlags;

    use super::*;

    #[test]
    pub fn build_input() {
        const FLAGS: uapi::v2::LineFlags = LineOptionBuilder::new()
            .input()
            .with_active(Active::Low)
            .with_bias(Bias::PullUp)
            .with_edge_detect(EdgeDetect::Both)
            .with_clock_source(EventClock::RealTime)
            .build_v2();

        let expected = LineFlags::INPUT
            | LineFlags::ACTIVE_LOW
            | LineFlags::BIAS_PULL_UP
            | LineFlags::EDGE_RISING
            | LineFlags::EDGE_FALLING
            | LineFlags::EVENT_CLOCK_REALTIME;

        assert_eq!(FLAGS, expected);
    }

    #[test]
    pub fn build_open_collector_output() {
        const FLAGS: uapi::v2::LineFlags = LineOptionBuilder::new()
            .output()
            .with_drive_open(Drive::OpenSource)
            .with_active(Active::Low)
            .with_bias(Bias::PullUp)
            .with_edge_detect(EdgeDetect::Both)
            .with_clock_source(EventClock::RealTime)
            .build_v2();

        let expected = LineFlags::OUTPUT
            | LineFlags::ACTIVE_LOW
            | LineFlags::OPEN_SOURCE
            | LineFlags::BIAS_PULL_UP
            | LineFlags::EDGE_RISING
            | LineFlags::EDGE_FALLING
            | LineFlags::EVENT_CLOCK_REALTIME;

        assert_eq!(FLAGS, expected);
    }
}
