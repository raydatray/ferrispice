use std::error::Error;
use std::fmt;

/// how an adaptive parameter sweep chooses its steps
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StepOptions {
    /// size of the first attempted step
    pub initial_step: f64,
    /// the sweep stops with an error once a failed step would shrink below this
    pub min_step: f64,
    /// upper bound on any step; `None` lets steps grow until they reach the end
    pub max_step: Option<f64>,
}

/// one attempted step from the last accepted parameter value
#[derive(Clone, Debug, PartialEq)]
pub struct StepAttempt<S, E> {
    pub from: f64,
    pub to: f64,
    /// the solved state when accepted, or why the attempt was rejected
    pub outcome: Result<S, E>,
}

impl<S, E> StepAttempt<S, E> {
    pub fn step(&self) -> f64 {
        self.to - self.from
    }

    pub fn accepted(&self) -> bool {
        self.outcome.is_ok()
    }
}

/// a sweep that reached its end
#[derive(Clone, Debug, PartialEq)]
pub struct SteppingOutcome<S, E> {
    /// state at the end of the sweep
    pub state: S,
    /// every attempt in order, accepted and rejected
    pub attempts: Vec<StepAttempt<S, E>>,
}

impl<S, E> SteppingOutcome<S, E> {
    /// accepted parameter values and their states, in order
    pub fn accepted(&self) -> impl Iterator<Item = (f64, &S)> {
        self.attempts.iter().filter_map(|attempt| {
            attempt
                .outcome
                .as_ref()
                .ok()
                .map(|state| (attempt.to, state))
        })
    }

    pub fn rejected(&self) -> usize {
        self.attempts
            .iter()
            .filter(|attempt| !attempt.accepted())
            .count()
    }
}

/// a sweep whose step shrank below the minimum before reaching its end
#[derive(Clone, Debug, PartialEq)]
pub struct SteppingError<S, E> {
    /// last accepted parameter value
    pub reached: f64,
    /// the step that would have been tried next
    pub step: f64,
    /// every attempt in order; the last one is the final failure
    pub attempts: Vec<StepAttempt<S, E>>,
}

impl<S, E> SteppingError<S, E> {
    /// why the final attempt failed
    pub fn last_failure(&self) -> Option<&E> {
        self.attempts
            .last()
            .and_then(|attempt| attempt.outcome.as_ref().err())
    }
}

impl<S, E: fmt::Display> fmt::Display for SteppingError<S, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "step size fell to {:e} after reaching {} in {} attempts",
            self.step,
            self.reached,
            self.attempts.len()
        )
    }
}

impl<S: fmt::Debug, E: Error + 'static> Error for SteppingError<S, E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.last_failure()
            .map(|error| error as &(dyn Error + 'static))
    }
}

/// advance a parameter from `start` to `end` in adaptive steps
///
/// each attempt solves from the last accepted state. An accepted attempt
/// doubles the step; a rejected attempt halves it and retries from the same
/// accepted state. The sweep never steps past `end`, and it stops with an
/// error when a halved step falls below `min_step`.
///
/// `attempt` receives `(from, to, state)`; DC source stepping uses the
/// source scale as the parameter, and transient analysis can use time.
///
/// # Panics
///
/// if `initial_step` or `min_step` is not positive, or `end < start`
pub fn advance<S: Clone, E>(
    start: f64,
    end: f64,
    initial: S,
    options: &StepOptions,
    mut attempt: impl FnMut(f64, f64, &S) -> Result<S, E>,
) -> Result<SteppingOutcome<S, E>, SteppingError<S, E>> {
    assert!(options.initial_step > 0.0, "initial step must be positive");
    assert!(options.min_step > 0.0, "minimum step must be positive");
    assert!(end >= start, "a sweep cannot run backwards");

    let mut position = start;
    let mut state = initial;
    let mut step = options.initial_step;
    let mut attempts = Vec::new();

    while position < end {
        let size = options.max_step.map_or(step, |max| step.min(max));
        // finish exactly at `end`, without leaving a sliver below `min_step`
        let to = if end - (position + size) < options.min_step {
            end
        } else {
            position + size
        };

        match attempt(position, to, &state) {
            Ok(next) => {
                tracing::trace!(from = position, to, "accepted step");
                attempts.push(StepAttempt {
                    from: position,
                    to,
                    outcome: Ok(next.clone()),
                });
                step = 2.0 * (to - position);
                position = to;
                state = next;
            }
            Err(error) => {
                tracing::trace!(from = position, to, "rejected step");
                attempts.push(StepAttempt {
                    from: position,
                    to,
                    outcome: Err(error),
                });
                step = 0.5 * (to - position);

                if step < options.min_step {
                    return Err(SteppingError {
                        reached: position,
                        step,
                        attempts,
                    });
                }
            }
        }
    }

    Ok(SteppingOutcome { state, attempts })
}
