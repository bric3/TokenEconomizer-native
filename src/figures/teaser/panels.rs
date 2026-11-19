use super::super::results::ROWS;
use super::painter::{Rect, Text};
use super::*;

impl Painter<'_> {
    pub fn draw_title(&mut self, f: f64) -> Result<()> {
        let cx = W / 2.0;
        let (a, dy) = rise(f, 2, 16.0, 16.0);
        let (head, tail, size) = ("Token", "Economizer", 76.0);
        let mut x = cx - self.width(&format!("{head}{tail}"), size, "bold")? / 2.0;
        self.text(
            [x, 214.0 + dy],
            head,
            size,
            Text {
                anchor: "ls",
                alpha: a,
                ..Text::bold(INK)
            },
        )?;
        let tx = x + self.width(head, size, "bold")?;
        self.text(
            [tx, 214.0 + dy],
            tail,
            size,
            Text {
                anchor: "ls",
                alpha: a,
                ..Text::bold(ACCENT)
            },
        )?;
        let u = ease_in_out(progress(f, 12, 14.0));
        if u > 0.0 {
            self.line(
                &[[cx - 36.0 * u, 242.0], [cx + 36.0 * u, 242.0]],
                ACCENT,
                3.0,
                1.0,
            )?;
        }
        let (a, dy) = rise(f, 14, 14.0, 12.0);
        let sub = "Experience-driven black-box cost control for coding agents";
        self.text(
            [cx, 284.0 + dy],
            sub,
            24.0,
            Text {
                anchor: "mm",
                alpha: a,
                ..Text::new(MUTED)
            },
        )?;
        ensure(
            self.width(sub, 24.0, "")? <= W - 80.0,
            "the subtitle outgrows the card",
        )?;
        let mut widths = Vec::new();
        for m in &MECHS {
            widths.push(36.0 + self.width(m.name, 16.0, "bold")? + 16.0);
        }
        x = cx - (sum_floats(widths.iter().copied()) + 14.0 * (MECHS.len() - 1) as f64) / 2.0;
        for (n, (m, w)) in MECHS.iter().zip(widths).enumerate() {
            let (a, dy) = rise(f, 24 + 5 * n, 12.0, 10.0);
            let y = 358.0 + dy;
            self.rect(
                [x, y - 19.0, x + w, y + 19.0],
                Rect {
                    fill: Some("#FFFFFF"),
                    outline: Some(&mix(m.light, m.color, 0.55)),
                    width: 1.6,
                    radius: 19.0,
                    alpha: a,
                },
            )?;
            self.circle(
                x + 19.0,
                y,
                11.0,
                Rect {
                    fill: Some(m.color),
                    alpha: a,
                    ..Rect::default()
                },
            )?;
            self.text(
                [x + 19.0, y + 0.5],
                &(n + 1).to_string(),
                13.0,
                Text {
                    anchor: "mm",
                    alpha: a,
                    ..Text::bold("#FFFFFF")
                },
            )?;
            self.text(
                [x + 36.0, y],
                m.name,
                16.0,
                Text {
                    alpha: a,
                    ..Text::bold(m.color)
                },
            )?;
            x += w + 14.0;
        }
        let (a, dy) = rise(f, 40, 14.0, 12.0);
        let lead = "Works with  ";
        let names = ["Claude Code", "Codex", "GitHub Copilot CLI", "Pi"].join("  ·  ");
        x = cx - (self.width(lead, 17.0, "")? + self.width(&names, 17.0, "bold")?) / 2.0;
        self.text(
            [x, 434.0 + dy],
            lead,
            17.0,
            Text {
                alpha: a,
                ..Text::new(MUTED)
            },
        )?;
        let tx = x + self.width(lead, 17.0, "")?;
        self.text(
            [tx, 434.0 + dy],
            &names,
            17.0,
            Text {
                alpha: a,
                ..Text::bold(INK)
            },
        )?;
        let (a, dy) = rise(f, 46, 14.0, 12.0);
        let logos = [("logo_berkeley.png", 40.0), ("logo_microsoft.png", 34.0)];
        let mut widths = Vec::new();
        for (name, h) in logos {
            widths.push(self.runtime.picture_width(name, h)?);
        }
        x = cx - (sum_floats(widths.iter().copied()) + 48.0 * (logos.len() - 1) as f64) / 2.0;
        for ((name, h), w) in logos.into_iter().zip(widths) {
            self.picture(name, x, 506.0 + dy, h, a)?;
            x += w + 48.0;
        }
        Ok(())
    }
    pub fn draw_results(&mut self, f: f64) -> Result<()> {
        let (a, dy) = rise(f, 0, 12.0, 8.0);
        self.text(
            [CX0, 56.0 + dy],
            "Measured on SWE-Bench Tasks",
            26.0,
            Text {
                anchor: "ls",
                alpha: a,
                ..Text::bold(INK)
            },
        )?;
        let cuts: Vec<_> = ROWS.iter().map(|row| -row.cost).collect();
        let minimum = cuts.iter().copied().reduce(f64::min).unwrap();
        let maximum = cuts.iter().copied().reduce(f64::max).unwrap();
        let up = ROWS.iter().filter(|row| row.te > row.base).count();
        let labels = [
            ["lower mean cost, across five", "agent–model combinations"],
            ["repository × combination cells", "cheaper than default"],
            ["agent–model combinations where", "Pass@1 also improves"],
        ];
        for (n, x) in [40.0, 394.0, 748.0].into_iter().enumerate() {
            let c = if n == 0 { ACCENT } else { INK };
            let (a, dy) = rise(f, 6 + 5 * n, 12.0, 10.0);
            let (y0, y1) = (84.0 + dy, 216.0 + dy);
            self.rect(
                [x, y0, x + 332.0, y1],
                Rect {
                    fill: Some("#FFFFFF"),
                    outline: Some(LINE_COLOR),
                    width: 1.2,
                    radius: 12.0,
                    alpha: a,
                },
            )?;
            let t = ease_out(progress(f, 10 + 5 * n, 26.0));
            let (big, small) = match n {
                0 => (
                    format!("{:.1}–{:.1}", minimum * t, maximum * t),
                    "%".to_string(),
                ),
                1 => (
                    format!("{:.0}", (30.0 * t).round_ties_even()),
                    " / 30".to_string(),
                ),
                _ => (
                    format!("{:.0}", (up as f64 * t).round_ties_even()),
                    format!(" of {}", ROWS.len()),
                ),
            };
            self.text(
                [x + 22.0, y0 + 56.0],
                &big,
                44.0,
                Text {
                    anchor: "ls",
                    alpha: a,
                    ..Text::bold(c)
                },
            )?;
            let tx = x + 22.0 + self.width(&big, 44.0, "bold")? + 2.0;
            self.text(
                [tx, y0 + 56.0],
                &small,
                24.0,
                Text {
                    anchor: "ls",
                    alpha: a,
                    ..Text::new(MUTED)
                },
            )?;
            for (line, ly) in labels[n].into_iter().zip([y0 + 86.0, y0 + 108.0]) {
                self.text(
                    [x + 22.0, ly],
                    line,
                    15.0,
                    Text {
                        alpha: a,
                        ..Text::new(MUTED)
                    },
                )?;
                ensure(
                    self.width(line, 15.0, "")? <= 332.0 - 40.0,
                    format!("{line:?} outgrows its tile"),
                )?;
            }
        }
        let (a, dy) = rise(f, 22, 12.0, 10.0);
        let y0 = 236.0 + dy;
        self.rect(
            [CX0, y0, W - 40.0, 556.0 + dy],
            Rect {
                fill: Some("#FFFFFF"),
                outline: Some(LINE_COLOR),
                width: 1.2,
                radius: 12.0,
                alpha: a,
            },
        )?;
        self.text(
            [CX0 + 22.0, y0 + 34.0],
            "Mean cost per task, lower than default",
            16.0,
            Text {
                anchor: "ls",
                alpha: a,
                ..Text::bold(INK)
            },
        )?;
        self.text(
            [W - 62.0, y0 + 34.0],
            "Pass@1 change",
            14.0,
            Text {
                anchor: "rs",
                alpha: a,
                ..Text::new(MUTED)
            },
        )?;
        let scale = (900.0 - 318.0) / maximum;
        for (n, (row, cut)) in ROWS.iter().zip(cuts).enumerate() {
            let y = 312.0 + 50.0 * n as f64 + dy;
            self.text(
                [CX0 + 22.0, y],
                row.agent,
                15.0,
                Text {
                    alpha: a,
                    ..Text::bold(INK)
                },
            )?;
            let agent_width = self.width(&format!("{}  ", row.agent), 15.0, "bold")?;
            self.text(
                [CX0 + 22.0 + agent_width, y],
                row.model,
                15.0,
                Text {
                    alpha: a,
                    ..Text::new(MUTED)
                },
            )?;
            ensure(
                CX0 + 22.0 + agent_width + self.width(row.model, 15.0, "")? <= 318.0 - 14.0,
                "result labels outgrow the gutter",
            )?;
            self.rect(
                [318.0, y - 11.0, 900.0, y + 11.0],
                Rect {
                    fill: Some(ACCENT_SOFT),
                    radius: 5.0,
                    alpha: 0.6 * a,
                    ..Rect::default()
                },
            )?;
            let t = ease_out(progress(f, 30 + 4 * n, 24.0));
            if t > 0.0 {
                let end = 318.0 + cut * scale * t;
                self.rect(
                    [318.0, y - 11.0, end, y + 11.0],
                    Rect {
                        fill: Some(ACCENT),
                        radius: 5.0,
                        ..Rect::default()
                    },
                )?;
                self.text(
                    [end + 10.0, y],
                    &format!("{:.1}%", cut * t),
                    15.0,
                    Text {
                        alpha: t,
                        ..Text::bold(ACCENT)
                    },
                )?;
            }
            let d = round_decimal(row.te - row.base, 1);
            self.text(
                [W - 62.0, y],
                &format!("{}{:.1}", if d > 0.0 { "+" } else { "−" }, d.abs()),
                15.0,
                Text {
                    anchor: "rm",
                    alpha: ease_out(progress(f, 46 + 4 * n, 10.0)),
                    ..Text::bold(if d > 0.0 { GOOD } else { MUTED })
                },
            )?;
        }
        let right_width = self.width("+0.0", 15.0, "bold")?;
        ensure(
            900.0 + 10.0 + self.width("30.9%", 15.0, "bold")? <= W - 62.0 - right_width - 24.0,
            "result values collide with Pass@1",
        )?;
        let (a, _) = rise(f, 64, 14.0, 12.0);
        self.text(
            [W / 2.0, 582.0],
            "arXiv 2610.22732   ·   tokeneconomizer.github.io",
            14.0,
            Text {
                anchor: "mm",
                alpha: a,
                ..Text::new(MUTED)
            },
        )
    }
}
