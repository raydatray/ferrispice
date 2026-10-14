mod support;

use std::path::PathBuf;

use miette::{IntoDiagnostic, WrapErr};

fn main() -> miette::Result<()> {
    let path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or_else(|| miette::miette!("usage: inspect <netlist>"))?;

    let input_name = path.display().to_string();

    let text = std::fs::read_to_string(&path)
        .into_diagnostic()
        .wrap_err_with(|| format!("could not read netlist {input_name:?}"))?;

    let parsed = match ferrispice_netlist::parse(&text) {
        Ok(parsed) => parsed,
        Err(error) => return Err(support::report(error, input_name, text)),
    };

    let circuit = match ferrispice_circuit::compile(&parsed) {
        Ok(circuit) => circuit,
        Err(error) => return Err(support::report(error, input_name, text)),
    };

    println!("{circuit:#?}");
    println!("DC requested: {}", parsed.requests_dc());

    Ok(())
}
