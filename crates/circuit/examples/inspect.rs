use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("usage: inspect <netlist>")?;

    let text = std::fs::read_to_string(path)?;

    let parsed = ferrispice_netlist::parse(&text)?;
    let circuit = ferrispice_circuit::compile(&parsed)?;

    println!("{circuit:#?}");
    println!("DC requested: {}", parsed.requests_dc());


    Ok(())
}
