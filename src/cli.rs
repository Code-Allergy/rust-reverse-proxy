use std::path::PathBuf;
use clap::{Arg, Command, Parser};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Name of the person to greet
    #[arg(short = 'f', long = "file", value_name = "FILE")]
    pub config: Option<PathBuf>,
}

pub fn build_cli() -> Command {
    Command::new("proxy")
        .version("0.1.0")
        .about("A basic reverse proxy server written in Rust")
        .arg(
            Arg::new("config")
                .short('f')
                .long("file")
                .default_value("default.toml")
                .help("Sets a custom config file")
        )
}