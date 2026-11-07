use std::collections::HashMap;

use ferrispice_circuit::{Circuit, Element, ElementId, ElementKind, NodeId};
use ferrispice_mna::{
    AssemblyError, MnaSystem, UnknownLayout, assemble, diode_current_and_derivative,
};
use nalgebra::{DMatrix, DVector};
use thiserror::Error;

use crate::{
    NewtonError, NewtonOptions, NewtonProblem, NewtonSolution, SolveError, solve_linear,
    solve_newton,
};

#[derive(Clone, Debug, Error, PartialEq)]
pub enum DcError {
    #[error(transparent)]
    Assembly(#[from] AssemblyError),

    #[error(transparent)]
    Solve(#[from] SolveError),

    #[error(transparent)]
    Newton(#[from] Box<NewtonError>),
}

/// how the DC operating point was found
#[derive(Clone, Debug, PartialEq)]
pub enum DcMethod {
    /// the circuit has no nonlinear devices: one LU solve of `G x = b`
    Linear,
    /// Newton-Raphson on the full circuit, starting from `x = 0`
    Newton(NewtonSolution),
}

/// DC operating point, queried by circuit IDs instead of matrix indices
#[derive(Clone, Debug, PartialEq)]
pub struct DcSolution {
    layout: UnknownLayout,
    unknowns: DVector<f64>,
    node_voltages: Vec<(NodeId, f64)>,
    element_currents: Vec<(ElementId, f64)>,
    voltage_index: HashMap<NodeId, usize>,
    current_index: HashMap<ElementId, usize>,
    method: DcMethod,
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

    pub fn method(&self) -> &DcMethod {
        &self.method
    }

    /// whether Newton-Raphson was needed because the circuit is nonlinear
    pub fn is_nonlinear(&self) -> bool {
        !matches!(self.method, DcMethod::Linear)
    }

    /// `J = G + ∂f/∂x` at the operating point; `None` for linear circuits
    pub fn jacobian(&self) -> Option<&DMatrix<f64>> {
        match &self.method {
            DcMethod::Linear => None,
            DcMethod::Newton(solution) => Some(&solution.jacobian),
        }
    }

    /// Newton iterations performed; zero for linear circuits
    pub fn newton_iterations(&self) -> usize {
        match &self.method {
            DcMethod::Linear => 0,
            DcMethod::Newton(solution) => solution.iterations.len(),
        }
    }
}

/// assemble and solve the DC equations `G x + f(x) = b` of a circuit
///
/// time derivatives are zero at DC, so `C` does not participate. Linear
/// circuits are solved directly; nonlinear circuits use Newton-Raphson.
pub fn solve_dc(circuit: &Circuit) -> Result<DcSolution, DcError> {
    let system = assemble(circuit)?;
    let _span = tracing::debug_span!("solve_dc", unknowns = system.layout.len()).entered();

    let (unknowns, method) = if system.is_linear() {
        (solve_linear(&system.g, &system.b)?, DcMethod::Linear)
    } else {
        let solution = solve_nonlinear(&system)?;
        (solution.x.clone(), DcMethod::Newton(solution))
    };

    Ok(interpret(circuit, &system, unknowns, method))
}

fn solve_nonlinear(system: &MnaSystem) -> Result<NewtonSolution, DcError> {
    let problem = NewtonProblem {
        system,
        a: &system.g,
        r: &system.b,
    };
    let initial = DVector::zeros(system.layout.len());

    solve_newton(&problem, &initial, &NewtonOptions::default())
        .map_err(|error| DcError::Newton(Box::new(error)))
}

/// map solved unknowns to node voltages and element currents
fn interpret(
    circuit: &Circuit,
    system: &MnaSystem,
    unknowns: DVector<f64>,
    method: DcMethod,
) -> DcSolution {
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

    DcSolution {
        voltage_index,
        current_index,
        layout: layout.clone(),
        unknowns,
        node_voltages,
        element_currents,
        method,
    }
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
        ElementKind::Diode {
            saturation_current,
            thermal_voltage,
        } => {
            let voltage = node_voltage(element.positive, layout, unknowns)
                - node_voltage(element.negative, layout, unknowns);
            // Newton evaluated every diode at the converged unknowns
            diode_current_and_derivative(saturation_current, thermal_voltage, voltage)
                .expect("diode current is finite at a converged operating point")
                .current
        }
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
