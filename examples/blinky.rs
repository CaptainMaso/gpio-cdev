// Copyright (c) 2018 The rust-gpio-cdev Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use gpio_cdev::line::options::LineOptions;
use gpio_cdev::line::values::LineValue;
use gpio_cdev::Chip;
use quicli::prelude::*;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Cli {
    /// The gpiochip device (e.g. /dev/gpiochip0)
    chip: PathBuf,
    /// The offset of the GPIO line for the provided chip
    line: u32,
    /// Period in milliseconds
    period_ms: u64,
    /// Duration over which to blink in milliseconds
    duration_ms: u64,
}

fn do_main(args: Cli) -> std::io::Result<()> {
    let chip = Chip::open(&args.chip)?;

    // NOTE: we set the default value to the desired state so
    // setting it separately is not required
    let mut handle = chip.open_line("blinky", LineOptions::OUTPUT, args.line)?;

    let duration = Duration::from_millis(args.duration_ms);
    let start_time = Instant::now();
    while start_time.elapsed() < duration {
        sleep(Duration::from_millis(args.period_ms));
        handle.write(LineValue::Inactive)?;
        sleep(Duration::from_millis(args.period_ms));
        handle.write(LineValue::Inactive)?;
    }

    Ok(())
}

fn main() -> CliResult {
    let args = Cli::from_args();
    do_main(args).or_else(|e| {
        error!("{:?}", e);
        Ok(())
    })
}
