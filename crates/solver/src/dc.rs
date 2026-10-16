use std::collections::HashMap;

use ferrispice_circuit::{Circuit, ElementId, NodeId};
use ferrispice_mna::{AssemblyError, MnaSystem, UnknownLayout, assemble};
use nalgebra::DVector;
use thiserror::Error;

use crate::{SolveError, solve_linear};

#[derive(Clone, Debug, Error, PartialEq)]
pub enum DcError {
    #[error(transparent)]
    Assembly(#[from] AssemblyError),

    #[error(transparent)]
    Solve(#[from] SolveError),
}

/// linear DC operating point, queried by circuit IDs instead of matrix indices
#[derive(Clone, Debug, PartialEq)]
pub struct DcSolution {
    layout: UnknownLayout,
    unknowns: DVector<f64>,
    node_voltages: Vec<(NodeId, f64)>,
    element_currents: Vec<(ElementId, f64)>,
    voltage_index: HashMap<NodeId, usize>,
    current_index: HashMap<ElementId, usize>,
}

impl DcSolution {
    /// ground is exactly zero, IDs not from this circuit return `None`
    pub fn node_voltage(&self, node: NodeId) -> Option<f64> {
        self.voltage_index
            .get(&node)
            .map(|&index| self.node_voltages[index].1)
    }

    /// positive current flows from the element's positive terminal to its negative terminal
    pub fn element_current(&self, element: ElementId) -> Option<f64> {
        self.current_index
            .get(&element)
            .map(|&index| self.element_currents[index].1)
    }

    /// every node, including ground, in circuit order
    pub fn node_voltages(&self) -> &[(NodeId, f64)] {
        &self.node_voltages
    }

    /// element currents in circuit order
    pub fn element_currents(&self) -> &[(ElementId, f64)] {
        &self.element_currents
    }

    /// raw solved unknowns, ordered by `layout()`
    pub fn unknowns(&self) -> &DVector<f64> {
        &self.unknowns
    }

    pub fn layout(&self) -> &UnknownLayout {
        &self.layout
    }
}

/// assemble and solve the DC equations of a circuit.
pub fn solve_dc(circuit: &Circuit) -> Result<DcSolution, DcError> {
    let system = assemble(circuit)?;
    solve_steady_state(circuit, &system)
}

/// solve the DC steady-state system `G x = b` already assembled from `circuit`
///
/// time derivatives are zero at DC, so `C` does not participate
fn solve_steady_state(circuit: &Circuit, system: &MnaSystem) -> Result<DcSolution, DcError> {
    let _span = tracing::debug_span!("solve_dc", unknowns = system.layout.len()).entered();

    let unknowns = solve_linear(&system.g, &system.b)?;
    let layout = &system.layout;

    let node_voltages: Vec<_> = circuit
        .nodes()
        .iter()
        .map(|node| (node.id, node_voltage(node.id, layout, &unknowns)))
        .collect();

    let element_currents: Vec<_> = circuit
        .elements()
        .iter()
        .filter_map(|element| {
            let current = layout
                .branch_index(element.id)
                .map(|index| unknowns[index])?;

            Some((element.id, current))
        })
        .collect();

    let voltage_index: HashMap<NodeId, usize> = node_voltages
        .iter()
        .enumerate()
        .map(|(index, (node, _))| (*node, index))
        .collect();

    let current_index: HashMap<ElementId, usize> = element_currents
        .iter()
        .enumerate()
        .map(|(index, (element, _))| (*element, index))
        .collect();

    tracing::debug!(nodes = node_voltages.len(), "solved DC operating point");

    Ok(DcSolution {
        voltage_index,
        current_index,
        layout: layout.clone(),
        unknowns,
        node_voltages,
        element_currents,
    })
}

fn node_voltage(node: NodeId, layout: &UnknownLayout, unknowns: &DVector<f64>) -> f64 {
    if node.is_ground() {
        return 0.0;
    }

    let index = layout
        .node_index(node)
        .expect("non-ground node must have a layout index");

    unknowns[index]
}
