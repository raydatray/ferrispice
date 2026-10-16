use std::collections::HashMap;

use ferrispice_circuit::{Circuit, Element, ElementId, ElementKind, NodeId};
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

    // calculate element currents using known source values and the solved
    // voltages/currents
    let element_currents: Vec<_> = circuit
        .elements()
        .iter()
        .map(|element| (element.id, element_current(element, layout, &unknowns)))
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

fn element_current(element: &Element, layout: &UnknownLayout, unknowns: &DVector<f64>) -> f64 {
    match element.kind {
        ElementKind::Resistor { resistance } => {
            (node_voltage(element.positive, layout, unknowns)
                - node_voltage(element.negative, layout, unknowns))
                / resistance
        }
        ElementKind::Capacitor { .. } => 0.0,
        ElementKind::CurrentSource { dc } => dc,
        ElementKind::Vccs {
            control_positive,
            control_negative,
            transconductance,
        } => {
            transconductance
                * (node_voltage(control_positive, layout, unknowns)
                    - node_voltage(control_negative, layout, unknowns))
        }
        ElementKind::Cccs { control, gain } => gain * branch_current(control, layout, unknowns),
        ElementKind::Inductor { .. }
        | ElementKind::VoltageSource { .. }
        | ElementKind::Vcvs { .. }
        | ElementKind::Ccvs { .. } => branch_current(element.id, layout, unknowns),
    }
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

fn branch_current(element: ElementId, layout: &UnknownLayout, unknowns: &DVector<f64>) -> f64 {
    let index = layout
        .branch_index(element)
        .expect("branch-current element must have a layout index");

    unknowns[index]
}
