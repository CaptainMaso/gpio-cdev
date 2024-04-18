// Copyright (c) 2018 The rust-gpio-cdev Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use bitflags::bitflags;
use nix::ioctl_readwrite;

bitflags! {
    /// Informational Flags
    ///
    /// Maps to kernel [`GPIOLINE_FLAG_*`] flags.
    ///
    /// [`GPIOLINE_FLAG_*`]: https://github.com/torvalds/linux/blob/v5.19/include/uapi/linux/gpio.h
    #[derive(Debug)]
    pub struct GPIOLINE_FLAG: u32 {
        const KERNEL = (1 << 0);
        const IS_OUT = (1 << 1);
        const ACTIVE_LOW = (1 << 2);
        const OPEN_DRAIN = (1 << 3);
        const OPEN_SOURCE = (1 << 4);
        const BIAS_PULL_UP = (1 << 5);
        const BIAS_PULL_DOWN = (1 << 6);
        const BIAS_DISABLE = (1 << 7);
    }
}

/// Information about a certain GPIO line
#[repr(C)]
pub struct gpio_line_info {
    /// The local offset on this GPIO device, fill this in when
    /// requesting the line information from the kernel.
    pub line_offset: u32,
    /// various flags for this line
    pub flags: GPIOLINE_FLAG,
    // the name of this GPIO line, such as the output pin of the line on the
    // chip, a rail or a pin header name on a board, as specified by the gpio
    // chip, may be empty (i.e. name[0] == '\0')
    pub name: [libc::c_char; 32],
    /// a functional name for the consumer of this GPIO line as set by
    /// whatever is using it, will be empty if there is no current user but may
    /// also be empty if the consumer doesn't set this up
    pub consumer: [libc::c_char; 32],
}

pub const GPIOHANDLES_MAX: usize = 64;

bitflags! {
    /// Possible line status change events
    ///
    /// Maps to kernel [`GPIOLINE_CHANGED_*`] IDs.
    ///
    /// [`GPIOLINE_CHANGED_*`]: https://github.com/torvalds/linux/blob/v5.19/include/uapi/linux/gpio.h
    pub struct GPIOLINE_CHANGED_ID: u32 {
        const REQUESTED = 1;
        const RELEASED = 2;
        const CONFIG = 3;
    }
}

/// Information about a certain GPIO line
#[repr(C)]
pub struct gpio_line_info_changed {
    /// updated line information
    info: gpio_line_info,
    /// estimate of time of status change occurrence, in nanoseconds,
    timestamp: u64,
    event_type: GPIOLINE_CHANGED_ID,
    _padding: [u32; 5],
}

bitflags! {
    /// Line Request Flags
    ///
    /// Maps to kernel [`GPIOHANDLE_REQUEST_*`] flags.
    ///
    /// [`GPIOHANDLE_REQUEST_*`]: https://github.com/torvalds/linux/blob/v5.19/include/uapi/linux/gpio.h#L58
    #[derive(Debug, Clone)]
    pub struct GPIOHANDLE_REQUEST_FLAGS: u32 {
        const INPUT = (1 << 0);
        const OUTPUT = (1 << 1);
        const ACTIVE_LOW = (1 << 2);
        const OPEN_DRAIN = (1 << 3);
        const OPEN_SOURCE = (1 << 4);
    }
}

/// Information about a GPIO handle request
#[repr(C)]
pub struct gpiohandle_request {
    ///an array of desired lines, specified by offset index for the associated GPIO device
    pub lineoffsets: [u32; GPIOHANDLES_MAX],
    /// desired flags for the desired GPIO lines.
    ///
    /// Note that even if multiple lines are requested, the same flags
    /// must be applicable to all of them, if you want lines with individual
    /// flags set, request them one by one. It is possible to select
    /// a batch of input or output lines, but they must all have the same
    /// characteristics, i.e. all inputs or all outputs, all active low etc
    pub flags: GPIOHANDLE_REQUEST_FLAGS,
    /// if [GPIOHANDLE_REQUEST::OUTPUT] is set for a requested
    /// line, this specifies the default output value, should be 0 (low) or
    /// 1 (high), anything else than 0 or 1 will be interpreted as 1 (high)
    pub default_values: [u8; GPIOHANDLES_MAX],
    /// a desired consumer label for the selected GPIO line(s)
    /// such as "my-bitbanged-relay"
    pub consumer_label: [libc::c_char; 32],
    /// number of lines requested in this request, i.e. the number of
    /// valid fields in the above arrays, set to 1 to request a single line
    pub lines: u32,
    ///  if successful this field will contain a valid anonymous file handle
    ///  after a [gpio_get_linehandle] operation, zero or negative value
    ///  means error.
    pub fd: libc::c_int,
}

