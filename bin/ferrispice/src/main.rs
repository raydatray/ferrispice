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
    let input_name = args.path.display().to_string();
    let text = std::fs::read_to_string(&args.path)
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
    let requests_dc = parsed.requests_dc();
    if args.dump_mna || args.dump_lu || !requests_dc {
        let system = match ferrispice_mna::assemble(&circuit) {
            Ok(system) => system,
            Err(error) => return Err(diagnostics::report(error, input_name, text)),
        };

        // matrices come before solving so they remain visible if the solve fails
        if args.dump_mna {
            output::print_mna(&circuit, &system)?;
        }

        if args.dump_lu {
            output::print_lu(&circuit, &system)?;
        }

        if !requests_dc {
            output::print_validation(&circuit, &system);
            return Ok(());
        }
    }

    let solution = match ferrispice_solver::solve_dc(&circuit) {
        Ok(solution) => solution,
        Err(error) => return Err(diagnostics::analysis(error, input_name, text)),
    };

    output::print_solution(&circuit, &solution)?;

    Ok(())
}
