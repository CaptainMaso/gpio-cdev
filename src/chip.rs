use std::{
    ffi::CStr,
    fs::File,
    io::Result,
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
    os::{
        fd::{AsFd, AsRawFd, BorrowedFd, IntoRawFd},
        unix::{ffi::OsStrExt, fs::DirEntryExt},
    },
    path::Path,
};

use bstr::{BStr, ByteSlice};

use crate::{
    fixed_str::FixedStr,
    line::{options::AsLineOptions, LineInfo},
    uapi,
};

pub trait AsLineSet {
    type Iter<'a>: Iterator<Item = u32>
    where
        Self: 'a;

    fn iter_offsets(&self) -> Self::Iter<'_>;

    fn get_lines(&self) -> Result<LineSet> {
        LineSet::from_iter(self.iter_offsets())
    }
}

impl AsLineSet for u32 {
    type Iter<'a> = core::iter::Once<u32> where Self: 'a;

    #[inline(always)]
    fn iter_offsets(&self) -> Self::Iter<'static> {
        core::iter::once(*self)
    }
}

impl AsLineSet for [u32] {
    type Iter<'a> = core::iter::Copied<core::slice::Iter<'a, u32>> where Self: 'a;

    #[inline(always)]
    fn iter_offsets(&self) -> Self::Iter<'_> {
        self.iter().copied()
    }
}

impl<const N: usize> AsLineSet for [u32; N] {
    type Iter<'a> = core::iter::Copied<core::slice::Iter<'a, u32>> where Self: 'a;

    #[inline(always)]
    fn iter_offsets(&self) -> Self::Iter<'_> {
        self.iter().copied()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LineSet(heapless::Vec<u32, { uapi::v2::GPIO_LINES_MAX }>);

impl LineSet {
    pub const fn empty() -> Self {
        Self(heapless::Vec::new())
    }

    pub fn add_offset(&mut self, offset: u32) -> Result<()> {
        match self.0.binary_search(&offset) {
            Ok(_) => Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "Line offset already in set",
            )),
            Err(e) => self.0.insert(e, offset).map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::OutOfMemory,
                    "Line set exceeded maximum number of items: 64",
                )
            }),
        }
    }

    pub fn remove_offset(&mut self, offset: u32) -> Result<()> {
        let idx = self.0.binary_search(&offset).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "Line offset not in line set")
        })?;
        self.0.remove(idx);
        Ok(())
    }

    pub fn join(&mut self, other: Self) -> Result<()> {
        // ASSUMPTIONS:
        // This function assumes the following:
        //  - All items in a line set are sorted
        //  - All items in a line set are deduplicated.

        let old = core::mem::take(&mut self.0);

        let mut a = old.iter().copied();
        let mut b = other.0.iter().copied();

        let mut a_pending = a.next();
        let mut b_pending = b.next();

        let res = loop {
            let a_next = a_pending.take().or_else(|| a.next());
            let b_next = b_pending.take().or_else(|| b.next());

            let next = match (a_next, b_next) {
                (None, None) => break Ok(()),
                (None, Some(n)) => {
                    // A is exhausted, drain B
                    if let Err(_) = self.0.push(n) {
                        break Err(());
                    }

                    if self.0.len() + b.len() > uapi::v2::GPIO_LINES_MAX {
                        break Err(());
                    }

                    self.0.extend(b);
                    break Ok(());
                }
                (Some(n), None) => {
                    // B is exhausted, drain A
                    if let Err(_) = self.0.push(n) {
                        break Err(());
                    }

                    if self.0.len() + a.len() > uapi::v2::GPIO_LINES_MAX {
                        break Err(());
                    }

                    self.0.extend(a);
                    break Ok(());
                }
                (Some(a_val), Some(b_val)) => {
                    match a_val.cmp(&b_val) {
                        std::cmp::Ordering::Equal => {
                            // Equal, this lets us dedup between iterators
                            a_val
                        }
                        std::cmp::Ordering::Less => {
                            b_pending = Some(b_val);
                            a_val
                        }
                        std::cmp::Ordering::Greater => {
                            a_pending = Some(a_val);
                            b_val
                        }
                    }
                }
            };

            if let Err(_) = self.0.push(next) {
                break Err(());
            }
        };

        match res {
            Ok(()) => Ok(()),
            Err(_) => {
                self.0 = old;
                Err(std::io::Error::new(
                    std::io::ErrorKind::OutOfMemory,
                    "Line set exceeded maximum number of items: 64",
                ))
            }
        }
    }

    pub fn from_iter(iter: impl IntoIterator<Item = u32>) -> Result<Self> {
        let mut iter = iter.into_iter();
        let mut vec: heapless::Vec<_, { uapi::v2::GPIO_LINES_MAX }> =
            iter.by_ref().take(uapi::v2::GPIO_LINES_MAX).collect();

        if iter.next() != None {
            return Err(std::io::Error::new(
                std::io::ErrorKind::OutOfMemory,
                "Line set exceeded maximum number of items: 64",
            ));
        }

        vec.sort();
        let init_len = vec.len();
        dedup(&mut vec, |a, b| a == b);
        if vec.len() != init_len {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Line set contained duplicate lines",
            ));
        }

        Ok(Self(vec))
    }

    pub fn extend(&mut self, iter: impl IntoIterator<Item = u32>) -> Result<()> {
        let iter = Self::from_iter(iter.into_iter())?;
        self.join(iter)
    }

    fn to_api_v2(self) -> (u32, [u32; uapi::v2::GPIO_LINES_MAX]) {
        let len = self.0.len() as u32;
        let mut lines = [0; uapi::v2::GPIO_LINES_MAX];
        for (offset, wr) in self.0.into_iter().zip(lines.iter_mut()) {
            *wr = offset;
        }
        (len, lines)
    }
}

