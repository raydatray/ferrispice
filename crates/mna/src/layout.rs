use std::collections::HashMap;

use ferrispice_circuit::{Circuit, ElementId, NodeId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnknownKind {
    NodeVoltage(NodeId),
    BranchCurrent(ElementId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unknown {
    pub kind: UnknownKind,
    pub label: String,
}

/// matrix indices for one circuit: node voltages first, then explicit currents
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownLayout {
    unknowns: Vec<Unknown>,
    nodes: HashMap<NodeId, usize>,
    branches: HashMap<ElementId, usize>,
}

impl UnknownLayout {
    /// allocate every unknown before stamping, including forward controllers
    pub fn new(circuit: &Circuit) -> Self {
        let mut layout = Self {
            unknowns: Vec::new(),
            nodes: HashMap::new(),
            branches: HashMap::new(),
        };

        for node in circuit.non_ground_nodes() {
            layout.nodes.insert(node.id, layout.unknowns.len());
            layout.unknowns.push(Unknown {
                kind: UnknownKind::NodeVoltage(node.id),
                label: format!("V({})", node.name),
            });
        }

        for element in circuit
            .elements()
            .iter()
            .filter(|element| element.kind.has_branch_current())
        {
            layout.branches.insert(element.id, layout.unknowns.len());
            layout.unknowns.push(Unknown {
                kind: UnknownKind::BranchCurrent(element.id),
                label: format!("I({})", element.name),
            });
        }
        layout
    }

    pub fn len(&self) -> usize {
        self.unknowns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.unknowns.is_empty()
    }

    pub fn unknowns(&self) -> &[Unknown] {
        &self.unknowns
    }

    pub fn labels(&self) -> impl ExactSizeIterator<Item = &str> {
        self.unknowns.iter().map(|unknown| unknown.label.as_str())
    }

    /// ground has no matrix index, IDs must belong to the circuit that
    /// built this layout
    pub fn node_index(&self, node: NodeId) -> Option<usize> {
        self.nodes.get(&node).copied()
    }

    /// only voltage sources, inductors, VCVS, and CCVS have explicit currents
    pub fn branch_index(&self, element: ElementId) -> Option<usize> {
        self.branches.get(&element).copied()
    }
}
