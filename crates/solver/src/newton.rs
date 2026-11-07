use ferrispice_mna::{DeviceError, MnaSystem, UnknownKind};
use nalgebra::{DMatrix, DVector};
use thiserror::Error;

use crate::{SolveError, solve_linear};

/// the nonlinear equation `Φ(x) = A x + f(x) - r = 0`
///
/// `A` and `r` are constant for one solve, so each analysis supplies its own:
/// DC uses `A = G` and `r = λ b`; backward Euler uses `A = G + C/h` and
/// `r = b(t) + (C/h) x_n`
#[derive(Clone, Copy, Debug)]
pub struct NewtonProblem<'a> {
    /// provides `f(x)`, `∂f/∂x`, and the unknown layout
    pub system: &'a MnaSystem,
    pub a: &'a DMatrix<f64>,
    pub r: &'a DVector<f64>,
}

/// iteration limit and convergence tolerances
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NewtonOptions {
    /// a modest limit makes hopeless attempts fail quickly so a caller can
    /// retry with a smaller step
    pub max_iterations: usize,
    /// relative tolerance on updates and residuals
    pub reltol: f64,
    /// absolute tolerance for voltages: voltage updates and branch residuals
    pub vntol: f64,
    /// absolute tolerance for currents: current updates and KCL residuals
    pub abstol: f64,
}

impl Default for NewtonOptions {
    fn default() -> Self {
        Self {
            max_iterations: 50,
            reltol: 1e-6,
            vntol: 1e-9,
            abstol: 1e-12,
        }
    }
}

/// one Newton iteration: where it linearized, the step it took, and the
/// residuals before and after
///
/// iteration `k + 1` linearizes where iteration `k` landed, so its
/// `linearized_at` is the previous `linearized_at + delta`, and its
/// `residual_in` is the previous `residual`
#[derive(Clone, Debug, PartialEq)]
pub struct NewtonIteration {
    /// `x_k`, where `f` and `J` were evaluated
    pub linearized_at: DVector<f64>,
    /// largest `|Φ_i(x_k)|`, the residual that drives this step
    pub residual_in: f64,
    /// `Δx` from `J(x_k) Δx = -Φ(x_k)`
    pub delta: DVector<f64>,
    /// unknown whose update is largest relative to its tolerance; `None` when
    /// there are no unknowns
    pub worst_unknown: Option<usize>,
    /// `|Δx|` of `worst_unknown`
    pub update: f64,
    /// largest `|Φ_i(x_{k+1})|`, the residual after the update
    pub residual: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewtonSolution {
    pub x: DVector<f64>,
    pub iterations: Vec<NewtonIteration>,
    /// `J = A + ∂f/∂x` at the converged `x`
    pub jacobian: DMatrix<f64>,
}

/// a failed solve, with the iterations completed before it failed
#[derive(Clone, Debug, Error, PartialEq)]
#[error("{kind}")]
pub struct NewtonError {
    pub kind: NewtonErrorKind,
    pub iterations: Vec<NewtonIteration>,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum NewtonErrorKind {
    #[error("Newton problem dimensions do not match the system's {unknowns} unknowns")]
    DimensionMismatch { unknowns: usize },

    #[error(
        "Newton-Raphson did not converge in {iterations} iterations; \
         the largest update was {update:e} and the largest residual {residual:e}"
    )]
    NonConvergence {
        iterations: usize,
        worst_unknown: Option<usize>,
        update: f64,
        residual: f64,
    },

    #[error("linear solve failed at Newton iteration {iteration}: {cause}")]
    LinearSolve { iteration: usize, cause: SolveError },

    #[error("device evaluation failed at Newton iteration {iteration}: {cause}")]
    Device {
        iteration: usize,
        cause: DeviceError,
    },

    #[error("Newton iteration {iteration} produced a non-finite value for unknown {index}")]
    NonFiniteIterate { iteration: usize, index: usize },
}

