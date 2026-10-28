//! Calculadora y conversor de unidades (fend): "2^10", "sqrt(2)*pi",
//! "5 km to mi", "0xff to decimal", "30% of 250"… Enter copia el resultado.

use super::{Action, Glyph, Icon, Item, Mode, Plugin, Query};
use parking_lot::Mutex;
use std::time::{Duration, Instant};

pub struct Calc {
    ctx: Mutex<fend_core::Context>,
}

struct Deadline(Instant);

impl fend_core::Interrupt for Deadline {
    fn should_interrupt(&self) -> bool {
        Instant::now() > self.0
    }
}

impl Calc {
    pub fn new() -> Self {
        Self { ctx: Mutex::new(fend_core::Context::new()) }
    }
}

fn looks_like_math(s: &str) -> bool {
    let has_digit = s.bytes().any(|b| b.is_ascii_digit());
    let lower = s.to_ascii_lowercase();
    let has_fn = ["sqrt", "pi", "sin", "cos", "tan", "log", "ln", "abs", "exp"].iter().any(|f| lower.contains(f));
    has_digit || has_fn
}

/// "3,5 * 2" → "3.5 * 2" cuando la coma se usa como decimal.
fn normalize(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    for (i, c) in s.char_indices() {
        if c == ','
            && i > 0
            && i + 1 < b.len()
            && b[i - 1].is_ascii_digit()
            && b[i + 1].is_ascii_digit()
            && !s[i + 1..].chars().take_while(|c| c.is_ascii_digit()).count().eq(&3)
        {
            out.push('.');
        } else if c == '×' {
            out.push('*');
        } else if c == '÷' {
            out.push('/');
        } else {
            out.push(c);
        }
    }
    out
}

impl Plugin for Calc {
    fn query(&self, q: &Query, out: &mut Vec<Item>) {
        if !matches!(q.mode, Mode::All | Mode::Calc) || q.text.is_empty() {
            return;
        }
        if q.mode == Mode::All && !looks_like_math(q.text) {
            return;
        }
        let input = normalize(q.text);
        let ctx = self.ctx.lock();
        let deadline = Deadline(Instant::now() + Duration::from_millis(60));
        let res = fend_core::evaluate_preview_with_interrupt(&input, &ctx, &deadline);
        let result = res.get_main_result().trim().to_string();
        if result.is_empty() {
            return;
        }
        // "2025" → "2025" no aporta nada.
        if result.replace(' ', "") == input.trim().replace(' ', "") {
            return;
        }
        let mut it = Item::new(
            result.clone(),
            format!("{} · Enter para copiar", q.text.trim()),
            Icon::Glyph(Glyph::Calc),
            Action::Copy(result),
            10_000,
        );
        it.complete = Some(it.title.clone());
        out.push(it);
    }
}
