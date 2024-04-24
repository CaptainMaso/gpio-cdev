use super::*;

use options::{Debounce, Direction};

use values::MaskedBits;

#[derive(Debug, Clone)]
pub struct LineInfo {
    name: FixedStr<{ uapi::v2::GPIO_MAX_NAME_SIZE }>,
    consumer: FixedStr<{ uapi::v2::GPIO_MAX_NAME_SIZE }>,
    offset: u32,
    flags: LineFlags,
    attrs: LineAttributes,
}

impl LineInfo {
    pub(crate) const fn empty() -> Self {
        Self {
            name: FixedStr::empty(),
            consumer: FixedStr::empty(),
            offset: 0,
            flags: LineFlags::empty(),
            attrs: LineAttributes {
                flags: None,
                values: None,
                debounce: None,
            },
        }
    }

    pub(crate) const fn new_get(offset: u32) -> Self {
        Self {
            offset,
            ..Self::empty()
        }
    }

    pub(crate) fn from_v2(info: uapi::v2::gpio_line_info) -> Result<Self> {
        let name = FixedStr::from_byte_array(info.name)?;
        let consumer = FixedStr::from_byte_array(info.name)?;
        let attrs = LineAttributes::from_attr_list(info.num_attrs as usize, info.attrs)?;
        let flags = info.flags;

        Ok(Self {
            name,
            consumer,
            offset: info.offset,
            flags,
            attrs,
        })
    }

    pub(crate) fn into_v2(self) -> uapi::v2::gpio_line_info {
        let mut attrs = [MaybeUninit::zeroed(); uapi::v2::GPIO_LINE_NUM_ATTRS_MAX];

        let a = [
            self.attrs.debounce.map(LineAttribute::Debounce),
            self.attrs.values.map(LineAttribute::Values),
        ];

        let num_attrs = a.iter().flatten().count() as u32;

        for (r, w) in a.into_iter().flatten().zip(attrs.iter_mut()) {
            w.write(r.into_v2());
        }

        let flags = self.attrs.flags.unwrap_or(self.flags);

        uapi::v2::gpio_line_info {
            name: self.name.into_byte_array(),
            consumer: self.consumer.into_byte_array(),
            offset: self.offset,
            num_attrs,
            flags,
            attrs,
            _padding: [0; 4],
        }
    }

    pub fn name(&self) -> Option<&str> {
        if self.name.is_empty() {
            None
        } else {
            Some(&self.name)
        }
    }

    pub fn consumer(&self) -> Option<&str> {
        if self.name.is_empty() {
            None
        } else {
            Some(&self.consumer)
        }
    }

    pub fn line_offset(&self) -> u32 {
        self.offset
    }

    pub fn flags(&self) -> LineFlags {
        self.attrs.flags.unwrap_or(self.flags)
    }

    pub fn debounce(&self) -> Option<Debounce> {
        self.attrs.debounce
    }

    pub fn value(&self) -> Option<bool> {
        let v = self.attrs.values.as_ref()?;
        v.get(0)
    }

    /// Get the direction of this GPIO if configured
    ///
    /// Lines are considered to be inputs if not explicitly
    /// marked as outputs in the line info flags by the kernel.
    pub fn direction(&self) -> Direction {
        if self.flags.contains(LineFlags::OUTPUT) {
            Direction::Output
        } else {
            Direction::Input
        }
    }

    /// True if the any flags for the device are set (input or output)
    pub fn is_used(&self) -> bool {
        self.flags.contains(LineFlags::USED)
    }

    /// True if this line is marked as active low in the kernel
    pub fn is_active_low(&self) -> bool {
        self.flags.contains(LineFlags::ACTIVE_LOW)
    }

    /// True if this line is marked as open drain in the kernel
    pub fn is_open_drain(&self) -> bool {
        self.flags.contains(LineFlags::OPEN_DRAIN)
    }

    /// True if this line is marked as open source in the kernel
    pub fn is_open_source(&self) -> bool {
        self.flags.contains(LineFlags::OPEN_SOURCE)
    }
}

impl Default for LineInfo {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct LineAttributes {
    flags: Option<LineFlags>,
    values: Option<MaskedBits>,
    debounce: Option<options::Debounce>,
}

impl LineAttributes {
    pub fn from_attr_list<const N: usize>(
        n_attrs: usize,
        attrs: [core::mem::MaybeUninit<uapi::v2::gpio_line_attribute>; N],
    ) -> Result<Self> {
        attrs
            .into_iter()
            .take(n_attrs)
            .map(|a| unsafe { a.assume_init() })
            .map(LineAttribute::new_v2)
            .try_fold(Self::default(), |attrs, f| match f? {
                LineAttribute::Flags(f) => {
                    let flags = match attrs.flags {
                        Some(acc) => Some(acc.union(f)),
                        None => Some(f),
                    };
                    std::io::Result::Ok(Self { flags, ..attrs })
                }
                LineAttribute::Values(values) => Ok(Self {
                    values: Some(values),
                    ..attrs
                }),
                LineAttribute::Debounce(debounce) => Ok(Self {
                    debounce: Some(debounce),
                    ..attrs
                }),
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum LineAttribute {
    Flags(LineFlags),
    Values(MaskedBits),
    /// The debounce duration in micro-seconds
    Debounce(Debounce),
}

impl LineAttribute {
    pub(crate) fn new_v2(attr: uapi::v2::gpio_line_attribute) -> Result<Self> {
        let res = unsafe {
            match attr.id {
                uapi::v2::LineAttrId::FLAGS => Self::Flags(attr.attribute.flags),
                uapi::v2::LineAttrId::OUTPUT_VALUES => Self::Values(MaskedBits {
                    bits: attr.attribute.values,
                    mask: u64::MAX,
                }),
                uapi::v2::LineAttrId::DEBOUNCE => {
                    Self::Debounce(Debounce::new_micros(attr.attribute.debounce_period))
                }
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
                    debounce_period: d.as_micros(),
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
