#![doc = include_str!("../README.md")]

use ::clap::Parser;
use ::log::LevelFilter;
use ::win_command_runner::Cli;

fn main() -> ::color_eyre::Result<()> {
    ::color_eyre::install()?;
    ::env_logger::builder()
        .filter_module("win_command_runner", LevelFilter::Debug)
        .init();

    Cli::parse().run()
}
