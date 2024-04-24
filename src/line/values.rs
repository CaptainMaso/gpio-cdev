use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineValue {
    Active,
    Inactive,
}

impl LineValue {
    pub const fn new(is_active: bool) -> Self {
        if is_active {
            Self::Active
        } else {
            Self::Inactive
        }
    }

    pub const fn is_active(&self) -> bool {
        matches!(self, LineValue::Active)
    }
}

pub trait AsValues {
    fn values<const N: usize>(&self, lines: &LineSet<N>) -> Result<MaskedBits>;
}

impl AsValues for LineValue {
    fn values<const N: usize>(&self, lines: &LineSet<N>) -> Result<MaskedBits> {
        let mask = lines.mask();

        let bits = if self.is_active() { mask } else { 0 };

        Ok(MaskedBits { bits, mask })
    }
}

impl AsValues for bool {
    fn values<const N: usize>(&self, lines: &LineSet<N>) -> Result<MaskedBits> {
        let mask = lines.mask();

        let bits = if *self { mask } else { 0 };

        Ok(MaskedBits { bits, mask })
    }
}

impl AsValues for [(u32, bool)] {
    fn values<const N: usize>(&self, lines: &LineSet<N>) -> Result<MaskedBits> {
        let mut iter = self.iter().copied().map(|(offset, val)| {
            let v = lines.find_idx(offset).map(|idx| (idx, val));
            (offset, v)
        });

        let mut bits = MaskedBits::empty();

        for (_offset, v) in iter.by_ref() {
            match v {
                Some((idx, val)) => {
                    bits.set_bit_value(idx, val);
                }
                None => {
                    use std::fmt::Write;
                    let mut missing = iter
                        .filter(|(_offset, v)| v.is_none())
                        .map(|(offset, _)| offset);

                    let mut msg = format!("Offsets not in line set: {}", missing.next().unwrap());

                    for m in missing {
                        let _ = write!(&mut msg, ", {m}");
                    }

                    return Err(std::io::Error::new(std::io::ErrorKind::NotFound, msg));
                }
            }
        }

        Ok(bits)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaskedBits {
    pub(crate) bits: u64,
    pub(crate) mask: u64,
}

impl MaskedBits {
    pub const fn new(bits: u64, mask: u64) -> Self {
        Self { bits, mask }
    }

    pub const fn empty() -> Self {
        MaskedBits { bits: 0, mask: 0 }
    }

    pub const fn all(set: bool) -> Self {
        if set {
            MaskedBits {
                bits: u64::MAX,
                mask: u64::MAX,
            }
        } else {
            MaskedBits {
                bits: 0,
                mask: u64::MAX,
            }
        }
    }

    pub const fn bits(&self) -> u64 {
        self.bits & self.mask
    }

    pub const fn mask(&self) -> u64 {
        self.mask
    }

    pub const fn len(&self) -> usize {
        self.mask.count_ones() as usize
    }

    pub const fn is_empty(&self) -> bool {
        self.mask == 0
    }

    #[inline(always)]
    pub const fn get(&self, bit: usize) -> Option<bool> {
        let bit = 1 << bit;
        if self.mask & bit > 0 {
            Some(self.bits & bit > 0)
        } else {
            None
        }
    }

    #[inline(always)]
    pub const fn with_bit_set(self, bit: usize) -> Self {
        let bit = 1 << bit;
        Self {
            bits: self.bits | bit,
            mask: self.mask | bit,
        }
    }

    #[inline(always)]
    pub const fn with_bit_cleared(self, bit: usize) -> Self {
        let bit = 1 << bit;
        Self {
            bits: self.bits & !bit,
            mask: self.mask | bit,
        }
    }

    #[inline(always)]
    pub const fn with_bit(self, bit: usize, value: bool) -> Self {
        if value {
            self.with_bit_set(bit)
        } else {
            self.with_bit_cleared(bit)
        }
    }

    #[inline]
    pub fn set_bit_value(&mut self, bit: usize, value: bool) {
        if value {
            self.set_bit(bit);
        } else {
            self.clear_bit(bit);
        }
    }

    #[inline(always)]
    pub fn set_bit(&mut self, bit: usize) {
        let bit = 1u64 << bit;

        self.bits |= bit;
        self.mask |= bit;
    }

    #[inline(always)]
    pub fn clear_bit(&mut self, bit: usize) {
        let bit = 1u64 << bit;

        self.bits &= !bit;
        self.mask |= bit;
    }

    pub fn iter(&self) -> impl Iterator<Item = (usize, bool)> + 'static {
        let mask = self.mask;
        let bits = self.bits;
        (0..64).filter_map(move |idx| {
            let bit = 1u64 << idx;

            if mask & bit > 0 {
                Some((idx, bits & bit > 0))
            } else {
                None
            }
        })
    }

    pub fn try_merge(&self, other: MaskedBits) -> Result<MaskedBits> {
        let s_bits = self.bits & self.mask;
        let o_bits = other.bits & other.mask;
        let conflicting = s_bits ^ o_bits;
        if conflicting > 0 {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Line values contained conflicting values",
            ))
        } else {
            Ok(Self {
                bits: s_bits | o_bits,
                mask: self.mask | other.mask,
            })
        }
    }
}

pub struct LineValuesRef<'a> {
    pub(crate) offsets: &'a LineSetRef,
    pub(crate) values: MaskedBits,
}

impl LineValuesRef<'_> {
    pub fn iter(&self) -> impl Iterator<Item = (u32, LineValue)> + '_ {
        self.offsets
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(idx, offset)| {
                let v = LineValue::new(self.values.get(idx)?);
                Some((offset, v))
            })
    }

    pub fn try_into_owned<const N: usize>(self) -> std::io::Result<LineValues<N>> {
        let offsets = self.offsets.try_into_owned()?;
        Ok(LineValues {
            offsets,
            values: self.values,
        })
    }

    fn fmt_inner(&self) -> impl std::fmt::Debug + '_ {
        struct F<'a>(&'a LineValuesRef<'a>);

        impl std::fmt::Debug for F<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut map = f.debug_map();

                for (offset, val) in self.0.iter() {
                    map.entry(&offset, &val);
                }

                map.finish()
            }
        }

        F(self)
    }
}

impl std::fmt::Debug for LineValuesRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fmt = self.fmt_inner();

        f.debug_tuple("LineValuesRef").field(&fmt).finish()
    }
}

pub struct LineValues<const N: usize> {
    offsets: LineSet<N>,
    values: MaskedBits,
}

impl<const N: usize> LineValues<N> {
    pub fn as_value_ref(&self) -> LineValuesRef<'_> {
        LineValuesRef {
            offsets: &self.offsets,
            values: self.values,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (u32, bool)> + '_ {
        self.offsets
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(idx, offset)| {
                let v = self.values.get(idx)?;
                Some((offset, v))
            })
    }
}

impl<const N: usize> std::fmt::Debug for LineValues<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let iter = self.as_value_ref();
        let iter = iter.fmt_inner();

        f.debug_tuple("LineValues").field(&iter).finish()
    }
}
