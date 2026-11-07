mod common;

use common::circuit;
use ferrispice_mna::{MnaSystem, assemble};
use ferrispice_solver::{
    NewtonErrorKind, NewtonOptions, NewtonProblem, SolveError, solve_linear, solve_newton,
};
use nalgebra::{DMatrix, DVector};

/// unknowns: V(n1), V(n2), I(Vin)
fn diode_system(volts: f64) -> MnaSystem {
    assemble(&circuit(&format!(
        "Vin n1 0 DC {volts}\nD1 n1 n2\nR n2 0 50"
    )))
    .unwrap()
}

/// series source, diode, and resistor solved by bisection on the junction voltage
fn bisect_series_diode(volts: f64) -> f64 {
    let (mut low, mut high) = (0.0, volts);
    for _ in 0..200 {
        let vd: f64 = 0.5 * (low + high);
        let diode = 1e-15 * (vd / 0.025).exp_m1();
        let resistor = (volts - vd) / 50.0;
        if diode < resistor {
            low = vd;
        } else {
            high = vd;
        }
    }
    0.5 * (low + high)
}

fn solve(system: &MnaSystem, initial: &DVector<f64>, options: &NewtonOptions) -> DVector<f64> {
    let problem = NewtonProblem {
        system,
        a: &system.g,
        r: &system.b,
    };
    solve_newton(&problem, initial, options).unwrap().x
}

#[test]
fn converges_on_the_handout_circuit_from_zero() {
    let system = diode_system(1.0);
    let problem = NewtonProblem {
        system: &system,
        a: &system.g,
        r: &system.b,
    };
    let solution = solve_newton(&problem, &DVector::zeros(3), &NewtonOptions::default()).unwrap();
    let vd = bisect_series_diode(1.0);

    assert!((solution.x[0] - 1.0).abs() < 1e-12);
    assert!((solution.x[1] - (1.0 - vd)).abs() < 1e-9, "{}", solution.x);
    assert!((solution.x[2] + (1.0 - vd) / 50.0).abs() < 1e-12);
    assert!((solution.x[1] - 0.2673).abs() < 5e-5);
    assert!(solution.iterations.len() < NewtonOptions::default().max_iterations);

    // the true residual at the solution satisfies KCL and the source constraint
    let residual = &system.g * &solution.x + system.f(&solution.x).unwrap() - &system.b;
    assert!(residual.amax() < 1e-12, "{residual}");

    // the final Jacobian is evaluated at the converged point
    let expected = &system.g + system.df_dx(&solution.x).unwrap();
    assert_eq!(solution.jacobian, expected);
}

#[test]
fn convergence_becomes_quadratic_near_the_root() {
    let system = diode_system(1.0);
    let problem = NewtonProblem {
        system: &system,
        a: &system.g,
        r: &system.b,
    };
    let solution = solve_newton(&problem, &DVector::zeros(3), &NewtonOptions::default()).unwrap();
    let residuals: Vec<_> = solution
        .iterations
        .iter()
        .map(|iteration| iteration.residual)
        .collect();

    // once close, each residual is far smaller than the previous one squared
    // would allow for linear convergence
    let close = residuals
        .iter()
        .position(|&residual| residual < 1e-4)
        .unwrap();
    for pair in residuals[close..].windows(2) {
        if pair[1] > 1e-15 {
            assert!(pair[1] < 1e3 * pair[0] * pair[0], "{residuals:?}");
        }
    }
}

#[test]
fn a_nearby_initial_guess_converges_quickly_to_the_same_solution() {
    let system = diode_system(1.0);
    let options = NewtonOptions::default();
    let from_zero = solve(&system, &DVector::zeros(3), &options);

    let problem = NewtonProblem {
        system: &system,
        a: &system.g,
        r: &system.b,
    };
    let near = DVector::from_vec(vec![1.0, 0.26, -0.005]);
    let solution = solve_newton(&problem, &near, &options).unwrap();

    assert!((solution.x.clone() - &from_zero).amax() < 1e-12);
    assert!(solution.iterations.len() <= 5, "{:?}", solution.iterations);
}

#[test]
fn a_linear_problem_converges_to_the_direct_solution() {
    let system = assemble(&circuit("Vin n1 0 DC 6\nR1 n1 n2 100\nR2 n2 0 50")).unwrap();
    let problem = NewtonProblem {
        system: &system,
        a: &system.g,
        r: &system.b,
    };
    let solution = solve_newton(&problem, &DVector::zeros(3), &NewtonOptions::default()).unwrap();
    let direct = solve_linear(&system.g, &system.b).unwrap();

    assert!((solution.x - direct).amax() < 1e-12);
    assert!(solution.iterations.len() <= 2);
    assert_eq!(solution.jacobian, system.g);
}

#[test]
fn supplied_a_and_r_replace_g_and_b() {
    // r = 0.5 b solves the circuit with its sources at half strength
    let system = diode_system(1.0);
    let half = &system.b * 0.5;
    let problem = NewtonProblem {
        system: &system,
        a: &system.g,
        r: &half,
    };
    let solution = solve_newton(&problem, &DVector::zeros(3), &NewtonOptions::default()).unwrap();
    let vd = bisect_series_diode(0.5);

    assert!((solution.x[0] - 0.5).abs() < 1e-12);
    assert!((solution.x[1] - (0.5 - vd)).abs() < 1e-9);
}

