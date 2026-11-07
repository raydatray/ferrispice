mod builder;
mod error;
mod layout;
mod stamps;

use ferrispice_circuit::{Circuit, ElementKind};
use nalgebra::{DMatrix, DVector};

pub use builder::MatrixBuilder;
pub use error::{AssemblyError, AssemblyErrorKind};
pub use layout::{Unknown, UnknownKind, UnknownLayout};
pub use stamps::diode::{DiodeOperatingPoint, DiodeStamp, NonFiniteDiode, diode_operating_point};

/// the circuit equation `G x + C dx/dt + f(x) = b`
///
/// `G`, `C`, and `b` are constant. `f(x)` holds the currents of nonlinear
/// devices leaving each node; it depends on `x`, so it is represented by the
/// devices that produce it
#[derive(Clone, Debug, PartialEq)]
pub struct MnaSystem {
    pub g: DMatrix<f64>,
    pub c: DMatrix<f64>,
    pub b: DVector<f64>,
    pub layout: UnknownLayout,
    /// devices contributing to `f(x)`, in circuit element order
    pub nonlinear: Vec<NonlinearDevice>,
}

/// a device whose current depends nonlinearly on the unknowns
#[derive(Clone, Debug, PartialEq)]
pub enum NonlinearDevice {
    Diode(DiodeStamp),
}

impl MnaSystem {
    /// whether `f(x)` is identically zero, so `G x = b` describes DC exactly
    pub fn is_linear(&self) -> bool {
        self.nonlinear.is_empty()
    }
}

/// allocate every unknown, then assemble without solving
pub fn assemble(circuit: &Circuit) -> Result<MnaSystem, AssemblyError> {
    let _span = tracing::debug_span!("assemble_mna", elements = circuit.elements().len()).entered();

    let mut builder = MatrixBuilder::new(UnknownLayout::new(circuit));

    for element in circuit.elements() {
        match element.kind {
            ElementKind::Resistor { resistance } => {
                stamps::passive::resistor(&mut builder, element, resistance)
            }
            ElementKind::Capacitor { capacitance } => {
                stamps::passive::capacitor(&mut builder, element, capacitance)
            }
            ElementKind::Inductor { inductance } => {
                stamps::passive::inductor(&mut builder, element, inductance)?
            }
            ElementKind::CurrentSource { dc } => {
                stamps::independent::current_source(&mut builder, element, dc)
            }
            ElementKind::VoltageSource { dc } => {
                stamps::independent::voltage_source(&mut builder, element, dc)?
            }
            ElementKind::Vcvs {
                control_positive,
                control_negative,
                gain,
            } => stamps::controlled::vcvs(
                &mut builder,
                element,
                control_positive,
                control_negative,
                gain,
            )?,
            ElementKind::Vccs {
                control_positive,
                control_negative,
                transconductance,
            } => stamps::controlled::vccs(
                &mut builder,
                element,
                control_positive,
                control_negative,
                transconductance,
            ),
            ElementKind::Ccvs {
                control,
                transresistance,
            } => stamps::controlled::ccvs(&mut builder, element, control, transresistance)?,
            ElementKind::Cccs { control, gain } => {
                stamps::controlled::cccs(&mut builder, element, control, gain)?
            }
            ElementKind::Diode {
                saturation_current,
                thermal_voltage,
            } => stamps::diode::diode(&mut builder, element, saturation_current, thermal_voltage),
        }
        tracing::trace!(element = element.name, "stamped element");
    }

    let system = builder.finish();

    tracing::debug!(unknowns = system.layout.len(), "assembled MNA system");

    Ok(system)
}
