//! Rasterizador CPU mínimo para las mallas de egui.
//!
//! Evita cargar un driver OpenGL/DirectX (~50 MB de memoria comprometida
//! en equipos con GPU híbrida) para pintar una ventana pequeña.
//! Trabaja en espacio gamma con alfa premultiplicado, igual que egui_glow,
//! y escribe BGRA premultiplicado listo para `UpdateLayeredWindow`.

use egui::epaint::{ClippedPrimitive, ImageData, Primitive, TextureId, Vertex, textures::TexturesDelta};
use egui::Color32;
use std::collections::HashMap;

struct Texture {
    w: usize,
    h: usize,
    px: Vec<Color32>,
}

#[derive(Default)]
pub struct Raster {
    textures: HashMap<TextureId, Texture>,
}

#[derive(Clone, Copy)]
struct Clip {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

impl Raster {
    pub fn set_textures(&mut self, delta: &TexturesDelta) {
        for (id, deltas) in &delta.set {
            for d in deltas {
                let ImageData::Color(img) = &d.image;
                let [w, h] = img.size;
                match d.pos {
                    None => {
                        self.textures.insert(*id, Texture { w, h, px: img.pixels.clone() });
                    }
                    Some([x, y]) => {
                        if let Some(t) = self.textures.get_mut(id) {
                            for row in 0..h {
                                let dst = (y + row) * t.w + x;
                                let src = row * w;
                                if y + row < t.h && x + w <= t.w {
                                    t.px[dst..dst + w].copy_from_slice(&img.pixels[src..src + w]);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn free_textures(&mut self, delta: &TexturesDelta) {
        for id in &delta.free {
            self.textures.remove(id);
        }
    }

    /// Pinta en `buf` (BGRA premultiplicado, `w`×`h` píxeles).
    pub fn paint(&self, prims: &[ClippedPrimitive], buf: &mut [u32], w: usize, h: usize, ppp: f32) {
        buf.fill(0);
        for p in prims {
            let Primitive::Mesh(mesh) = &p.primitive else { continue };
            let r = p.clip_rect;
            let clip = Clip {
                x0: ((r.min.x * ppp).round() as i32).max(0),
                y0: ((r.min.y * ppp).round() as i32).max(0),
                x1: ((r.max.x * ppp).round() as i32).min(w as i32),
                y1: ((r.max.y * ppp).round() as i32).min(h as i32),
            };
            if clip.x0 >= clip.x1 || clip.y0 >= clip.y1 {
                continue;
            }
            let tex = self.textures.get(&mesh.texture_id);
            for t in mesh.indices.chunks_exact(3) {
                let v = [
                    &mesh.vertices[t[0] as usize],
                    &mesh.vertices[t[1] as usize],
                    &mesh.vertices[t[2] as usize],
                ];
                triangle(buf, w, clip, v, tex, ppp);
            }
        }
    }
}

#[inline]
fn sample(t: &Texture, u: f32, v: f32) -> [f32; 4] {
    let x = (u * t.w as f32 - 0.5).clamp(0.0, (t.w - 1) as f32);
    let y = (v * t.h as f32 - 0.5).clamp(0.0, (t.h - 1) as f32);
    let (x0, y0) = (x as usize, y as usize);
    let (x1, y1) = ((x0 + 1).min(t.w - 1), (y0 + 1).min(t.h - 1));
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let p = |x: usize, y: usize| t.px[y * t.w + x].to_array();
    let (a, b, c, d) = (p(x0, y0), p(x1, y0), p(x0, y1), p(x1, y1));
    let mut out = [0.0; 4];
    for i in 0..4 {
        let top = a[i] as f32 + (b[i] as f32 - a[i] as f32) * fx;
        let bot = c[i] as f32 + (d[i] as f32 - c[i] as f32) * fx;
        out[i] = top + (bot - top) * fy;
    }
    out
}

/// src y dst premultiplicados. `src` en orden RGBA (0..255), `dst` BGRA empaquetado.
#[inline]
fn blend(dst: &mut u32, r: f32, g: f32, b: f32, a: f32) {
    if a <= 0.0 && r <= 0.0 && g <= 0.0 && b <= 0.0 {
        return;
    }
    let inv = 1.0 - a / 255.0;
    let d = *dst;
    let db = (d & 0xFF) as f32;
    let dg = ((d >> 8) & 0xFF) as f32;
    let dr = ((d >> 16) & 0xFF) as f32;
    let da = ((d >> 24) & 0xFF) as f32;
    let ob = (b + db * inv + 0.5).min(255.0) as u32;
    let og = (g + dg * inv + 0.5).min(255.0) as u32;
    let or = (r + dr * inv + 0.5).min(255.0) as u32;
    let oa = (a + da * inv + 0.5).min(255.0) as u32;
    *dst = ob | (og << 8) | (or << 16) | (oa << 24);
}

/// RGBA premultiplicado (f32) → BGRA empaquetado.
fn pack(c: [f32; 4]) -> u32 {
    let q = |x: f32| (x + 0.5).clamp(0.0, 255.0) as u32;
    q(c[2]) | (q(c[1]) << 8) | (q(c[0]) << 16) | (q(c[3]) << 24)
}

/// Mezcla "over" entera para colores constantes; los opacos se escriben directo.
#[inline]
fn blend_int(dst: &mut u32, src: u32) {
    let a = src >> 24;
    if a == 255 {
        *dst = src;
        return;
    }
    if src == 0 {
        return;
    }
    let inv = 255 - a;
    let d = *dst;
    // Dos canales a la vez (B,R) y (G,A) en 16 bits cada uno.
    let rb = ((d & 0x00FF_00FF) * inv + 0x0080_0080) as u64;
    let ga = (((d >> 8) & 0x00FF_00FF) * inv + 0x0080_0080) as u64;
    let rb = (((rb + ((rb >> 8) & 0x00FF_00FF)) >> 8) & 0x00FF_00FF) as u32;
    let ga = (((ga + ((ga >> 8) & 0x00FF_00FF)) >> 8) & 0x00FF_00FF) as u32;
    let scaled = rb | (ga << 8);
    // Suma por canal saturada (premultiplicado: no debería desbordar).
    let mut out = 0u32;
    for sh in [0, 8, 16, 24] {
        let v = ((src >> sh) & 0xFF) + ((scaled >> sh) & 0xFF);
        out |= v.min(255) << sh;
    }
    *dst = out;
}

#[inline]
fn edge(ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32) -> f32 {
    (bx - ax) * (py - ay) - (by - ay) * (px - ax)
}

/// Regla top-left: dos triángulos que comparten arista no pintan dos veces
/// el mismo píxel (evita líneas en diagonal en rellenos semitransparentes).
#[inline]
fn is_top_left(ax: f32, ay: f32, bx: f32, by: f32) -> bool {
    (ay == by && bx > ax) || by < ay
}

fn triangle(buf: &mut [u32], w: usize, clip: Clip, v: [&Vertex; 3], tex: Option<&Texture>, ppp: f32) {
    let mut p = [
        (v[0].pos.x * ppp, v[0].pos.y * ppp),
        (v[1].pos.x * ppp, v[1].pos.y * ppp),
        (v[2].pos.x * ppp, v[2].pos.y * ppp),
    ];
    let mut v = v;
    let mut area = edge(p[0].0, p[0].1, p[1].0, p[1].1, p[2].0, p[2].1);
    if area.abs() < 1e-6 {
        return;
    }
    if area < 0.0 {
        p.swap(1, 2);
        v.swap(1, 2);
        area = -area;
    }
    let minx = p.iter().map(|q| q.0).fold(f32::MAX, f32::min).floor() as i32;
    let maxx = p.iter().map(|q| q.0).fold(f32::MIN, f32::max).ceil() as i32;
    let miny = p.iter().map(|q| q.1).fold(f32::MAX, f32::min).floor() as i32;
    let maxy = p.iter().map(|q| q.1).fold(f32::MIN, f32::max).ceil() as i32;
    let (x0, x1) = (minx.max(clip.x0), maxx.min(clip.x1));
    let (y0, y1) = (miny.max(clip.y0), maxy.min(clip.y1));
    if x0 >= x1 || y0 >= y1 {
        return;
    }

    // Aristas: e0 = (p1→p2), e1 = (p2→p0), e2 = (p0→p1); peso de vértice i = ei/area.
    let edges = [(1, 2), (2, 0), (0, 1)];
    let tl = edges.map(|(a, b)| is_top_left(p[a].0, p[a].1, p[b].0, p[b].1));
    let step_x = edges.map(|(a, b)| -(p[b].1 - p[a].1));

    let c: [[f32; 4]; 3] = v.map(|v| v.color.to_array().map(|x| x as f32));
    let uniform_color = v[0].color == v[1].color && v[1].color == v[2].color;
    let uniform_uv = v[0].uv == v[1].uv && v[1].uv == v[2].uv;

    // Ruta rápida: color y textura constantes (rellenos, la mayoría de la UI).
    let solid: Option<[f32; 4]> = (uniform_color && (tex.is_none() || uniform_uv)).then(|| {
        let mut col = c[0];
        if let Some(t) = tex {
            let s = sample(t, v[0].uv.x, v[0].uv.y);
            for i in 0..4 {
                col[i] = col[i] * s[i] / 255.0;
            }
        }
        col
    });
    if solid.is_some_and(|c| c.iter().all(|&x| x < 0.5)) {
        return;
    }

    let inv_area = 1.0 / area;
    let solid_px = solid.map(pack);
    for y in y0..y1 {
        let py = y as f32 + 0.5;
        let px0 = x0 as f32 + 0.5;
        let mut e = edges.map(|(a, b)| edge(p[a].0, p[a].1, p[b].0, p[b].1, px0, py));

        // Tramo [xa, xb) de la fila que puede estar dentro: evita recorrer
        // la mitad vacía del rectángulo envolvente.
        let (mut xa, mut xb) = (x0 as f32, x1 as f32);
        let mut empty = false;
        for i in 0..3 {
            let (e0, s) = (e[i], step_x[i]);
            if s == 0.0 {
                if e0 < 0.0 || (e0 == 0.0 && !tl[i]) {
                    empty = true;
                }
            } else {
                let cross = x0 as f32 - e0 / s;
                if s > 0.0 {
                    xa = xa.max(cross.floor() - 1.0);
                } else {
                    xb = xb.min(cross.ceil() + 1.0);
                }
            }
        }
        if empty || xa >= xb {
            continue;
        }
        let (sx, ex) = ((xa as i32).max(x0), (xb as i32).min(x1));
        let skip = (sx - x0) as f32;
        for i in 0..3 {
            e[i] += step_x[i] * skip;
        }

        let row = y as usize * w;
        for x in sx..ex {
            let inside = (0..3).all(|i| if tl[i] { e[i] >= 0.0 } else { e[i] > 0.0 });
            if inside {
                let dst = &mut buf[row + x as usize];
                match solid_px {
                    Some(src) => blend_int(dst, src),
                    None => {
                        let (w0, w1, w2) = (e[0] * inv_area, e[1] * inv_area, e[2] * inv_area);
                        let mut col = [0.0f32; 4];
                        for i in 0..4 {
                            col[i] = c[0][i] * w0 + c[1][i] * w1 + c[2][i] * w2;
                        }
                        if let Some(t) = tex {
                            let u = v[0].uv.x * w0 + v[1].uv.x * w1 + v[2].uv.x * w2;
                            let vv = v[0].uv.y * w0 + v[1].uv.y * w1 + v[2].uv.y * w2;
                            let s = sample(t, u, vv);
                            for i in 0..4 {
                                col[i] = col[i] * s[i] / 255.0;
                            }
                        }
                        blend(dst, col[0], col[1], col[2], col[3]);
                    }
                }
            }
            for i in 0..3 {
                e[i] += step_x[i];
            }
        }
    }
}
