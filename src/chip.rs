use std::{
    io::Result,
    ops::Deref,
    os::{
        fd::{AsFd, AsRawFd, BorrowedFd},
        unix::ffi::OsStrExt,
    },
    path::Path,
};

use itertools::Itertools;

use bstr::ByteSlice;

use crate::{
    fixed_str::FixedStr,
    line::{options::AsLineOptions, set::AsLineSet, LineInfo, LineSet, Lines},
    uapi,
};

pub struct ChipInfo {
    name: FixedStr<{ uapi::v2::GPIO_MAX_NAME_SIZE }>,
    label: FixedStr<{ uapi::v2::GPIO_MAX_NAME_SIZE }>,
    lines: u32,
}

impl ChipInfo {
    /// The name of the device driving this GPIO chip in the kernel
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// A functional name for this GPIO chip, such as a product number.  Might
    /// be an empty string.
    ///
    /// As an example, the SoC GPIO chip on a Raspberry Pi is "pinctrl-bcm2835"
    pub fn label(&self) -> &str {
        self.label.as_str()
    }

    /// The number of lines/pins indexable through this chip
    ///
    /// Not all of these may be usable depending on how the hardware is
    /// configured/muxed.
    pub const fn num_lines(&self) -> u32 {
        self.lines
    }
}

/// A GPIO Chip maps to the actual device driver instance in hardware that
/// one interacts with to interact with individual GPIOs.  Often these chips
/// map to IP chunks on an SoC but could also be enumerated within the kernel
/// via something like a PCI or USB bus.
///
/// The Linux kernel itself enumerates GPIO character devices at two paths:
/// 1. `/dev/gpiochipN`
/// 2. `/sys/bus/gpiochipN`
///
/// It is best not to assume that a device will always be enumerated in the
/// same order (especially if it is connected via a bus).  In order to reliably
/// find the correct chip, there are a few approaches that one could reasonably
/// take:
///
/// 1. Create a udev rule that will match attributes of the device and
///    setup a symlink to the device.
/// 2. Iterate over all available chips using the [`chips()`] call to find the
///    device with matching criteria.
/// 3. For simple cases, just using the enumerated path is fine (demo work).  This
///    is discouraged for production.
///
/// [`chips()`]: fn.chips.html
#[derive(Debug)]
#[repr(transparent)]
pub struct Chip {
    fd: std::os::fd::OwnedFd,
}

impl Chip {
    /// Open the GPIO Chip at the provided path (e.g. `/dev/gpiochip<N>`)
    pub fn open(p: &Path) -> Result<Self> {
        let f = std::fs::OpenOptions::new().read(true).write(true).open(p)?;
        let fd = std::os::fd::OwnedFd::from(f);
        let this = Self { fd };
        let _ = this.chip_info()?;
        Ok(this)
    }

