//! TinyStories 260K inference, ported from karpathy/llama2.c (MIT).
#![no_std]
extern crate alloc;
pub mod elan;
use alloc::{string::String, vec, vec::Vec};
use sha2::{Digest, Sha256};
const D: usize = 64;
const H: usize = 172;
const KV: usize = 32;
const SEQ: usize = 512;
fn zeros(n: usize) -> Result<Vec<f32>, &'static str> {
    let mut v = Vec::new();
    v.try_reserve_exact(n).map_err(|_| "Out of memory")?;
    v.resize(n, 0.);
    Ok(v)
}
fn norm(x: &[f32], w: &[f32], out: &mut [f32]) {
    let mut ss = 0.;
    for &v in x {
        ss += v * v;
    }
    let scale = 1. / libm::sqrtf(ss / x.len() as f32 + 1e-5);
    for i in 0..x.len() {
        out[i] = w[i] * (scale * x[i]);
    }
}
fn mat(x: &[f32], w: &[f32], out: &mut [f32]) {
    for (i, o) in out.iter_mut().enumerate() {
        let mut v = 0.;
        for (j, &a) in x.iter().enumerate() {
            v += w[i * x.len() + j] * a;
        }
        *o = v;
    }
}
fn softmax(x: &mut [f32]) {
    let max = x.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut sum = 0.;
    for v in x.iter_mut() {
        *v = libm::expf(*v - max);
        sum += *v;
    }
    for v in x {
        *v /= sum;
    }
}
pub struct Model {
    weights: Vec<f32>,
    keys: Vec<f32>,
    values: Vec<f32>,
}
impl Model {
    pub fn load(bytes: &[u8]) -> Result<Self, &'static str> {
        if Sha256::digest(bytes).as_slice()
            != [
                0xb0, 0xa5, 0x07, 0xe7, 0xad, 0x0f, 0x62, 0x66, 0x24, 0xf1, 0x71, 0x12, 0x32, 0x5e,
                0x66, 0x69, 0x1f, 0x90, 0x76, 0xd6, 0x22, 0xe1, 0xd3, 0x27, 0x4d, 0x10, 0x3d, 0x00,
                0x29, 0x9f, 0x26, 0x96,
            ]
        {
            return Err("File checksum mismatch");
        }
        if bytes.len() != 1056540 {
            return Err("Invalid model length");
        }
        for (i, w) in [64i32, 172, 5, 8, 4, 512, 512].iter().enumerate() {
            if bytes[i * 4..i * 4 + 4] != w.to_le_bytes() {
                return Err("Unsupported model");
            }
        }
        let mut weights = zeros((bytes.len() - 28) / 4)?;
        for (v, b) in weights
            .iter_mut()
            .zip(bytes[28..].as_chunks::<4>().0.iter())
        {
            *v = f32::from_le_bytes(*b);
            if !v.is_finite() {
                return Err("Non-finite model weight");
            }
        }
        Ok(Self {
            weights,
            keys: zeros(5 * SEQ * KV)?,
            values: zeros(5 * SEQ * KV)?,
        })
    }
    pub fn reset(&mut self) {
        self.keys.fill(0.);
        self.values.fill(0.);
    }
    pub fn allocated_bytes(&self) -> usize {
        (self.weights.capacity() + self.keys.capacity() + self.values.capacity()) * 4
    }
    pub fn forward(&mut self, token: usize, pos: usize) -> usize {
        assert!(token < 512 && pos < SEQ);
        let w = &self.weights;
        let mut x = [0.; D];
        x.copy_from_slice(&w[token * D..(token + 1) * D]);
        let mut xb = [0.; D];
        let mut xb2 = [0.; D];
        let mut q = [0.; D];
        let mut hb = [0.; H];
        let mut hb2 = [0.; H];
        let mut att = [0.; SEQ];
        let rms_att = 512 * D;
        let wq = rms_att + 5 * D;
        let wk = wq + 5 * D * D;
        let wv = wk + 5 * D * KV;
        let wo = wv + 5 * D * KV;
        let rms_ffn = wo + 5 * D * D;
        let w1 = rms_ffn + 5 * D;
        let w2 = w1 + 5 * D * H;
        let w3 = w2 + 5 * D * H;
        let rms_final = w3 + 5 * D * H;
        for l in 0..5 {
            norm(&x, &w[rms_att + l * D..], &mut xb);
            let off = (l * SEQ + pos) * KV;
            mat(&xb, &w[wq + l * D * D..], &mut q);
            mat(&xb, &w[wk + l * D * KV..], &mut self.keys[off..off + KV]);
            mat(&xb, &w[wv + l * D * KV..], &mut self.values[off..off + KV]);
            for i in (0..D).step_by(2) {
                let freq = 1. / libm::powf(10000., (i % 8) as f32 / 8.);
                let c = libm::cosf(pos as f32 * freq);
                let s = libm::sinf(pos as f32 * freq);
                let a = q[i];
                let b = q[i + 1];
                q[i] = a * c - b * s;
                q[i + 1] = a * s + b * c;
                if i < KV {
                    let a = self.keys[off + i];
                    let b = self.keys[off + i + 1];
                    self.keys[off + i] = a * c - b * s;
                    self.keys[off + i + 1] = a * s + b * c;
                }
            }
            for h in 0..8 {
                for (t, a) in att.iter_mut().enumerate().take(pos + 1) {
                    let k = (l * SEQ + t) * KV + (h / 2) * 8;
                    let mut score = 0.;
                    for j in 0..8 {
                        score += q[h * 8 + j] * self.keys[k + j];
                    }
                    *a = score / libm::sqrtf(8.);
                }
                softmax(&mut att[..=pos]);
                xb[h * 8..h * 8 + 8].fill(0.);
                for (t, &a) in att.iter().enumerate().take(pos + 1) {
                    let v = (l * SEQ + t) * KV + (h / 2) * 8;
                    for j in 0..8 {
                        xb[h * 8 + j] += a * self.values[v + j];
                    }
                }
            }
            mat(&xb, &w[wo + l * D * D..], &mut xb2);
            for i in 0..D {
                x[i] += xb2[i];
            }
            norm(&x, &w[rms_ffn + l * D..], &mut xb);
            mat(&xb, &w[w1 + l * D * H..], &mut hb);
            mat(&xb, &w[w3 + l * D * H..], &mut hb2);
            for i in 0..H {
                hb[i] *= 1. / (1. + libm::expf(-hb[i]));
                hb[i] *= hb2[i];
            }
            mat(&hb, &w[w2 + l * D * H..], &mut xb);
            for i in 0..D {
                x[i] += xb[i];
            }
        }
        norm(&x, &w[rms_final..], &mut xb);
        let mut logits = [0.; 512];
        mat(&xb, w, &mut logits);
        let mut best = 0;
        for i in 1..512 {
            if logits[i] > logits[best] {
                best = i;
            }
        }
        best
    }
}
pub struct Tokenizer {
    vocab: Vec<Vec<u8>>,
    scores: Vec<f32>,
}
impl Tokenizer {
    pub fn load(bytes: &[u8]) -> Result<Self, &'static str> {
        if Sha256::digest(bytes).as_slice()
            != [
                0x03, 0x7c, 0xb3, 0x35, 0xab, 0xb2, 0x5d, 0x1f, 0xa9, 0xe8, 0xec, 0xae, 0x30, 0xed,
                0x2a, 0x3a, 0x8a, 0xce, 0x93, 0x02, 0x86, 0x2e, 0xbc, 0xdc, 0x05, 0xd5, 0x1a, 0x6b,
                0xbb, 0x10, 0xc3, 0x12,
            ]
        {
            return Err("File checksum mismatch");
        }
        if bytes.len() < 4 {
            return Err("Invalid tokenizer");
        }
        let max = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
        if max == 0 || max > 128 {
            return Err("Invalid token size");
        }
        let mut vocab = Vec::new();
        let mut scores = Vec::new();
        vocab.try_reserve_exact(512).map_err(|_| "Out of memory")?;
        scores.try_reserve_exact(512).map_err(|_| "Out of memory")?;
        let mut p = 4;
        for _ in 0..512 {
            let header = bytes.get(p..p + 8).ok_or("Truncated tokenizer")?;
            let score = f32::from_le_bytes(header[..4].try_into().unwrap());
            let n = u32::from_le_bytes(header[4..].try_into().unwrap()) as usize;
            p += 8;
            if n > max || !score.is_finite() {
                return Err("Invalid tokenizer entry");
            }
            let raw = bytes.get(p..p + n).ok_or("Truncated token")?;
            let mut token = Vec::new();
            token.try_reserve_exact(n).map_err(|_| "Out of memory")?;
            token.extend_from_slice(raw);
            vocab.push(token);
            scores.push(score);
            p += n;
        }
        if p != bytes.len() || !vocab.iter().any(|v| v == b" ") {
            return Err("Invalid tokenizer");
        }
        Ok(Self { vocab, scores })
    }
    pub fn encode(&self, text: &str) -> Vec<usize> {
        assert!(text.is_ascii() && text.len() <= 128);
        let mut tokens = vec![1];
        if !text.is_empty() {
            tokens.push(self.vocab.iter().position(|v| v == b" ").unwrap());
        }
        for b in text.bytes() {
            tokens.push(match self.vocab.iter().position(|v| v.as_slice() == [b]) {
                Some(i) => i,
                None => usize::from(b) + 3,
            });
        }
        loop {
            let mut best = None;
            let mut score = f32::NEG_INFINITY;
            for i in 0..tokens.len() - 1 {
                let mut pair = self.vocab[tokens[i]].clone();
                pair.extend_from_slice(&self.vocab[tokens[i + 1]]);
                if let Some(id) = self.vocab.iter().position(|v| *v == pair)
                    && self.scores[id] > score
                {
                    score = self.scores[id];
                    best = Some((i, id));
                }
            }
            let Some((i, id)) = best else { break };
            tokens[i] = id;
            tokens.remove(i + 1);
        }
        tokens
    }
    pub fn decode(&self, previous: usize, token: usize) -> String {
        let mut v = self.vocab[token].as_slice();
        if previous == 1 && v.starts_with(b" ") {
            v = &v[1..];
        }
        if v.len() == 6 && v.starts_with(b"<0x") && v.ends_with(b">") {
            let Ok(text) = core::str::from_utf8(&v[3..5]) else {
                return String::from("?");
            };
            if let Ok(b) = u8::from_str_radix(text, 16) {
                return if b.is_ascii_graphic() || b == b' ' || b == b'\n' {
                    String::from(b as char)
                } else {
                    String::new()
                };
            }
        }
        String::from_utf8_lossy(v).into_owned()
    }
}
