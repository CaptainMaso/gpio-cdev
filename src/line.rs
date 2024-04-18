use std::{io::Result, mem::MaybeUninit, time::Duration};

use crate::{
    fixed_str::FixedStr,
    uapi::{
        self,
        v2::{gpio_line_attribute, LineFlags},
    },
};

mod option_builder;
pub mod options;

#[derive(Debug, Clone)]
pub struct LineInfo {
    name: FixedStr<{ uapi::v2::GPIO_MAX_NAME_SIZE }>,
    consumer: FixedStr<{ uapi::v2::GPIO_MAX_NAME_SIZE }>,
    offset: u32,
    flags: LineFlags,
    attrs: heapless::Vec<LineAttribute, { uapi::v2::GPIO_LINE_NUM_ATTRS_MAX }>,
}

impl LineInfo {
    const fn empty() -> Self {
        Self {
            name: FixedStr::empty(),
            consumer: FixedStr::empty(),
            offset: 0,
            flags: LineFlags::empty(),
            attrs: heapless::Vec::new(),
        }
    }

    pub const fn new_get(offset: u32) -> Self {
        Self {
            name: FixedStr::empty(),
            consumer: FixedStr::empty(),
            offset,
            flags: LineFlags::empty(),
            attrs: heapless::Vec::new(),
        }
    }

    pub fn new_set(
        offset: u32,
        name: &str,
        consumer: &str,
        flags: LineFlags,
        debounce: Option<Debounce>,
    ) -> Result<Self> {
        let name = FixedStr::new(name)?;
        let consumer = FixedStr::new(consumer)?;
        let debounce = debounce.map(LineAttribute::Debounce);

        let attrs = [debounce].into_iter().flatten().collect();

        Ok(Self {
            name,
            consumer,
            offset,
            flags,
            attrs,
        })
    }

    pub(crate) fn from_v2(info: uapi::v2::gpio_line_info) -> Result<Self> {
        let name = FixedStr::from_byte_array(info.name)?;
        let consumer = FixedStr::from_byte_array(info.name)?;
        let attrs = info
            .attrs
            .into_iter()
            .take(info.num_attrs as usize)
            .map(|a| unsafe { a.assume_init() })
            .map(LineAttribute::new_v2)
            .collect::<Result<_>>()?;

        Ok(Self {
            name,
            consumer,
            offset: info.offset,
            flags: info.flags,
            attrs,
        })
    }

    pub(crate) fn into_v2(self) -> uapi::v2::gpio_line_info {
        let num_attrs = self.attrs.len() as u32;
        let mut attrs = [MaybeUninit::zeroed(); uapi::v2::GPIO_LINE_NUM_ATTRS_MAX];

        for (r, w) in self.attrs.into_iter().zip(attrs.iter_mut()) {
            w.write(r.into_v2());
        }

        uapi::v2::gpio_line_info {
            name: self.name.into_byte_array(),
            consumer: self.consumer.into_byte_array(),
            offset: self.offset,
            num_attrs,
            flags: self.flags,
            attrs,
            _padding: [0; 4],
        }
    }

    pub fn flags(&self) -> LineFlags {
        self.attrs
            .iter()
            .filter_map(|f| match f {
                LineAttribute::Flags(f) => Some(f),
                _ => None,
            })
            .copied()
            .fold(None::<LineFlags>, |acc, f| match acc {
                Some(acc) => Some(acc.union(f)),
                None => Some(f),
            })
            .unwrap_or(self.flags)
    }
}

impl Default for LineInfo {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum LineAttribute {
    Flags(LineFlags),
    Values(LineValues),
    /// The debounce duration in micro-seconds
    Debounce(Debounce),
}

impl LineAttribute {
    pub(crate) fn new_v2(attr: uapi::v2::gpio_line_attribute) -> Result<Self> {
        let res = unsafe {
            match attr.id {
                uapi::v2::LineAttrId::FLAGS => Self::Flags(attr.attribute.flags),
                uapi::v2::LineAttrId::OUTPUT_VALUES => Self::Values(LineValues {
                    bits: attr.attribute.values,
                    mask: u64::MAX,
                }),
                uapi::v2::LineAttrId::DEBOUNCE => Self::Debounce(Debounce {
                    d: attr.attribute.debounce_period,
                }),
                invalid => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Unsupported,
                        format!("Invalid gpio line attribute ID: 0x{invalid:X}"),
                    ))
                }
            }
        };
        Ok(res)
    }

    pub(crate) const fn into_v2(self) -> uapi::v2::gpio_line_attribute {
        let (id, attribute) = match self {
            LineAttribute::Flags(flags) => (
                uapi::v2::LineAttrId::FLAGS,
                uapi::v2::gpio_line_attribute_union { flags },
            ),
            LineAttribute::Values(v) => (
                uapi::v2::LineAttrId::OUTPUT_VALUES,
                uapi::v2::gpio_line_attribute_union { values: v.bits },
            ),
            LineAttribute::Debounce(d) => (
                uapi::v2::LineAttrId::DEBOUNCE,
                uapi::v2::gpio_line_attribute_union {
                    debounce_period: d.d,
                },
            ),
        };

        uapi::v2::gpio_line_attribute {
            id,
            _padding: 0,
            attribute,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineValues {
    bits: u64,
    mask: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Debounce {
    d: u32,
}

impl Debounce {
    pub fn new(d: Duration) -> Result<Self> {
        let d = d.as_micros().try_into().map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Debounce period must be at most 4294 seconds",
            )
        })?;
        Ok(Self { d })
    }

    pub const unsafe fn new_unchecked(d: Duration) -> Self {
        Self {
            d: d.as_micros() as u32,
        }
    }

    pub const fn as_duration(&self) -> Duration {
        Duration::from_micros(self.d as u64)
    }
}
