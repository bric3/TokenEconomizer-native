use super::{
    Result,
    matplotlib::{Matplotlib, object},
};
use serde_json::{Value, json};
use std::{collections::HashMap, ffi::OsStr, io, path::Path};

const INK: &str = "#20303C";
const MUTED: &str = "#63727D";
const LINE: &str = "#C6D0D6";
const PANEL: &str = "#FBFCFD";
const SKILL: &str = "#3E73A8";
const SKILL_LIGHT: &str = "#DFEAF4";
const TOOLS: &str = "#BC7D14";
const TOOLS_LIGHT: &str = "#F7E8C9";
const SPIRAL: &str = "#BF4F4B";
const SPIRAL_LIGHT: &str = "#F4DCDA";
const NEUTRAL_FC: &str = "#EDF0F2";
const NEUTRAL_EC: &str = "#B7C2C9";
const DROPPED_FC: &str = "#EDEFF1";
const DROPPED_EC: &str = "#BAC4CA";
const DROPPED_TC: &str = "#98A4AC";
const MONO: &str = "DejaVu Sans Mono";
const ELLIPSIS: &str = "…";
const FS_PANEL: f64 = 6.0;
const FS_ROW: f64 = 5.6;
const FS_CHIP: f64 = 5.8;
const FS_BODY: f64 = 4.6;
const FS_SMALL: f64 = 4.4;
const FS_TAG: f64 = 4.5;
const FS_MONO: f64 = 4.1;
const FS_MARK: f64 = 4.3;
const FS_DOTS: f64 = 6.2;
const CTRL_X0: f64 = 0.50;
const CTRL_X1: f64 = 15.90;
const AGENT_X0: f64 = 17.60;
const AGENT_X1: f64 = 54.50;
const PANEL_Y0: f64 = 6.15;
const PANEL_Y1: f64 = 27.35;
const PAD: f64 = 0.85;
const AIN_X0: f64 = 19.20;
const AIN_X1: f64 = 53.20;
const CHIP_X0: f64 = CTRL_X0 + PAD;
const CHIP_X1: f64 = CTRL_X1 - PAD;
const HDR_TITLE_Y: f64 = 26.40;
const HDR_SUB_Y: f64 = 25.70;
const HDR_RULE: f64 = 23.85;
const ROW_TOP: f64 = 23.30;
const ROW_PITCH: f64 = 5.95;
const BAR_H: f64 = 1.50;
const CHIP_H: f64 = 4.30;
const LEARN_Y0: f64 = 0.25;
const LEARN_Y1: f64 = 4.55;
const LEG_Y: f64 = 2.40;
const LEG_X: f64 = 50.00;
const TOOL_GAP: f64 = 0.70;
type Mechanism = (
    i32,
    &'static str,
    &'static str,
    &'static str,
    [&'static str; 2],
);
const MECHANISMS: [Mechanism; 3] = [
    (
        1,
        "Micro-skills",
        SKILL,
        SKILL_LIGHT,
        [
            "repository facts learned from failures",
            "that recur across finished sessions",
        ],
    ),
    (
        2,
        "Tool pruning",
        TOOLS,
        TOOLS_LIGHT,
        [
            "tools left unused session after session",
            "are left out of the next sessions",
        ],
    ),
    (
        3,
        "Failure-loop intervention",
        SPIRAL,
        SPIRAL_LIGHT,
        [
            "learns how long turns here usually run;",
            "one redirect, then a notice to you",
        ],
    ),
];
fn bar_y(number: i32) -> f64 {
    ROW_TOP - f64::from(number - 1) * ROW_PITCH - 1.70 - BAR_H
}
fn axis_y(number: i32) -> f64 {
    bar_y(number) + BAR_H / 2.0
}
fn ensure(condition: bool, message: impl Into<String>) -> Result<()> {
    if !condition {
        return Err(io::Error::other(message.into()).into());
    }
    Ok(())
}
struct Drawing {
    plot: Matplotlib,
    figure: String,
    axis: String,
}
impl Drawing {
    fn text(&mut self, x: f64, y: f64, text: &str, options: Value) -> Result<()> {
        self.plot
            .call(&format!("{}.text", self.axis), json!([x, y, text]), options)?;
        Ok(())
    }
    fn line(&mut self, x: [f64; 2], y: [f64; 2], options: Value) -> Result<()> {
        self.plot
            .call(&format!("{}.plot", self.axis), json!([x, y]), options)?;
        Ok(())
    }
    fn patch(&mut self, kind: &str, args: Value, options: Value) -> Result<()> {
        let patch = self.plot.call(kind, args, options)?;
        self.plot.call(
            &format!("{}.add_patch", self.axis),
            json!([object(&patch)]),
            json!({}),
        )?;
        Ok(())
    }
    fn rounded(&mut self, xy: [f64; 2], width: f64, height: f64, mut options: Value) -> Result<()> {
        let options = options.as_object_mut().unwrap();
        let radius = options
            .remove("radius")
            .and_then(|value| value.as_f64())
            .unwrap_or(0.32);
        let mut defaults = json!({"boxstyle":format!("round,pad=0,rounding_size={radius}"),"facecolor":"white","edgecolor":LINE,"linewidth":0.7,"zorder":1,"alpha":1.0,"linestyle":"solid"});
        defaults.as_object_mut().unwrap().extend(options.clone());
        self.patch("FancyBboxPatch", json!([xy, width, height]), defaults)
    }
    fn arrow(&mut self, p0: [f64; 2], p1: [f64; 2], color: &str, zorder: i32) -> Result<()> {
        self.patch("FancyArrowPatch",json!([p0,p1]),json!({"arrowstyle":"-|>","mutation_scale":4.5,"linewidth":0.85,"color":color,"shrinkA":0,"shrinkB":0,"zorder":zorder}))
    }
    fn width_of(
        &mut self,
        text: &str,
        size: f64,
        weight: &str,
        family: Option<&str>,
    ) -> Result<f64> {
        // Placement and collision checks use the renderer's font metrics in drawing units.
        let probe = self.plot.call(
            &format!("{}.text", self.figure),
            json!([0, 0, text]),
            json!({"fontsize":size,"fontweight":weight,"family":family}),
        )?;
        let renderer = self.plot.call(
            &format!("{}.canvas.get_renderer", self.figure),
            json!([]),
            json!({}),
        )?;
        let extent = self.plot.call(
            &format!("{probe}.get_window_extent"),
            json!([]),
            json!({"renderer":object(&renderer)}),
        )?;
        self.plot
            .call(&format!("{probe}.remove"), json!([]), json!({}))?;
        let width = self
            .plot
            .value(&format!("{extent}.width"))?
            .as_f64()
            .ok_or("invalid renderer text width")?;
        let dpi = self
            .plot
            .value(&format!("{}.dpi", self.figure))?
            .as_f64()
            .ok_or("invalid renderer DPI")?;
        Ok(width / dpi * 10.0)
    }
    fn badge(
        &mut self,
        x: f64,
        y: f64,
        number: i32,
        color: &str,
        radius: f64,
        size: f64,
    ) -> Result<()> {
        self.patch(
            "Circle",
            json!([[x, y], radius]),
            json!({"facecolor":color,"edgecolor":"none","zorder":7}),
        )?;
        self.text(x,y,&number.to_string(),json!({"ha":"center","va":"center","fontsize":size,"color":"white","fontweight":"bold","zorder":8}))
    }
    fn tag(
        &mut self,
        right: f64,
        y: f64,
        text: &str,
        color: &str,
        light: &str,
        weight: &str,
    ) -> Result<f64> {
        let width = self.width_of(text, FS_TAG, weight, None)? + 1.10;
        self.rounded(
            [right - width, y - 0.62],
            width,
            1.24,
            json!({"facecolor":light,"edgecolor":"none","radius":0.62,"zorder":5}),
        )?;
        self.text(right-width/2.0,y,text,json!({"ha":"center","va":"center","fontsize":FS_TAG,"color":color,"fontweight":weight,"zorder":6}))?;
        Ok(right - width)
    }
    fn annotate(&mut self, number: i32, x0: f64, x1: f64, text: &str, color: &str) -> Result<()> {
        let y = bar_y(number) - 0.35;
        self.line([x0, x1], [y, y], json!({"color":color,"lw":0.7,"zorder":6}))?;
        for x in [x0, x1] {
            self.line(
                [x, x],
                [y, y + 0.36],
                json!({"color":color,"lw":0.7,"zorder":6}),
            )?;
        }
        self.text((x0+x1)/2.0,y-0.42,text,json!({"fontsize":FS_SMALL,"color":color,"ha":"center","va":"top","fontweight":"bold","zorder":6}))
    }
    fn controller(&mut self) -> Result<()> {
        self.rounded([CTRL_X0,PANEL_Y0],CTRL_X1-CTRL_X0,PANEL_Y1-PANEL_Y0,json!({"facecolor":PANEL,"edgecolor":"#9FADB6","linewidth":0.9,"radius":0.55,"zorder":1}))?;
        self.text(CHIP_X0,HDR_TITLE_Y,"PLUGIN",json!({"fontsize":FS_PANEL,"color":INK,"fontweight":"bold","ha":"left","va":"center","zorder":5}))?;
        self.text(CHIP_X0,HDR_SUB_Y,"what it knows about this repository,\nfrom the sessions before this one",json!({"fontsize":FS_BODY,"color":MUTED,"ha":"left","va":"top","linespacing":1.30,"zorder":5}))?;
        self.line(
            [CHIP_X0, CHIP_X1],
            [HDR_RULE; 2],
            json!({"color":LINE,"lw":0.6,"zorder":4}),
        )?;
        for (number, title, color, _, body) in MECHANISMS {
            let top = axis_y(number) + CHIP_H / 2.0;
            self.rounded([CHIP_X0,top-CHIP_H],CHIP_X1-CHIP_X0,CHIP_H,json!({"facecolor":"white","edgecolor":color,"linewidth":0.85,"radius":0.40,"zorder":3}))?;
            self.badge(CHIP_X0 + 0.75, top - 1.05, number, color, 0.72, 4.6)?;
            let room = CHIP_X1 - (CHIP_X0 + 1.80) - 0.30;
            let used = self.width_of(title, FS_CHIP, "bold", None)?;
            ensure(
                used <= room,
                format!("card {number} title is {used:.2} units, room {room:.2}"),
            )?;
            self.text(CHIP_X0+1.80,top-1.05,title,json!({"fontsize":FS_CHIP,"color":color,"fontweight":"bold","ha":"left","va":"center","zorder":5}))?;
            let body_room = CHIP_X1 - CHIP_X0 - 0.70;
            let mut body_used: f64 = 0.0;
            for line in body {
                body_used = body_used.max(self.width_of(line, FS_BODY, "normal", None)?);
            }
            ensure(
                body_used <= body_room,
                format!("card {number} body is {body_used:.2} units, room {body_room:.2}"),
            )?;
            self.text(CHIP_X0+0.35,top-2.25,&body.join("\n"),json!({"fontsize":FS_BODY,"color":MUTED,"ha":"left","va":"top","linespacing":1.30,"zorder":5}))?;
        }
        Ok(())
    }
    fn learners(&mut self) -> Result<()> {
        self.rounded([CTRL_X0,LEARN_Y0],CTRL_X1-CTRL_X0,LEARN_Y1-LEARN_Y0,json!({"facecolor":"white","edgecolor":"#9FADB6","linewidth":0.85,"radius":0.45,"zorder":3}))?;
        let header = "SESSION END";
        self.text(CHIP_X0,LEARN_Y1-1.00,header,json!({"fontsize":FS_ROW,"color":INK,"fontweight":"bold","ha":"left","va":"center","zorder":5}))?;
        let first_badge = CHIP_X1 - 0.58 - 2.0 * 1.45 - 0.58;
        ensure(
            CHIP_X0 + self.width_of(header, FS_ROW, "bold", None)? <= first_badge - 0.30,
            header,
        )?;
        for (i, (number, _, color, _, _)) in MECHANISMS.iter().enumerate() {
            self.badge(
                CHIP_X1 - 0.58 - (2 - i) as f64 * 1.45,
                LEARN_Y1 - 1.00,
                *number,
                color,
                0.58,
                4.0,
            )?;
        }
        self.text(CHIP_X0,LEARN_Y1-2.05,"its log updates what all three\nmechanisms know about the repository",json!({"fontsize":FS_BODY,"color":MUTED,"ha":"left","va":"top","linespacing":1.30,"zorder":5}))?;
        self.arrow(
            [CHIP_X0 + 1.55, LEARN_Y1 + 0.05],
            [CHIP_X0 + 1.55, PANEL_Y0 - 0.05],
            MUTED,
            7,
        )?;
        self.text(
            CHIP_X0 + 2.30,
            (LEARN_Y1 + PANEL_Y0) / 2.0,
            "for the next session",
            json!({"fontsize":FS_SMALL,"color":MUTED,"ha":"left","va":"center","zorder":7}),
        )
    }
    fn agent_header(&mut self) -> Result<()> {
        let title = "YOUR CODING AGENT";
        self.text(AIN_X0,HDR_TITLE_Y,title,json!({"fontsize":FS_PANEL,"color":INK,"fontweight":"bold","ha":"left","va":"center","zorder":5}))?;
        let note_x = AIN_X0 + self.width_of(title, FS_PANEL, "bold", None)? + 0.70;
        self.text(
            note_x,
            HDR_TITLE_Y,
            "(unchanged)",
            json!({"fontsize":FS_BODY,"color":MUTED,"ha":"left","va":"center","zorder":5}),
        )?;
        let left = self.tag(
            AIN_X1,
            HDR_TITLE_Y,
            "Claude Code  ·  Codex  ·  Copilot CLI  ·  Pi",
            INK,
            "#E7ECEF",
            "normal",
        )?;
        ensure(
            note_x + self.width_of("(unchanged)", FS_BODY, "normal", None)? <= left - 0.50,
            "header collides",
        )?;
        self.line(
            [AIN_X0, AIN_X1],
            [HDR_RULE; 2],
            json!({"color":LINE,"lw":0.6,"zorder":4}),
        )
    }
    fn row_title(
        &mut self,
        number: i32,
        eyebrow: &str,
        title: &str,
        color: &str,
        timing: &str,
        light: &str,
    ) -> Result<()> {
        let y = ROW_TOP - f64::from(number - 1) * ROW_PITCH - 0.65;
        self.badge(AIN_X0 + 0.55, y, number, color, 0.62, 4.2)?;
        let mut x = AIN_X0 + 1.50;
        self.text(x,y,eyebrow,json!({"fontsize":FS_ROW,"color":color,"fontweight":"bold","ha":"left","va":"center","zorder":5}))?;
        x += self.width_of(eyebrow, FS_ROW, "bold", None)? + 0.65;
        self.text(
            x,
            y,
            title,
            json!({"fontsize":FS_ROW,"color":INK,"ha":"left","va":"center","zorder":5}),
        )?;
        self.tag(AIN_X1, y, timing, color, light, "bold")?;
        Ok(())
    }
    fn prefix_row(&mut self) -> Result<()> {
        self.row_title(
            1,
            "KNOWLEDGE",
            "session context",
            SKILL,
            "at session start",
            SKILL_LIGHT,
        )?;
        let y = bar_y(1);
        let skill_x = AIN_X0 + 13.40;
        let skill_w = 10.40;
        for (x, w, fc, ec, label, tc, weight) in [
            (
                AIN_X0,
                13.40,
                NEUTRAL_FC,
                NEUTRAL_EC,
                "system prompt + your instructions",
                MUTED,
                "normal",
            ),
            (
                skill_x,
                skill_w,
                SKILL_LIGHT,
                SKILL,
                "+ learned micro-skills",
                SKILL,
                "bold",
            ),
            (
                skill_x + skill_w,
                AIN_X1 - skill_x - skill_w,
                "white",
                NEUTRAL_EC,
                "your prompt",
                MUTED,
                "normal",
            ),
        ] {
            self.patch(
                "Rectangle",
                json!([[x, y], w, BAR_H]),
                json!({"facecolor":fc,"edgecolor":ec,"linewidth":0.7,"zorder":4}),
            )?;
            ensure(
                self.width_of(label, FS_SMALL, weight, None)? <= w - 0.30,
                label,
            )?;
            self.text(x+w/2.0,y+BAR_H/2.0,label,json!({"fontsize":FS_SMALL,"color":tc,"ha":"center","va":"center","fontweight":weight,"zorder":5}))?;
        }
        self.annotate(1, skill_x, skill_x + skill_w, "active facts only", SKILL)
    }
    fn tools_row(&mut self) -> Result<()> {
        self.row_title(
            2,
            "INTERFACE",
            "loaded tool set",
            TOOLS,
            "from the next session",
            TOOLS_LIGHT,
        )?;
        let y = bar_y(2);
        let chips = [
            ("Bash", "kept"),
            ("Read", "kept"),
            ("Edit", "kept"),
            ("Write", "kept"),
            ("Grep", "kept"),
            ("Glob", "kept"),
            ("CronCreate", "dropped"),
            ("Workflow", "dropped"),
            ("NotebookEdit", "dropped"),
            (ELLIPSIS, "more"),
        ];
        let mut names = Vec::new();
        for (name, _) in chips {
            names.push(self.width_of(name, FS_MONO, "normal", Some(MONO))?);
        }
        let pad =
            (AIN_X1 - AIN_X0 - TOOL_GAP * (names.len() - 1) as f64 - names.iter().sum::<f64>())
                / names.len() as f64;
        ensure(
            pad >= 0.45,
            format!("tool names leave {pad:.2} units of padding"),
        )?;
        let mut x = AIN_X0;
        let mut dropped = Vec::new();
        for ((name, state), used) in chips.into_iter().zip(names) {
            let w = used + pad;
            if state == "more" {
                self.text(x+w/2.0,y+BAR_H/2.0,name,json!({"fontsize":FS_DOTS,"color":DROPPED_TC,"ha":"center","va":"center","zorder":5}))?;
                dropped.push((x, x + w));
                x += w + TOOL_GAP;
                continue;
            }
            let (fc, ec, tc) = if state == "kept" {
                (TOOLS_LIGHT, TOOLS, "#74500C")
            } else {
                dropped.push((x, x + w));
                (DROPPED_FC, DROPPED_EC, DROPPED_TC)
            };
            self.rounded(
                [x, y],
                w,
                BAR_H,
                json!({"facecolor":fc,"edgecolor":ec,"linewidth":0.7,"radius":0.26,"zorder":4}),
            )?;
            self.text(x+w/2.0,y+BAR_H/2.0,name,json!({"fontsize":FS_MONO,"color":tc,"family":MONO,"ha":"center","va":"center","zorder":5}))?;
            if state == "dropped" {
                self.line(
                    [x + 0.28, x + w - 0.28],
                    [y + BAR_H / 2.0; 2],
                    json!({"color":DROPPED_TC,"lw":0.6,"zorder":6}),
                )?;
            }
            x += w + TOOL_GAP;
        }
        self.annotate(
            2,
            dropped[0].0,
            dropped.last().unwrap().1,
            "left out, and announced",
            DROPPED_TC,
        )
    }
    fn rollout_row(&mut self) -> Result<()> {
        self.row_title(
            3,
            "CONTROL",
            "one prompt-turn",
            SPIRAL,
            "during each turn",
            SPIRAL_LIGHT,
        )?;
        let y = bar_y(3);
        let boxes = [
            (ELLIPSIS, "earlier", 1.40, None),
            ("edit", "repeat", 2.55, Some("loop")),
            ("test", "repeat", 2.55, Some("loop")),
            (ELLIPSIS, "more", 1.40, Some("loop")),
            ("REDIRECT", "redirect", 3.70, None),
            ("edit", "repeat", 2.55, Some("again")),
            ("test", "repeat", 2.55, Some("again")),
            (ELLIPSIS, "more", 1.40, Some("again")),
            ("NOTICE", "abort", 3.70, None),
            ("edit", "repeat", 2.55, Some("after")),
            ("test", "repeat", 2.55, Some("after")),
        ];
        let gap = (AIN_X1 - AIN_X0 - boxes.iter().map(|(_, _, w, _)| w).sum::<f64>())
            / (boxes.len() - 1) as f64;
        ensure(
            (0.35..=1.20).contains(&gap),
            format!("box widths leave a {gap:.2} unit gap"),
        )?;
        let mut spans: HashMap<&str, (f64, f64)> = HashMap::new();
        let mut x = AIN_X0;
        for (label, state, w, group) in boxes {
            if state == "earlier" || state == "more" {
                self.text(x+w/2.0,y+BAR_H/2.0,label,json!({"fontsize":FS_DOTS,"color":if state=="more" { SPIRAL } else { DROPPED_TC },"ha":"center","va":"center","zorder":5}))?;
            } else {
                let (fc, ec, tc, family, weight) = match state {
                    "repeat" => (SPIRAL_LIGHT, SPIRAL, SPIRAL, Some(MONO), "normal"),
                    "redirect" => (SPIRAL, SPIRAL, "white", None, "bold"),
                    "abort" => ("white", SPIRAL, SPIRAL, None, "bold"),
                    "dropped" => (DROPPED_FC, DROPPED_EC, DROPPED_TC, Some(MONO), "normal"),
                    _ => return Err(format!("unknown rollout box style: {state}").into()),
                };
                self.rounded([x,y],w,BAR_H,json!({"facecolor":fc,"edgecolor":ec,"linewidth":if state=="abort" {1.1} else {0.7},"radius":0.26,"zorder":4}))?;
                let size = if state == "redirect" || state == "abort" {
                    FS_MARK
                } else {
                    FS_MONO
                };
                ensure(
                    self.width_of(label, size, weight, family)? <= w - 0.30,
                    label,
                )?;
                self.text(x+w/2.0,y+BAR_H/2.0,label,json!({"fontsize":size,"color":tc,"family":family,"fontweight":weight,"ha":"center","va":"center","zorder":5}))?;
                if state == "dropped" {
                    self.line(
                        [x + 0.28, x + w - 0.28],
                        [y + BAR_H / 2.0; 2],
                        json!({"color":DROPPED_TC,"lw":0.6,"zorder":6}),
                    )?;
                }
            }
            if let Some(group) = group {
                spans.entry(group).or_insert((x, x + w)).1 = x + w;
            }
            x += w + gap;
        }
        let mut placed = Vec::new();
        for (group, text) in [
            ("loop", "repeated failures"),
            ("again", "still repeating"),
            ("after", "until you stop it"),
        ] {
            let (x0, x1) = spans[group];
            self.annotate(3, x0, x1, text, SPIRAL)?;
            let half = self.width_of(text, FS_SMALL, "bold", None)? / 2.0;
            placed.push(((x0 + x1) / 2.0 - half, (x0 + x1) / 2.0 + half));
        }
        for pair in placed.windows(2) {
            ensure(pair[1].0 > pair[0].1 + 0.30, "rollout notes collide")?;
        }
        Ok(())
    }
    fn agent(&mut self) -> Result<()> {
        self.rounded([AGENT_X0,PANEL_Y0],AGENT_X1-AGENT_X0,PANEL_Y1-PANEL_Y0,json!({"facecolor":"white","edgecolor":"#9FADB6","linewidth":0.9,"radius":0.55,"zorder":1}))?;
        self.agent_header()?;
        self.prefix_row()?;
        self.tools_row()?;
        self.rollout_row()
    }
    fn wiring(&mut self) -> Result<()> {
        for (number, _, color, _, _) in MECHANISMS {
            let y = axis_y(number);
            self.arrow([CTRL_X1 + 0.10, y], [AGENT_X0 + 1.05, y], color, 7)?;
        }
        self.line(
            [LEG_X, LEG_X],
            [PANEL_Y0, LEG_Y],
            json!({"color":MUTED,"lw":0.85,"zorder":6}),
        )?;
        self.arrow([LEG_X, LEG_Y], [CTRL_X1 + 0.20, LEG_Y], MUTED, 6)?;
        self.text(
            (CTRL_X1 + LEG_X) / 2.0,
            LEG_Y + 0.55,
            "the finished session's log",
            json!({"fontsize":FS_SMALL,"color":MUTED,"ha":"center","va":"bottom","zorder":6}),
        )
    }
}
pub fn render(out: &Path, python: &OsStr) -> Result<()> {
    let mut plot = Matplotlib::new(python)?;
    // Outline SVG glyphs so exported diagrams keep their fonts in other viewers.
    plot.call("plt.rcParams.update",json!([{"font.family":"DejaVu Sans","font.size":10,"svg.fonttype":"path","svg.hashsalt":"framework_overview","pdf.fonttype":42,"ps.fonttype":42,"axes.unicode_minus":false}]),json!({}))?;
    let figure = plot.call("plt.figure", json!([]), json!({"figsize":[5.5,2.775]}))?;
    let axis = plot.call(
        &format!("{figure}.add_axes"),
        json!([[0, 0, 1, 1]]),
        json!({}),
    )?;
    plot.call(&format!("{axis}.set_xlim"), json!([0, 55.0]), json!({}))?;
    plot.call(&format!("{axis}.set_ylim"), json!([0, 27.75]), json!({}))?;
    plot.call(&format!("{axis}.axis"), json!(["off"]), json!({}))?;
    plot.call(
        &format!("{figure}.patch.set_facecolor"),
        json!(["white"]),
        json!({}),
    )?;
    let mut drawing = Drawing { plot, figure, axis };
    drawing.controller()?;
    drawing.learners()?;
    drawing.agent()?;
    drawing.wiring()?;
    std::fs::create_dir_all(out)?;
    let png = out.join("framework_overview.png");
    let svg = out.join("framework_overview.svg");
    drawing.plot.call(
        &format!("{}.savefig", drawing.figure),
        json!([png]),
        json!({"dpi":600,"facecolor":"white"}),
    )?;
    drawing.plot.call(
        &format!("{}.savefig", drawing.figure),
        json!([svg]),
        json!({"facecolor":"white","metadata":{"Date":null}}),
    )?;
    println!("wrote {}\n      {}", png.display(), svg.display());
    Ok(())
}
