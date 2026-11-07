use ferrispice_circuit::{Element, ElementId};
use nalgebra::DVector;
use thiserror::Error;

use crate::{DeviceError, MatrixBuilder, NonlinearDevice};

/// a diode's contribution to `f(x)`, with terminals resolved to matrix indices
#[derive(Clone, Debug, PartialEq)]
pub struct DiodeStamp {
    /// the diode in the circuit, which owns its source location
    pub element: ElementId,
    /// element name, for labels and error messages
    pub name: String,
    /// matrix index of the anode voltage; `None` for ground
    pub anode: Option<usize>,
    /// matrix index of the cathode voltage; `None` for ground
    pub cathode: Option<usize>,
    /// `Is`, in amperes
    pub saturation_current: f64,
    /// `Vt`, in volts
    pub thermal_voltage: f64,
}

impl DiodeStamp {
    /// junction voltage `vd = v[anode] - v[cathode]`; ground is zero
    pub fn voltage(&self, x: &DVector<f64>) -> f64 {
        let voltage = |index: Option<usize>| index.map_or(0.0, |index| x[index]);
        voltage(self.anode) - voltage(self.cathode)
    }

    /// current and its derivative `d id / d vd` at the junction voltage given by `x`
    pub fn current_and_derivative(
        &self,
        x: &DVector<f64>,
    ) -> Result<DiodeCurrentAndDerivative, DeviceError> {
        diode_current_and_derivative(
            self.saturation_current,
            self.thermal_voltage,
            self.voltage(x),
        )
        .map_err(|cause| DeviceError {
            id: self.element,
            element: self.name.clone(),
            cause,
        })
    }
}

/// record the diode for `f(x)`; it adds nothing to `G`, `C`, or `b`
pub(crate) fn diode(
    builder: &mut MatrixBuilder,
    element: &Element,
    saturation_current: f64,
    thermal_voltage: f64,
) {
    let anode = builder.node_index(element.positive);
    let cathode = builder.node_index(element.negative);

    builder.add_nonlinear(NonlinearDevice::Diode(DiodeStamp {
        element: element.id,
        name: element.name.clone(),
        anode,
        cathode,
        saturation_current,
        thermal_voltage,
    }));
}

/// diode current and its derivative at one junction voltage
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiodeCurrentAndDerivative {
    /// junction voltage `vd = v[anode] - v[cathode]`, in volts
    pub voltage: f64,
    /// `id = Is * (exp(vd / Vt) - 1)`, flowing from anode to cathode, in amperes
    pub current: f64,
    /// `d id / d vd = Is / Vt * exp(vd / Vt)`, in siemens; the diode's
    /// small-signal conductance `gd`, which Newton puts in the Jacobian
    pub derivative: f64,
}

/// the diode equation has no finite value at this junction voltage
///
/// `exp` overflows above an argument of about 709, so with `Vt = 0.025 V`
/// any junction voltage above roughly 17.7 V cannot be evaluated
#[derive(Clone, Copy, Debug, Error, PartialEq)]
#[error("diode equation is not finite at junction voltage {voltage} V")]
pub struct NonFiniteDiode {
    pub voltage: f64,
}

/// evaluate the diode current `id` and its derivative `d id / d vd`
/// without clamping
///
/// `exp_m1` keeps the current accurate near zero bias, where
/// `exp(vd / Vt) - 1` would cancel
pub fn diode_current_and_derivative(
    saturation_current: f64,
    thermal_voltage: f64,
    voltage: f64,
) -> Result<DiodeCurrentAndDerivative, NonFiniteDiode> {
    let argument = voltage / thermal_voltage;

    let value = DiodeCurrentAndDerivative {
        voltage,
        current: saturation_current * argument.exp_m1(),
        derivative: saturation_current / thermal_voltage * argument.exp(),
    };

    if !(value.voltage.is_finite() && value.current.is_finite() && value.derivative.is_finite()) {
        return Err(NonFiniteDiode { voltage });
    }

    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    const IS: f64 = 1e-15;
    const VT: f64 = 0.025;

    fn evaluate(voltage: f64) -> DiodeCurrentAndDerivative {
        diode_current_and_derivative(IS, VT, voltage).unwrap()
    }

    fn assert_relative(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance * expected.abs(),
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn matches_the_closed_form_in_reverse_zero_and_forward_bias() {
        for voltage in [-1.0, -0.1, 0.0, 0.3, 0.7, 0.75] {
            let value = evaluate(voltage);
            let exponential = (voltage / VT).exp();

            assert_eq!(value.voltage, voltage);
            assert_relative(value.derivative, IS / VT * exponential, 1e-12);
            if voltage != 0.0 {
                assert_relative(value.current, IS * (exponential - 1.0), 1e-9);
            }
        }

        let zero = evaluate(0.0);
        assert_eq!(zero.current, 0.0);
        assert_relative(zero.derivative, IS / VT, 1e-15);

        // strong reverse bias saturates at -Is with an almost zero derivative
        let reverse = evaluate(-5.0);
        assert_relative(reverse.current, -IS, 1e-12);
        assert!(reverse.derivative < 1e-100);
    }

    #[test]
    fn derivative_matches_a_finite_difference_of_the_current() {
        for voltage in [-0.2, 0.0, 0.1, 0.5, 0.7327] {
            let step = 1e-6;
            let numerical = (evaluate(voltage + step).current - evaluate(voltage - step).current)
                / (2.0 * step);

            assert_relative(evaluate(voltage).derivative, numerical, 1e-6);
        }
    }

    #[test]
    fn current_stays_accurate_near_zero_bias() {
        let voltage = 1e-12;
        let value = evaluate(voltage);

        // first-order expansion: Is * vd / Vt
        assert_relative(value.current, IS * voltage / VT, 1e-9);
        assert_relative(evaluate(-voltage).current, -IS * voltage / VT, 1e-9);
    }

    #[test]
    fn reports_overflowing_junction_voltages_instead_of_infinity() {
        assert!(diode_current_and_derivative(IS, VT, 17.0).is_ok());

        for voltage in [20.0, 100.0, f64::INFINITY, f64::NAN] {
            let error = diode_current_and_derivative(IS, VT, voltage).unwrap_err();

            assert!(error.voltage.is_nan() || error.voltage == voltage);
            assert!(error.to_string().contains("not finite"), "{error}");
        }
    }

    #[test]
    fn uses_the_supplied_parameters() {
        let value = diode_current_and_derivative(2e-14, 0.05, 0.1).unwrap();

        assert_relative(value.current, 2e-14 * (2.0f64.exp() - 1.0), 1e-12);
        assert_relative(value.derivative, 2e-14 / 0.05 * 2.0f64.exp(), 1e-12);
    }
}
