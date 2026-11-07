mod dc;
mod linear;
mod newton;
mod stepping;

pub use dc::{DcError, DcMethod, DcSolution, SOURCE_STEPPING, SourceSteppingFailure, solve_dc};
pub use linear::{ABSOLUTE_TOLERANCE, RELATIVE_TOLERANCE, SolveError, solve_linear};
pub use newton::{
    NewtonError, NewtonErrorKind, NewtonIteration, NewtonOptions, NewtonProblem, NewtonSolution,
    solve_newton,
};
pub use stepping::{StepAttempt, StepOptions, SteppingError, SteppingOutcome, advance};
