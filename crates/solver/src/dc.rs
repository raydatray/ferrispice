use std::collections::HashMap;

use ferrispice_circuit::{Circuit, Element, ElementId, ElementKind, NodeId};
use ferrispice_mna::{
    AssemblyError, MnaSystem, UnknownLayout, assemble, diode_current_and_derivative,
};
use nalgebra::{DMatrix, DVector};
use thiserror::Error;

use crate::{
    NewtonError, NewtonOptions, NewtonProblem, NewtonSolution, SolveError, StepOptions,
    SteppingError, SteppingOutcome, advance, solve_linear, solve_newton,
};

/// source stepping ramps `λ` from 0 to 1, starting with a step of 0.1
///
/// the minimum step is tiny because a diode at `x = 0` conducts only
/// `Is / Vt`: a current source driving it must be scaled down to roughly
/// `Is` before the first Newton update stays in range, after which the
/// doubling steps reach full strength quickly
pub const SOURCE_STEPPING: StepOptions = StepOptions {
    initial_step: 0.1,
    min_step: 1e-15,
    max_step: None,
};

#[derive(Clone, Debug, Error, PartialEq)]
pub enum DcError {
    #[error(transparent)]
    Assembly(#[from] AssemblyError),

    #[error(transparent)]
    Solve(#[from] SolveError),

    #[error(transparent)]
    SourceStepping(#[from] Box<SourceSteppingFailure>),
}

/// Newton failed on the full circuit, and ramping the sources did not reach it
#[derive(Clone, Debug, Error, PartialEq)]
#[error(
    "Newton-Raphson failed on the full circuit, and source stepping stopped with the \
     sources scaled by {:.3e}: {}",
    stepping.reached,
    stepping.last_failure().map_or_else(|| direct.to_string(), ToString::to_string)
)]
pub struct SourceSteppingFailure {
    /// failure of the first attempt, Newton on the full circuit
    pub direct: NewtonError,
    /// every source-stepping attempt; the last one is the final failure
    pub stepping: SteppingError<NewtonSolution, NewtonError>,
}

/// how the DC operating point was found
#[derive(Clone, Debug, PartialEq)]
pub enum DcMethod {
    /// the circuit has no nonlinear devices: one LU solve of `G x = b`
    Linear,
    /// Newton-Raphson on the full circuit, starting from `x = 0`
    Newton(NewtonSolution),
    /// Newton on the full circuit failed, so the sources were ramped up:
    /// each attempt solves `G x + f(x) = λ b` from the last accepted `λ`
    SourceStepping {
        direct: NewtonError,
        stepping: SteppingOutcome<NewtonSolution, NewtonError>,
    },
}

impl DcMethod {
    fn final_newton(&self) -> Option<&NewtonSolution> {
        match self {
            Self::Linear => None,
            Self::Newton(solution) => Some(solution),
            Self::SourceStepping { stepping, .. } => Some(&stepping.state),
        }
    }
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
        self.method
            .final_newton()
            .map(|solution| &solution.jacobian)
    }

    /// Newton iterations performed across every attempt; zero for linear circuits
    pub fn newton_iterations(&self) -> usize {
        match &self.method {
            DcMethod::Linear => 0,
            DcMethod::Newton(solution) => solution.iterations.len(),
            DcMethod::SourceStepping { direct, stepping } => {
                direct.iterations.len()
                    + stepping
                        .attempts
                        .iter()
                        .map(|attempt| match &attempt.outcome {
                            Ok(solution) => solution.iterations.len(),
                            Err(error) => error.iterations.len(),
                        })
                        .sum::<usize>()
            }
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

    if system.is_linear() {
        let x = solve_linear(&system.g, &system.b)?;
        return Ok(interpret(circuit, &system, x, DcMethod::Linear));
    }

    let method = solve_nonlinear(&system)?;
    let x = method
        .final_newton()
        .expect("nonlinear methods end with a Newton solution")
        .x
        .clone();

    Ok(interpret(circuit, &system, x, method))
}

/// Newton on the full circuit, then source stepping if that fails
fn solve_nonlinear(system: &MnaSystem) -> Result<DcMethod, DcError> {
    let options = NewtonOptions::default();
    let zero = DVector::zeros(system.layout.len());

    // sources scaled by λ appear only in `b`, so the right-hand side is `λ b`
    let solve_at = |scale: f64, initial: &DVector<f64>| {
        let r = &system.b * scale;
        let problem = NewtonProblem {
            system,
            a: &system.g,
            r: &r,
        };
        solve_newton(&problem, initial, &options)
    };

    let direct = match solve_at(1.0, &zero) {
        Ok(solution) => return Ok(DcMethod::Newton(solution)),
        Err(error) => error,
    };

    tracing::debug!(%direct, "direct Newton failed; stepping the sources");

    // with every source off, `x = 0` solves `G x + f(x) = 0` exactly
    let off = NewtonSolution {
        x: zero.clone(),
        iterations: Vec::new(),
        jacobian: system.g.clone(),
    };

    match advance(0.0, 1.0, off, &SOURCE_STEPPING, |_, to, state| {
        solve_at(to, &state.x)
    }) {
        Ok(stepping) => Ok(DcMethod::SourceStepping { direct, stepping }),
        Err(stepping) => Err(DcError::SourceStepping(Box::new(SourceSteppingFailure {
            direct,
            stepping,
        }))),
    }
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
