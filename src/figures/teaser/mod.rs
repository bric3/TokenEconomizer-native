//! The pinned teaser's timeline, frame layout and ffmpeg exports.
mod method;
mod painter;
mod panels;

use super::{
    Result,
    renderer::{Renderer, object},
};
use crate::{
    plugin::{process, sidecall::WorkDir},
    value::{round_decimal, sum_floats},
};
use painter::{Painter, Runtime};
use serde_json::json;
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const W: f64 = 1120.0;
const H: f64 = 600.0;
const SS: i32 = 3;
const FPS: usize = 25;
const N: usize = 6;
const WORK: f64 = 160.0;
const BG: &str = "#F2F5F9";
const INK: &str = "#132031";
const MUTED: &str = "#5D6978";
const FAINT: &str = "#A9B2BD";
const LINE_COLOR: &str = "#C3CCD6";
const PANEL: &str = "#FFFFFF";
const ACCENT: &str = "#285E9F";
const ACCENT_SOFT: &str = "#E5EDF7";
const GOOD: &str = "#2B7149";
const DROPPED_FC: &str = "#EDEFF1";
const DROPPED_EC: &str = "#BAC4CA";
const DROPPED_TC: &str = "#98A4AC";
const WORK_COLOR: &str = "#C9D1DA";
struct Mechanism {
    key: &'static str,
    full: f64,
    left: f64,
    after: usize,
    name: &'static str,
    waste: &'static str,
    does: &'static str,
    caption: &'static str,
    color: &'static str,
    light: &'static str,
    top: f64,
}
const MECHS: [Mechanism; 3] = [
    Mechanism {
        key: "facts",
        full: 40.0,
        left: 8.0,
        after: 1,
        name: "Micro-skills",
        waste: "re-learning the repo",
        does: "Remembers how the repo works",
        caption: "told to the agent as each session starts",
        color: "#3E73A8",
        light: "#E6EEF7",
        top: 112.0,
    },
    Mechanism {
        key: "prune",
        full: 44.0,
        left: 22.0,
        after: 2,
        name: "Tool pruning",
        waste: "tools never used",
        does: "Skips tools never used here",
        caption: "unused tools are no longer loaded",
        color: "#A86C0C",
        light: "#F8EEDB",
        top: 266.0,
    },
    Mechanism {
        key: "guard",
        full: 40.0,
        left: 16.0,
        after: 3,
        name: "Failure-loop intervention",
        waste: "stuck in loops",
        does: "Spots loops and steps in",
        caption: "a nudge when the same error repeats",
        color: "#B84843",
        light: "#F8E5E3",
        top: 420.0,
    },
];
const CX0: f64 = 40.0;
const CX1: f64 = 672.0;
const GX: f64 = 202.0;
const BX0: f64 = 226.0;
const PITCH: f64 = (CX1 - BX0) / N as f64;
const BAR_W: f64 = 46.0;
const BASE_Y: f64 = 528.0;
const UNIT: f64 = 1.52;
const SESSION_Y: f64 = 550.0;
const FULL_Y: f64 = BASE_Y - (WORK + 40.0 + 44.0 + 40.0) * UNIT;
const LEAN_Y: f64 = BASE_Y - (WORK + 8.0 + 22.0 + 16.0) * UNIT;
const CARD_X0: f64 = 720.0;
const CARD_X1: f64 = 1072.0;
const CARD_H: f64 = 140.0;
const TITLE_LEN: usize = 80;
const METHOD_HOLD: usize = 36;
const RESULTS_LEN: usize = 130;
const XFADE: usize = 8;
const FADE: usize = 10;
const GROW_FIRST: f64 = 22.0;
const GROW: f64 = 12.0;
const TRIM: f64 = 14.0;
const FLY: f64 = 12.0;
const LEARN: f64 = 12.0;
const SAVED: f64 = 12.0;

