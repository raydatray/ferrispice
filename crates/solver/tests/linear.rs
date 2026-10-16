use ferrispice_solver::{SolveError, solve_linear};
use nalgebra::{DMatrix, DVector};

fn matrix(rows: usize, values: &[f64]) -> DMatrix<f64> {
    DMatrix::from_row_slice(rows, values.len() / rows, values)
}

fn assert_vector(actual: &DVector<f64>, expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    for (&actual, &expected) in actual.iter().zip(expected) {
        assert!(
            (actual - expected).abs() <= 1e-12 * (1.0 + expected.abs()),
            "{actual} != {expected}"
        );
    }
}

#[test]
fn solves_a_known_nonsymmetric_system() {
    let a = matrix(3, &[2.0, 1.0, -1.0, -3.0, -1.0, 2.0, -2.0, 1.0, 2.0]);
    let b = DVector::from_vec(vec![8.0, -11.0, -3.0]);
    assert_vector(&solve_linear(&a, &b).unwrap(), &[2.0, 3.0, -1.0]);
}

#[test]
fn pivots_past_zero_diagonal_entries() {
    let a = matrix(3, &[0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0]);
    let b = DVector::from_vec(vec![2.0, 3.0, 4.0]);
    assert_vector(&solve_linear(&a, &b).unwrap(), &[4.0, 2.0, 3.0]);
}

#[test]
fn solves_the_divider_mna_system() {
    let a = matrix(3, &[0.01, -0.01, 1.0, -0.01, 0.03, 0.0, 1.0, 0.0, 0.0]);
    let b = DVector::from_vec(vec![0.0, 0.0, 6.0]);
    assert_vector(&solve_linear(&a, &b).unwrap(), &[6.0, 2.0, -0.04]);
}

#[test]
fn widely_scaled_rows_still_meet_the_residual_bound() {
    let a = matrix(2, &[1e9, 1.0, 1.0, 1e-3]);
    let b = DVector::from_vec(vec![1e9 + 2.0, 1.0 + 2e-3]);
    assert_vector(&solve_linear(&a, &b).unwrap(), &[1.0, 2.0]);
}

#[test]
fn empty_system_has_an_empty_solution() {
    assert_eq!(
        solve_linear(&DMatrix::zeros(0, 0), &DVector::zeros(0))
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn rejects_non_square_and_mismatched_dimensions() {
    assert_eq!(
        solve_linear(&DMatrix::zeros(2, 3), &DVector::zeros(2)),
        Err(SolveError::NotSquare {
            rows: 2,
            columns: 3
        })
    );
    assert_eq!(
        solve_linear(&DMatrix::identity(2, 2), &DVector::zeros(3)),
        Err(SolveError::DimensionMismatch { rows: 2, rhs: 3 })
    );
}

#[test]
fn rejects_non_finite_inputs() {
    let mut a = DMatrix::identity(2, 2);
    a[(1, 0)] = f64::INFINITY;
    assert!(matches!(
        solve_linear(&a, &DVector::zeros(2)),
        Err(SolveError::NonFiniteMatrix {
            row: 1,
            column: 0,
            ..
        })
    ));
    let b = DVector::from_vec(vec![0.0, f64::NAN]);
    assert!(matches!(
        solve_linear(&DMatrix::identity(2, 2), &b),
        Err(SolveError::NonFiniteRhs { row: 1, .. })
    ));
}

#[test]
fn reports_exactly_and_numerically_singular_matrices() {
    for a in [
        DMatrix::zeros(2, 2),
        matrix(2, &[1.0, 2.0, 2.0, 4.0]),
        matrix(2, &[0.1, -0.1, -0.1, 0.1]),
        matrix(3, &[0.0, 1.0, 1.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0]),
    ] {
        let error = solve_linear(&a, &DVector::from_element(a.nrows(), 1.0)).unwrap_err();
        assert!(
            matches!(error, SolveError::Singular { .. }),
            "{a}: {error:?}"
        );
    }
}
