use std::collections::HashMap;
use std::fmt;

use ferrispice_netlist::{ByteSpan, NetlistPosition};

pub const GROUND: NodeId = NodeId(0);

/// stable identifier for a node within a compiled circuit
/// this is not MNA row or column index, use `UnknownLayout::node_index`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(pub(crate) usize);

impl NodeId {
    pub fn is_ground(self) -> bool {
        self == GROUND
    }
}

/// stable identifier for an element within a compiled circuit
/// this is not branch-current matrix index, use `UnknownLayout::branch_index`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ElementId(pub(crate) usize);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub id: NodeId,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    pub id: ElementId,
    pub name: String,
    /// positive current flows from this terminal to `negative`
    pub positive: NodeId,
    pub negative: NodeId,
    pub kind: ElementKind,
    pub location: NetlistPosition,
    pub span: ByteSpan,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ElementKind {
    Resistor {
        resistance: f64,
    },
    Capacitor {
        capacitance: f64,
    },
    Inductor {
        inductance: f64,
    },
    VoltageSource {
        dc: f64,
    },
    CurrentSource {
        dc: f64,
    },
    Vcvs {
        control_positive: NodeId,
        control_negative: NodeId,
        gain: f64,
    },
    Vccs {
        control_positive: NodeId,
        control_negative: NodeId,
        transconductance: f64,
    },
    Ccvs {
        control: ElementId,
        transresistance: f64,
    },
    Cccs {
        control: ElementId,
        gain: f64,
    },
}

impl ElementKind {
    /// whether this element get a current unknown in the MNA
    pub fn has_branch_current(&self) -> bool {
        matches!(
            self,
            Self::VoltageSource { .. }
                | Self::Inductor { .. }
                | Self::Vcvs { .. }
                | Self::Ccvs { .. }
        )
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Resistor { .. } => "resistor",
            Self::Capacitor { .. } => "capacitor",
            Self::Inductor { .. } => "inductor",
            Self::VoltageSource { .. } => "voltage source",
            Self::CurrentSource { .. } => "current source",
            Self::Vcvs { .. } => "VCVS",
            Self::Vccs { .. } => "VCCS",
            Self::Ccvs { .. } => "CCVS",
            Self::Cccs { .. } => "CCCS",
        }
    }
}

/// IDs are scoped to this circuit, lookup is case-insensitive
#[derive(Clone)]
pub struct Circuit {
    title: String,
    nodes: Vec<Node>,
    elements: Vec<Element>,
    node_ids: HashMap<String, NodeId>,
    element_ids: HashMap<String, ElementId>,
}

impl Circuit {
    pub fn title(&self) -> &str {
        &self.title
    }

    /// ground first, followed by non-ground nodes in first-appearance order
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn non_ground_nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.iter().skip(1)
    }

    /// elements in netlist order, including their source locations
    pub fn elements(&self) -> &[Element] {
        &self.elements
    }

    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.0)
    }

    pub fn element(&self, id: ElementId) -> Option<&Element> {
        self.elements.get(id.0)
    }

    pub fn find_node(&self, name: &str) -> Option<NodeId> {
        self.node_ids.get(&name_key(name)).copied()
    }

    pub fn find_element(&self, name: &str) -> Option<ElementId> {
        self.element_ids.get(&name_key(name)).copied()
    }
}

impl fmt::Debug for Circuit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Circuit")
            .field("title", &self.title)
            .field("nodes", &self.nodes)
            .field("elements", &self.elements)
            .finish()
    }
}

pub(crate) fn name_key(name: &str) -> String {
    name.to_ascii_lowercase()
}
