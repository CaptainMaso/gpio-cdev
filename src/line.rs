use std::{
    fs::File, io::Result, mem::MaybeUninit, os::fd::{AsFd, AsRawFd, FromRawFd, IntoRawFd, OwnedFd}, task::Poll, time::Duration
};

use crate::{
    chip::ChipRef,
    fixed_str::FixedStr,
    line::event::LineEvent,
    uapi::{self, v2::LineFlags},
    Chip,
};

mod event;
mod info;
mod option_builder;
pub mod options;
pub mod set;
pub mod values;

pub use info::LineInfo;
pub use set::LineSet;
pub use values::{LineValues, LineValuesRef};

use set::LineSetRef;
use values::MaskedBits;

pub struct Lines<const N: usize> {
    chip: Chip,
    line_fd: File,
    consumer: FixedStr<{ uapi::v2::GPIO_MAX_NAME_SIZE }>,
    offsets: LineSet<N>,
}

impl<const N: usize> Lines<N> {
    pub(crate) fn new(
        chip: ChipRef<'_>,
        consumer: &str,
        offsets: impl set::AsLineSet,
        options: impl options::AsLineOptions,
    ) -> Result<Self> {
        let consumer = FixedStr::new(consumer)?;
        let offsets: LineSet<N> = offsets.as_line_set()?;
        unsafe {
            let mut req = uapi::v2::gpio_line_request::zeroed();

            let (n_lines, lines) = offsets.to_api_v2();
            req.num_lines = n_lines;
            req.offsets = lines;
            req.config.flags = options.build_v2();
            req.consumer = consumer.into_byte_array();

            let _ = uapi::v2::gpio_get_line(chip.as_raw_fd(), &mut req)?;

            let line_fd = std::fs::File::from_raw_fd(req.fd);

            let chip = chip.try_to_owned()?;

            Ok(Self {
                chip,
                line_fd,
                offsets,
                consumer,
            })
        }
    }

    pub fn consumer(&self) -> &str {
        &self.consumer
    }

    pub fn len(&self) -> usize {
        self.offsets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.offsets.is_empty()
    }

    pub fn lines(&self) -> impl Iterator<Item = Result<(u32, LineInfo)>> + '_ {
        self.offsets
            .iter()
            .copied()
            .map(|offset| Ok((offset, self.chip.line_info(offset)?)))
    }

    pub fn line_info(&self, offset: u32) -> Result<LineInfo> {
        let _idx = self.offsets.find_idx(offset).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "Offset not found in Lines")
        })?;
        self.chip.line_info(offset)
    }

    pub fn read(&self) -> Result<values::LineValuesRef<'_>> {
        unsafe {
            let mask = self.offsets.mask();
            let mut data = uapi::v2::gpio_line_values { bits: 0, mask };
            let _ = uapi::v2::gpio_line_get_values(self.line_fd.as_raw_fd(), &mut data)?;
            let bits = MaskedBits {
                bits: data.bits,
                mask: data.mask,
            };

            Ok(values::LineValuesRef {
                offsets: &self.offsets,
                values: bits,
            })
        }
    }

    pub fn write(&mut self, values: impl values::AsValues) -> Result<values::LineValuesRef<'_>> {
        let offset_len = self.offsets.len();
        let mask = 2u64
            .checked_pow(offset_len as u32)
            .map(|p| p - 1)
            .unwrap_or(u64::MAX);

        let values = values.values(&self.offsets)?;

        let mut data = uapi::v2::gpio_line_values {
            bits: values.bits,
            mask: values.mask & mask,
        };

        unsafe {
            let _ = uapi::v2::gpio_line_set_values(self.line_fd.as_raw_fd(), &mut data)?;
        }

        let values = MaskedBits {
            bits: data.bits,
            mask: data.mask,
        };

        Ok(values::LineValuesRef {
            offsets: &self.offsets,
            values,
        })
    }

    /// Helper function which returns the line event if a complete event was read, Ok(None) if not
    /// enough data was read or the error returned by `read()`.
    pub(crate) fn try_read_event(&mut self) -> Poll<Result<Option<LineEvent>>> {
        use std::io::Read;

        let mut buf = [0; std::mem::size_of::<uapi::v2::gpio_line_event>()];
        {
            let mut buf_ptr = &mut buf[..];

            loop {
                match self.line_fd.read(&mut buf_ptr) {
                    Ok(read) => buf_ptr = &mut buf_ptr[read..],
                    Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock) => {
                        return Poll::Pending;
                    }
                    Err(e) if matches!(e.kind(), std::io::ErrorKind::Interrupted) => (),
                    Err(e) => return Poll::Ready(Err(e)),
                }

                if buf_ptr.is_empty() {
                    break;
                }
            }
        }

        let data = unsafe { uapi::v2::gpio_line_event::from_bytes(buf) };

        Ok(Some(data))
    }
}

fn wait_for_readable(
    fd: std::os::fd::BorrowedFd<'_>,
    timeout: Option<std::time::Duration>,
) -> std::result::Result<bool, std::io::Error> {
    let pollfd = nix::poll::PollFd::new(fd, nix::poll::PollFlags::POLLIN);
    let timeout = timeout
        .as_ref()
        .map(Duration::as_millis)
        .map(std::convert::TryInto::try_into)
        .transpose()
        .unwrap_or(Some(nix::poll::PollTimeout::MAX));

    if nix::poll::poll(&mut [pollfd], timeout)? == 0 {
        Ok(false)
    } else {
        Ok(true)
    }
}
