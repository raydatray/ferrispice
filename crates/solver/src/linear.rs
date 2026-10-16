use nalgebra::{DMatrix, DVector};
use thiserror::Error;

/// absolute residual allowance per equation, for rows whose terms are all near zero
pub const ABSOLUTE_TOLERANCE: f64 = 1e-12;

/// residual allowance relative to each row's own scale: `sum_j |A_ij x_j| + |b_i|`
///
/// scaling per row means a large coefficient in one equation cannot hide a poor
/// result in another
pub const RELATIVE_TOLERANCE: f64 = 1e-9;

#[derive(Clone, Debug, Error, PartialEq)]
pub enum SolveError {
    #[error("matrix must be square, found {rows}x{columns}")]
    NotSquare { rows: usize, columns: usize },

    #[error("right-hand side has {rhs} entries for a {rows}-row matrix")]
    DimensionMismatch { rows: usize, rhs: usize },

    #[error("matrix entry ({row}, {column}) is not finite: {value}")]
    NonFiniteMatrix {
        row: usize,
        column: usize,
        value: f64,
    },

    #[error("right-hand side entry {row} is not finite: {value}")]
    NonFiniteRhs { row: usize, value: f64 },

    #[error("matrix is singular: LU pivot {pivot} is zero relative to the matrix scale")]
    Singular { pivot: usize },

    #[error("solution entry {index} is not finite: {value}")]
    NonFiniteSolution { index: usize, value: f64 },

    #[error("equation {row} has residual {residual:e}, exceeding tolerance {tolerance:e}")]
    Residual {
        row: usize,
        residual: f64,
        tolerance: f64,
    },
}

/// solve `a x = b` using LU decomposition
///
/// inputs must be finite, treats the matrix as numerically singular if an LU
/// pivot is too small relative to the matrix scale, successful solutions are
/// finite and satifies every equation within tolerance
pub fn solve_linear(a: &DMatrix<f64>, b: &DVector<f64>) -> Result<DVector<f64>, SolveError> {
    let (rows, columns) = a.shape();
    if rows != columns {
        return Err(SolveError::NotSquare { rows, columns });
    }
    if b.len() != rows {
        return Err(SolveError::DimensionMismatch { rows, rhs: b.len() });
    }

    if let Some((row, column)) = a.row_iter().enumerate().find_map(|(row, values)| {
        values
            .iter()
            .position(|value| !value.is_finite())
            .map(|column| (row, column))
    }) {
        return Err(SolveError::NonFiniteMatrix {
            row,
            column,
            value: a[(row, column)],
        });
    }

    if let Some((row, &value)) = b.iter().enumerate().find(|(_, value)| !value.is_finite()) {
        return Err(SolveError::NonFiniteRhs { row, value });
    }

    if rows == 0 {
        return Ok(DVector::zeros(0));
    }

    let threshold = rows as f64 * f64::EPSILON * a.amax();

    let lu = a.clone().lu(); // do elimination and row swaps (pivots are ready)
    let u = lu.u(); // extract resulting upper-trianglar matrix
    if let Some(pivot) = (0..rows).find(|&i| u[(i, i)].abs() <= threshold) {
        return Err(SolveError::Singular { pivot });
    }

    let x = lu.solve(b).ok_or(SolveError::Singular { pivot: 0 })?;

    check_solution(a, b, &x)?;

    tracing::debug!(unknowns = rows, "solved linear system");

    Ok(x)
}

fn check_solution(a: &DMatrix<f64>, b: &DVector<f64>, x: &DVector<f64>) -> Result<(), SolveError> {
    if let Some((index, &value)) = x.iter().enumerate().find(|(_, value)| !value.is_finite()) {
        return Err(SolveError::NonFiniteSolution { index, value });
    }

    for row in 0..a.nrows() {
        let (sum, scale) = (0..a.ncols()).fold((-b[row], b[row].abs()), |(sum, scale), column| {
            let term = a[(row, column)] * x[column];
            (sum + term, scale + term.abs())
        });

        let residual = sum.abs();
        let tolerance = ABSOLUTE_TOLERANCE + RELATIVE_TOLERANCE * scale;
        if residual > tolerance {
            return Err(SolveError::Residual {
                row,
                residual,
                tolerance,
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_check_rejects_a_poor_row_hidden_by_a_large_one() {
        let a = DMatrix::from_row_slice(2, 2, &[1e12, 0.0, 0.0, 1.0]);
        let b = DVector::from_vec(vec![1e12, 1.0]);
        assert!(check_solution(&a, &b, &DVector::from_vec(vec![1.0, 1.0])).is_ok());
        let error = check_solution(&a, &b, &DVector::from_vec(vec![1.0, 1.001])).unwrap_err();
        assert!(
            matches!(error, SolveError::Residual { row: 1, .. }),
            "{error:?}"
        );
    }

    #[test]
    fn residual_check_rejects_non_finite_solutions() {
        let a = DMatrix::identity(2, 2);
        let b = DVector::from_vec(vec![1.0, 1.0]);
        let error = check_solution(&a, &b, &DVector::from_vec(vec![1.0, f64::NAN])).unwrap_err();
        assert!(matches!(
            error,
            SolveError::NonFiniteSolution { index: 1, .. }
        ));
    }
}