impl std::ops::Deref for LineSet {
    type Target = [u32];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl AsLineSet for LineSet {
    type Iter<'a> = core::iter::Copied<core::slice::Iter<'a, u32>> where Self: 'a;

    fn iter_offsets(&self) -> Self::Iter<'_> {
        self.0.iter().copied()
    }

    fn get_lines(&self) -> Result<LineSet> {
        Ok(self.clone())
    }
}

pub trait AsGpioChip: std::os::fd::AsRawFd {
    fn get_info(&self) -> Result<ChipInfo> {
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

    /// Get the information of a line at a given offset.
    fn line_info(&self, offset: u32) -> Result<LineInfo> {
        unsafe {
            let info = LineInfo::new_get(offset);
            let mut info = info.into_v2();

            let _ = uapi::v2::gpio_get_lineinfo(self.as_raw_fd(), &mut info)?;

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
    fn line<O: AsLineOptions>(
        &self,
        consumer: &str,
        options: O,
        offset: u32,
    ) -> Result<Lines<'_, u32>> {
        self.lines(consumer, options, offset)
    }

    /// Get a handle to multiple GPIO line at a given offsets
    ///
    /// The group of lines can be manipulated simultaneously.
    fn lines<O: AsLineOptions, L: AsLineSet>(
        &self,
        consumer: &str,
        options: O,
        line_offsets: L,
    ) -> Result<Lines<'_, L>> {
        let chip = unsafe { ChipRef::from_raw(self.as_raw_fd()) };
        Lines::new(chip, consumer, options, line_offsets)
    }

    /// Get a handle to all the GPIO lines on the chip
    ///
    /// The group of lines can be manipulated simultaneously.
    fn get_all_lines<O: AsLineOptions>(
        &self,
        consumer: &str,
        options: O,
    ) -> Result<Lines<LineSet>> {
        let info = self.get_info()?;

        if info.num_lines() as usize > uapi::v2::GPIO_LINES_MAX {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "Too many lines on chip '{}' to get all lines: {} > {}",
                    info.name(),
                    info.num_lines(),
                    uapi::v2::GPIO_LINES_MAX
                ),
            ));
        }

        let offsets = LineSet((0..info.num_lines()).collect());
        self.lines(consumer, options, offsets)
    }
}

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

pub struct Lines<'a, S> {
    chip: ChipRef<'a>,
    offsets: S,
}

impl<'a, S> Lines<'a, S>
where
    S: AsLineSet,
{
    pub(crate) fn new(
        chip: ChipRef<'a>,
        consumer: &str,
        options: impl crate::line::options::AsLineOptions,
        offsets: S,
    ) -> Result<Self> {
        unsafe {
            let mut req = uapi::v2::gpio_line_request::zeroed();

            let (n_lines, lines) = offsets.get_lines()?.to_api_v2();
            req.num_lines = n_lines;
            req.offsets = lines;
            req.config.flags = options.build_v2();
            req.consumer = FixedStr::new(consumer)?.into_byte_array();

            let _ = uapi::v2::gpio_get_line(chip.as_raw_fd(), &mut req)?;
        };

        Ok(Self { chip, offsets })
    }

    pub fn get_info(&self) {}
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
pub struct Chip {
    fd: std::os::fd::OwnedFd,
}

impl Chip {
    /// Open the GPIO Chip at the provided path (e.g. `/dev/gpiochip<N>`)
    pub fn open(p: &Path) -> Result<Self> {
        let f = std::fs::OpenOptions::new().read(true).write(true).open(p)?;
        let fd = std::os::fd::OwnedFd::from(f);
        let this = Self { fd };
        let _ = this.get_info()?;
        Ok(this)
    }

    #[inline(always)]
    pub fn borrow(&self) -> ChipRef<'_> {
        ChipRef {
            fd: self.fd.as_fd(),
        }
    }
}

impl AsGpioChip for Chip {}

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

pub struct ChipRef<'a> {
    fd: std::os::fd::BorrowedFd<'a>,
}

impl<'a> ChipRef<'a> {
    pub const unsafe fn from_raw(fd: std::os::fd::RawFd) -> Self {
        Self {
            fd: std::os::fd::BorrowedFd::borrow_raw(fd),
        }
    }

