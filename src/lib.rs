// Copyright (c) 2018 The rust-gpio-cdev Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The `gpio-cdev` crate provides access to the [GPIO character device
//! ABI](https://www.kernel.org/doc/Documentation/ABI/testing/gpio-cdev).  This API,
//! stabilized with Linux v4.4, deprecates the legacy sysfs interface to GPIOs that is
//! planned to be removed from the upstream kernel after
//! year 2020 (which is coming up quickly).
//!
//! This crate attempts to wrap this interface in a moderately direction fashion
//! while retaining safety and using Rust idioms (where doing so could be mapped
//! to the underlying abstraction without significant overhead or loss of
//! functionality).
//!
//! For additional context for why the kernel is moving from the sysfs API to the
//! character device API, please see the main [README on Github].
//!
//! # Examples
//!
//! The following example reads the state of a GPIO line/pin and writes the matching
//! state to another line/pin.
//!
//! ```no_run
//! use gpio_cdev::{Chip, LineRequestFlags, EventRequestFlags, EventType};
//!
//! // Lines are offset within gpiochip0; see docs for more info on chips/lines
//! fn mirror_gpio(inputline: u32, outputline: u32) -> Result<(), gpio_cdev::Error> {
//!     let mut chip = Chip::new("/dev/gpiochip0")?;
//!     let input = chip.get_line(inputline)?;
//!     let output = chip.get_line(outputline)?;
//!     let output_handle = output.request(LineRequestFlags::OUTPUT, 0, "mirror-gpio")?;
//!     for event in input.events(
//!         LineRequestFlags::INPUT,
//!         EventRequestFlags::BOTH_EDGES,
//!         "mirror-gpio",
//!     )? {
//!         let evt = event?;
//!         println!("{:?}", evt);
//!         match evt.event_type() {
//!             EventType::RisingEdge => {
//!                 output_handle.set_value(1)?;
//!             }
//!             EventType::FallingEdge => {
//!                 output_handle.set_value(0)?;
//!             }
//!         }
//!     }
//!
//!     Ok(())
//! }
//!
//! # fn main() -> Result<(), gpio_cdev::Error> {
//! #     mirror_gpio(0, 1)
//! # }
//! ```
//!
//! To get the state of a GPIO Line on a given chip:
//!
//! ```no_run
//! use gpio_cdev::{Chip, LineRequestFlags};
//!
//! # fn main() -> Result<(), gpio_cdev::Error> {
//! // Read the state of GPIO4 on a raspberry pi.  /dev/gpiochip0
//! // maps to the driver for the SoC (builtin) GPIO controller.
//! // The LineHandle returned by request must be assigned to a
//! // variable (in this case the variable handle) to ensure that
//! // the corresponding file descriptor is not closed.
//! let mut chip = Chip::new("/dev/gpiochip0")?;
//! let handle = chip
//!     .get_line(4)?
//!     .request(LineRequestFlags::INPUT, 0, "read-input")?;
//! for _ in 1..4 {
//!     println!("Value: {:?}", handle.get_value()?);
//! }
//! # Ok(()) }
//! ```
//!
//! [README on Github]: https://github.com/rust-embedded/rust-gpio-cdev

#![cfg_attr(docsrs, feature(doc_cfg))]

mod errors;

pub mod fixed_str;

#[allow(non_camel_case_types)]
pub mod uapi;

pub mod chip;

pub mod line;

pub use chip::{chips, Chip};
