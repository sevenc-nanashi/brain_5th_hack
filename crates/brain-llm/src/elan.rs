//! Fixed ElanMT tiny English-to-Japanese Marian transformer, INT8 weights and activations.
use crate::{softmax, zeros};
use alloc::{string::String, vec, vec::Vec};
const D: usize = 256;
const H: usize = 1024;
const L: usize = 4;
const V: usize = 32001;
pub const LIMIT: usize = 128;
fn u32_at(b: &[u8], p: usize) -> u32 {
    u32::from_le_bytes(b[p..p + 4].try_into().unwrap())
}
fn f32_at(b: &[u8], p: usize) -> f32 {
    f32::from_bits(u32_at(b, p))
}
#[derive(Clone, Copy)]
struct Tensor {
    offset: usize,
    kind: u32,
    rows: usize,
    cols: usize,
}
struct Weights {
    bytes: Vec<u8>,
    tensors: Vec<Tensor>,
}
impl Weights {
    fn load(bytes: Vec<u8>) -> Result<Self, &'static str> {
        if bytes.len() < 12 + 171 * 16 || &bytes[..8] != b"BELAN001" || u32_at(&bytes, 8) != 171 {
            return Err("INVALID ELAN HEADER");
        }
        let mut tensors = Vec::new();
        tensors
            .try_reserve_exact(171)
            .map_err(|_| "OUT OF MEMORY")?;
        for i in 0..171 {
            let p = 12 + i * 16;
            let t = Tensor {
                offset: u32_at(&bytes, p) as usize,
                kind: u32_at(&bytes, p + 4),
                rows: u32_at(&bytes, p + 8) as usize,
                cols: u32_at(&bytes, p + 12) as usize,
            };
            let (kind, rows, cols) = match i {
                0 => (1, V, D),
                1 => (0, 512, D),
                2 => (0, 1, V),
                _ => {
                    let n = if i < 67 { (i - 3) % 16 } else { (i - 67) % 26 };
                    let ff = if i < 67 { 10 } else { 20 };
                    if n < ff {
                        match n % 10 {
                            0 | 2 | 4 | 6 => (1, D, D),
                            _ => (0, 1, D),
                        }
                    } else {
                        match n - ff {
                            0 => (1, H, D),
                            1 => (0, 1, H),
                            2 => (1, D, H),
                            _ => (0, 1, D),
                        }
                    }
                }
            };
            let len = if kind == 1 {
                rows * (4 + cols)
            } else {
                rows * cols * 4
            };
            if (t.kind, t.rows, t.cols) != (kind, rows, cols)
                || t.offset < 12 + 171 * 16
                || t.offset.checked_add(len).is_none_or(|e| e > bytes.len())
            {
                return Err("INVALID ELAN TENSOR");
            }
            tensors.push(t);
        }
        Ok(Self { bytes, tensors })
    }
    fn vector(&self, id: usize, i: usize) -> f32 {
        let t = self.tensors[id];
        assert!(t.kind == 0 && i < t.rows * t.cols);
        f32_at(&self.bytes, t.offset + i * 4)
    }
    fn embed(&self, token: usize, pos: usize, out: &mut [f32]) {
        assert!(token < V && pos < 512 && out.len() == D);
        let t = self.tensors[0];
        let p = t.offset + token * (D + 4);
        let scale = f32_at(&self.bytes, p) * 16.;
        for (i, o) in out.iter_mut().enumerate() {
            *o = self.bytes[p + 4 + i] as i8 as f32 * scale + self.vector(1, pos * D + i);
        }
    }
    fn matrix(&self, id: usize, x: &[f32], out: &mut [f32], first: usize) {
        let t = self.tensors[id];
        assert!(t.kind == 1 && x.len() == t.cols && first + out.len() <= t.rows);
        let max = x.iter().fold(0.0f32, |a, v| a.max(v.abs()));
        let scale = if max == 0. { 1. } else { max / 127. };
        let mut quant = [0i8; H];
        for (q, &v) in quant.iter_mut().zip(x) {
            *q = libm::roundf(v / scale).clamp(-127., 127.) as i8;
        }
        for (i, o) in out.iter_mut().enumerate() {
            let p = t.offset + (first + i) * (4 + t.cols);
            let mut sum = 0i32;
            for (&a, &w) in quant[..t.cols]
                .iter()
                .zip(&self.bytes[p + 4..p + 4 + t.cols])
            {
                sum += a as i32 * w as i8 as i32;
            }
            *o = sum as f32 * scale * f32_at(&self.bytes, p);
        }
    }
    fn linear(&self, id: usize, x: &[f32], out: &mut [f32]) {
        self.matrix(id, x, out, 0);
        for (i, o) in out.iter_mut().enumerate() {
            *o += self.vector(id + 1, i);
        }
    }
    fn norm(&self, id: usize, x: &mut [f32]) {
        let mean = x.iter().sum::<f32>() / D as f32;
        let variance = x.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / D as f32;
        let inv = 1. / libm::sqrtf(variance + 1e-5);
        for (i, v) in x.iter_mut().enumerate() {
            *v = (*v - mean) * inv * self.vector(id, i) + self.vector(id + 1, i);
        }
    }
    fn residual(&self, linear: usize, norm: usize, input: &[f32], x: &mut [f32]) {
        let mut out = [0.; D];
        self.linear(linear, input, &mut out);
        for (i, v) in x.iter_mut().enumerate() {
            *v += out[i];
        }
        self.norm(norm, x);
    }
    fn ffn(&self, id: usize, x: &mut [f32]) {
        let mut hidden = [0.; H];
        self.linear(id, x, &mut hidden);
        for h in &mut hidden {
            *h /= 1. + libm::expf(-*h);
        }
        self.residual(id + 2, id + 4, &hidden, x);
    }
}
fn attention(q: &[f32], k: &[f32], v: &[f32], out: &mut [f32]) {
    let n = k.len() / D;
    assert!(n > 0 && n <= LIMIT && v.len() == k.len());
    let mut scores = [0.; LIMIT];
    out.fill(0.);
    for h in 0..4 {
        for t in 0..n {
            let mut s = 0.;
            for i in 0..64 {
                s += q[h * 64 + i] * k[t * D + h * 64 + i];
            }
            scores[t] = s / 8.;
        }
        softmax(&mut scores[..n]);
        for t in 0..n {
            for i in 0..64 {
                out[h * 64 + i] += scores[t] * v[t * D + h * 64 + i];
            }
        }
    }
}
pub struct Elan {
    weights: Weights,
    pub tokenizer: Tokenizer,
    enc: Vec<f32>,
    eq: Vec<f32>,
    ek: Vec<f32>,
    ev: Vec<f32>,
    cross_k: Vec<f32>,
    cross_v: Vec<f32>,
    keys: Vec<f32>,
    values: Vec<f32>,
    x: Vec<f32>,
    source: usize,
    layer: usize,
    item: usize,
    stage: u8,
    pos: usize,
    row: usize,
    best: usize,
    best_score: f32,
}
impl Elan {
    pub fn load(model: Vec<u8>, tokenizer: Vec<u8>) -> Result<Self, &'static str> {
        Ok(Self {
            weights: Weights::load(model)?,
            tokenizer: Tokenizer::load(tokenizer)?,
            enc: zeros(LIMIT * D)?,
            eq: zeros(LIMIT * D)?,
            ek: zeros(LIMIT * D)?,
            ev: zeros(LIMIT * D)?,
            cross_k: zeros(L * LIMIT * D)?,
            cross_v: zeros(L * LIMIT * D)?,
            keys: zeros(L * LIMIT * D)?,
            values: zeros(L * LIMIT * D)?,
            x: zeros(D)?,
            source: 0,
            layer: 0,
            item: 0,
            stage: 5,
            pos: 0,
            row: 0,
            best: 0,
            best_score: f32::NEG_INFINITY,
        })
    }
    pub fn allocated_bytes(&self) -> usize {
        self.weights.bytes.capacity()
            + self.tokenizer.bytes.capacity()
            + (20 * LIMIT * D + D) * 4
            + self.weights.tensors.capacity() * core::mem::size_of::<Tensor>()
    }
    pub fn start(&mut self, prompt: &str) -> Result<(), &'static str> {
        let tokens = self.tokenizer.encode(prompt)?;
        self.source = tokens.len();
        self.layer = 0;
        self.item = 0;
        self.stage = 0;
        self.pos = 0;
        for (i, &token) in tokens.iter().enumerate() {
            self.weights
                .embed(token, i, &mut self.enc[i * D..(i + 1) * D]);
        }
        Ok(())
    }
    pub fn tick(&mut self) -> Result<Option<usize>, &'static str> {
        let w = &self.weights;
        let start = self.item * D;
        match self.stage {
            0 => {
                let b = 3 + self.layer * 16;
                let x = &self.enc[start..start + D];
                w.linear(b, x, &mut self.eq[start..start + D]);
                w.linear(b + 2, x, &mut self.ek[start..start + D]);
                w.linear(b + 4, x, &mut self.ev[start..start + D]);
                self.item += 1;
                if self.item == self.source {
                    self.item = 0;
                    self.stage = 1;
                }
            }
            1 => {
                let b = 3 + self.layer * 16;
                let mut att = [0.; D];
                attention(
                    &self.eq[start..start + D],
                    &self.ek[..self.source * D],
                    &self.ev[..self.source * D],
                    &mut att,
                );
                let x = &mut self.enc[start..start + D];
                w.residual(b + 6, b + 8, &att, x);
                w.ffn(b + 10, x);
                self.item += 1;
                if self.item == self.source {
                    self.item = 0;
                    self.layer += 1;
                    self.stage = 0;
                    if self.layer == L {
                        self.layer = 0;
                        self.stage = 2;
                    }
                }
            }
            2 => {
                let b = 67 + self.layer * 26 + 10;
                let offset = (self.layer * LIMIT + self.item) * D;
                let x = &self.enc[start..start + D];
                w.linear(b + 2, x, &mut self.cross_k[offset..offset + D]);
                w.linear(b + 4, x, &mut self.cross_v[offset..offset + D]);
                self.item += 1;
                if self.item == self.source {
                    self.item = 0;
                    self.layer += 1;
                    if self.layer == L {
                        self.layer = 0;
                        self.stage = 3;
                        w.embed(32000, 0, &mut self.x);
                    }
                }
            }
            3 => {
                let b = 67 + self.layer * 26;
                let off = (self.layer * LIMIT + self.pos) * D;
                let base = self.layer * LIMIT * D;
                let mut q = [0.; D];
                let mut att = [0.; D];
                w.linear(b, &self.x, &mut q);
                w.linear(b + 2, &self.x, &mut self.keys[off..off + D]);
                w.linear(b + 4, &self.x, &mut self.values[off..off + D]);
                attention(
                    &q,
                    &self.keys[base..off + D],
                    &self.values[base..off + D],
                    &mut att,
                );
                w.residual(b + 6, b + 8, &att, &mut self.x);
                w.linear(b + 10, &self.x, &mut q);
                attention(
                    &q,
                    &self.cross_k[base..base + self.source * D],
                    &self.cross_v[base..base + self.source * D],
                    &mut att,
                );
                w.residual(b + 16, b + 18, &att, &mut self.x);
                w.ffn(b + 20, &mut self.x);
                self.layer += 1;
                if self.layer == L {
                    self.stage = 4;
                    self.row = 0;
                    self.best = 0;
                    self.best_score = f32::NEG_INFINITY;
                }
            }
            4 => {
                let mut logits = [0.; 256];
                let count = 256.min(V - 1 - self.row);
                w.matrix(0, &self.x, &mut logits[..count], self.row);
                for (i, &v) in logits[..count].iter().enumerate() {
                    let score = v + w.vector(2, self.row + i);
                    if !score.is_finite() {
                        return Err("NON-FINITE INFERENCE");
                    }
                    if score > self.best_score {
                        self.best_score = score;
                        self.best = self.row + i;
                    }
                }
                self.row += count;
                if self.row == V - 1 {
                    let token = self.best;
                    self.pos += 1;
                    self.layer = 0;
                    if token == 0 || self.pos == LIMIT {
                        self.stage = 5;
                    } else {
                        w.embed(token, self.pos, &mut self.x);
                        self.stage = 3;
                    }
                    return Ok(Some(token));
                }
            }
            _ => return Err("TRANSLATION FINISHED"),
        }
        Ok(None)
    }
    pub fn progress(&self) -> (&'static str, usize, usize) {
        match self.stage {
            0 | 1 => ("ENCODE", self.layer + 1, L),
            2 => ("PREPARE", self.layer + 1, L),
            _ => ("TRANSLATE", self.pos + 1, LIMIT),
        }
    }
}
pub struct Tokenizer {
    bytes: Vec<u8>,
    scores: usize,
    indices: usize,
    blob: usize,
    count: usize,
    max: usize,
}
impl Tokenizer {
    pub fn load(bytes: Vec<u8>) -> Result<Self, &'static str> {
        let scores = 12 + (V + 1) * 4;
        let indices = scores + V * 4;
        if bytes.len() < indices || &bytes[..8] != b"BELANT01" {
            return Err("INVALID ELAN TOKENIZER");
        }
        let count = u32_at(&bytes, 8) as usize;
        if count > V || bytes.len() < indices + count * 4 {
            return Err("INVALID ELAN VOCABULARY");
        }
        let blob = indices + count * 4;
        let mut prev = 0;
        let mut max = 0;
        for i in 0..=V {
            let off = u32_at(&bytes, 12 + i * 4) as usize;
            if off < prev || off > bytes.len() - blob {
                return Err("INVALID TOKEN OFFSET");
            }
            max = max.max(off - prev);
            prev = off;
        }
        let t = Self {
            bytes,
            scores,
            indices,
            blob,
            count,
            max,
        };
        for i in 0..count {
            let id = u32_at(&t.bytes, indices + i * 4) as usize;
            if id >= V || !f32_at(&t.bytes, scores + id * 4).is_finite() {
                return Err("INVALID TOKEN INDEX");
            }
            core::str::from_utf8(t.piece(id)).map_err(|_| "INVALID TOKEN UTF8")?;
            if i > 0 && t.piece(u32_at(&t.bytes, indices + (i - 1) * 4) as usize) >= t.piece(id) {
                return Err("UNSORTED VOCABULARY");
            }
        }
        Ok(t)
    }
    pub fn piece(&self, id: usize) -> &[u8] {
        assert!(id < V);
        let a = u32_at(&self.bytes, 12 + id * 4) as usize;
        let b = u32_at(&self.bytes, 12 + (id + 1) * 4) as usize;
        &self.bytes[self.blob + a..self.blob + b]
    }
    fn lookup(&self, s: &[u8]) -> Option<usize> {
        let (mut lo, mut hi) = (0, self.count);
        while lo < hi {
            let mid = (lo + hi) / 2;
            let id = u32_at(&self.bytes, self.indices + mid * 4) as usize;
            match self.piece(id).cmp(s) {
                core::cmp::Ordering::Less => lo = mid + 1,
                core::cmp::Ordering::Greater => hi = mid,
                core::cmp::Ordering::Equal => return Some(id),
            }
        }
        None
    }
    pub fn encode(&self, prompt: &str) -> Result<Vec<usize>, &'static str> {
        if !prompt.is_ascii() || prompt.len() > 128 {
            return Err("ASCII INPUT ONLY (128 MAX)");
        }
        let mut normalized = String::new();
        for word in prompt.split_ascii_whitespace() {
            normalized.push('▁');
            normalized.push_str(word);
        }
        if normalized.is_empty() {
            return Err("ENTER ENGLISH TEXT");
        }
        let b = normalized.as_bytes();
        let mut score = vec![f32::NEG_INFINITY; b.len() + 1];
        let mut previous = vec![(0, 0); b.len() + 1];
        score[0] = 0.;
        let min = (0..self.count)
            .map(|i| {
                f32_at(
                    &self.bytes,
                    self.scores + u32_at(&self.bytes, self.indices + i * 4) as usize * 4,
                )
            })
            .fold(f32::INFINITY, f32::min)
            - 10.;
        for (start, c) in normalized.char_indices() {
            let mut single = false;
            for end in start + 1..=(start + self.max).min(b.len()) {
                if let Some(id) = self.lookup(&b[start..end]) {
                    single |= end == start + c.len_utf8();
                    let next = score[start] + f32_at(&self.bytes, self.scores + id * 4);
                    if next > score[end] {
                        score[end] = next;
                        previous[end] = (start, id);
                    }
                }
            }
            if !single {
                let end = start + c.len_utf8();
                let next = score[start] + min;
                if next > score[end] {
                    score[end] = next;
                    previous[end] = (start, 1);
                }
            }
        }
        let mut tokens = Vec::new();
        let mut end = b.len();
        while end > 0 {
            let (start, id) = previous[end];
            tokens.push(id);
            end = start;
        }
        tokens.reverse();
        tokens.dedup_by(|a, b| *a == 1 && *b == 1);
        tokens.push(0);
        if tokens.len() > LIMIT {
            return Err("INPUT TOO LONG");
        }
        Ok(tokens)
    }
    pub fn decode(&self, token: usize, first: bool) -> String {
        let s = core::str::from_utf8(self.piece(token))
            .expect("validated tokenizer")
            .replace('▁', " ");
        if first {
            String::from(s.trim_start_matches(' '))
        } else {
            s
        }
    }
}
