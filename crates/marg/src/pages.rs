//! Page textures: which renders the view needs, the texture cache, and
//! drawing pages with the best texture available.

use eframe::egui::{self, Color32, Pos2, Rect, Vec2};
use marg_core::config::RenderConfig;
use marg_core::lru::ByteLru;
use marg_core::view::View;
use marg_pdf::{Pixels, Recolor, RenderKey, Scale, Tile};

pub struct PageCache {
    textures: ByteLru<RenderKey, egui::TextureHandle>,
    /// Dark pages: how renders are wanted now.
    recolor: Option<Recolor>,
}

/// The renders wanted for the current frame, most urgent first.
pub struct Plan {
    pub keys: Vec<RenderKey>,
}

impl PageCache {
    pub fn new(cfg: &RenderConfig) -> Self {
        Self {
            textures: ByteLru::new(cfg.cache_mb * 1024 * 1024),
            recolor: None,
        }
    }

    /// Recolor the pages from now on (or not). Renders with the old colors
    /// stay as stand-ins until the new ones arrive.
    pub fn set_recolor(&mut self, recolor: Option<Recolor>) {
        self.recolor = recolor;
    }

    pub fn clear(&mut self) {
        self.textures.clear();
    }

    pub fn bytes(&self) -> usize {
        self.textures.bytes()
    }

    pub fn len(&self) -> usize {
        self.textures.len()
    }

    pub fn insert(&mut self, ctx: &egui::Context, key: RenderKey, px: Pixels) {
        let size = [px.width as usize, px.height as usize];
        let image = egui::ColorImage::from_rgba_premultiplied(size, &px.rgba);
        let name = format!("page{}r{}@{}{:?}", key.page, key.rev, key.scale.0, key.tile);
        let tex = ctx.load_texture(name, image, egui::TextureOptions::LINEAR);
        self.textures.insert(key, tex, px.rgba.len());
        // A full page after an edit replaces everything from before it.
        if key.tile.is_none() {
            self.textures
                .retain(|k| k.page != key.page || k.rev >= key.rev);
        }
    }

    /// Whether a full-page render of `page` at revision `rev` or later is
    /// cached.
    pub fn has_rev(&self, page: usize, rev: u32) -> bool {
        self.textures
            .iter()
            .any(|(k, _)| k.page == page && k.tile.is_none() && k.rev >= rev)
    }

    /// Plan the renders for this frame and mark the cached ones as used.
    /// Returns the keys not in the cache, most urgent first.
    pub fn plan(&mut self, view: &View, cfg: &RenderConfig, ppp: f32, revs: &[u32]) -> Plan {
        self.textures.next_frame();
        let recolor = self.recolor;
        let full = |page: usize, scale| full(page, revs[page], scale, recolor);
        let scale = Scale::from_f32(view.zoom() * ppp);
        let tiled = view.zoom() > cfg.tile_above_zoom;
        let visible = view.visible_pages();
        let mut wanted = Vec::new();
        if tiled {
            // Tiles of visible pages, plus a low-resolution full page under
            // them so a new page never shows blank.
            let low = Scale::from_f32(cfg.tile_above_zoom * ppp);
            for page in visible.clone() {
                wanted.extend(visible_tiles(
                    view,
                    page,
                    revs[page],
                    scale,
                    cfg.tile_px,
                    ppp,
                    recolor,
                ));
            }
            for page in visible.clone() {
                wanted.push(full(page, low));
            }
        } else {
            wanted.extend(visible.clone().map(|p| full(p, scale)));
            let n = cfg.prerender_pages;
            for d in 1..=n {
                if let Some(p) = visible.end().checked_add(d)
                    && p < view.page_count()
                {
                    wanted.push(full(p, scale));
                }
                if let Some(p) = visible.start().checked_sub(d) {
                    wanted.push(full(p, scale));
                }
            }
        }
        let keys = wanted
            .into_iter()
            .filter(|k| self.textures.get(k).is_none())
            .collect();
        Plan { keys }
    }