#[derive(Clone, Copy)]
struct Timeline {
    grow: [usize; N],
    trim: [Option<usize>; N],
    fly: [Option<usize>; N],
    learn: [usize; 3],
    saved: usize,
    end: usize,
}
impl Timeline {
    fn new() -> Self {
        let mut at = Self {
            grow: [0; N],
            trim: [None; N],
            fly: [None; N],
            learn: [0; 3],
            saved: 0,
            end: 0,
        };
        let mut t = 8;
        for i in 1..=N {
            at.grow[i - 1] = t;
            t += if i == 1 {
                GROW_FIRST as usize + 18
            } else {
                GROW as usize
            };
            if MECHS.iter().any(|m| m.after == i - 1) {
                at.trim[i - 1] = Some(t);
                t += TRIM as usize + 6;
            }
            let learns: Vec<_> = MECHS
                .iter()
                .enumerate()
                .filter(|(_, m)| m.after == i)
                .collect();
            if !learns.is_empty() {
                at.fly[i - 1] = Some(t);
                t += FLY as usize;
                for (index, _) in learns {
                    at.learn[index] = t;
                }
                t += LEARN as usize + 16;
            } else {
                t += 3;
            }
        }
        at.saved = t;
        t += SAVED as usize;
        at.end = t;
        at
    }
}
fn clamp(x: f64) -> f64 {
    x.clamp(0.0, 1.0)
}
fn ease_out(x: f64) -> f64 {
    1.0 - (1.0 - clamp(x)).powf(3.0)
}
fn ease_in_out(x: f64) -> f64 {
    let x = clamp(x);
    if x < 0.5 {
        4.0 * x.powf(3.0)
    } else {
        1.0 - (-2.0 * x + 2.0).powf(3.0) / 2.0
    }
}
fn back_out(x: f64) -> f64 {
    let x = clamp(x);
    1.0 + (1.9 + 1.0) * (x - 1.0).powf(3.0) + 1.9 * (x - 1.0).powf(2.0)
}
fn progress(f: f64, start: usize, length: f64) -> f64 {
    clamp((f - start as f64) / length)
}
fn rise(f: f64, start: usize, length: f64, by: f64) -> (f64, f64) {
    let a = ease_out(progress(f, start, length));
    (a, by * (1.0 - a))
}
fn rgb(color: &str) -> [u8; 3] {
    std::array::from_fn(|i| u8::from_str_radix(&color[1 + i * 2..3 + i * 2], 16).unwrap())
}
fn mix(c0: &str, c1: &str, t: f64) -> String {
    let a = rgb(c0);
    let b = rgb(c1);
    let values: [i32; 3] = std::array::from_fn(|i| {
        (f64::from(a[i]) + (f64::from(b[i]) - f64::from(a[i])) * t).round_ties_even() as i32
    });
    format!("#{:02X}{:02X}{:02X}", values[0], values[1], values[2])
}
fn col_x(i: usize) -> f64 {
    BX0 + PITCH * (i as f64 - 0.5)
}
fn ensure(condition: bool, message: impl Into<String>) -> Result<()> {
    if !condition {
        return Err(std::io::Error::other(message.into()).into());
    }
    Ok(())
}

fn find_font(folder: &Path, name: &str) -> Option<PathBuf> {
    let entries: Vec<_> = fs::read_dir(folder)
        .ok()?
        .filter_map(std::result::Result::ok)
        .collect();
    for entry in &entries {
        if entry.file_name() == name && entry.path().is_file() {
            return Some(entry.path());
        }
    }
    for entry in entries {
        if entry.file_type().is_ok_and(|kind| kind.is_dir())
            && let Some(path) = find_font(&entry.path(), name)
        {
            return Some(path);
        }
    }
    None
}
fn plex(folder: &Path) -> Result<std::collections::HashMap<&'static str, PathBuf>> {
    let mut fonts = std::collections::HashMap::new();
    for (style, name) in [
        ("", "IBMPlexSans-Regular"),
        ("bold", "IBMPlexSans-SemiBold"),
        ("italic", "IBMPlexSans-Italic"),
        ("mono", "IBMPlexMono-Regular"),
        ("mono bold", "IBMPlexMono-SemiBold"),
    ] {
        let found = ["ttf", "woff"]
            .iter()
            .find_map(|ext| find_font(folder, &format!("{name}.{ext}")))
            .ok_or_else(|| format!("no {name}.ttf or .woff under {}", folder.display()))?;
        fonts.insert(style, found);
    }
    Ok(fonts)
}

