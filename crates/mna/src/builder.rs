use ferrispice_circuit::{Element, ElementId, NodeId};
use nalgebra::{DMatrix, DVector};

use crate::{AssemblyError, AssemblyErrorKind, MnaSystem, UnknownLayout};

/// accumulates element contributions into the MNA matrics
/// `None` represents ground, whose matrix entries are skipped
/// `Some(index)` mus be a valid matrix index from this builder's layout
pub struct MatrixBuilder {
    system: MnaSystem,
}

impl MatrixBuilder {
    pub fn new(layout: UnknownLayout) -> Self {
        let size = layout.len();

        Self {
            system: MnaSystem {
                g: DMatrix::zeros(size, size),
                c: DMatrix::zeros(size, size),
                b: DVector::zeros(size),
                layout,
            },
        }
    }

    pub fn node_index(&self, node: NodeId) -> Option<usize> {
        self.system.layout.node_index(node)
    }

    /// look up an output or controlling current, on failure return responsible
    /// element
    pub fn branch_index(
        &self,
        element: &Element,
        branch: ElementId,
    ) -> Result<usize, AssemblyError> {
        self.system.layout.branch_index(branch).ok_or_else(|| {
            AssemblyError::new(element, AssemblyErrorKind::MissingBranchCurrent { branch })
        })
    }

    pub fn add_g(&mut self, row: Option<usize>, column: Option<usize>, value: f64) {
        if let (Some(row), Some(column)) = (row, column) {
            self.system.g[(row, column)] += value;
        }
    }

    pub fn add_c(&mut self, row: Option<usize>, column: Option<usize>, value: f64) {
        if let (Some(row), Some(column)) = (row, column) {
            self.system.c[(row, column)] += value;
        }
    }

    pub fn add_b(&mut self, row: Option<usize>, value: f64) {
        if let Some(row) = row {
            self.system.b[row] += value;
        }
    }

    pub fn add_g_two_terminal(
        &mut self,
        positive: Option<usize>,
        negative: Option<usize>,
        value: f64,
    ) {
        self.add_g(positive, positive, value);
        self.add_g(positive, negative, -value);
        self.add_g(negative, positive, -value);
        self.add_g(negative, negative, value);
    }

    pub fn add_c_two_terminal(
        &mut self,
        positive: Option<usize>,
        negative: Option<usize>,
        value: f64,
    ) {
        self.add_c(positive, positive, value);
        self.add_c(positive, negative, -value);
        self.add_c(negative, positive, -value);
        self.add_c(negative, negative, value);
    }

    /// add the +1/-1 matrix entries connecting a branch to its two terminals
    /// `branch` is the matrix index of the element's unknown current
    pub fn add_branch_connections(
        &mut self,
        positive: Option<usize>,
        negative: Option<usize>,
        branch: usize,
    ) {
        self.add_g(positive, Some(branch), 1.0);
        self.add_g(negative, Some(branch), -1.0);
        self.add_g(Some(branch), positive, 1.0);
        self.add_g(Some(branch), negative, -1.0);
    }

    pub fn finish(self) -> MnaSystem {
        self.system
    }
}