/// solve `A x + f(x) - r = 0` by Newton-Raphson from `initial`
///
/// each iteration linearizes `Φ` at `x_k` and solves `J(x_k) Δx = -Φ(x_k)`
/// with `J = A + ∂f/∂x`, then sets `x_{k+1} = x_k + Δx`. The solve converges
/// only when every update and every residual of `Φ(x_{k+1})` is within
/// tolerance. A failure never returns its last iterate as a solution.
pub fn solve_newton(
    problem: &NewtonProblem<'_>,
    initial: &DVector<f64>,
    options: &NewtonOptions,
) -> Result<NewtonSolution, NewtonError> {
    let NewtonProblem { system, a, r } = *problem;
    let unknowns = system.layout.len();
    let mut iterations = Vec::new();

    if a.shape() != (unknowns, unknowns) || r.len() != unknowns || initial.len() != unknowns {
        return Err(failure(
            NewtonErrorKind::DimensionMismatch { unknowns },
            iterations,
        ));
    }

    let _span = tracing::debug_span!("solve_newton", unknowns).entered();

    // unknown i is a voltage or a current; row i is a KCL (current) equation
    // for a node voltage and a branch (voltage) equation for a branch current
    let voltage_unknown: Vec<_> = system
        .layout
        .unknowns()
        .iter()
        .map(|unknown| matches!(unknown.kind, UnknownKind::NodeVoltage(_)))
        .collect();

    let mut x = initial.clone();
    let mut f = match system.f(&x) {
        Ok(f) => f,
        Err(cause) => return Err(device(1, cause, iterations)),
    };

    for iteration in 1..=options.max_iterations {
        let jacobian = match system.df_dx(&x) {
            Ok(df_dx) => a + df_dx,
            Err(cause) => return Err(device(iteration, cause, iterations)),
        };
        let residual = a * &x + &f - r;
        let residual_in = residual.amax();

        let delta = match solve_linear(&jacobian, &-residual) {
            Ok(delta) => delta,
            Err(cause) => {
                return Err(failure(
                    NewtonErrorKind::LinearSolve { iteration, cause },
                    iterations,
                ));
            }
        };

        let next = &x + &delta;
        if let Some(index) = next.iter().position(|value| !value.is_finite()) {
            return Err(failure(
                NewtonErrorKind::NonFiniteIterate { iteration, index },
                iterations,
            ));
        }

        let f_next = match system.f(&next) {
            Ok(f) => f,
            Err(cause) => return Err(device(iteration, cause, iterations)),
        };
        let residual_next = a * &next + &f_next - r;

        let (worst_unknown, updates_converged) =
            check_updates(&x, &next, &voltage_unknown, options);
        let residuals_converged = check_residuals(
            a,
            &next,
            &f_next,
            r,
            &residual_next,
            &voltage_unknown,
            options,
        );

        let update = worst_unknown.map_or(0.0, |index| delta[index].abs());
        let residual_out = residual_next.amax();

        tracing::trace!(
            iteration,
            residual_in,
            update,
            residual = residual_out,
            "newton iteration"
        );

        iterations.push(NewtonIteration {
            linearized_at: x.clone(),
            residual_in,
            delta,
            worst_unknown,
            update,
            residual: residual_out,
        });

        if updates_converged && residuals_converged {
            let jacobian = match system.df_dx(&next) {
                Ok(df_dx) => a + df_dx,
                Err(cause) => return Err(device(iteration, cause, iterations)),
            };

            tracing::debug!(iterations = iteration, "newton converged");

            return Ok(NewtonSolution {
                x: next,
                iterations,
                jacobian,
            });
        }

        x = next;
        f = f_next;
    }

    let (worst_unknown, update, residual) =
        iterations.last().map_or((None, 0.0, f64::NAN), |last| {
            (last.worst_unknown, last.update, last.residual)
        });

    Err(failure(
        NewtonErrorKind::NonConvergence {
            iterations: iterations.len(),
            worst_unknown,
            update,
            residual,
        },
        iterations,
    ))
}

/// `|Δx_i| <= reltol * max(|x_{k+1,i}|, |x_{k,i}|) + tol_i` for every unknown,
/// and the unknown with the largest update relative to its tolerance
fn check_updates(
    x: &DVector<f64>,
    next: &DVector<f64>,
    voltage_unknown: &[bool],
    options: &NewtonOptions,
) -> (Option<usize>, bool) {
    let mut worst = None;
    let mut worst_ratio = f64::NEG_INFINITY;
    let mut converged = true;

    for index in 0..x.len() {
        let absolute = if voltage_unknown[index] {
            options.vntol
        } else {
            options.abstol
        };
        let tolerance = options.reltol * next[index].abs().max(x[index].abs()) + absolute;
        let ratio = (next[index] - x[index]).abs() / tolerance;

        converged &= ratio <= 1.0;
        if ratio > worst_ratio {
            worst_ratio = ratio;
            worst = Some(index);
        }
    }

    (worst, converged)
}

/// each `|Φ_i| <= tol_i + reltol * scale_i`, where `scale_i` is the largest
/// magnitude among the row's terms `A_ij x_j`, `f_i`, and `r_i`
fn check_residuals(
    a: &DMatrix<f64>,
    x: &DVector<f64>,
    f: &DVector<f64>,
    r: &DVector<f64>,
    residual: &DVector<f64>,
    voltage_unknown: &[bool],
    options: &NewtonOptions,
) -> bool {
    (0..x.len()).all(|row| {
        let scale = (0..x.len())
            .map(|column| (a[(row, column)] * x[column]).abs())
            .fold(f[row].abs().max(r[row].abs()), f64::max);
        // a node voltage's row is KCL, in amperes; a branch current's row is
        // a branch equation, in volts
        let absolute = if voltage_unknown[row] {
            options.abstol
        } else {
            options.vntol
        };

        residual[row].abs() <= absolute + options.reltol * scale
    })
}

fn failure(kind: NewtonErrorKind, iterations: Vec<NewtonIteration>) -> NewtonError {
    tracing::debug!(%kind, "newton failed");
    NewtonError { kind, iterations }
}

fn device(iteration: usize, cause: DeviceError, iterations: Vec<NewtonIteration>) -> NewtonError {
    failure(NewtonErrorKind::Device { iteration, cause }, iterations)
}
