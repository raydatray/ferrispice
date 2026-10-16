use ferrispice_circuit::Circuit;
use ferrispice_mna::MnaSystem;
use ferrispice_solver::DcSolution;

pub(crate) fn print_validation(circuit: &Circuit, system: &MnaSystem) {
    println!("{}", circuit.title());
    println!(
        "validated circuit: {} non-ground nodes, {} elements, {} unknowns",
        circuit.non_ground_nodes().count(),
        circuit.elements().len(),
        system.layout.len(),
    );
    println!("no analysis requested; add .dc to run linear DC analysis");
}

pub(crate) fn print_solution(circuit: &Circuit, solution: &DcSolution) {
    println!("{}", circuit.title());
    println!("linear DC analysis");
    println!();
    for (label, value) in solution.layout().labels().zip(solution.unknowns().iter()) {
        println!("{label} = {value}");
    }
}
