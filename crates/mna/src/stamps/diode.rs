use thiserror::Error;

/// diode current and its derivative at one junction voltage
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiodeOperatingPoint {
    /// junction voltage `vd = v[anode] - v[cathode]`, in volts
    pub voltage: f64,
    /// `id = Is * (exp(vd / Vt) - 1)`, flowing from anode to cathode, in amperes
    pub current: f64,
    /// `gd = d id / d vd = Is / Vt * exp(vd / Vt)`, in siemens
    pub conductance: f64,
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

/// evaluate the diode equation and its derivative without clamping
///
/// `exp_m1` keeps the current accurate near zero bias, where
/// `exp(vd / Vt) - 1` would cancel
pub fn diode_operating_point(
    saturation_current: f64,
    thermal_voltage: f64,
    voltage: f64,
) -> Result<DiodeOperatingPoint, NonFiniteDiode> {
    let argument = voltage / thermal_voltage;

    let point = DiodeOperatingPoint {
        voltage,
        current: saturation_current * argument.exp_m1(),
        conductance: saturation_current / thermal_voltage * argument.exp(),
    };

    if !(point.voltage.is_finite() && point.current.is_finite() && point.conductance.is_finite()) {
        return Err(NonFiniteDiode { voltage });
    }

    Ok(point)
}

#[cfg(test)]
mod tests {
    use super::*;

    const IS: f64 = 1e-15;
    const VT: f64 = 0.025;

    fn evaluate(voltage: f64) -> DiodeOperatingPoint {
        diode_operating_point(IS, VT, voltage).unwrap()
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
            let point = evaluate(voltage);
            let exponential = (voltage / VT).exp();

            assert_eq!(point.voltage, voltage);
            assert_relative(point.conductance, IS / VT * exponential, 1e-12);
            if voltage != 0.0 {
                assert_relative(point.current, IS * (exponential - 1.0), 1e-9);
            }
        }

        let zero = evaluate(0.0);
        assert_eq!(zero.current, 0.0);
        assert_relative(zero.conductance, IS / VT, 1e-15);

        // strong reverse bias saturates at -Is with almost no conductance
        let reverse = evaluate(-5.0);
        assert_relative(reverse.current, -IS, 1e-12);
        assert!(reverse.conductance < 1e-100);
    }

    #[test]
    fn conductance_is_the_derivative_of_the_current() {
        for voltage in [-0.2, 0.0, 0.1, 0.5, 0.7327] {
            let step = 1e-6;
            let derivative = (evaluate(voltage + step).current - evaluate(voltage - step).current)
                / (2.0 * step);

            assert_relative(evaluate(voltage).conductance, derivative, 1e-6);
        }
    }

    #[test]
    fn current_stays_accurate_near_zero_bias() {
        let voltage = 1e-12;
        let point = evaluate(voltage);

        // first-order expansion: Is * vd / Vt
        assert_relative(point.current, IS * voltage / VT, 1e-9);
        assert_relative(evaluate(-voltage).current, -IS * voltage / VT, 1e-9);
    }

    #[test]
    fn reports_overflowing_junction_voltages_instead_of_infinity() {
        assert!(diode_operating_point(IS, VT, 17.0).is_ok());

        for voltage in [20.0, 100.0, f64::INFINITY, f64::NAN] {
            let error = diode_operating_point(IS, VT, voltage).unwrap_err();

            assert!(error.voltage.is_nan() || error.voltage == voltage);
            assert!(error.to_string().contains("not finite"), "{error}");
        }
    }

    #[test]
    fn uses_the_supplied_parameters() {
        let point = diode_operating_point(2e-14, 0.05, 0.1).unwrap();

        assert_relative(point.current, 2e-14 * (2.0f64.exp() - 1.0), 1e-12);
        assert_relative(point.conductance, 2e-14 / 0.05 * 2.0f64.exp(), 1e-12);
    }
}
