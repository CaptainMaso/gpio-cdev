// Copyright (c) 2018 The rust-gpio-cdev Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use core::mem::MaybeUninit;

use bitflags::bitflags;
use nix::ioctl_readwrite;

pub const GPIO_LINES_MAX: usize = 64;
pub const GPIO_MAX_NAME_SIZE: usize = 32;
pub const GPIO_LINE_NUM_ATTRS_MAX: usize = 10;

bitflags! {
    /// Informational Flags
    ///
    /// Maps to kernel [`GPIO_V2_LINE_FLAG_*`] flags.
    ///
    /// [`GPIO_V2_LINE_FLAG_*`]: https://github.com/torvalds/linux/blob/v5.19/include/uapi/linux/gpio.h
    #[derive(Debug, Default,Clone,Copy,PartialEq, Eq)]
    pub struct LineFlags: u64 {
        const USED = (1 << 0);
        const ACTIVE_LOW = (1 << 1);
        const INPUT = (1 << 2);
        const OUTPUT = (1 << 3);
        const EDGE_RISING = (1 << 4);
        const EDGE_FALLING = (1 << 5);
        const OPEN_DRAIN = (1 << 6);
        const OPEN_SOURCE = (1 << 7);
        const BIAS_PULL_UP = (1 << 8);
        const BIAS_PULL_DOWN = (1 << 9);
        const BIAS_DISABLED = (1 << 10);
        const EVENT_CLOCK_REALTIME = (1 << 11);
        const EVENT_CLOCK_HTE = (1 << 12);
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub(crate) struct gpio_line_values {
    pub(crate) bits: u64,
    pub(crate) mask: u64,
}

bitflags! {
    /// Attribute IDs
    ///
    /// Maps to kernel [`GPIO_V2_LINE_ATTR_ID_*`] flags.
    ///
    /// [`GPIO_V2_LINE_ATTR_ID_*`]: https://github.com/torvalds/linux/blob/v5.19/include/uapi/linux/gpio.h
    #[derive(Debug,Clone,Copy,PartialEq, Eq)]
    pub struct LineAttrId: u32 {
        const FLAGS = 1;
        const OUTPUT_VALUES = 2;
        const DEBOUNCE = 3;
    }
}

/// a configurable attribute of a line
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct gpio_line_attribute {
    /// attribute identifier
    pub(crate) id: LineAttrId,
    /// reserved for future use and must be zero filled
    pub(crate) _padding: u32,
    /// A tagged union when combined with `id`
    pub(crate) attribute: gpio_line_attribute_union,
}

impl core::fmt::Debug for gpio_line_attribute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = unsafe {
            match self.id {
                LineAttrId::FLAGS => &self.attribute.flags as &dyn core::fmt::Debug,
                LineAttrId::OUTPUT_VALUES => &self.attribute.values as &dyn core::fmt::Debug,
                LineAttrId::DEBOUNCE => &self.attribute.debounce_period as &dyn core::fmt::Debug,
                _ => &"unknown line attribute" as &dyn core::fmt::Debug,
            }
        };
        f.debug_struct("gpio_line_attribute")
            .field("id", &self.id)
            .field("attribute", value)
            .finish()
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) union gpio_line_attribute_union {
    /// if `gpio_line_attribute.id` is [`LineAttrId::OUTPUT_VALUES`](LineAttrId::FLAGS),
    /// the flags for the GPIO line, with values from [`LineFlags`](LineFlags), added together.  This
    /// overrides the default flags contained in the [`gpio_line_config`](gpio_line_config) for the associated line.
    pub(crate) flags: LineFlags,
    /// if `gpio_line_attribute.id` is [`LineAttrId::OUTPUT_VALUES`](LineAttrId::OUTPUT_VALUES), a bitmap
    /// containing the values to which the lines will be set, with each bit
    /// number corresponding to the index into gpio_v2_line_request.offsets
    pub(crate) values: u64,
    /// if `gpio_line_attribute.id` is [`LineAttrId::OUTPUT_VALUES`](LineAttrId::DEBOUNCE), the
    /// desired debounce period, in microseconds
    pub(crate) debounce_period: u32,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub(crate) struct gpio_line_config_attribute {
    pub(crate) attr: gpio_line_attribute,
    pub(crate) mask: u64,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub(crate) struct gpio_line_config {
    pub(crate) flags: LineFlags,
    pub(crate) num_attrs: u32,
    _padding: [u32; 5],
    pub(crate) attrs: [MaybeUninit<gpio_line_config_attribute>; 10],
}

impl gpio_line_config {
    pub const fn zeroed() -> Self {
        Self {
            flags: LineFlags::empty(),
            num_attrs: 0,
            _padding: [0; 5],
            attrs: [MaybeUninit::zeroed(); 10],
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub(crate) struct gpio_line_request {
    pub(crate) offsets: [u32; GPIO_LINES_MAX],
    pub(crate) consumer: [u8; GPIO_MAX_NAME_SIZE],
    pub(crate) config: gpio_line_config,
    pub(crate) num_lines: u32,
    pub(crate) event_buffer_size: u32,
    _padding: [u32; 5],
    pub(crate) fd: std::os::fd::RawFd,
}

impl gpio_line_request {
    pub const fn zeroed() -> Self {
        Self {
            offsets: [0; GPIO_LINES_MAX],
            consumer: [0; GPIO_MAX_NAME_SIZE],
            config: gpio_line_config::zeroed(),
            num_lines: 0,
            event_buffer_size: 0,
            _padding: [0; 5],
            fd: 0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub(crate) struct gpio_line_info {
    pub(crate) name: [u8; GPIO_MAX_NAME_SIZE],
    pub(crate) consumer: [u8; GPIO_MAX_NAME_SIZE],
    pub(crate) offset: u32,
    pub(crate) num_attrs: u32,
    pub(crate) flags: LineFlags,
    pub(crate) attrs: [MaybeUninit<gpio_line_attribute>; GPIO_LINE_NUM_ATTRS_MAX],
    pub(crate) _padding: [u32; 4],
}

impl gpio_line_info {
    pub const fn zeroed() -> Self {
        Self {
            name: [0; GPIO_MAX_NAME_SIZE],
            consumer: [0; GPIO_MAX_NAME_SIZE],
            offset: 0,
            num_attrs: 0,
            flags: LineFlags::empty(),
            attrs: [MaybeUninit::zeroed(); GPIO_LINE_NUM_ATTRS_MAX],
            _padding: [0; 4],
        }
    }
}

impl Default for gpio_line_info {
    #[inline(always)]
    fn default() -> Self {
        Self::zeroed()
    }
}

bitflags! {
    /// Changed Type
    ///
    /// Maps to kernel [`GPIO_V2_LINE_CHANGED_*`] flags.
    ///
    /// [`GPIO_V2_LINE_CHANGED_*`]: https://github.com/torvalds/linux/blob/v5.19/include/uapi/linux/gpio.h
    #[derive(Debug,Clone,Copy,PartialEq, Eq)]
    pub struct LineChangedType: u32 {
        const REQUESTED = 1;
        const RELEASED = 2;
        const CONFIG = 3;
    }
}

/// gpioline_info_changed
///
/// Information about a change in status of a GPIO line
#[repr(C)]
pub(crate) struct gpio_line_info_changed {
    info: gpio_line_info,
    timestamp_ns: u64,
    event_type: LineChangedType,
    /* Pad struct to 64-bit boundary and reserve space for future use. */
    _padding: [MaybeUninit<u32>; 5],
}

bitflags! {
    /// Line Event ID
    ///
    /// Maps to kernel [`GPIO_V2_LINE_EVENT_*`] flags.
    ///
    /// [`GPIO_V2_LINE_EVENT_*`]: https://github.com/torvalds/linux/blob/v5.19/include/uapi/linux/gpio.h
    #[derive(Debug,Clone,Copy,PartialEq, Eq)]
    pub struct LineEventId: u32 {
        const RISING_EDGE = 1;
        const FALLING_EDGE = 2;
    }
}

#[repr(C)]
pub(crate) struct gpio_line_event {
    pub(crate) timestamp_ns: u64,
    pub(crate) id: LineEventId,
    pub(crate) offset: u32,
    pub(crate) seqno: u32,
    pub(crate) line_seqno: u32,
    /* Space reserved for future use. */
    _padding: [MaybeUninit<u32>; 6],
}

impl gpio_line_event {
    #[inline(always)]
    pub const fn zeroed() -> Self {
        Self {
            timestamp_ns: 0,
            id: LineEventId::empty(),
            offset: 0,
            seqno: 0,
            line_seqno: 0,
            _padding: [MaybeUninit::zeroed(); 6],
        }
    }

    /// # Safety:
    ///
    /// Caller must ensure that the bytes are valid to be converted to this type
    pub const unsafe fn from_bytes(bytes: [u8; std::mem::size_of::<Self>()]) -> Self {
        let buf_ptr = (&bytes as *const _) as *const Self;
        let data = unsafe { std::ptr::read_unaligned(buf_ptr) };
        data
    }
}

impl Default for gpio_line_event {
    #[inline(always)]
    fn default() -> Self {
        Self::zeroed()
    }
}

ioctl_readwrite!(gpio_get_line, 0xB4, 0x07, gpio_line_request);

ioctl_readwrite!(gpio_get_line_info, 0xB4, 0x05, gpio_line_info);
ioctl_readwrite!(gpio_get_line_info_watch, 0xB4, 0x06, gpio_line_info);

ioctl_readwrite!(gpio_line_set_config, 0xB4, 0x0D, gpio_line_config);

ioctl_readwrite!(gpio_line_get_values, 0xB4, 0x0E, gpio_line_values);
ioctl_readwrite!(gpio_line_set_values, 0xB4, 0x0F, gpio_line_values);
