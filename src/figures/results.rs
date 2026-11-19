use super::{
    Result,
    matplotlib::{Matplotlib, object},
};
use serde_json::json;
use std::{ffi::OsStr, io, path::Path};

// Quoted from the paper's Table 1; upstream shares these rows with the teaser.
struct Row {
    agent: &'static str,
    model: &'static str,
    cost: f64,
    sd: f64,
    base: f64,
    te: f64,
}
const ROWS: [Row; 5] = [
    Row {
        agent: "Claude Code",
        model: "Sonnet 5",
        cost: -23.6,
        sd: 1.8,
        base: 74.7,
        te: 76.0,
    },
    Row {
        agent: "Claude Code",
        model: "Opus 4.8",
        cost: -30.9,
        sd: 2.1,
        base: 74.0,
        te: 73.0,
    },
    Row {
        agent: "Codex",
        model: "GPT-5.6 Terra",
        cost: -24.7,
        sd: 1.2,
        base: 73.8,
        te: 74.7,
    },
    Row {
        agent: "Codex",
        model: "GPT-5.6 Sol",
        cost: -26.1,
        sd: 2.1,
        base: 80.1,
        te: 81.7,
    },
    Row {
        agent: "Copilot CLI",
        model: "Sonnet 5",
        cost: -18.4,
        sd: 1.5,
        base: 75.0,
        te: 77.6,
    },
];
const TE: &str = "#285E9F";
const DEFAULT: &str = "#8A939C";
const INK: &str = "#132031";
const MUTED: &str = "#5D6978";
const GRID: &str = "#E6EAEF";
const AXIS: &str = "#A9B2BD";
const SURFACE: &str = "#FFFFFF";
const FS_TITLE: f64 = 8.6;
const FS_LABEL: f64 = 8.0;
const FS_VALUE: f64 = 7.6;
const FS_TICK: f64 = 7.0;
const FS_NOTE: f64 = 6.6;
const BAR_H: f64 = 0.46;
const DOT: f64 = 6.2;
const RING: f64 = 1.3;
const STEM: f64 = 1.4;

fn style_axis(plot: &mut Matplotlib, axis: &str) -> Result<()> {
    for side in ["top", "right", "left"] {
        plot.call(
            &format!("{axis}.spines.{side}.set_visible"),
            json!([false]),
            json!({}),
        )?;
    }
    plot.call(
        &format!("{axis}.spines.bottom.set_color"),
        json!([AXIS]),
        json!({}),
    )?;
    plot.call(
        &format!("{axis}.spines.bottom.set_linewidth"),
        json!([0.6]),
        json!({}),
    )?;
    plot.call(
        &format!("{axis}.tick_params"),
        json!([]),
        json!({"axis":"x","colors":MUTED,"labelsize":FS_TICK,"length":0,"pad":3}),
    )?;
    plot.call(
        &format!("{axis}.tick_params"),
        json!([]),
        json!({"axis":"y","length":0}),
    )?;
    plot.call(
        &format!("{axis}.grid"),
        json!([]),
        json!({"axis":"x","color":GRID,"linewidth":0.6,"zorder":0}),
    )?;
    plot.call(&format!("{axis}.set_axisbelow"), json!([true]), json!({}))?;
    Ok(())
}
fn rounded_bar(plot: &mut Matplotlib, figure: &str, axis: &str, y: i32, length: f64) -> Result<()> {
    // Convert the four-pixel corner radius to data units at the renderer's current DPI.
    plot.call(&format!("{figure}.canvas.draw"), json!([]), json!({}))?;
    let points = plot.call(
        &format!("{axis}.transData.transform"),
        json!([[[0, 0], [1, 1]]]),
        json!({}),
    )?;
    let points = plot.value(&points)?;
    let coordinate = |row, col| {
        points[row][col]
            .as_f64()
            .ok_or_else(|| io::Error::other("Matplotlib returned invalid transformed coordinates"))
    };
    let px_per_x = (coordinate(1, 0)? - coordinate(0, 0)?).abs();
    let px_per_y = (coordinate(1, 1)? - coordinate(0, 1)?).abs();
    let radius = 4.0 / px_per_x;
    let patch = plot.call("FancyBboxPatch", json!([[-2.0*radius, f64::from(y)-BAR_H/2.0],length+2.0*radius,BAR_H]),
        json!({"boxstyle":format!("round,pad=0,rounding_size={radius}"),"mutation_aspect":px_per_x/px_per_y,"facecolor":TE,"edgecolor":"none","zorder":3}))?;
    plot.call(
        &format!("{axis}.add_patch"),
        json!([object(&patch)]),
        json!({}),
    )?;
    Ok(())
}

