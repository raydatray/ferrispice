use ferrispice_solver::{StepOptions, advance};

const OPTIONS: StepOptions = StepOptions {
    initial_step: 0.1,
    min_step: 1e-6,
    max_step: None,
};

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() < 1e-12
}

/// accepted parameter values, rounded for comparison
fn accepted<E>(outcome: &ferrispice_solver::SteppingOutcome<f64, E>) -> Vec<f64> {
    outcome.accepted().map(|(to, _)| to).collect()
}

#[test]
fn doubles_the_step_after_each_success() {
    let outcome = advance(0.0, 1.0, 0.0, &OPTIONS, |_, to, _| Ok::<_, ()>(to)).unwrap();
    let values = accepted(&outcome);

    assert_eq!(values.len(), 4);
    for (actual, expected) in values.iter().zip([0.1, 0.3, 0.7, 1.0]) {
        assert!(close(*actual, expected), "{values:?}");
    }
    assert_eq!(outcome.state, 1.0);
    assert_eq!(outcome.rejected(), 0);
}

#[test]
fn halves_after_a_rejection_and_retries_from_the_last_accepted_state() {
    // any step larger than 0.3 fails; the state records the last accepted value
    let mut calls = Vec::new();
    let outcome = advance(0.0, 1.0, 0.0, &OPTIONS, |from, to, &state: &f64| {
        calls.push((from, to, state));
        if to - from > 0.3 {
            Err("too far")
        } else {
            Ok(to)
        }
    })
    .unwrap();

    for &(from, _, state) in &calls {
        assert_eq!(from, state, "each attempt starts from the accepted state");
    }

    let steps: Vec<_> = outcome
        .attempts
        .iter()
        .map(|attempt| (attempt.step(), attempt.accepted()))
        .collect();
    let expected = [
        (0.1, true),
        (0.2, true),
        (0.4, false),
        (0.2, true),
        (0.4, false),
        (0.2, true),
        (0.3, true),
    ];
    assert_eq!(steps.len(), expected.len(), "{steps:?}");
    for ((step, accepted), (expected_step, expected_accepted)) in steps.iter().zip(expected) {
        assert!(close(*step, expected_step), "{steps:?}");
        assert_eq!(*accepted, expected_accepted, "{steps:?}");
    }

    // a rejected attempt is retried from the same starting point
    assert_eq!(outcome.attempts[2].from, outcome.attempts[3].from);
    assert_eq!(outcome.attempts[2].outcome, Err("too far"));
    assert_eq!(outcome.rejected(), 2);
    assert_eq!(outcome.state, 1.0);
}

#[test]
fn never_steps_past_the_end() {
    for end in [0.05, 0.25, 1.0, 3.7] {
        let outcome = advance(0.0, end, 0.0, &OPTIONS, |_, to, _| Ok::<_, ()>(to)).unwrap();

        assert!(outcome.attempts.iter().all(|attempt| attempt.to <= end));
        assert_eq!(outcome.attempts.last().unwrap().to, end);
        assert_eq!(outcome.state, end);
    }
}

#[test]
fn stops_with_an_error_when_the_step_falls_below_the_minimum() {
    let error = advance(0.0, 1.0, 0.0, &OPTIONS, |from, _, _| {
        if from < 0.3 {
            Ok(0.3)
        } else {
            Err(format!("failed from {from}"))
        }
    })
    .map(|outcome| outcome.state)
    .unwrap_err();

    assert!(close(error.reached, 0.3));
    assert!(error.step < OPTIONS.min_step);
    assert!(error.last_failure().unwrap().starts_with("failed from 0.3"));
    assert!(
        error
            .attempts
            .iter()
            .skip(2)
            .all(|attempt| !attempt.accepted())
    );
    assert!(error.to_string().contains("step size fell to"), "{}", error);

    // each rejection halves the step: 0.4, 0.2, ... until below 1e-6
    let rejected = error.attempts.len() - 2;
    assert_eq!(rejected, 19);
}

#[test]
fn max_step_caps_growth() {
    let options = StepOptions {
        max_step: Some(0.25),
        ..OPTIONS
    };
    let outcome = advance(0.0, 1.0, 0.0, &options, |_, to, _| Ok::<_, ()>(to)).unwrap();

    assert!(
        outcome
            .attempts
            .iter()
            .all(|attempt| attempt.step() <= 0.25 + 1e-12)
    );
    assert_eq!(outcome.state, 1.0);
}

#[test]
fn time_like_parameters_receive_both_ends_of_each_step() {
    // a transient-style caller derives its step size from `to - from`
    let mut steps = Vec::new();
    let outcome = advance(1.0, 2.0, 1.0, &OPTIONS, |from, to, _| {
        steps.push(to - from);
        Ok::<_, ()>(to)
    })
    .unwrap();

    assert!(close(steps.iter().sum::<f64>(), 1.0));
    assert_eq!(outcome.accepted().count(), steps.len());
}

#[test]
fn an_empty_sweep_returns_the_initial_state() {
    let outcome = advance(1.0, 1.0, 42, &OPTIONS, |_, _, _| Err::<i32, ()>(())).unwrap();

    assert_eq!(outcome.state, 42);
    assert!(outcome.attempts.is_empty());
}
