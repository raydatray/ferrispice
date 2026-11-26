mod builder;
mod error;
mod layout;
mod stamps;

use ferrispice_circuit::{Circuit, ElementId, ElementKind, Waveform};
use nalgebra::{DMatrix, DVector};

pub use builder::MatrixBuilder;
pub use error::{AssemblyError, AssemblyErrorKind, DeviceError};
pub use layout::{Unknown, UnknownKind, UnknownLayout};
pub use stamps::diode::{
    DiodeCurrentAndDerivative, DiodeStamp, NonFiniteDiode, diode_current_and_derivative,
};

/// the circuit equation `G x + C dx/dt + f(x) = b(t)`
///
/// `G` and `C` are constant. `f(x)` holds the currents of nonlinear devices
/// leaving each node, and `b(t)` the independent sources; each depends on
/// something that changes, so each is represented by what produces it: `f`
/// by its devices and `b` by its source terms
#[derive(Clone, Debug, PartialEq)]
pub struct MnaSystem {
    pub g: DMatrix<f64>,
    pub c: DMatrix<f64>,
    /// every independent source's contributions to `b(t)`, in circuit element
    /// order; the only record of `b`
    pub sources: Vec<SourceTerm>,
    pub layout: UnknownLayout,
    /// devices contributing to `f(x)`, in circuit element order
    pub nonlinear: Vec<NonlinearDevice>,
}

/// a device whose current depends nonlinearly on the unknowns
#[derive(Clone, Debug, PartialEq)]
pub enum NonlinearDevice {
    Diode(DiodeStamp),
}

/// one signed contribution of an independent source to a row of `b(t)`
#[derive(Clone, Debug, PartialEq)]
pub struct SourceTerm {
    pub element: ElementId,
    /// matrix index; ground rows are never recorded
    pub row: usize,
    /// `+1` or `-1`
    pub sign: f64,
    pub waveform: Waveform,
}

/// one signed device current in a row of `f(x)`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FTerm {
    pub element: ElementId,
    pub name: String,
    /// `+` for current leaving the row's node, `-` for current entering it
    pub sign: TermSign,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TermSign {
    Positive,
    Negative,
}

impl MnaSystem {
    /// `b(t)`: the right-hand side with every source evaluated at `t`
    pub fn b_at(&self, t: f64) -> DVector<f64> {
        let mut b = DVector::zeros(self.layout.len());

        for term in &self.sources {
            b[term.row] += term.sign * term.waveform.value_at(t);
        }

        b
    }

    /// `b(0)`, the right-hand side DC analysis uses
    pub fn b(&self) -> DVector<f64> {
        self.b_at(0.0)
    }

    /// whether `b(t)` is the same at every time
    pub fn is_time_invariant(&self) -> bool {
        self.sources.iter().all(|term| term.waveform.is_constant())
    }

    /// whether `f(x)` is identically zero, so `G x = b` describes DC exactly
    pub fn is_linear(&self) -> bool {
        self.nonlinear.is_empty()
    }

    /// nonlinear device currents leaving each node at `x`
    ///
    /// `x` must have one entry per unknown in `layout`
    pub fn f(&self, x: &DVector<f64>) -> Result<DVector<f64>, DeviceError> {
        let mut f = DVector::zeros(self.layout.len());

        for device in &self.nonlinear {
            match device {
                NonlinearDevice::Diode(diode) => {
                    let value = diode.current_and_derivative(x)?;
                    add_entry(&mut f, diode.anode, value.current);
                    add_entry(&mut f, diode.cathode, -value.current);
                }
            }
        }

        Ok(f)
    }

    /// Jacobian `∂f/∂x` at `x`, from each device's analytic derivative
    ///
    /// a diode adds its derivative `d id / d vd` in the same four-entry
    /// pattern as a resistor
    pub fn df_dx(&self, x: &DVector<f64>) -> Result<DMatrix<f64>, DeviceError> {
        let size = self.layout.len();
        let mut jacobian = DMatrix::zeros(size, size);

        for device in &self.nonlinear {
            match device {
                NonlinearDevice::Diode(diode) => {
                    let derivative = diode.current_and_derivative(x)?.derivative;
                    let (anode, cathode) = (diode.anode, diode.cathode);
                    add_matrix_entry(&mut jacobian, anode, anode, derivative);
                    add_matrix_entry(&mut jacobian, anode, cathode, -derivative);
                    add_matrix_entry(&mut jacobian, cathode, anode, -derivative);
                    add_matrix_entry(&mut jacobian, cathode, cathode, derivative);
                }
            }
        }

        Ok(jacobian)
    }

    /// the device terms in each row of `f(x)`, for inspection
    ///
    /// rows follow `layout`; branch rows and rows without devices are empty,
    /// and a diode whose terminals share a node contributes nothing
    pub fn describe_f(&self) -> Vec<Vec<FTerm>> {
        let mut rows = vec![Vec::new(); self.layout.len()];

        for device in &self.nonlinear {
            match device {
                NonlinearDevice::Diode(diode) => {
                    if diode.anode == diode.cathode {
                        continue;
                    }

                    for (index, sign) in [
                        (diode.anode, TermSign::Positive),
                        (diode.cathode, TermSign::Negative),
                    ] {
                        if let Some(index) = index {
                            rows[index].push(FTerm {
                                element: diode.element,
                                name: diode.name.clone(),
                                sign,
                            });
                        }
                    }
                }
            }
        }

        rows
    }
}

fn add_entry(vector: &mut DVector<f64>, row: Option<usize>, value: f64) {
    if let Some(row) = row {
        vector[row] += value;
    }
}

fn add_matrix_entry(
    matrix: &mut DMatrix<f64>,
    row: Option<usize>,
    column: Option<usize>,
    value: f64,
) {
    if let (Some(row), Some(column)) = (row, column) {
        matrix[(row, column)] += value;
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
            ElementKind::CurrentSource { waveform } => {
                stamps::independent::current_source(&mut builder, element, waveform)
            }
            ElementKind::VoltageSource { waveform } => {
                stamps::independent::voltage_source(&mut builder, element, waveform)?
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
