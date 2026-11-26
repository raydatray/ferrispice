use ferrispice_circuit::{Circuit, ElementId, ElementKind, NodeId};
use ferrispice_mna::{MnaSystem, Unknown, UnknownLayout, assemble};
use nalgebra::{DMatrix, DVector};
use thiserror::Error;

use crate::dc::{element_current as dc_element_current, node_voltage};
use crate::linear::LuFactors;
use crate::{DcError, DcSolution, SolveError, solve_dc};

/// a step count within this relative distance of an integer is that integer,
/// so `0.05 / 1e-4` is 500 steps even if the division rounds up slightly
const GRID_TOLERANCE: f64 = 1e-9;

#[derive(Clone, Debug, Error, PartialEq)]
pub enum TransientError {
    /// the initial state could not be found
    #[error("could not find the DC operating point that starts the transient at t = 0")]
    OperatingPoint(#[source] DcError),

    /// a linear circuit's `G + C/h` could not be factored, or a step's solve
    /// failed its checks
    #[error("the backward-Euler solve for t = {time} s failed")]
    Singular {
        time: f64,
        #[source]
        source: SolveError,
    },

    #[error("backward Euler for circuits with nonlinear devices is not available yet")]
    Nonlinear,
}

/// what a transient analysis did
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TransientStats {
    /// time steps from `t = 0` to the stop time
    pub steps: usize,
    /// LU factorizations of `G + C/h`; one per distinct step size for a
    /// linear circuit
    pub factorizations: usize,
}

/// node voltages and currents at every time point, queried by circuit IDs
#[derive(Clone, Debug, PartialEq)]
pub struct TransientSolution {
    layout: UnknownLayout,
    times: Vec<f64>,
    /// `states[i]` holds the unknowns at `times[i]`; `states[0]` is `x_0`
    states: Vec<DVector<f64>>,
    element_currents: Vec<(ElementId, Vec<f64>)>,
    operating_point: DcSolution,
    stats: TransientStats,
}

impl TransientSolution {
    /// every time point, from 0 to the stop time inclusive
    pub fn times(&self) -> &[f64] {
        &self.times
    }

    /// the unknowns at each time point, ordered by `layout()`
    pub fn states(&self) -> &[DVector<f64>] {
        &self.states
    }

    pub fn layout(&self) -> &UnknownLayout {
        &self.layout
    }

    /// the DC operating point at `t = 0`, which is the initial state
    pub fn operating_point(&self) -> &DcSolution {
        &self.operating_point
    }

    pub fn stats(&self) -> &TransientStats {
        &self.stats
    }

    /// the node's voltage at each time point; ground is zero throughout
    ///
    /// `None` for a non-ground node without a matrix index, such as one from
    /// another circuit
    pub fn node_voltage(&self, node: NodeId) -> Option<Vec<f64>> {
        if node.is_ground() {
            return Some(vec![0.0; self.times.len()]);
        }
        let index = self.layout.node_index(node)?;
        Some(self.column(index))
    }

    /// the element's current at each time point, flowing from its positive
    /// terminal to its negative terminal
    pub fn element_current(&self, element: ElementId) -> Option<Vec<f64>> {
        self.element_currents
            .iter()
            .find(|(id, _)| *id == element)
            .map(|(_, current)| current.clone())
    }

    /// each unknown with its values at every time point, in layout order
    pub fn columns(&self) -> impl Iterator<Item = (&Unknown, Vec<f64>)> {
        self.layout
            .unknowns()
            .iter()
            .enumerate()
            .map(|(index, unknown)| (unknown, self.column(index)))
    }

