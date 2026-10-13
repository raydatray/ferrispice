use std::collections::HashMap;

use ferrispice_netlist::{ParsedElement, ParsedKind, ParsedNetlist};

use crate::model::name_key;
use crate::{
    Circuit, CircuitError, CircuitErrorKind, Element, ElementId, ElementKind, GROUND, Node, NodeId,
};

/// resolve names without allocating any matrix indices
pub fn compile(parsed: &ParsedNetlist) -> Result<Circuit, CircuitError> {
    let _span = tracing::debug_span!("compile_circuit", elements = parsed.elements.len()).entered();

    let registry = Registry::build(parsed)?;
    let elements = parsed
        .elements
        .iter()
        .enumerate()
        .map(|(index, element)| {
            let kind = resolve_kind(element, &registry)?;

            Ok(Element {
                id: ElementId(index),
                name: element.name.clone(),
                positive: registry.node_id(&element.positive),
                negative: registry.node_id(&element.negative),
                kind,
                location: element.location,
                span: element.span,
            })
        })
        .collect::<Result<Vec<_>, CircuitError>>()?;

    for element in &elements {
        tracing::trace!(element = element.name, id = ?element.id, "resolved element");
    }

    tracing::debug!(
        nodes = registry.nodes.len(),
        elements = elements.len(),
        "compiled circuit"
    );

    Ok(Circuit::new(
        parsed.title.clone(),
        registry.nodes,
        elements,
        registry.node_ids,
        registry.element_ids,
    ))
}

struct Registry {
    nodes: Vec<Node>,
    node_ids: HashMap<String, NodeId>,
    element_ids: HashMap<String, ElementId>,
}

impl Registry {
    fn build(parsed: &ParsedNetlist) -> Result<Self, CircuitError> {
        let mut registry = Self {
            nodes: vec![Node {
                id: GROUND,
                name: "0".to_owned(),
            }],
            node_ids: HashMap::from([("0".to_owned(), GROUND)]),
            element_ids: HashMap::new(),
        };

        for (index, element) in parsed.elements.iter().enumerate() {
            validate_name(element, "element", &element.name)?;

            let key = name_key(&element.name);

            if let Some(previous) = registry.element_ids.get(&key) {
                return Err(error(
                    element,
                    CircuitErrorKind::DuplicateElement {
                        first: parsed.elements[previous.0].location,
                        first_span: parsed.elements[previous.0].span,
                    },
                ));
            }

            registry.element_ids.insert(key, ElementId(index));
            registry.register_node(element, &element.positive)?;
            registry.register_node(element, &element.negative)?;

            // voltage controlled elements need their nodes registered too
            if let ParsedKind::Vcvs {
                control_positive,
                control_negative,
                ..
            }
            | ParsedKind::Vccs {
                control_positive,
                control_negative,
                ..
            } = &element.kind
            {
                registry.register_node(element, control_positive)?;
                registry.register_node(element, control_negative)?;
            }
        }

        Ok(registry)
    }

    fn register_node(&mut self, element: &ParsedElement, name: &str) -> Result<(), CircuitError> {
        validate_name(element, "node", name)?;

        let key = name_key(name);

        if !self.node_ids.contains_key(&key) {
            let id = NodeId(self.nodes.len());

            self.nodes.push(Node {
                id,
                name: name.to_owned(),
            });

            self.node_ids.insert(key, id);
        }

        Ok(())
    }

    fn node_id(&self, name: &str) -> NodeId {
        *self
            .node_ids
            .get(&name_key(name))
            .expect("all terminal nodes were registered in the first pass")
    }

    fn controller_id(
        &self,
        element: &ParsedElement,
        name: &str,
    ) -> Result<ElementId, CircuitError> {
        self.element_ids
            .get(&name_key(name))
            .copied()
            .ok_or_else(|| {
                error(
                    element,
                    CircuitErrorKind::UnknownController {
                        name: name.to_owned(),
                    },
                )
            })
    }
}

fn resolve_kind(element: &ParsedElement, registry: &Registry) -> Result<ElementKind, CircuitError> {
    Ok(match &element.kind {
        ParsedKind::Resistor { resistance } => ElementKind::Resistor {
            resistance: *resistance,
        },
        ParsedKind::Capacitor { capacitance } => ElementKind::Capacitor {
            capacitance: *capacitance,
        },
        ParsedKind::Inductor { inductance } => ElementKind::Inductor {
            inductance: *inductance,
        },
        ParsedKind::VoltageSource { dc } => ElementKind::VoltageSource { dc: *dc },
        ParsedKind::CurrentSource { dc } => ElementKind::CurrentSource { dc: *dc },
        ParsedKind::Vcvs {
            control_positive,
            control_negative,
            gain,
        } => ElementKind::Vcvs {
            control_positive: registry.node_id(control_positive),
            control_negative: registry.node_id(control_negative),
            gain: *gain,
        },
        ParsedKind::Vccs {
            control_positive,
            control_negative,
            transconductance,
        } => ElementKind::Vccs {
            control_positive: registry.node_id(control_positive),
            control_negative: registry.node_id(control_negative),
            transconductance: *transconductance,
        },
        ParsedKind::Ccvs {
            control,
            transresistance,
        } => ElementKind::Ccvs {
            control: registry.controller_id(element, control)?,
            transresistance: *transresistance,
        },
        ParsedKind::Cccs { control, gain } => ElementKind::Cccs {
            control: registry.controller_id(element, control)?,
            gain: *gain,
        },
    })
}

fn validate_name(
    element: &ParsedElement,
    field: &'static str,
    name: &str,
) -> Result<(), CircuitError> {
    if name.is_empty() || name.chars().any(char::is_whitespace) {
        return Err(error(
            element,
            CircuitErrorKind::InvalidName {
                field,
                name: name.to_owned(),
            },
        ));
    }

    Ok(())
}

fn error(element: &ParsedElement, kind: CircuitErrorKind) -> CircuitError {
    CircuitError {
        location: element.location,
        span: element.span,
        element: element.name.clone(),
        kind,
    }
}
