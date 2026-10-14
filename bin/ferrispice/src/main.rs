mod args;
mod diagnostics;
mod logger;

use clap::Parser;
use miette::{IntoDiagnostic, WrapErr};

fn main() {
    let args = args::Args::parse();

    logger::init();

    if let Err(error) = run(args) {
        tracing::error!("{error}");
        eprint!("{error:?}");

        std::process::exit(1);
    }
}

fn run(args: args::Args) -> miette::Result<()> {
    let path = args.path;

    let input_name = path.display().to_string();
    let text = std::fs::read_to_string(&path)
        .into_diagnostic()
        .wrap_err_with(|| format!("could not read netlist {input_name:?}"))?;

    let parsed = match ferrispice_netlist::parse(&text) {
        Ok(parsed) => parsed,
        Err(error) => return Err(diagnostics::report(error, input_name, text)),
    };

    let circuit = match ferrispice_circuit::compile(&parsed) {
        Ok(circuit) => circuit,
        Err(error) => return Err(diagnostics::report(error, input_name, text)),
    };

    println!("{}", circuit.title());
    println!(
        "validated circuit: {} non-ground nodes, {} elements",
        circuit.non_ground_nodes().count(),
        circuit.elements().len(),
    );
    println!("DC requested: {}", parsed.requests_dc());

    Ok(())
}
