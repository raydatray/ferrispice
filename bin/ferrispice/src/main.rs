mod args;
mod csv;
mod diagnostics;
mod logger;
mod output;

use clap::Parser;
use ferrispice_circuit::Circuit;
use ferrispice_mna::MnaSystem;
use ferrispice_solver::{DcError, DcSolution, TransientError};
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
    let transient = parsed.transient_request();
    let requests_analysis = requests_dc || transient.is_some();

    let transient_outputs = args.transient_outputs();
    if transient.is_none() && !transient_outputs.is_empty() {
        return Err(miette::miette!(
            help = "add `.tran tstop tstep` to the netlist to run a transient analysis",
            "{input_name}: {} writes transient waveforms, but the netlist requests no \
             transient analysis",
            transient_outputs.join(" and "),
        ));
    }

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
            } else if !requests_analysis {
                output::print_no_jacobian("no analysis was requested")?;
            }
        }

        Some(system)
    } else {
        None
    };

    if !requests_analysis {
        // the summary needs only the unknown layout, not the stamped matrices
        output::print_validation(&circuit, &ferrispice_mna::UnknownLayout::new(&circuit));
        return Ok(());
    }

    if requests_dc {
        let solution = match ferrispice_solver::solve_dc(&circuit) {
            Ok(solution) => solution,
            Err(error) => {
                report_failed_operating_point(&args, &circuit, system.as_ref(), &error)?;
                return Err(diagnostics::analysis(error, &circuit, input_name, text));
            }
        };

        print_operating_point(&args, &circuit, system.as_ref(), &solution)?;
        output::print_solution(&circuit, &solution)?;
    }

    if let Some((stop, step)) = transient {
        let solution = match ferrispice_solver::solve_transient(&circuit, stop, step) {
            Ok(solution) => solution,
            Err(error) => {
                // with `.dc` the operating point was already solved and reported
                if let (false, TransientError::OperatingPoint(error)) = (requests_dc, &error) {
                    report_failed_operating_point(&args, &circuit, system.as_ref(), error)?;
                }
                return Err(diagnostics::transient(error, &circuit, input_name, text));
            }
        };

        // the operating-point flags describe the initial state when no `.dc`
        // printed it already
        if !requests_dc {
            print_operating_point(&args, &circuit, system.as_ref(), solution.operating_point())?;
        }

        let mut written = Vec::new();
        if let Some(path) = &args.csv {
            csv::write_csv(path, &solution)
                .into_diagnostic()
                .wrap_err_with(|| format!("could not write CSV {:?}", path.display()))?;
            written.push(path.as_path());
        }

        output::print_transient(&circuit, &solution, stop, step, &written)?;
    }

    Ok(())
}

/// the Newton trace, `f(x*)`, and the Jacobian's factors at an operating point
fn print_operating_point(
    args: &args::Args,
    circuit: &Circuit,
    system: Option<&MnaSystem>,
    solution: &DcSolution,
) -> miette::Result<()> {
    if args.trace_newton {
        output::print_trace(circuit, solution)?;
    }

    if let Some(system) = system {
        if args.dump_mna {
            output::print_f_at_solution(circuit, system, solution)?;
        }
        if args.dump_lu && !system.is_linear() {
            output::print_jacobian_lu(circuit, system, solution)?;
        }
    }

    Ok(())
}

/// what remains inspectable after the operating point could not be found
fn report_failed_operating_point(
    args: &args::Args,
    circuit: &Circuit,
    system: Option<&MnaSystem>,
    error: &DcError,
) -> miette::Result<()> {
    // the iterations leading up to a failure stay inspectable
    if let (true, DcError::SourceStepping(failure)) = (args.trace_newton, error) {
        output::print_failed_trace(circuit, failure)?;
    }
    if args.dump_lu && system.is_some_and(|system| !system.is_linear()) {
        output::print_no_jacobian("the nonlinear solve did not converge")?;
    }

    Ok(())
}
