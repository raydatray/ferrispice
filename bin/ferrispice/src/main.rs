mod args;
mod diagnostics;
mod logger;
mod output;

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

    // repeated `.dc` records request the same single analysis
    if !parsed.requests_dc() {
        let system = match ferrispice_mna::assemble(&circuit) {
            Ok(system) => system,
            Err(error) => return Err(diagnostics::report(error, input_name, text)),
        };
        output::print_validation(&circuit, &system);
        return Ok(());
    }

    let solution = match ferrispice_solver::solve_dc(&circuit) {
        Ok(solution) => solution,
        Err(error) => return Err(diagnostics::analysis(error, input_name, text)),
    };

    output::print_solution(&circuit, &solution);

    Ok(())
}
