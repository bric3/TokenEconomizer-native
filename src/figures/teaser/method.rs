use super::painter::{Chip, Rect, Text};
use super::*;

impl Runtime {
    fn amount(&self, m: usize, i: usize, f: f64) -> f64 {
        let mechanism = &MECHS[m];
        if mechanism.after >= i {
            mechanism.full
        } else if mechanism.after == i - 1 {
            mechanism.full
                - (mechanism.full - mechanism.left)
                    * ease_in_out(progress(f, self.at.trim[i - 1].unwrap(), TRIM))
        } else {
            mechanism.left
        }
    }
    fn stack(&self, i: usize, f: f64) -> Vec<(f64, &'static str, Option<usize>)> {
        let mut stack = vec![(WORK, WORK_COLOR, None)];
        for m in (0..MECHS.len()).rev() {
            stack.push((self.amount(m, i, f), MECHS[m].color, Some(m)));
        }
        stack
    }
    fn trim_glow(&self, m: usize, f: f64) -> f64 {
        let i = MECHS[m].after + 1;
        if i > N {
            return 0.0;
        }
        self.at.trim[i - 1].map_or(0.0, |start| {
            (std::f64::consts::PI * progress(f, start - 2, TRIM + 8.0)).sin()
        })
    }
}
impl Painter<'_> {
    pub fn draw_static(&mut self) -> Result<()> {
        let title = "Each finished session makes the next ones waste less";
        self.text(
            [CX0, 46.0],
            title,
            22.0,
            Text {
                anchor: "ls",
                ..Text::bold(INK)
            },
        )?;
        ensure(
            CX0 + self.width(title, 22.0, "bold")? <= CX1,
            "the title outgrows the chart",
        )?;
        self.line(&[[BX0 - 8.0, BASE_Y], [CX1, BASE_Y]], FAINT, 1.4, 1.0)?;
        self.rect(
            [704.0, 24.0, 1088.0, 576.0],
            Rect {
                fill: Some(PANEL),
                outline: Some(LINE_COLOR),
                width: 1.6,
                radius: 14.0,
                ..Rect::default()
            },
        )?;
        self.text(
            [CARD_X0 + 4.0, 56.0],
            "TokenEconomizer",
            20.0,
            Text::bold(INK),
        )?;
        self.text(
            [CARD_X0 + 4.0, 82.0],
            "learns from each finished session",
            15.0,
            Text::new(MUTED),
        )?;
        self.line(&[[CARD_X0, 100.0], [CARD_X1, 100.0]], LINE_COLOR, 1.2, 1.0)
    }
    fn draw_bar(&mut self, i: usize, f: f64) -> Result<()> {
        let at = self.runtime.at;
        let start = at.grow[i - 1];
        let grow = if i == 1 { GROW_FIRST } else { GROW };
        let label = format!("session {i}");
        self.text(
            [col_x(i), SESSION_Y],
            &label,
            14.0,
            Text {
                anchor: "mm",
                ..Text::new(&mix(FAINT, INK, clamp((f - start as f64) / 4.0)))
            },
        )?;
        ensure(
            self.width(&label, 14.0, "")? <= PITCH - 8.0,
            "session labels meet",
        )?;
        if f < start as f64 {
            return Ok(());
        }
        let segments = self.runtime.stack(i, f);
        let total = sum_floats(segments.iter().map(|segment| segment.0));
        let mut shown = total * ease_out(progress(f, start, grow));
        let (x0, x1) = (col_x(i) - BAR_W / 2.0, col_x(i) + BAR_W / 2.0);
        let mut y = BASE_Y;
        for (n, (v, c, _)) in segments.iter().enumerate() {
            let v = v.min(shown);
            if v <= 0.0 {
                break;
            }
            self.rect(
                [x0, y - v * UNIT, x1, y - if n > 0 { 1.4 } else { 0.0 }],
                Rect {
                    fill: Some(c),
                    ..Rect::default()
                },
            )?;
            y -= v * UNIT;
            shown -= v;
        }
        if let Some(start) = at.trim[i - 1] {
            let t = progress(f, start, TRIM);
            let m = MECHS.iter().position(|m| m.after == i - 1).unwrap();
            let cut = MECHS[m].full - self.runtime.amount(m, i, f);
            if t > 0.0 && t < 1.0 && cut > 0.3 {
                self.rect(
                    [x0, y - cut * UNIT, x1, y - 1.4],
                    Rect {
                        fill: Some(MECHS[m].color),
                        alpha: 0.35 * (1.0 - t),
                        ..Rect::default()
                    },
                )?;
            }
        }
        if i == 1 {
            self.draw_names(&segments, total * ease_out(progress(f, start, grow)))?;
        }
        Ok(())
    }
    fn draw_names(&mut self, segments: &[(f64, &str, Option<usize>)], shown: f64) -> Result<()> {
        let mut below = 0.0;
        let x = col_x(1) - BAR_W / 2.0;
        for &(v, _, m) in segments {
            let mid = below + v / 2.0;
            let a = clamp((shown - mid) / 12.0);
            let y = BASE_Y - mid * UNIT;
            let (label, tc, style) = m.map_or(("the work itself", MUTED, ""), |m| {
                (MECHS[m].waste, MECHS[m].color, "bold")
            });
            self.text(
                [GX, y],
                label,
                15.0,
                Text {
                    style,
                    anchor: "rm",
                    alpha: a,
                    ..Text::new(tc)
                },
            )?;
            ensure(
                GX - self.width(label, 15.0, style)? >= CX0 - 16.0,
                format!("{label:?} outgrows the gutter"),
            )?;
            self.line(&[[GX + 6.0, y], [x - 5.0, y]], &mix(BG, tc, 0.6), 1.2, a)?;
            below += v;
        }
        Ok(())
    }
    fn draw_without(&mut self, f: f64) -> Result<()> {
        let start = self.runtime.at.trim[1].unwrap();
        if f < start as f64 {
            return Ok(());
        }
        let x0 = col_x(1) + BAR_W / 2.0 + 6.0;
        let end = x0 + (CX1 - x0) * ease_out(progress(f, start, 12.0));
        self.dashed([x0, FULL_Y], [end, FULL_Y], MUTED, 1.3, 1.0)?;
        self.text(
            [CX1, FULL_Y - 13.0],
            "without TokenEconomizer",
            14.0,
            Text {
                style: "italic",
                anchor: "rm",
                alpha: progress(f, start + 12 - 4, 8.0),
                ..Text::new(MUTED)
            },
        )
    }
    fn draw_saved(&mut self, f: f64) -> Result<()> {
        let a = progress(f, self.runtime.at.saved, SAVED);
        if a <= 0.0 {
            return Ok(());
        }
        let x = col_x(N);
        let mid = (FULL_Y + LEAN_Y) / 2.0;
        let reach = (LEAN_Y - FULL_Y) / 2.0 - 5.0;
        let r = reach * ease_out(a);
        for s in [-1.0, 1.0] {
            let tip = mid + s * r;
            self.line(&[[x, mid + s * 11.0], [x, tip]], INK, 1.5, a)?;
            self.line(
                &[[x - 4.0, tip - s * 5.0], [x, tip], [x + 4.0, tip - s * 5.0]],
                INK,
                1.5,
                a,
            )?;
        }
        self.rect(
            [x - 26.0, mid - 10.0, x + 26.0, mid + 10.0],
            Rect {
                fill: Some(BG),
                alpha: a,
                ..Rect::default()
            },
        )?;
        self.text(
            [x, mid],
            "saved",
            14.0,
            Text {
                anchor: "mm",
                alpha: a,
                ..Text::bold(INK)
            },
        )?;
        ensure(
            self.width("saved", 14.0, "bold")? <= 52.0,
            "saved label outgrows its box",
        )
    }
    fn draw_card(&mut self, m: usize, f: f64) -> Result<()> {
        let mechanism = &MECHS[m];
        let top = mechanism.top;
        let c = mechanism.color;
        let bounds = [CARD_X0, top, CARD_X1, top + CARD_H];
        let start = self.runtime.at.learn[m];
        let a = ease_in_out(progress(f, start, LEARN));
        if a < 1.0 {
            self.dashed_rect(bounds, FAINT, 1.2, 1.0 - a)?;
        }
        if a <= 0.0 {
            return Ok(());
        }
        let glow = self.runtime.trim_glow(m, f);
        let pop = 1.0 - (-6.0 * progress(f, start, LEARN + 8.0)).exp();
        self.rect(
            bounds,
            Rect {
                fill: Some(&mix(
                    "#FFFFFF",
                    mechanism.light,
                    0.7 * glow + 0.5 * (1.0 - pop),
                )),
                outline: Some(c),
                width: 1.8 + 1.4 * glow,
                radius: 10.0,
                alpha: a,
            },
        )?;
        let dy = 8.0 * (1.0 - a);
        let x = CARD_X0 + 20.0;
        let y = top + dy;
        self.circle(
            x + 10.0,
            y + 24.0,
            10.5,
            Rect {
                fill: Some(c),
                alpha: a,
                ..Rect::default()
            },
        )?;
        self.text(
            [x + 10.0, y + 24.5],
            &(m + 1).to_string(),
            12.0,
            Text {
                anchor: "mm",
                alpha: a,
                ..Text::bold("#FFFFFF")
            },
        )?;
        self.text(
            [x + 28.0, y + 24.0],
            mechanism.name,
            14.0,
            Text {
                alpha: a,
                ..Text::bold(c)
            },
        )?;
        let s = back_out(progress(f, start + 4, 9.0));
        self.circle(
            CARD_X1 - 24.0,
            top + 24.0,
            11.0 * s,
            Rect {
                fill: Some(c),
                alpha: a,
                ..Rect::default()
            },
        )?;
        self.check(
            CARD_X1 - 24.0,
            top + 24.0,
            4.8 * s,
            "#FFFFFF",
            if s > 0.3 { a } else { 0.0 },
        )?;
        ensure(
            x + 28.0 + self.width(mechanism.name, 14.0, "bold")? <= CARD_X1 - 44.0,
            format!("{}'s name meets its check", mechanism.key),
        )?;
        self.text(
            [x, y + 53.0],
            mechanism.does,
            17.0,
            Text {
                alpha: a,
                ..Text::bold(INK)
            },
        )?;
        ensure(
            x + self.width(mechanism.does, 17.0, "bold")? <= CARD_X1 - 16.0,
            format!("{}'s line outgrows its card", mechanism.key),
        )?;
        let right = self.draw_example(m, x, y + 88.0, a)?;
        ensure(
            right <= CARD_X1 - 16.0,
            format!("{}'s example outgrows its card", mechanism.key),
        )?;
        self.text(
            [x, y + 119.0],
            mechanism.caption,
            13.0,
            Text {
                alpha: a,
                ..Text::new(MUTED)
            },
        )?;
        ensure(
            x + self.width(mechanism.caption, 13.0, "")? <= CARD_X1 - 16.0,
            format!("{}'s caption outgrows its card", mechanism.key),
        )
    }
    fn draw_example(&mut self, m: usize, mut x: f64, cy: f64, a: f64) -> Result<f64> {
        let c = MECHS[m].color;
        let light = MECHS[m].light;
        if m == 0 {
            return self.chip(
                x,
                cy,
                &[
                    ("Run tests with ", 13.0, "", c),
                    ("python -m pytest", 13.0, "mono bold", c),
                ],
                light,
                c,
                Chip {
                    alpha: a,
                    ..Chip::default()
                },
            );
        }
        if m == 1 {
            for tool in ["Read", "Edit"] {
                x = self.chip(
                    x,
                    cy,
                    &[(tool, 13.0, "", INK)],
                    "#FFFFFF",
                    LINE_COLOR,
                    Chip {
                        pad: 8.0,
                        alpha: a,
                        ..Chip::default()
                    },
                )? + 6.0;
            }
            for tool in ["Notebook", "Browser"] {
                x = self.chip(
                    x,
                    cy,
                    &[(tool, 13.0, "", DROPPED_TC)],
                    DROPPED_FC,
                    DROPPED_EC,
                    Chip {
                        pad: 8.0,
                        strike: true,
                        alpha: a,
                        ..Chip::default()
                    },
                )? + 6.0;
            }
            return Ok(x - 6.0);
        }
        x = self.chip(
            x,
            cy,
            &[("same error ×3", 13.0, "", c)],
            light,
            c,
            Chip {
                pad: 8.0,
                icon: true,
                alpha: a,
                ..Chip::default()
            },
        )?;
        self.text(
            [x + 13.0, cy],
            "→",
            16.0,
            Text {
                anchor: "mm",
                alpha: a,
                ..Text::bold(c)
            },
        )?;
        self.chip(
            x + 26.0,
            cy,
            &[("try another way", 13.0, "bold", "#FFFFFF")],
            c,
            c,
            Chip {
                pad: 9.0,
                alpha: a,
                ..Chip::default()
            },
        )
    }
    fn draw_fly(&mut self, i: usize, f: f64) -> Result<()> {
        let Some(start) = self.runtime.at.fly[i - 1] else {
            return Ok(());
        };
        let t = progress(f, start, FLY);
        if f < start as f64 || t >= 1.0 {
            return Ok(());
        }
        let top =
            BASE_Y - sum_floats(self.runtime.stack(i, f).iter().map(|segment| segment.0)) * UNIT;
        let m = MECHS.iter().position(|m| m.after == i).unwrap();
        let (x0, y0) = (col_x(i), top - 8.0);
        let (x1, y1) = (CARD_X0 - 2.0, MECHS[m].top + CARD_H / 2.0);
        let (cxp, cyp) = ((x0 + x1) / 2.0, 100.0_f64.max(y0.min(y1) - 70.0));
        let at = |u: f64| {
            [
                (1.0 - u).powf(2.0) * x0 + 2.0 * (1.0 - u) * u * cxp + u.powf(2.0) * x1,
                (1.0 - u).powf(2.0) * y0 + 2.0 * (1.0 - u) * u * cyp + u.powf(2.0) * y1,
            ]
        };
        let u = ease_in_out(t);
        for k in (1..=5).rev() {
            let [x, y] = at(clamp(u - 0.045 * f64::from(k)));
            self.circle(
                x,
                y,
                5.5 - 0.7 * f64::from(k),
                Rect {
                    fill: Some(INK),
                    alpha: 0.14 * f64::from(6 - k),
                    ..Rect::default()
                },
            )?;
        }
        let [x, y] = at(u);
        self.circle(
            x,
            y,
            5.5,
            Rect {
                fill: Some(INK),
                ..Rect::default()
            },
        )
    }
    pub fn draw_method(&mut self, f: f64) -> Result<()> {
        self.draw_without(f)?;
        for i in 1..=N {
            self.draw_bar(i, f)?;
        }
        self.draw_saved(f)?;
        for m in 0..MECHS.len() {
            self.draw_card(m, f)?;
        }
        for i in 1..=N {
            self.draw_fly(i, f)?;
        }
        Ok(())
    }
}
