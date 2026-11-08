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
    let system = if args.dump_mna || args.dump_lu {
        let system = match ferrispice_mna::assemble(&circuit) {
            Ok(system) => system,
            Err(error) => return Err(diagnostics::report(error, input_name, text)),
        };

        // matrices come before solving so they remain visible if the solve fails
        if args.dump_mna {
            output::print_mna(&circuit, &system)?;
        }

        // a nonlinear circuit's Jacobian is factored once the solve converges
        if args.dump_lu {
            if system.is_linear() {
                output::print_lu(&circuit, &system)?;
            } else if !requests_dc {
                output::print_no_jacobian("no analysis was requested")?;
            }
        }

        Some(system)
    } else {
        None
    };

    if !requests_dc {
        // the summary needs only the unknown layout, not the stamped matrices
        output::print_validation(&circuit, &ferrispice_mna::UnknownLayout::new(&circuit));
        return Ok(());
    }

    let solution = match ferrispice_solver::solve_dc(&circuit) {
        Ok(solution) => solution,
        Err(error) => {
            // the iterations leading up to a failure stay inspectable
            if let (true, ferrispice_solver::DcError::SourceStepping(failure)) =
                (args.trace_newton, &error)
            {
                output::print_failed_trace(&circuit, failure)?;
            }
            if args.dump_lu && system.as_ref().is_some_and(|system| !system.is_linear()) {
                output::print_no_jacobian("the nonlinear solve did not converge")?;
            }
            return Err(diagnostics::analysis(error, &circuit, input_name, text));
        }
    };

    if args.trace_newton {
        output::print_trace(&circuit, &solution)?;
    }

    if let Some(system) = &system {
        if args.dump_mna {
            output::print_f_at_solution(&circuit, system, &solution)?;
        }
        if args.dump_lu && !system.is_linear() {
            output::print_jacobian_lu(&circuit, system, &solution)?;
        }
    }

    output::print_solution(&circuit, &solution)?;

    Ok(())
}