#[test]
fn reaching_the_iteration_limit_is_nonconvergence_with_history() {
    // from zero, a 1.5 V source overshoots and backs off about Vt per iteration
    let system = diode_system(1.5);
    let problem = NewtonProblem {
        system: &system,
        a: &system.g,
        r: &system.b,
    };
    let options = NewtonOptions {
        max_iterations: 3,
        ..NewtonOptions::default()
    };
    let error = solve_newton(&problem, &DVector::zeros(3), &options).unwrap_err();

    assert_eq!(error.iterations.len(), 3);
    let NewtonErrorKind::NonConvergence {
        iterations,
        worst_unknown,
        update,
        residual,
    } = error.kind
    else {
        panic!("expected nonconvergence, found {error:?}");
    };
    assert_eq!(iterations, 3);
    assert!(worst_unknown.is_some());
    assert_eq!(update, error.iterations[2].update);
    assert_eq!(residual, error.iterations[2].residual);
    assert!(
        error
            .to_string()
            .contains("did not converge in 3 iterations")
    );
}

#[test]
fn a_singular_jacobian_reports_the_linear_solve_failure() {
    // nodes b and c float: nothing ties them to the rest of the circuit
    let system = assemble(&circuit("V1 a 0 DC 1\nD1 a 0\nR1 b c 1")).unwrap();
    let problem = NewtonProblem {
        system: &system,
        a: &system.g,
        r: &system.b,
    };
    let error = solve_newton(&problem, &DVector::zeros(4), &NewtonOptions::default()).unwrap_err();

    assert!(
        matches!(
            error.kind,
            NewtonErrorKind::LinearSolve {
                iteration: 1,
                cause: SolveError::Singular { .. }
            }
        ),
        "{error:?}"
    );
}

#[test]
fn overflowing_device_evaluation_fails_instead_of_returning_infinity() {
    // the first update puts about 20 V across the diode, beyond exp's range
    let system = diode_system(20.0);
    let problem = NewtonProblem {
        system: &system,
        a: &system.g,
        r: &system.b,
    };
    let error = solve_newton(&problem, &DVector::zeros(3), &NewtonOptions::default()).unwrap_err();
    let NewtonErrorKind::Device { iteration, cause } = &error.kind else {
        panic!("expected a device failure, found {error:?}");
    };
    assert_eq!(*iteration, 1);
    assert_eq!(cause.element, "D1");
    assert!(error.iterations.is_empty());

    // an initial guess that cannot be evaluated fails before any update
    let initial = DVector::from_vec(vec![100.0, 0.0, 0.0]);
    let error = solve_newton(&problem, &initial, &NewtonOptions::default()).unwrap_err();
    assert!(matches!(
        error.kind,
        NewtonErrorKind::Device { iteration: 1, .. }
    ));
}

#[test]
fn mismatched_dimensions_are_rejected() {
    let system = diode_system(1.0);
    let wrong = DMatrix::zeros(2, 2);
    let problem = NewtonProblem {
        system: &system,
        a: &wrong,
        r: &system.b,
    };
    let error = solve_newton(&problem, &DVector::zeros(3), &NewtonOptions::default()).unwrap_err();

    assert_eq!(
        error.kind,
        NewtonErrorKind::DimensionMismatch { unknowns: 3 }
    );

    let problem = NewtonProblem {
        system: &system,
        a: &system.g,
        r: &system.b,
    };
    assert!(solve_newton(&problem, &DVector::zeros(2), &NewtonOptions::default()).is_err());
}

#[test]
fn iterations_record_where_each_step_linearized_and_its_residuals() {
    let system = diode_system(1.0);
    let problem = NewtonProblem {
        system: &system,
        a: &system.g,
        r: &system.b,
    };
    let solution = solve_newton(&problem, &DVector::zeros(3), &NewtonOptions::default()).unwrap();
    let iterations = &solution.iterations;

    // the first step starts from the initial guess, where only the source
    // constraint V(n1) - 1 = 0 is violated
    assert_eq!(iterations[0].linearized_at, DVector::zeros(3));
    assert_eq!(iterations[0].residual_in, 1.0);

    // each step linearizes where the previous one landed, and its input
    // residual is the previous output residual
    for pair in iterations.windows(2) {
        assert_eq!(
            pair[1].linearized_at,
            &pair[0].linearized_at + &pair[0].delta
        );
        assert_eq!(pair[1].residual_in, pair[0].residual);
    }
    let last = iterations.last().unwrap();
    assert_eq!(solution.x, &last.linearized_at + &last.delta);

    // the first update puts the whole source across the diode; the second is
    // Newton's step of about Vt back down the exponential
    assert!((iterations[1].linearized_at[0] - 1.0).abs() < 1e-12);
    assert!(iterations[1].linearized_at[1].abs() < 1e-9);
    assert!((iterations[0].residual - 235.385).abs() < 1e-3);
    assert!((iterations[1].delta[1] - 0.025).abs() < 1e-6);

    for iteration in iterations {
        let index = iteration.worst_unknown.unwrap();
        assert_eq!(iteration.update, iteration.delta[index].abs());
    }
}
