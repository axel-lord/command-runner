#![doc = include_str!("../README.md")]

use ::clap::Parser;
use ::command_runner::Cli;
use ::log::LevelFilter;

fn main() -> ::color_eyre::Result<()> {
    ::color_eyre::install()?;
    ::env_logger::builder()
        .filter_module("command_runner", LevelFilter::Debug)
        .init();

    Cli::parse().run()
}