#[derive(Clone, Copy)]
enum Act {
    Title,
    Method,
    Results,
}
impl Runtime {
    fn bases(&mut self, k: f64) -> Result<(String, String)> {
        let blank = Painter::new(self, k, None)?.im;
        let method = {
            let mut painter = Painter::new(self, k, None)?;
            painter.draw_static()?;
            painter.im
        };
        self.keep(&[blank.clone(), method.clone()])?;
        Ok((blank, method))
    }
    fn act(&mut self, act: Act, f: f64, k: f64, blank: &str, method: &str) -> Result<String> {
        let mut painter = Painter::new(
            self,
            k,
            Some(if matches!(act, Act::Method) {
                method
            } else {
                blank
            }),
        )?;
        match act {
            Act::Title => painter.draw_title(f)?,
            Act::Method => painter.draw_method(f)?,
            Act::Results => painter.draw_results(f)?,
        }
        painter.reduced()
    }
    fn save(&mut self, image: &str, path: &Path, optimize: bool) -> Result<()> {
        self.renderer.call(
            &format!("{image}.save"),
            json!([path]),
            if optimize {
                json!({"optimize":true})
            } else {
                json!({})
            },
        )?;
        Ok(())
    }
    fn animation(&mut self, scale: f64, out: &Path) -> Result<usize> {
        let k = f64::from(SS) * scale;
        let (blank, method) = self.bases(k)?;
        let mut last: Option<String> = None;
        let mut count = 0;
        for (act, length, settled) in [
            (Act::Title, TITLE_LEN, TITLE_LEN - 20),
            (Act::Method, self.at.end + METHOD_HOLD, self.at.end),
            (Act::Results, RESULTS_LEN, RESULTS_LEN - 50),
        ] {
            if let Some(last) = &last {
                let first = self.act(act, 0.0, k, &blank, &method)?;
                for j in 0..XFADE {
                    let image = self.renderer.call(
                        "Image.blend",
                        json!([
                            object(last),
                            object(&first),
                            ease_in_out((j + 1) as f64 / (XFADE + 1) as f64)
                        ]),
                        json!({}),
                    )?;
                    self.save(&image, &out.join(format!("{count:04}.png")), false)?;
                    count += 1;
                    self.keep(&[blank.clone(), method.clone(), last.clone(), first.clone()])?;
                }
            }
            let held = self.act(act, settled as f64, k, &blank, &method)?;
            for f in 0..length {
                let image = if f >= settled {
                    held.clone()
                } else {
                    self.act(act, f as f64, k, &blank, &method)?
                };
                self.save(&image, &out.join(format!("{count:04}.png")), false)?;
                count += 1;
                self.keep(&[blank.clone(), method.clone(), held.clone()])?;
            }
            last = Some(held);
        }
        let first = self.act(Act::Title, 0.0, k, &blank, &method)?;
        let last = last.unwrap();
        for j in 0..FADE {
            let image = self.renderer.call(
                "Image.blend",
                json!([
                    object(&last),
                    object(&first),
                    ease_in_out((j + 1) as f64 / (FADE + 1) as f64)
                ]),
                json!({}),
            )?;
            self.save(&image, &out.join(format!("{count:04}.png")), false)?;
            count += 1;
            self.keep(&[blank.clone(), method.clone(), last.clone(), first.clone()])?;
        }
        Ok(count)
    }
    fn poster(&mut self, out: &Path) -> Result<()> {
        let k = f64::from(SS) * 2.0;
        let (blank, method) = self.bases(k)?;
        let image = self.act(Act::Method, self.at.end as f64, k, &blank, &method)?;
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        self.save(&image, out, true)?;
        println!("wrote {}", out.display());
        Ok(())
    }
}

pub fn render(
    fonts: &Path,
    out: &Path,
    page: bool,
    still_only: bool,
    python: &OsStr,
) -> Result<()> {
    let ffmpeg = if still_only {
        None
    } else {
        Some(process::which("ffmpeg").ok_or("teaser needs ffmpeg on PATH")?)
    };
    let mut runtime = Runtime::new(plex(fonts)?, python)?;
    if let Some(ffmpeg) = ffmpeg {
        let frames = WorkDir::new("te-teaser-")?;
        let count = runtime.animation(if page { 2.0 } else { 1.25 }, frames.path())?;
        fs::create_dir_all(out)?;
        let outputs = if page {
            vec![
                (
                    "teaser.webm",
                    "format=yuv420p",
                    vec![
                        "-c:v",
                        "libvpx-vp9",
                        "-crf",
                        "34",
                        "-b:v",
                        "0",
                        "-row-mt",
                        "1",
                        "-an",
                    ],
                ),
                (
                    "teaser.mp4",
                    "format=yuv420p",
                    vec![
                        "-c:v",
                        "libx264",
                        "-crf",
                        "20",
                        "-preset",
                        "slow",
                        "-tune",
                        "animation",
                        "-movflags",
                        "+faststart",
                        "-an",
                    ],
                ),
            ]
        } else {
            vec![(
                "teaser.gif",
                "split[a][b];[a]palettegen=max_colors=256:stats_mode=full[p];[b][p]paletteuse=dither=none:diff_mode=rectangle",
                vec!["-loop", "0"],
            )]
        };
        for (name, filters, extra) in outputs {
            let file = out.join(name);
            let status = Command::new(&ffmpeg)
                .args([
                    "-loglevel",
                    "error",
                    "-y",
                    "-framerate",
                    &FPS.to_string(),
                    "-i",
                ])
                .arg(frames.path().join("%04d.png"))
                .args(["-vf", filters])
                .args(extra)
                .arg(&file)
                .status()?;
            ensure(status.success(), format!("ffmpeg failed: {status}"))?;
            println!(
                "wrote {} ({:.2} MB, {:.1} s)",
                file.display(),
                fs::metadata(&file)?.len() as f64 / 1e6,
                count as f64 / FPS as f64
            );
        }
    }
    runtime.poster(&out.join("teaser.png"))
}
