use crate::uapi;

use std::io::Result;

pub trait AsLineSet {
    fn as_line_set<const N: usize>(&self) -> Result<LineSet<N>>;
}

impl AsLineSet for u32 {
    fn as_line_set<const N: usize>(&self) -> Result<LineSet<N>> {
        LineSet::try_from_iter([1])
    }
}

impl AsLineSet for [u32] {
    fn as_line_set<const N: usize>(&self) -> Result<LineSet<N>> {
        LineSet::try_from_iter(self.iter().copied())
    }
}

impl<const M: usize> AsLineSet for [u32; M] {
    fn as_line_set<const N: usize>(&self) -> Result<LineSet<N>> {
        LineSet::try_from_iter(*self)
    }
}

#[repr(transparent)]
pub struct LineSetRef([u32]);

impl LineSetRef {
    /// # Safety:
    ///
    /// The caller must ensure that the slice is sorted and de-duplicated
    pub(crate) const unsafe fn new(offsets: &[u32]) -> &Self {
        unsafe { core::mem::transmute(offsets) }
    }

    pub const fn empty() -> &'static Self {
        unsafe { Self::new(&[]) }
    }

    pub(crate) fn mask(&self) -> u64 {
        1u64.checked_shl(self.len() as u32)
            .map(|v| v - 1)
            .unwrap_or(u64::MAX)
    }

    pub fn try_into_owned<const N: usize>(&self) -> Result<LineSet<N>> {
        if N < self.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::OutOfMemory,
                format!("Line set exceeded maximum number of items: {N}"),
            ));
        }
        Ok(LineSet(heapless::Vec::from_iter(self.iter().copied())))
    }

    pub fn get_offset(&self, idx: usize) -> Option<u32> {
        self.0.get(idx).copied()
    }

    pub fn find_idx(&self, offset: u32) -> Option<usize> {
        self.0.binary_search(&offset).ok()
    }

    pub(crate) fn to_api_v2(&self) -> (u32, [u32; uapi::v2::GPIO_LINES_MAX]) {
        let len = self.0.len() as u32;
        let mut lines = [0; uapi::v2::GPIO_LINES_MAX];
        for (offset, wr) in self.0.iter().zip(lines.iter_mut()) {
            *wr = *offset;
        }
        (len, lines)
    }
}

impl std::ops::Deref for LineSetRef {
    type Target = [u32];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<const N: usize> std::borrow::Borrow<LineSetRef> for LineSet<N> {
    fn borrow(&self) -> &LineSetRef {
        unsafe { LineSetRef::new(&self.0[..]) }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LineSet<const N: usize = { uapi::v2::GPIO_LINES_MAX }>(heapless::Vec<u32, { N }>);

impl<const N: usize> LineSet<N> {
    pub const fn empty() -> LineSet<0> {
        LineSet(heapless::Vec::new())
    }

    pub const fn capacity(&self) -> usize {
        if N > uapi::v2::GPIO_LINES_MAX {
            uapi::v2::GPIO_LINES_MAX
        } else {
            N
        }
    }

    pub fn add_offset(&mut self, offset: u32) -> Result<()> {
        match self.0.binary_search(&offset) {
            Ok(_) => Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "Line offset already in set",
            )),
            Err(e) => self.0.insert(e, offset).map_err(|_e| {
                std::io::Error::new(
                    std::io::ErrorKind::OutOfMemory,
                    format!("Line set exceeded maximum number of items: {N}"),
                )
            }),
        }
    }

    pub fn remove_offset(&mut self, offset: u32) -> Result<()> {
        let idx = self.find_idx(offset).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Line offset not found in line set",
            )
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
                    if self.0.push(n).is_err() {
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
                    if self.0.push(n).is_err() {
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

            if self.0.push(next).is_err() {
                break Err(());
            }
        };

        match res {
            Ok(()) => Ok(()),
            Err(_) => {
                self.0 = old;
                Err(std::io::Error::new(
                    std::io::ErrorKind::OutOfMemory,
                    format!(
                        "Line set exceeded maximum number of items: {}",
                        self.capacity()
                    ),
                ))
            }
        }
    }

    pub fn try_from_iter(iter: impl IntoIterator<Item = u32>) -> Result<Self> {
        let mut iter = iter.into_iter();
        let mut vec: heapless::Vec<_, N> = iter.by_ref().take(N).collect();

        if iter.next().is_some() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::OutOfMemory,
                format!("Line set exceeded maximum number of items: {N}"),
            ));
        }

        vec.sort();
        dedup(&mut vec, |a, b| *a == *b);

        Ok(Self(vec))
    }

    pub fn try_extend(&mut self, iter: impl IntoIterator<Item = u32>) -> Result<()> {
        let iter = Self::try_from_iter(iter.into_iter())?;
        self.join(iter)
    }
}

impl<const N: usize> std::ops::Deref for LineSet<N> {
    type Target = LineSetRef;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self
    }
}

impl<const M: usize> AsLineSet for LineSet<M> {
    fn as_line_set<const N: usize>(&self) -> Result<LineSet<N>> {
        LineSet::try_from_iter(self.0.iter().copied())
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