    fn column(&self, index: usize) -> Vec<f64> {
        self.states.iter().map(|state| state[index]).collect()
    }
}

/// integrate `G x + C dx/dt + f(x) = b(t)` from `t = 0` to `stop` with the
/// backward Euler method and a fixed step
///
/// each step from `t_n` to `t_{n+1} = t_n + h` solves
/// `(G + C/h) x_{n+1} + f(x_{n+1}) = b(t_{n+1}) + (C/h) x_n`. The initial
/// state is the DC operating point. A final step is shortened to land exactly
/// on `stop` when `stop` is not a whole number of steps.
///
/// # Panics
///
/// if `stop` or `step` is not positive and finite, or `step > stop`; compiled
/// `.tran` requests always satisfy this
pub fn solve_transient(
    circuit: &Circuit,
    stop: f64,
    step: f64,
) -> Result<TransientSolution, TransientError> {
    assert!(
        stop.is_finite() && step.is_finite() && step > 0.0 && step <= stop,
        "transient times must satisfy 0 < step <= stop"
    );

    let operating_point = initial_state(circuit)?;
    let system = assemble(circuit).expect("the operating point assembled this circuit");
    let _span = tracing::debug_span!("solve_transient", stop, step).entered();

    let grid = TimeGrid::new(stop, step);
    let mut stats = TransientStats {
        steps: grid.steps.len(),
        ..TransientStats::default()
    };

    if !system.is_linear() {
        return Err(TransientError::Nonlinear);
    }

    let states = integrate_linear(
        &system,
        &grid,
        operating_point.unknowns().clone(),
        &mut stats,
    )?;

    tracing::debug!(points = grid.times.len(), "integrated transient");

    let element_currents = element_currents(circuit, &system.layout, &grid, &states);

    Ok(TransientSolution {
        layout: system.layout.clone(),
        times: grid.times,
        states,
        element_currents,
        operating_point,
        stats,
    })
}

/// `x_0`: the DC operating point with every source at its `t = 0` value,
/// capacitors open and inductors shorted
fn initial_state(circuit: &Circuit) -> Result<DcSolution, TransientError> {
    solve_dc(circuit).map_err(TransientError::OperatingPoint)
}

/// fixed time points `t_n = n h`, with the last clamped to the stop time
struct TimeGrid {
    times: Vec<f64>,
    /// `steps[n]` is the step from `times[n]` to `times[n + 1]`
    steps: Vec<f64>,
}

impl TimeGrid {
    fn new(stop: f64, step: f64) -> Self {
        let ratio = stop / step;
        let nearest = ratio.round();
        let count = if (ratio - nearest).abs() <= GRID_TOLERANCE * nearest {
            nearest
        } else {
            ratio.ceil()
        } as usize;

        // computed as `n h`, not by repeated addition, so the points do not drift
        let times: Vec<f64> = (0..=count)
            .map(|n| if n == count { stop } else { n as f64 * step })
            .collect();

        // interior steps are exactly `h`, so one factorization serves them all
        let last = stop - (count - 1) as f64 * step;
        let last = if (last - step).abs() <= GRID_TOLERANCE * step {
            step
        } else {
            last
        };
        let steps = (0..count)
            .map(|n| if n + 1 == count { last } else { step })
            .collect();

        Self { times, steps }
    }
}

/// `G + C/h` and `C/h` for one step size
struct Companion {
    step: f64,
    a: DMatrix<f64>,
    c_over_h: DMatrix<f64>,
}

impl Companion {
    fn new(system: &MnaSystem, step: f64) -> Self {
        let c_over_h = &system.c / step;
        Self {
            step,
            a: &system.g + &c_over_h,
            c_over_h,
        }
    }

    /// `r = b(t_{n+1}) + (C/h) x_n`
    fn rhs(&self, system: &MnaSystem, time: f64, previous: &DVector<f64>) -> DVector<f64> {
        system.b_at(time) + &self.c_over_h * previous
    }
}

/// a linear circuit factors `G + C/h` once per step size, then each step is
/// one back-substitution
fn integrate_linear(
    system: &MnaSystem,
    grid: &TimeGrid,
    initial: DVector<f64>,
    stats: &mut TransientStats,
) -> Result<Vec<DVector<f64>>, TransientError> {
    let mut states = Vec::with_capacity(grid.times.len());
    states.push(initial);
    let mut factored: Option<(Companion, LuFactors)> = None;

    for (n, &step) in grid.steps.iter().enumerate() {
        let time = grid.times[n + 1];
        let singular = |source| TransientError::Singular { time, source };

        if factored
            .as_ref()
            .is_none_or(|(companion, _)| companion.step != step)
        {
            let companion = Companion::new(system, step);
            let factors = LuFactors::new(&companion.a).map_err(singular)?;
            stats.factorizations += 1;
            factored = Some((companion, factors));
        }
        let (companion, factors) = factored.as_ref().expect("factored above");

        let r = companion.rhs(system, time, &states[n]);
        let x = factors.solve(&r).map_err(singular)?;
        states.push(x);
    }

    Ok(states)
}

/// every element's current at every time point
///
/// a capacitor carries `C (v_{n+1} - v_n) / h`, its backward-Euler current,
/// and none at `t = 0`, where the operating point treats it as open;
/// independent current sources follow their waveforms
fn element_currents(
    circuit: &Circuit,
    layout: &UnknownLayout,
    grid: &TimeGrid,
    states: &[DVector<f64>],
) -> Vec<(ElementId, Vec<f64>)> {
    circuit
        .elements()
        .iter()
        .map(|element| {
            let voltage = |n: usize| {
                node_voltage(element.positive, layout, &states[n])
                    - node_voltage(element.negative, layout, &states[n])
            };

            let current = (0..states.len())
                .map(|n| match element.kind {
                    ElementKind::Capacitor { capacitance } => match n {
                        0 => 0.0,
                        n => capacitance * (voltage(n) - voltage(n - 1)) / grid.steps[n - 1],
                    },
                    ElementKind::CurrentSource { waveform } => waveform.value_at(grid.times[n]),
                    _ => dc_element_current(element, layout, &states[n]),
                })
                .collect();

            (element.id, current)
        })
        .collect()
}