/// Configuration for a GPIO handle request
#[repr(C)]
struct gpiohandle_config {
    /// updated flags for the requested GPIO lines
    flags: GPIOHANDLE_REQUEST_FLAGS,
    /// if the [GPIOHANDLE_REQUEST::OUTPUT] is set in flags,
    ///  this specifies the default output value, should be 0 (low) or
    ///  1 (high), anything else than 0 or 1 will be interpreted as 1 (high)
    default_values: [u8; GPIOHANDLES_MAX],
    _padding: [u32; 4],
}

/// Information of values on a GPIO handle
#[repr(C)]
pub struct gpiohandle_data {
    /// When getting the state of lines this contains the current
    /// state of a line
    ///
    /// When setting the state of lines these should contain
    /// the desired target state
    pub values: [u8; GPIOHANDLES_MAX],
}

bitflags! {
    /// Event request flags
    ///
    /// Maps to kernel [`GPIOEVENT_REQUEST_*`] flags.
    ///
    /// [`GPIOEVENT_REQUEST_*`]: https://github.com/torvalds/linux/blob/v5.19/include/uapi/linux/gpio.
    pub struct GPIOEVENT_REQUEST_FLAGS: u32 {
        const RISING_EDGE = (1 << 0);
        const FALLING_EDGE = (1 << 1);
        const BOTH_EDGES = Self::RISING_EDGE.bits() | Self::FALLING_EDGE.bits();
    }
}

/// Information about a GPIO event request
#[repr(C)]
pub struct gpioevent_request {
    /// the desired line to subscribe to events from, specified by
    /// offset index for the associated GPIO device
    pub lineoffset: u32,
    /// desired handle flags for the desired GPIO line
    pub handleflags: GPIOHANDLE_REQUEST_FLAGS,
    /// desired flags for the desired GPIO event line
    pub eventflags: GPIOEVENT_REQUEST_FLAGS,
    /// a desired consumer label for the selected GPIO line(s) such as "my-listener"
    pub consumer_label: [libc::c_char; 32],
    /// if successful this field will contain a valid anonymous file handle
    /// after a [gpio_get_lineevent] operation, zero or negative value
    /// means error
    pub fd: libc::c_int,
}

bitflags! {
    /// Event flags
    ///
    /// Maps to kernel [`GPIOEVENT_*`] IDs.
    ///
    /// [`GPIOEVENT_*`]: https://github.com/torvalds/linux/blob/v5.19/include/uapi/linux/gpio.h#L109
    pub struct GPIOEVENT_EVENT_ID: u32 {
        const RISING_EDGE = 1;
        const FALLING_EDGE = 2;
    }
}

/// The actual event being pushed to userspace
#[repr(C)]
pub struct gpioevent_data {
    /// best estimate of time of event occurrence, in nanoseconds
    pub timestamp: u64,
    /// event identifier
    pub id: GPIOEVENT_EVENT_ID,
}

ioctl_readwrite!(gpio_get_lineinfo, 0xB4, 0x02, gpio_line_info);
ioctl_readwrite!(gpio_get_linehandle, 0xB4, 0x03, gpiohandle_request);
ioctl_readwrite!(gpio_get_lineevent, 0xB4, 0x04, gpioevent_request);

ioctl_readwrite!(gpiohandle_get_line_values, 0xB4, 0x08, gpiohandle_data);
ioctl_readwrite!(gpiohandle_set_line_values, 0xB4, 0x09, gpiohandle_data);
ioctl_readwrite!(gpiohandle_set_config, 0xB4, 0x0A, gpiohandle_config);

ioctl_readwrite!(gpio_get_lineinfo_watch, 0xB4, 0x0B, gpio_line_info);
