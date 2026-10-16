use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    about = "parse, validate, and assemble a circuit netlist",
    long_about = "parse, validate, and assemble a circuit netlist.\n\n\
        A .dc record in the netlist requests linear DC analysis and prints the MNA \
        unknown vector: non-ground node voltages and explicit branch currents."
)]
pub(crate) struct Args {
    /// path to the circuit netlist
    #[arg(value_name = "netlist")]
    pub path: PathBuf,
}
