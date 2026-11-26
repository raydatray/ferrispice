use std::collections::HashMap;

use ferrispice_netlist::{Analysis, ParsedElement, ParsedKind, ParsedNetlist, ParsedWaveform};

use crate::model::name_key;
use crate::{
    Circuit, CircuitError, DEFAULT_SATURATION_CURRENT, DEFAULT_THERMAL_VOLTAGE, Element,
    ElementError, ElementErrorKind, ElementId, ElementKind, GROUND, Node, NodeId,
    TransientRequestError, TransientRequestErrorKind, Waveform,
};

/// resolve names and validate parameters without allocating any matrix indices
///
/// a `.tran` request is validated here too, so its times are usable by any
/// later analysis: both positive, and the step no longer than the stop time
///
/// A nonempty circuit must connect some element terminal to ground. Without one,
/// every node voltage lacks a reference and no analysis can succeed. Floating
/// subcircuits beside a grounded one are left to the solver.
pub fn compile(parsed: &ParsedNetlist) -> Result<Circuit, CircuitError> {
    let _span = tracing::debug_span!("compile_circuit", elements = parsed.elements.len()).entered();

    validate_transient(parsed)?;

    let registry = Registry::build(parsed)?;
    let elements = parsed
        .elements
        .iter()
        .enumerate()
        .map(|(index, element)| {
            let kind = resolve_kind(element, &registry)?;
            validate_parameters(element, &kind)?;

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
        .collect::<Result<Vec<_>, ElementError>>()?;

    for element in &elements {
        if let ElementKind::Ccvs { control, .. } | ElementKind::Cccs { control, .. } = element.kind
        {
            let controller = &elements[control.0];
            // current controlled elements need to reference a branch current
            if !controller.kind.has_branch_current() {
                return Err(ElementError {
                    location: element.location,
                    span: element.span,
                    element: element.name.clone(),
                    kind: ElementErrorKind::UnsupportedController {
                        name: controller.name.clone(),
                        kind: controller.kind.name(),
                    },
                }
                .into());
            }
        }

        tracing::trace!(element = element.name, id = ?element.id, "resolved element");
    }

    // one of the elements must reference a ground ndoe
    if !elements.is_empty()
        && !elements
            .iter()
            .any(|element| element.positive.is_ground() || element.negative.is_ground())
    {
        return Err(CircuitError::MissingGround);
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
    fn build(parsed: &ParsedNetlist) -> Result<Self, ElementError> {
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
                    ElementErrorKind::DuplicateElement {
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

    fn register_node(&mut self, element: &ParsedElement, name: &str) -> Result<(), ElementError> {
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
    ) -> Result<ElementId, ElementError> {
        self.element_ids
            .get(&name_key(name))
            .copied()
            .ok_or_else(|| {
                error(
                    element,
                    ElementErrorKind::UnknownController {
                        name: name.to_owned(),
                    },
                )
            })
    }
}

fn resolve_kind(element: &ParsedElement, registry: &Registry) -> Result<ElementKind, ElementError> {
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
        ParsedKind::VoltageSource { waveform } => ElementKind::VoltageSource {
            waveform: resolve_waveform(*waveform),
        },
        ParsedKind::CurrentSource { waveform } => ElementKind::CurrentSource {
            waveform: resolve_waveform(*waveform),
        },
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
        // the netlist has no diode parameters; every diode uses the defaults
        ParsedKind::Diode => ElementKind::Diode {
            saturation_current: DEFAULT_SATURATION_CURRENT,
            thermal_voltage: DEFAULT_THERMAL_VOLTAGE,
        },
    })
}

fn resolve_waveform(waveform: ParsedWaveform) -> Waveform {
    match waveform {
        ParsedWaveform::Dc(value) => Waveform::Dc(value),
        ParsedWaveform::Cos {
            amplitude,
            frequency,
        } => Waveform::Cos {
            amplitude,
            frequency,
        },
        ParsedWaveform::Sin {
            amplitude,
            frequency,
        } => Waveform::Sin {
            amplitude,
            frequency,
        },
    }
}

fn validate_transient(parsed: &ParsedNetlist) -> Result<(), TransientRequestError> {
    for request in &parsed.analyses {
        let Analysis::Tran { stop, step } = request.kind else {
            continue;
        };

        let kind = if stop.is_nan() || stop <= 0.0 {
            TransientRequestErrorKind::NonPositiveStop { stop }
        } else if step.is_nan() || step <= 0.0 {
            TransientRequestErrorKind::NonPositiveStep { step }
        } else if step > stop {
            TransientRequestErrorKind::StepExceedsStop { stop, step }
        } else {
            continue;
        };

        return Err(TransientRequestError {
            location: request.location,
            span: request.span,
            kind,
        });
    }

    Ok(())
}

fn validate_parameters(element: &ParsedElement, kind: &ElementKind) -> Result<(), ElementError> {
    match *kind {
        ElementKind::Resistor { resistance } => {
            finite(element, "resistance", resistance)?;
            if resistance == 0.0 {
                return Err(error(element, ElementErrorKind::ZeroResistance));
            }
        }
        ElementKind::Capacitor { capacitance } => nonnegative(element, "capacitance", capacitance)?,
        ElementKind::Inductor { inductance } => nonnegative(element, "inductance", inductance)?,
        ElementKind::VoltageSource { waveform } | ElementKind::CurrentSource { waveform } => {
            match waveform {
                Waveform::Dc(value) => finite(element, "DC value", value)?,
                Waveform::Cos {
                    amplitude,
                    frequency,
                } => validate_periodic(
                    element,
                    "COS amplitude",
                    "COS frequency",
                    amplitude,
                    frequency,
                )?,
                Waveform::Sin {
                    amplitude,
                    frequency,
                } => validate_periodic(
                    element,
                    "SIN amplitude",
                    "SIN frequency",
                    amplitude,
                    frequency,
                )?,
            }
        }
        ElementKind::Vcvs { gain, .. } | ElementKind::Cccs { gain, .. } => {
            finite(element, "gain", gain)?
        }
        ElementKind::Vccs {
            transconductance, ..
        } => finite(element, "transconductance", transconductance)?,
        ElementKind::Ccvs {
            transresistance, ..
        } => finite(element, "transresistance", transresistance)?,
        // diode parameters are fixed positive constants, not netlist values
        ElementKind::Diode { .. } => {}
    }
    Ok(())
}

fn validate_periodic(
    element: &ParsedElement,
    amplitude_label: &'static str,
    frequency_label: &'static str,
    amplitude: f64,
    frequency: f64,
) -> Result<(), ElementError> {
    finite(element, amplitude_label, amplitude)?;
    finite(element, frequency_label, frequency)?;
    if frequency <= 0.0 {
        return Err(error(
            element,
            ElementErrorKind::NonPositiveParameter {
                parameter: frequency_label,
                value: frequency,
            },
        ));
    }
    Ok(())
}

fn finite(
    element: &ParsedElement,
    parameter: &'static str,
    value: f64,
) -> Result<(), ElementError> {
    if !value.is_finite() {
        return Err(error(
            element,
            ElementErrorKind::NonFiniteParameter { parameter, value },
        ));
    }
    Ok(())
}

fn nonnegative(
    element: &ParsedElement,
    parameter: &'static str,
    value: f64,
) -> Result<(), ElementError> {
    finite(element, parameter, value)?;
    if value < 0.0 {
        return Err(error(
            element,
            ElementErrorKind::NegativeParameter { parameter, value },
        ));
    }
    Ok(())
}

fn validate_name(
    element: &ParsedElement,
    field: &'static str,
    name: &str,
) -> Result<(), ElementError> {
    if name.is_empty() || name.chars().any(char::is_whitespace) {
        return Err(error(
            element,
            ElementErrorKind::InvalidName {
                field,
                name: name.to_owned(),
            },
        ));
    }

    Ok(())
}

fn error(element: &ParsedElement, kind: ElementErrorKind) -> ElementError {
    ElementError {
        location: element.location,
        span: element.span,
        element: element.name.clone(),
        kind,
    }
}