    pub fn borrow(&self) -> ChipRef<'a> {
        ChipRef { fd: self.fd }
    }
}

impl AsGpioChip for ChipRef<'_> {}

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
                    return Some(Err(e.into()));
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

fn dedup<T, const N: usize>(
    v: &mut heapless::Vec<T, N>,
    mut same_bucket: impl FnMut(&T, &T) -> bool,
) {
    let len = v.len();
    if len <= 1 {
        return;
    }

    // Check if we ever want to remove anything.
    // This allows to use copy_non_overlapping in next cycle.
    // And avoids any memory writes if we don't need to remove anything.
    let mut first_duplicate_idx: usize = 1;
    let start = v.as_mut_ptr();
    while first_duplicate_idx != len {
        let found_duplicate = unsafe {
            // SAFETY: first_duplicate always in range [1..len)
            // Note that we start iteration from 1 so we never overflow.
            let prev = start.add(first_duplicate_idx.wrapping_sub(1));
            let current = start.add(first_duplicate_idx);
            // We explicitly say in docs that references are reversed.
            same_bucket(&mut *current, &mut *prev)
        };
        if found_duplicate {
            break;
        }
        first_duplicate_idx += 1;
    }
    // Don't need to remove anything.
    // We cannot get bigger than len.
    if first_duplicate_idx == len {
        return;
    }

    /* INVARIANT: vec.len() > read > write > write-1 >= 0 */
    struct FillGapOnDrop<'a, T, const N: usize> {
        /* Offset of the element we want to check if it is duplicate */
        read: usize,

        /* Offset of the place where we want to place the non-duplicate
         * when we find it. */
        write: usize,

        /* The Vec that would need correction if `same_bucket` panicked */
        vec: &'a mut heapless::Vec<T, N>,
    }

    impl<'a, T, const N: usize> Drop for FillGapOnDrop<'a, T, N> {
        fn drop(&mut self) {
            /* This code gets executed when `same_bucket` panics */

            /* SAFETY: invariant guarantees that `read - write`
             * and `len - read` never overflow and that the copy is always
             * in-bounds. */
            unsafe {
                let ptr = self.vec.as_mut_ptr();
                let len = self.vec.len();

                /* How many items were left when `same_bucket` panicked.
                 * Basically vec[read..].len() */
                let items_left = len.wrapping_sub(self.read);

                /* Pointer to first item in vec[write..write+items_left] slice */
                let dropped_ptr = ptr.add(self.write);
                /* Pointer to first item in vec[read..] slice */
                let valid_ptr = ptr.add(self.read);

                /* Copy `vec[read..]` to `vec[write..write+items_left]`.
                 * The slices can overlap, so `copy_nonoverlapping` cannot be used */
                core::ptr::copy(valid_ptr, dropped_ptr, items_left);

                /* How many items have been already dropped
                 * Basically vec[read..write].len() */
                let dropped = self.read.wrapping_sub(self.write);

                self.vec.set_len(len - dropped);
            }
        }
    }

    /* Drop items while going through Vec, it should be more efficient than
     * doing slice partition_dedup + truncate */

    // Construct gap first and then drop item to avoid memory corruption if `T::drop` panics.
    let mut gap = FillGapOnDrop {
        read: first_duplicate_idx + 1,
        write: first_duplicate_idx,
        vec: v,
    };
    unsafe {
        // SAFETY: we checked that first_duplicate_idx in bounds before.
        // If drop panics, `gap` would remove this item without drop.
        core::ptr::drop_in_place(start.add(first_duplicate_idx));
    }

    /* SAFETY: Because of the invariant, read_ptr, prev_ptr and write_ptr
     * are always in-bounds and read_ptr never aliases prev_ptr */
    unsafe {
        while gap.read < len {
            let read_ptr = start.add(gap.read);
            let prev_ptr = start.add(gap.write.wrapping_sub(1));

            // We explicitly say in docs that references are reversed.
            let found_duplicate = same_bucket(&mut *read_ptr, &mut *prev_ptr);
            if found_duplicate {
                // Increase `gap.read` now since the drop may panic.
                gap.read += 1;
                /* We have found duplicate, drop it in-place */
                core::ptr::drop_in_place(read_ptr);
            } else {
                let write_ptr = start.add(gap.write);

                /* read_ptr cannot be equal to write_ptr because at this point
                 * we guaranteed to skip at least one element (before loop starts).
                 */
                core::ptr::copy_nonoverlapping(read_ptr, write_ptr, 1);

                /* We have filled that place, so go further */
                gap.write += 1;
                gap.read += 1;
            }
        }

        /* Technically we could let `gap` clean up with its Drop, but
         * when `same_bucket` is guaranteed to not panic, this bloats a little
         * the codegen, so we just do it manually */
        gap.vec.set_len(gap.write);
        core::mem::forget(gap);
    }
}
