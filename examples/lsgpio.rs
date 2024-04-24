// Copyright (c) 2018 The rust-gpio-cdev Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Clone of functionality of linux/tools/gpio/lsgpio.c

use gpio_cdev::{line::options::Direction, *};

fn main() {
    let chip_iterator = match chips() {
        Ok(chips) => chips,
        Err(e) => {
            println!("Failed to get chip iterator: {:?}", e);
            return;
        }
    };

    for chip in chip_iterator {
        if let Ok(chip) = chip {
            let chip_info = chip.chip_info().unwrap();
            println!(
                "GPIO chip: \"{}\", \"{}\", {} GPIO Lines",
                chip_info.name(),
                chip_info.label(),
                chip_info.num_lines()
            );
            for (lineno, line) in chip.lines().enumerate() {
                let (offset, info) = match line {
                    Ok(l) => l,
                    Err(e) => {
                        eprintln!("\tline {lineno:>3}: error {e}");
                        continue;
                    }
                };

                let mut flags = vec![];

                if info.is_used() {
                    flags.push("used");
                }

                if info.direction() == Direction::Output {
                    flags.push("output");
                }

                if info.is_active_low() {
                    flags.push("active-low");
                }
                if info.is_open_drain() {
                    flags.push("open-drain");
                }
                if info.is_open_source() {
                    flags.push("open-source");
                }

                let usage = if !flags.is_empty() {
                    format!("[{}]", flags.join(" "))
                } else {
                    "".to_owned()
                };

                println!(
                    "\tline {lineno:>3}: {name} {consumer} {usage}",
                    lineno = offset,
                    name = info.name().unwrap_or("unused"),
                    consumer = info.consumer().unwrap_or("unused"),
                    usage = usage,
                );
            }
            println!();
        }
    }
}
