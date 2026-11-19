use super::*;
use std::collections::HashMap;

pub(super) struct Runtime {
    pub renderer: Renderer,
    pub at: Timeline,
    fonts: HashMap<&'static str, PathBuf>,
    font_cache: HashMap<(PathBuf, u64, u64), String>,
    pictures: HashMap<(&'static str, u64, u64), String>,
}
impl Runtime {
    pub fn new(fonts: HashMap<&'static str, PathBuf>, python: &OsStr) -> Result<Self> {
        Ok(Self {
            renderer: Renderer::new(python, "pillow")?,
            at: Timeline::new(),
            fonts,
            font_cache: HashMap::new(),
            pictures: HashMap::new(),
        })
    }
    pub fn keep(&mut self, images: &[String]) -> Result<()> {
        // Frames own temporary SDK objects; retain only live images and shared font/picture caches.
        let handles: Vec<_> = images
            .iter()
            .chain(self.font_cache.values())
            .chain(self.pictures.values())
            .cloned()
            .collect();
        self.renderer.keep(&handles)
    }
    fn logo(&mut self, name: &'static str) -> Result<String> {
        let bytes: &[u8] = match name {
            "logo_berkeley.png" => include_bytes!("../assets/logo_berkeley.png"),
            "logo_microsoft.png" => include_bytes!("../assets/logo_microsoft.png"),
            _ => return Err(format!("unknown teaser logo: {name}").into()),
        };
        let bytes = self.renderer.call("bytes", json!([bytes]), json!({}))?;
        let input = self
            .renderer
            .call("BytesIO", json!([object(&bytes)]), json!({}))?;
        self.renderer
            .call("Image.open", json!([object(&input)]), json!({}))
    }
    pub fn picture_width(&mut self, name: &'static str, height: f64) -> Result<f64> {
        let image = self.logo(name)?;
        let width = self
            .renderer
            .value(&format!("{image}.width"))?
            .as_f64()
            .ok_or("invalid logo width")?;
        let actual_height = self
            .renderer
            .value(&format!("{image}.height"))?
            .as_f64()
            .ok_or("invalid logo height")?;
        self.renderer
            .call(&format!("{image}.close"), json!([]), json!({}))?;
        Ok(width * height / actual_height)
    }
}
pub(super) struct Painter<'a> {
    pub runtime: &'a mut Runtime,
    pub k: f64,
    pub im: String,
    draw: String,
}
pub(super) struct Rect<'a> {
    pub fill: Option<&'a str>,
    pub outline: Option<&'a str>,
    pub width: f64,
    pub radius: f64,
    pub alpha: f64,
}
impl Default for Rect<'_> {
    fn default() -> Self {
        Self {
            fill: None,
            outline: None,
            width: 1.0,
            radius: 0.0,
            alpha: 1.0,
        }
    }
}
pub(super) struct Text<'a> {
    pub color: &'a str,
    pub style: &'a str,
    pub anchor: &'a str,
    pub alpha: f64,
}
impl<'a> Text<'a> {
    pub fn new(color: &'a str) -> Self {
        Self {
            color,
            style: "",
            anchor: "lm",
            alpha: 1.0,
        }
    }
    pub fn bold(color: &'a str) -> Self {
        Self {
            style: "bold",
            ..Self::new(color)
        }
    }
}
impl<'a> Painter<'a> {
    pub fn new(runtime: &'a mut Runtime, k: f64, base: Option<&str>) -> Result<Self> {
        let im = if let Some(base) = base {
            runtime
                .renderer
                .call(&format!("{base}.copy"), json!([]), json!({}))?
        } else {
            runtime.renderer.call(
                "Image.new",
                json!([
                    "RGB",
                    [
                        (W * k).round_ties_even() as i64,
                        (H * k).round_ties_even() as i64
                    ],
                    BG
                ]),
                json!({}),
            )?
        };
        let draw =
            runtime
                .renderer
                .call("ImageDraw.Draw", json!([object(&im), "RGBA"]), json!({}))?;
        Ok(Self {
            runtime,
            k,
            im,
            draw,
        })
    }
    pub fn reduced(self) -> Result<String> {
        self.runtime
            .renderer
            .call(&format!("{}.reduce", self.im), json!([SS]), json!({}))
    }
    fn font(&mut self, size: f64, style: &str) -> Result<String> {
        let path = self
            .runtime
            .fonts
            .get(style)
            .ok_or_else(|| format!("unknown font style: {style}"))?
            .clone();
        let key = (path.clone(), size.to_bits(), self.k.to_bits());
        if let Some(font) = self.runtime.font_cache.get(&key) {
            return Ok(font.clone());
        }
        let font = self.runtime.renderer.call(
            "ImageFont.truetype",
            json!([path, (size * self.k).round_ties_even() as i64]),
            json!({}),
        )?;
        self.runtime.font_cache.insert(key, font.clone());
        Ok(font)
    }
    pub fn width(&mut self, text: &str, size: f64, style: &str) -> Result<f64> {
        let font = self.font(size, style)?;
        let result =
            self.runtime
                .renderer
                .call(&format!("{font}.getlength"), json!([text]), json!({}))?;
        Ok(self
            .runtime
            .renderer
            .value(&result)?
            .as_f64()
            .ok_or("invalid text width")?
            / self.k)
    }
    fn rgba(color: &str, alpha: f64) -> serde_json::Value {
        let c = rgb(color);
        json!({"$tuple":[c[0],c[1],c[2],(255.0 * clamp(alpha)).round_ties_even() as i64]})
    }
    fn bounds(&self, b: [f64; 4]) -> [f64; 4] {
        let [x0, y0, x1, y1] = b.map(|x| x * self.k);
        [x0, y0, x0.max(x1), y0.max(y1)]
    }
    pub fn rect(&mut self, b: [f64; 4], style: Rect<'_>) -> Result<()> {
        if style.alpha <= 0.0 {
            return Ok(());
        }
        let mut kw = json!({"fill":style.fill.map(|c|Self::rgba(c,style.alpha)),"outline":style.outline.map(|c|Self::rgba(c,style.alpha)),"width":1.max((style.width*self.k).round_ties_even() as i64)});
        let method = if style.radius != 0.0 {
            kw["radius"] = json!(style.radius * self.k);
            "rounded_rectangle"
        } else {
            "rectangle"
        };
        self.runtime.renderer.call(
            &format!("{}.{method}", self.draw),
            json!([self.bounds(b)]),
            kw,
        )?;
        Ok(())
    }
    pub fn circle(&mut self, cx: f64, cy: f64, r: f64, style: Rect<'_>) -> Result<()> {
        if style.alpha > 0.0 && r > 0.0 {
            self.rect(
                [cx - r, cy - r, cx + r, cy + r],
                Rect { radius: r, ..style },
            )?;
        }
        Ok(())
    }
    pub fn line(&mut self, points: &[[f64; 2]], color: &str, width: f64, alpha: f64) -> Result<()> {
        if alpha > 0.0 {
            let points: Vec<_> = points.iter().flatten().map(|x| x * self.k).collect();
            self.runtime.renderer.call(&format!("{}.line",self.draw),json!([points]),json!({"fill":Self::rgba(color,alpha),"width":1.max((width*self.k).round_ties_even() as i64),"joint":"curve"}))?;
        }
        Ok(())
    }
    pub fn picture(
        &mut self,
        name: &'static str,
        x: f64,
        cy: f64,
        height: f64,
        alpha: f64,
    ) -> Result<()> {
        let key = (name, height.to_bits(), self.k.to_bits());
        let image = if let Some(image) = self.runtime.pictures.get(&key) {
            image.clone()
        } else {
            let original = self.runtime.logo(name)?;
            let rgba = self.runtime.renderer.call(
                &format!("{original}.convert"),
                json!(["RGBA"]),
                json!({}),
            )?;
            let w = self
                .runtime
                .renderer
                .value(&format!("{rgba}.width"))?
                .as_f64()
                .ok_or("invalid logo width")?;
            let h0 = self
                .runtime
                .renderer
                .value(&format!("{rgba}.height"))?
                .as_f64()
                .ok_or("invalid logo height")?;
            let h = (height * self.k).round_ties_even();
            let resized = self.runtime.renderer.call(
                &format!("{rgba}.resize"),
                json!([
                    [(w * h / h0).round_ties_even() as i64, h as i64],
                    object("Image.Resampling.LANCZOS")
                ]),
                json!({}),
            )?;
            self.runtime.pictures.insert(key, resized.clone());
            resized
        };
        if alpha > 0.0 {
            let channel = self.runtime.renderer.call(
                &format!("{image}.getchannel"),
                json!(["A"]),
                json!({}),
            )?;
            let table: Vec<_> = (0..256)
                .map(|v| (f64::from(v) * clamp(alpha)).round_ties_even() as i64)
                .collect();
            let mask = self.runtime.renderer.call(
                &format!("{channel}.point"),
                json!([table]),
                json!({}),
            )?;
            let rgb = self.runtime.renderer.call(
                &format!("{image}.convert"),
                json!(["RGB"]),
                json!({}),
            )?;
            self.runtime.renderer.call(
                &format!("{}.paste", self.im),
                json!([
                    object(&rgb),
                    [
                        (x * self.k).round_ties_even() as i64,
                        ((cy - height / 2.0) * self.k).round_ties_even() as i64
                    ],
                    object(&mask)
                ]),
                json!({}),
            )?;
        }
        Ok(())
    }
    pub fn arc(
        &mut self,
        center: [f64; 2],
        r: f64,
        angles: [f64; 2],
        color: &str,
        width: f64,
        alpha: f64,
    ) -> Result<()> {
        if alpha > 0.0 {
            let [x, y] = center;
            self.runtime.renderer.call(&format!("{}.arc",self.draw),json!([self.bounds([x-r,y-r,x+r,y+r]),angles[0],angles[1]]),json!({"fill":Self::rgba(color,alpha),"width":1.max((width*self.k).round_ties_even() as i64)}))?;
        }
        Ok(())
    }
    pub fn text(&mut self, xy: [f64; 2], text: &str, size: f64, style: Text<'_>) -> Result<()> {
        if style.alpha > 0.0 {
            let font = self.font(size, style.style)?;
            self.runtime.renderer.call(&format!("{}.text",self.draw),json!([xy.map(|x|x*self.k),text]),json!({"font":object(&font),"fill":Self::rgba(style.color,style.alpha),"anchor":style.anchor}))?;
        }
        Ok(())
    }
    pub fn dashed(
        &mut self,
        a: [f64; 2],
        b: [f64; 2],
        color: &str,
        width: f64,
        alpha: f64,
    ) -> Result<()> {
        let length = (a[0] - b[0]).hypot(a[1] - b[1]);
        let mut s: f64 = 0.0;
        while s < length {
            let e = length.min(s + 5.0);
            let points = [s, e].map(|u| {
                [
                    a[0] + (b[0] - a[0]) * u / length,
                    a[1] + (b[1] - a[1]) * u / length,
                ]
            });
            self.line(&points, color, width, alpha)?;
            s = e + 4.0;
        }
        Ok(())
    }
    pub fn dashed_rect(&mut self, b: [f64; 4], color: &str, width: f64, alpha: f64) -> Result<()> {
        let [x0, y0, x1, y1] = b;
        for (a, b) in [
            ([x0, y0], [x1, y0]),
            ([x1, y0], [x1, y1]),
            ([x1, y1], [x0, y1]),
            ([x0, y1], [x0, y0]),
        ] {
            self.dashed(a, b, color, width, alpha)?;
        }
        Ok(())
    }
    pub fn check(&mut self, cx: f64, cy: f64, r: f64, color: &str, alpha: f64) -> Result<()> {
        self.line(
            &[[-1.0, 0.0], [-0.3, 0.7], [1.0, -0.7]].map(|[u, v]| [cx + r * u, cy + r * v]),
            color,
            2.2,
            alpha,
        )
    }
    pub fn loop_icon(&mut self, cx: f64, cy: f64, r: f64, color: &str, alpha: f64) -> Result<()> {
        self.arc([cx, cy], r, [-50.0, 250.0], color, 1.8, alpha)?;
        let a = 250.0_f64.to_radians();
        let tip = [cx + r * a.cos(), cy + r * a.sin()];
        let head = a.cos().atan2(-a.sin());
        let wings = [0.6, -0.6].map(|s| {
            [
                tip[0] - 4.5 * (head + s).cos(),
                tip[1] - 4.5 * (head + s).sin(),
            ]
        });
        self.line(&[wings[0], tip, wings[1]], color, 1.8, alpha)
    }
    pub fn chip(
        &mut self,
        x: f64,
        cy: f64,
        parts: &[(&str, f64, &str, &str)],
        fill: &str,
        edge: &str,
        options: Chip,
    ) -> Result<f64> {
        let lead = if options.icon { 20.0 } else { 0.0 };
        let mut widths = Vec::new();
        for (text, size, style, _) in parts {
            widths.push(self.width(text, *size, style)?);
        }
        let w = lead + sum_floats(widths) + 2.0 * options.pad;
        self.rect(
            [x, cy - 13.0, x + w, cy + 13.0],
            Rect {
                fill: Some(fill),
                outline: Some(edge),
                width: 1.4,
                radius: 5.0,
                alpha: options.alpha,
            },
        )?;
        if options.icon {
            self.loop_icon(
                x + options.pad + 7.0,
                cy,
                6.0,
                MECHS[2].color,
                options.alpha,
            )?;
        }
        let mut tx = x + options.pad + lead;
        for (text, size, style, color) in parts {
            self.text(
                [tx, cy],
                text,
                *size,
                Text {
                    style,
                    alpha: options.alpha,
                    ..Text::new(color)
                },
            )?;
            tx += self.width(text, *size, style)?;
        }
        if options.strike {
            self.line(
                &[[x + options.pad - 3.0, cy], [x + w - options.pad + 3.0, cy]],
                parts[0].3,
                1.3,
                options.alpha,
            )?;
        }
        Ok(x + w)
    }
}
pub(super) struct Chip {
    pub pad: f64,
    pub strike: bool,
    pub icon: bool,
    pub alpha: f64,
}
impl Default for Chip {
    fn default() -> Self {
        Self {
            pad: 9.0,
            strike: false,
            icon: false,
            alpha: 1.0,
        }
    }
}
