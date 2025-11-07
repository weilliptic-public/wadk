use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct AxisData {
    label: String,
}

#[derive(Serialize, Deserialize)]
struct Series {
    label: String,
    data: Vec<(f32, f32)>,
}

/// A graph plotting data-structure providing APIs to add sequence of two dimensional data-points.
/// This plottable then can be consumed by the MCP clients like Weilliptic Icarus chatbot to plot the
/// graph using the data.
#[derive(Serialize, Deserialize)]
pub struct Plottable {
    #[serde(rename = "type")]
    ty: String,
    label: String,
    #[serde(rename = "xAxis")]
    x_axis: AxisData,
    #[serde(rename = "yAxis")]
    y_axis: AxisData,
    series: Vec<Series>,
}

impl Plottable {
    /// returns new time series `Plottable` whose x axis is timestamp.
    pub fn new_with_time_series() -> Self {
        Plottable {
            ty: "time_series".to_string(),
            label: String::default(),
            x_axis: AxisData {
                label: String::default(),
            },
            y_axis: AxisData {
                label: String::default(),
            },
            series: vec![],
        }
    }

    /// returns new generic `Plottable` whose x and y axis can represent any entities.
    pub fn new_with_graph() -> Self {
        Plottable {
            ty: "graph".to_string(),
            label: String::default(),
            x_axis: AxisData {
                label: String::default(),
            },
            y_axis: AxisData {
                label: String::default(),
            },
            series: vec![],
        }
    }

    /// builder method to add x axis label.
    pub fn x_axis_label(mut self, name: String) -> Self {
        self.x_axis = AxisData { label: name };

        self
    }

    /// builder method to add y axis label.
    pub fn y_axis_label(mut self, name: String) -> Self {
        self.y_axis = AxisData { label: name };

        self
    }

    /// builder method to add label to the graph.
    pub fn label(mut self, label: String) -> Self {
        self.label = label;

        self
    }

    /// builder method to add series with label and sequence of two dimensional data points.
    pub fn add_series(&mut self, label: String, data: Vec<(f32, f32)>) {
        self.series.push(Series { label, data });
    }
}
