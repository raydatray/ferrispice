use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(about = "parse and validate a circuit netlist")]
pub(crate) struct Args {
    /// path to the circuit netlist
    #[arg(value_name = "netlist")]
    pub path: PathBuf,
}