pub fn render(out: &Path, python: &OsStr) -> Result<()> {
    let mut plot = Matplotlib::new(python)?;
    plot.call(
        "plt.rcParams.update",
        json!([{"font.family":"DejaVu Sans"}]),
        json!({}),
    )?;
    let figure = plot.call(
        "plt.figure",
        json!([]),
        json!({"figsize":[8.0,2.95],"facecolor":SURFACE}),
    )?;
    let left = plot.call(
        &format!("{figure}.add_axes"),
        json!([[0.205, 0.24, 0.42, 0.58]]),
        json!({}),
    )?;
    let right = plot.call(
        &format!("{figure}.add_axes"),
        json!([[0.665, 0.24, 0.25, 0.58]]),
        json!({"sharey":object(&left)}),
    )?;
    let n = ROWS.len() as i32;
    let ys: Vec<_> = (0..n).rev().collect();
    style_axis(&mut plot, &left)?;
    plot.call(&format!("{left}.set_xlim"), json!([0, 37]), json!({}))?;
    plot.call(
        &format!("{left}.set_ylim"),
        json!([-0.6, f64::from(n) - 0.4]),
        json!({}),
    )?;
    plot.call(
        &format!("{left}.set_xticks"),
        json!([[0, 10, 20, 30], ["0%", "10%", "20%", "30%"]]),
        json!({}),
    )?;
    let labels: Vec<_> = ROWS
        .iter()
        .map(|row| format!("{} · {}", row.agent, row.model))
        .collect();
    plot.call(
        &format!("{left}.set_yticks"),
        json!([ys, labels]),
        json!({"fontsize":FS_LABEL,"color":INK}),
    )?;
    plot.call(
        &format!("{left}.axvline"),
        json!([0]),
        json!({"color":AXIS,"linewidth":0.8,"zorder":4}),
    )?;
    for (&y, row) in ys.iter().zip(&ROWS) {
        let drop = -row.cost;
        rounded_bar(&mut plot, &figure, &left, y, drop)?;
        plot.call(
            &format!("{left}.plot"),
            json!([[drop - row.sd, drop + row.sd], [y, y]]),
            json!({"color":INK,"linewidth":0.7,"solid_capstyle":"butt","zorder":4}),
        )?;
        plot.call(&format!("{left}.text"), json!([drop+row.sd+0.8,y,format!("{drop:.1}%")]), json!({"va":"center","ha":"left","fontsize":FS_VALUE,"color":INK,"fontweight":"bold","zorder":5}))?;
    }
    plot.call(
        &format!("{left}.set_title"),
        json!(["Mean cost per task, lower than default"]),
        json!({"loc":"left","fontsize":FS_TITLE,"color":INK,"fontweight":"bold","pad":8}),
    )?;
    style_axis(&mut plot, &right)?;
    plot.call(&format!("{right}.set_xlim"), json!([71.5, 83.5]), json!({}))?;
    plot.call(
        &format!("{right}.set_xticks"),
        json!([[72, 76, 80], ["72", "76", "80"]]),
        json!({}),
    )?;
    plot.call(
        &format!("{right}.tick_params"),
        json!([]),
        json!({"axis":"y","labelleft":false}),
    )?;
    for (&y, row) in ys.iter().zip(&ROWS) {
        plot.call(
            &format!("{right}.plot"),
            json!([[row.base, row.te], [y, y]]),
            json!({"color":AXIS,"linewidth":STEM,"zorder":2,"solid_capstyle":"round"}),
        )?;
        for (x, color) in [(row.base, DEFAULT), (row.te, TE)] {
            plot.call(&format!("{right}.plot"), json!([[x],[y]]), json!({"marker":"o","markersize":DOT,"color":color,"markeredgecolor":SURFACE,"markeredgewidth":RING,"zorder":3}))?;
        }
        let transform = plot.call(
            &format!("{right}.get_yaxis_transform"),
            json!([]),
            json!({}),
        )?;
        plot.call(&format!("{right}.text"), json!([1.06,y,format!("{:+.1}",row.te-row.base).replace('-',"−")]), json!({"transform":object(&transform),"va":"center","ha":"left","fontsize":FS_VALUE,"color":INK,"fontweight":"bold"}))?;
    }
    let transform = plot.call(
        &format!("{right}.get_yaxis_transform"),
        json!([]),
        json!({}),
    )?;
    plot.call(&format!("{right}.text"), json!([1.06,f64::from(n)-0.4,"points"]), json!({"transform":object(&transform),"va":"bottom","ha":"left","fontsize":FS_NOTE,"color":MUTED}))?;
    plot.call(
        &format!("{right}.set_title"),
        json!(["Pass@1 (%)"]),
        json!({"loc":"left","fontsize":FS_TITLE,"color":INK,"fontweight":"bold","pad":8}),
    )?;
    let mut handles = Vec::new();
    for (color, label) in [(DEFAULT, "default"), (TE, "TokenEconomizer")] {
        let handle = plot.call("Line2D", json!([[],[]]), json!({"marker":"o","linestyle":"none","markersize":DOT,"color":color,"markeredgecolor":SURFACE,"markeredgewidth":RING,"label":label}))?;
        handles.push(object(&handle));
    }
    plot.call(&format!("{figure}.legend"), json!([]), json!({"handles":handles,"loc":"center left","bbox_to_anchor":[0.655,0.09],"ncol":2,"frameon":false,"fontsize":FS_TICK,"labelcolor":INK,"handletextpad":0.2,"columnspacing":1.0,"borderaxespad":0}))?;
    plot.call(
        &format!("{figure}.text"),
        json!([
            0.035,
            0.09,
            "SWE-Bench: 277 tasks from 6 repositories, each run in order. Whiskers: ±1 s.d."
        ]),
        json!({"fontsize":FS_NOTE,"color":MUTED,"ha":"left","va":"center"}),
    )?;
    std::fs::create_dir_all(out)?;
    plot.call(
        &format!("{figure}.savefig"),
        json!([out.join("results_overview.png")]),
        json!({"dpi":300,"facecolor":SURFACE,"bbox_inches":"tight","pad_inches":0.12}),
    )?;
    plot.call(&format!("{figure}.savefig"), json!([out.join("results_overview.svg")]), json!({"facecolor":SURFACE,"metadata":{"Date":null},"bbox_inches":"tight","pad_inches":0.12}))?;
    Ok(())
}