    /// Draw the visible pages into `area` (the page view on screen).
    /// Returns whether every visible page was drawn from a full-page
    /// texture at the exact scale (not tiled, no fallback), and the visible
    /// pages drawn from a render older than their revision in `revs` (or
    /// not at all). `edge`: the line around each page.
    pub fn draw(
        &mut self,
        painter: &egui::Painter,
        area: Rect,
        view: &View,
        cfg: &RenderConfig,
        revs: &[u32],
        edge: Color32,
    ) -> (bool, Vec<usize>) {
        let mut all_exact = true;
        let mut stale = Vec::new();
        let ppp = painter.pixels_per_point();
        let scale = Scale::from_f32(view.zoom() * ppp);
        let tiled = view.zoom() > cfg.tile_above_zoom;
        // Under pages still rendering.
        let paper = self.recolor.map_or(Color32::WHITE, |r| {
            Color32::from_rgb(r.bg[0], r.bg[1], r.bg[2])
        });
        for page in view.visible_pages() {
            let r = view.page_rect_on_screen(page);
            // Snap the page origin to device pixels so 1:1 textures are sharp.
            let snap = |v: f32| (v * ppp).round() / ppp;
            let origin = Pos2::new(snap(area.min.x + r.x), snap(area.min.y + r.y));
            let page_rect = Rect::from_min_size(origin, Vec2::new(r.w, r.h));
            painter.rect_filled(page_rect, 0.0, paper);

            let rev = revs[page];
            let exact = full(page, rev, scale, self.recolor);
            if !tiled && let Some(tex) = self.textures.get(&exact) {
                let size = tex.size_vec2() / ppp;
                painter.image(
                    tex.id(),
                    Rect::from_min_size(origin, size),
                    UV,
                    Color32::WHITE,
                );
            } else {
                all_exact = false;
                let fallback = self.best_fallback(page, rev, scale);
                if fallback.is_none_or(|k| k.rev < rev) {
                    stale.push(page);
                }
                if let Some(key) = fallback
                    && let Some(tex) = self.textures.get(&key)
                {
                    painter.image(tex.id(), page_rect, UV, Color32::WHITE);
                }
                if tiled {
                    self.draw_tiles(painter, origin, exact, cfg.tile_px, ppp);
                }
            }
            painter.rect_stroke(
                page_rect,
                0.0,
                egui::Stroke::new(1.0 / ppp, edge),
                egui::StrokeKind::Outside,
            );
        }
        (all_exact, stale)
    }

    /// Draw the cached tiles of `want`'s page, revision and scale.
    fn draw_tiles(
        &mut self,
        painter: &egui::Painter,
        origin: Pos2,
        want: RenderKey,
        tile_px: u32,
        ppp: f32,
    ) {
        let tiles: Vec<RenderKey> = self
            .textures
            .iter()
            .filter(|(k, _)| {
                k.page == want.page
                    && k.recolor == want.recolor
                    && k.rev == want.rev
                    && k.scale == want.scale
                    && k.tile.is_some()
            })
            .map(|(k, _)| *k)
            .collect();
        for key in tiles {
            let Some(t) = key.tile.filter(|t| t.size == tile_px) else {
                continue;
            };
            let Some(tex) = self.textures.get(&key) else {
                continue;
            };
            let min = origin + Vec2::new((t.col * t.size) as f32, (t.row * t.size) as f32) / ppp;
            let rect = Rect::from_min_size(min, tex.size_vec2() / ppp);
            painter.image(tex.id(), rect, UV, Color32::WHITE);
        }
    }

    /// The cached full-page texture of `page` closest in scale to
    /// `target`, preferring the current colors, then revision `rev`.
    fn best_fallback(&self, page: usize, rev: u32, target: Scale) -> Option<RenderKey> {
        let t = target.get();
        self.textures
            .iter()
            .map(|(k, _)| *k)
            .filter(|k| k.page == page && k.tile.is_none())
            .min_by(|a, b| {
                let da = (a.scale.get() / t).ln().abs();
                let db = (b.scale.get() / t).ln().abs();
                let other = |k: &RenderKey| (k.recolor != self.recolor, k.rev != rev);
                other(a).cmp(&other(b)).then(da.total_cmp(&db))
            })
    }
}

const UV: Rect = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));

fn full(page: usize, rev: u32, scale: Scale, recolor: Option<Recolor>) -> RenderKey {
    RenderKey {
        recolor,
        page,
        rev,
        scale,
        tile: None,
    }
}

/// Tiles of `page` that intersect the viewport, plus a one-tile margin.
fn visible_tiles(
    view: &View,
    page: usize,
    rev: u32,
    scale: Scale,
    size: u32,
    ppp: f32,
    recolor: Option<Recolor>,
) -> impl Iterator<Item = RenderKey> {
    let r = view.page_rect_on_screen(page);
    let (vw, vh) = view.viewport();
    let (pw, ph) = view.page_size(page);
    let s = scale.get();
    let (cols, rows) = (
        ((pw * s).ceil() as u32).div_ceil(size),
        ((ph * s).ceil() as u32).div_ceil(size),
    );
    // Visible part of the page in device pixels from its top-left corner.
    let x0 = (-r.x).max(0.0) * ppp;
    let x1 = (vw - r.x).min(r.w).max(0.0) * ppp;
    let y0 = (-r.y).max(0.0) * ppp;
    let y1 = (vh - r.y).min(r.h).max(0.0) * ppp;
    let range = |a: f32, b: f32, n: u32| {
        let first = ((a / size as f32) as u32).saturating_sub(1);
        let last = ((b / size as f32) as u32 + 1).min(n.saturating_sub(1));
        first..=last
    };
    let (col_range, row_range) = (range(x0, x1, cols), range(y0, y1, rows));
    row_range.flat_map(move |row| {
        col_range.clone().map(move |col| RenderKey {
            recolor,
            page,
            rev,
            scale,
            tile: Some(Tile { col, row, size }),
        })
    })
}