    #[inline(always)]
    pub fn borrow(&self) -> ChipRef<'_> {
        ChipRef {
            fd: self.fd.as_fd(),
        }
    }

    pub fn chip_info(&self) -> Result<ChipInfo> {
        let mut info: uapi::gpio_chip_info = unsafe { std::mem::zeroed() };
        // Error condition: -1, already handled
        let _ = unsafe { uapi::gpio_get_chipinfo(self.as_raw_fd(), &mut info)? };

        let info = ChipInfo {
            name: FixedStr::from_byte_array(info.name)?,
            label: FixedStr::from_byte_array(info.label)?,
            lines: info.lines,
        };
        Ok(info)
    }

    pub fn lines(&self) -> impl Iterator<Item = Result<(u32, LineInfo)>> + '_ {
        std::iter::once(self.chip_info())
            .map_ok(move |m| {
                (0..m.lines).map(move |offset| {
                    let line_info = self.line_info(offset)?;
                    std::io::Result::Ok((offset, line_info))
                })
            })
            .flatten_ok()
            .map(|r| r?)
    }

    /// Get the information of a line at a given offset.
    pub fn line_info(&self, offset: u32) -> Result<LineInfo> {
        unsafe {
            let info = LineInfo::new_get(offset);
            let mut info = info.into_v2();

            let _ = uapi::v2::gpio_get_line_info(self.as_raw_fd(), &mut info)?;

            LineInfo::from_v2(info)
        }
    }

    /// Get a handle to the GPIO line at a given offset
    ///
    /// The actual physical line corresponding to a given offset
    /// is completely dependent on how the driver/hardware for
    /// the chip works as well as the associated board layout.
    ///
    /// For a device like the NXP i.mx6 SoC GPIO controller there
    /// are several banks of GPIOs with each bank containing 32
    /// GPIOs.  For this hardware and driver something like
    /// `GPIO2_5` would map to offset 37.
    #[inline(always)]
    pub fn open_line<O: AsLineOptions>(
        &self,
        consumer: &str,
        options: O,
        offset: u32,
    ) -> Result<Lines<1>> {
        self.open_lines(consumer, options, offset)
    }

    /// Get a handle to multiple GPIO line at a given offsets
    ///
    /// The group of lines can be manipulated simultaneously.
    pub fn open_lines<O: AsLineOptions, L: AsLineSet, const LINES: usize>(
        &self,
        consumer: &str,
        options: O,
        line_offsets: L,
    ) -> Result<Lines<{ LINES }>> {
        let chip = self.borrow();
        Lines::new(chip, consumer, line_offsets, options)
    }

    /// Get a handle to all the GPIO lines on the chip
    ///
    /// The group of lines can be manipulated simultaneously.
    pub fn open_all_lines<O: AsLineOptions, const L: usize>(
        &self,
        consumer: &str,
        options: O,
    ) -> Result<Lines<L>> {
        let info = self.chip_info()?;

        let offsets = LineSet::<L>::try_from_iter(0..info.num_lines()).map_err(|_e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "Too many lines on chip '{}' to get all lines: {} > {}",
                    info.name(),
                    info.num_lines(),
                    uapi::v2::GPIO_LINES_MAX
                ),
            )
        })?;

        self.open_lines(consumer, options, offsets)
    }
}

impl Deref for ChipRef<'_> {
    type Target = Chip;

    fn deref(&self) -> &Self::Target {
        // SAFETY: This is safe because both Chip and ChipRef are repr(transparent), and
        // OwnedFd and BorrowedFd specify repr(transparent) in the documentation.
        unsafe { core::mem::transmute(self) }
    }
}

impl std::os::fd::AsRawFd for Chip {
    #[inline(always)]
    fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
        self.fd.as_raw_fd()
    }
}

impl std::os::fd::AsFd for Chip {
    #[inline(always)]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

#[repr(transparent)]
pub struct ChipRef<'a> {
    fd: std::os::fd::BorrowedFd<'a>,
}

impl<'a> ChipRef<'a> {
    /// # Safety
    ///
    /// The caller must ensure that the file descriptor is not closed for the arbitrary lifetime
    /// that is returned by this function.
    pub const unsafe fn from_raw(fd: std::os::fd::RawFd) -> Self {
        Self {
            fd: std::os::fd::BorrowedFd::borrow_raw(fd),
        }
    }

    pub fn try_to_owned(&self) -> Result<Chip> {
        let fd = self.fd.try_clone_to_owned()?;
        Ok(Chip { fd })
    }

    pub fn borrow(&self) -> ChipRef<'a> {
        ChipRef { fd: self.fd }
    }
}

impl std::os::fd::AsRawFd for ChipRef<'_> {
    #[inline(always)]
    fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
        self.fd.as_raw_fd()
    }
}

impl std::os::fd::AsFd for ChipRef<'_> {
    #[inline(always)]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

/// Iterate over all GPIO chips currently present on this system
pub fn chips() -> crate::errors::Result<ChipIterator> {
    Ok(ChipIterator {
        readdir: std::fs::read_dir("/dev")?,
    })
}

/// Iterator over chips
#[derive(Debug)]
pub struct ChipIterator {
    readdir: std::fs::ReadDir,
}

impl Iterator for ChipIterator {
    type Item = Result<Chip>;

    fn next(&mut self) -> Option<Result<Chip>> {
        for entry in &mut self.readdir {
            let e = match entry {
                Ok(e) => e,
                Err(e) => {
                    return Some(Err(e));
                }
            };
            let p = e.path();
            let Some(f) = p.file_name() else {
                continue;
            };
            if f.as_bytes().contains_str("gpiochip") {
                return Some(Chip::open(&p));
            }
        }

        None
    }
}
