use std::path::Path;

use ferrispice_circuit::Node;
use ferrispice_solver::TransientSolution;
use plotters::prelude::*;

/// line colors, distinct on a white background
const COLORS: [RGBColor; 6] = [
    RGBColor(31, 119, 180),
    RGBColor(255, 127, 14),
    RGBColor(44, 160, 44),
    RGBColor(214, 39, 40),
    RGBColor(148, 103, 189),
    RGBColor(140, 86, 75),
];

/// backend coordinates are whole pixels, so a large canvas keeps 500-point
/// waveforms smooth; the SVG scales to any display size
const SIZE: (u32, u32) = (1600, 960);

/// write an SVG of `nodes`' voltages against time, titled
/// `Node voltages: <title>`, with a legend in the lower left
///
/// parent directories are created. Text is emitted as SVG `<text>`, so no
/// fonts are needed to draw it
pub(crate) fn write_svg(
    path: &Path,
    title: &str,
    solution: &TransientSolution,
    nodes: &[&Node],
) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let times = solution.times();
    let series: Vec<(&str, Vec<(f64, f64)>)> = nodes
        .iter()
        .map(|node| {
            let voltages = solution
                .node_voltage(node.id)
                .expect("plotted nodes belong to the solved circuit");
            let points = times.iter().copied().zip(voltages).collect();
            (node.name.as_str(), points)
        })
        .collect();

    let (low, high) = voltage_range(&series);
    let stop = times.last().copied().unwrap_or(0.0);

    let root = SVGBackend::new(path, SIZE).into_drawing_area();
    let error = |error: DrawingAreaErrorKind<_>| error.to_string();
    root.fill(&WHITE).map_err(error)?;

    let mut chart = ChartBuilder::on(&root)
        .caption(caption(title), ("sans-serif", 40))
        .margin(30)
        .x_label_area_size(80)
        .y_label_area_size(110)
        .build_cartesian_2d(0.0..stop, low..high)
        .map_err(error)?;

    chart
        .configure_mesh()
        .x_desc("Time [s]")
        .y_desc("Voltage [V]")
        .label_style(("sans-serif", 22))
        .axis_desc_style(("sans-serif", 26))
        .max_light_lines(1)
        .light_line_style(RGBColor(242, 242, 242))
        .bold_line_style(RGBColor(215, 215, 215))
        .draw()
        .map_err(error)?;

    for (index, (name, points)) in series.into_iter().enumerate() {
        let color = COLORS[index % COLORS.len()];
        chart
            .draw_series(LineSeries::new(points, color.stroke_width(3)))
            .map_err(error)?
            .label(format!("V({name})"))
            .legend(move |(x, y)| PathElement::new([(x, y), (x + 30, y)], color.stroke_width(3)));
    }

    chart
        .configure_series_labels()
        .position(SeriesLabelPosition::LowerLeft)
        .label_font(("sans-serif", 22))
        .margin(20)
        .background_style(WHITE.mix(0.9))
        .border_style(BLACK)
        .draw()
        .map_err(error)?;

    root.present().map_err(error)
}

/// `Node voltages: <title>`, without the `*` that often starts a netlist title
fn caption(title: &str) -> String {
    match title.trim().trim_start_matches('*').trim() {
        "" => "Node voltages".to_owned(),
        title => format!("Node voltages: {title}"),
    }
}

/// the plotted range: the data with a 10% margin above and below
fn voltage_range(series: &[(&str, Vec<(f64, f64)>)]) -> (f64, f64) {
    let (low, high) = series
        .iter()
        .flat_map(|(_, points)| points.iter().map(|&(_, voltage)| voltage))
        .fold(
            (f64::INFINITY, f64::NEG_INFINITY),
            |(low, high), voltage| (low.min(voltage), high.max(voltage)),
        );

    if !low.is_finite() {
        return (-1.0, 1.0);
    }

    let margin = if high > low {
        0.1 * (high - low)
    } else {
        0.1 * low.abs().max(1.0)
    };
    (low - margin, high + margin)
}

#[cfg(test)]
mod tests {
    use super::{caption, voltage_range};

    #[test]
    fn the_caption_names_the_circuit() {
        assert_eq!(
            caption("* a half-wave recitifier circuit"),
            "Node voltages: a half-wave recitifier circuit"
        );
        assert_eq!(caption("RC"), "Node voltages: RC");
        assert_eq!(caption("  *  "), "Node voltages");
    }

    #[test]
    fn the_range_has_a_margin_around_the_data() {
        let series = [
            ("n1", vec![(0.0, -1.0), (1.0, 1.0)]),
            ("n2", vec![(0.0, 0.5)]),
        ];
        let (low, high) = voltage_range(&series);
        assert!((low + 1.2).abs() < 1e-12 && (high - 1.2).abs() < 1e-12);

        let (low, high) = voltage_range(&[("flat", vec![(0.0, 0.0), (1.0, 0.0)])]);
        assert!(low < 0.0 && high > 0.0);
        assert_eq!(voltage_range(&[]), (-1.0, 1.0));
    }
}
