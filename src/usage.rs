//! Frecuencia + recencia de uso, y "aprendizaje" de qué elige el usuario
//! para cada consulta (si con "c" abres Chrome, "c" acabará en Chrome).

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Default)]
pub struct Usage {
    items: HashMap<String, (u32, u64)>,
    /// consulta → (clave elegida, veces)
    queries: HashMap<String, (String, u32)>,
    path: PathBuf,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl Usage {
    pub fn load(path: PathBuf) -> Self {
        let mut u = Usage { path, ..Default::default() };
        if let Ok(text) = std::fs::read_to_string(&u.path) {
            for line in text.lines() {
                let mut it = line.split('\t');
                match (it.next(), it.next(), it.next(), it.next()) {
                    (Some("i"), Some(c), Some(t), Some(k)) => {
                        if let (Ok(c), Ok(t)) = (c.parse(), t.parse()) {
                            u.items.insert(k.to_string(), (c, t));
                        }
                    }
                    (Some("q"), Some(c), Some(q), Some(k)) => {
                        if let Ok(c) = c.parse() {
                            u.queries.insert(q.to_string(), (k.to_string(), c));
                        }
                    }
                    _ => {}
                }
            }
        }
        u
    }

    pub fn save(&self) {
        let mut s = String::new();
        for (k, (c, t)) in &self.items {
            s.push_str(&format!("i\t{c}\t{t}\t{k}\n"));
        }
        for (q, (k, c)) in &self.queries {
            s.push_str(&format!("q\t{c}\t{q}\t{k}\n"));
        }
        let _ = std::fs::write(&self.path, s);
    }

    pub fn record(&mut self, key: &str, query: &str) {
        let e = self.items.entry(key.to_string()).or_insert((0, 0));
        e.0 = e.0.saturating_add(1);
        e.1 = now();
        let q = query.trim().to_lowercase();
        if !q.is_empty() && !q.contains('\t') {
            let entry = self.queries.entry(q).or_insert_with(|| (key.to_string(), 0));
            if entry.0 == key {
                entry.1 += 1;
            } else {
                *entry = (key.to_string(), 1);
            }
        }
        // Evita crecer sin límite.
        if self.items.len() > 5000 {
            let mut v: Vec<_> = self.items.iter().map(|(k, (_, t))| (*t, k.clone())).collect();
            v.sort();
            for (_, k) in v.into_iter().take(1000) {
                self.items.remove(&k);
            }
        }
        self.save();
    }

    pub fn bonus(&self, key: &str, query_lower: &str) -> i32 {
        let mut b = 0;
        if let Some((count, last)) = self.items.get(key) {
            b += (18.0 * (1.0 + *count as f32).ln()) as i32;
            let age_h = now().saturating_sub(*last) / 3600;
            b += match age_h {
                0..=1 => 25,
                2..=24 => 15,
                25..=168 => 6,
                _ => 0,
            };
        }
        if let Some((k, c)) = self.queries.get(query_lower.trim()) {
            if k == key {
                b += 120 + (*c).min(10) as i32 * 5;
            }
        }
        b
    }
}
