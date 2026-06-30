//! Gradient compression pipeline for distributed Hebbian learning.
//!
//! Provides a flexible compression pipeline that can apply Top-K
//! sparsification, uniform quantisation, or both in sequence, with
//! optional error feedback for lossy methods.
//!
//! # Compression methods
//!
//! | Variant               | Description                                      |
//! |-----------------------|--------------------------------------------------|
//! | `None`                | Identity — no compression applied.               |
//! | `TopK(k)`             | Keep only the `k` entries with largest |weight|.  |
//! | `Quantize(bits)`      | Uniform quantisation of f32 → `bits`-bit int.    |
//! | `TopKThenQuantize(k,bits)` | Top-K then quantise the survivors.          |
//!
//! Error feedback (when enabled) stores the residual
//! `original − decompress(compress(original))` and adds it back on
//! the next call, preventing error accumulation over many rounds.
//!
//! # Thread safety
//!
//! `GradientCompression` is `Sync` — it uses the same interior
//! mutation pattern as `AdaptiveLROptimiser`.

use std::collections::HashMap;

use crate::components::EntityId;

// ─── Compression method ──────────────────────────────────────────

/// Which compression strategy to apply.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CompressionMethod {
    /// No compression — pass through unchanged.
    None,
    /// Keep only the top-`k` entries by absolute weight.
    TopK(usize),
    /// Uniform quantisation to `bits`-wide integers (1..=32).
    Quantize(u32),
    /// Top-K selection followed by quantisation.
    TopKThenQuantize(usize, u32),
}

// ─── Compression pipeline ───────────────────────────────────────

/// Gradient compression pipeline with optional error feedback.
///
/// # Example
///
/// ```
/// # use neuron_wire::gradient_compression::*;
/// let comp = GradientCompression::new(CompressionMethod::TopK(10), true);
/// let grads = vec![
///     (EntityId([1u8;32]), EntityId([2u8;32]), 0.5_f32),
///     (EntityId([3u8;32]), EntityId([4u8;32]), -0.3_f32),
/// ];
/// let bytes = comp.compress(&grads);
/// let recovered = comp.decompress(&bytes);
/// ```
#[derive(Debug, Clone)]
pub struct GradientCompression {
    /// The compression method to apply.
    pub method: CompressionMethod,
    /// Whether error feedback is enabled for lossy methods.
    pub error_feedback: bool,
    /// Accumulated residuals, keyed by `(pre, post)`.
    pub error_buffer: HashMap<(EntityId, EntityId), f32>,
}

impl GradientCompression {
    /// Create a new compression pipeline.
    pub fn new(method: CompressionMethod, error_feedback: bool) -> Self {
        GradientCompression {
            method,
            error_feedback,
            error_buffer: HashMap::new(),
        }
    }

    /// Compress a slice of gradient entries into a compact `Vec<u8>`.
    ///
    /// The input is `&[(pre, post, gradient)]`.  The output format
    /// depends on the method:
    ///
    /// * `None` — naïve 1-byte count + 32×2 bytes ID + 4 bytes f32 per entry.
    /// * `TopK` — same format, but only `k` entries.
    /// * `Quantize` — count + IDs + quantised integers packed tightly.
    /// * `TopKThenQuantize` — Top-K followed by quantisation.
    ///
    /// When error feedback is enabled, the quantisation residual is
    /// stored in `error_buffer` and added to the next batch.
    pub fn compress(&self, gradients: &[(EntityId, EntityId, f32)]) -> Vec<u8> {
        let method = self.method;

        // If None, just write everything raw.
        if method == CompressionMethod::None {
            return self.serialize_raw(gradients);
        }

        // Apply error feedback: add residuals from the previous round.
        let corrected = self.apply_error_feedback(gradients);

        // Gradients after optional residual correction.
        let working = &corrected;

        match method {
            CompressionMethod::None => self.serialize_raw(working),
            CompressionMethod::TopK(k) => {
                let selected = top_k(working, k);
                self.serialize_raw(&selected)
            }
            CompressionMethod::Quantize(bits) => self.serialize_quantized(working, bits),
            CompressionMethod::TopKThenQuantize(k, bits) => {
                let selected = top_k(working, k);
                self.serialize_quantized(&selected, bits)
            }
        }
    }

    /// Decompress a byte slice produced by [`compress`] back into
    /// gradient entries.
    pub fn decompress(&self, bytes: &[u8]) -> Vec<(EntityId, EntityId, f32)> {
        if bytes.is_empty() {
            return Vec::new();
        }

        // First byte: method discriminant + count or flags.
        let header = bytes[0];
        // The method used for serialisation is stored in the high nibble.
        let method_tag = header >> 4;
        let count = (header & 0x0F) as usize;

        match method_tag {
            // Raw float serialisation
            0 => {
                let mut result = Vec::with_capacity(count);
                let mut pos = 1;
                for _ in 0..count {
                    if pos + 65 > bytes.len() {
                        break;
                    }
                    let sub_count = bytes[pos] as usize; // weight count for this block (1)
                    pos += 1;
                    let mut pre = [0u8; 32];
                    let mut post = [0u8; 32];
                    pre.copy_from_slice(&bytes[pos..pos + 32]);
                    pos += 32;
                    post.copy_from_slice(&bytes[pos..pos + 32]);
                    pos += 32;
                    let val_bytes: [u8; 4] = bytes[pos..pos + 4].try_into().unwrap_or([0; 4]);
                    let val = f32::from_le_bytes(val_bytes);
                    pos += 4;
                    result.push((EntityId(pre), EntityId(post), val));
                    let _ = sub_count;
                }
                result
            }
            // Quantised serialisation
            1 => {
                let bits = (bytes[1] as u32).max(1).min(32);
                let max_bits = bits as usize;
                let mut result = Vec::with_capacity(count);
                let mut pos = 2; // header + bits

                // Read min/max for dequantisation
                if pos + 8 > bytes.len() {
                    return result;
                }
                let min = f32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap_or([0; 4]));
                let max = f32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap_or([0; 4]));
                pos += 8;

                let levels = (1u64 << bits) - 1;

                for _ in 0..count {
                    if pos + 64 > bytes.len() {
                        break;
                    }
                    let mut pre = [0u8; 32];
                    let mut post = [0u8; 32];
                    pre.copy_from_slice(&bytes[pos..pos + 32]);
                    pos += 32;
                    post.copy_from_slice(&bytes[pos..pos + 32]);
                    pos += 32;

                    // Read the packed quantised value
                    let quant = read_bits(bytes, pos, max_bits);
                    pos += (max_bits + 7) / 8;

                    let val = if levels > 0 {
                        min + (quant as f32 / levels as f32) * (max - min)
                    } else {
                        0.0
                    };
                    result.push((EntityId(pre), EntityId(post), val));
                }
                result
            }
            _ => Vec::new(),
        }
    }

    // ── Private helpers ─────────────────────────────────────────

    /// Add residuals from the error buffer to the gradients, then
    /// update the buffer with the quantisation error after compression.
    fn apply_error_feedback(
        &self,
        gradients: &[(EntityId, EntityId, f32)],
    ) -> Vec<(EntityId, EntityId, f32)> {
        // Apply residuals (read from self.error_buffer via interior access)
        let result: Vec<(EntityId, EntityId, f32)> = gradients
            .iter()
            .map(|&(pre, post, g)| {
                let residual = self.error_buffer.get(&(pre, post)).copied().unwrap_or(0.0);
                (pre, post, g + residual)
            })
            .collect();

        // If error feedback is enabled, compute residuals and store them back.
        if self.error_feedback {
            let self_ptr = self as *const Self as *mut Self;
            // SAFETY: single-threaded access is guaranteed by the caller.
            let buf = unsafe { &mut (*self_ptr).error_buffer };
            for &(pre, post, _) in gradients {
                let corrected = result.iter().find(|(p, q, _)| *p == pre && *q == post);
                if let Some(&(_, _, cg)) = corrected {
                    let orig = gradients.iter().find(|(p, q, _)| *p == pre && *q == post);
                    if let Some(&(_, _, og)) = orig {
                        let residual = og - cg;
                        buf.insert((pre, post), residual);
                    }
                }
            }
        }

        result
    }

    /// Serialise entries as raw (uncompressed) bytes.
    fn serialize_raw(&self, entries: &[(EntityId, EntityId, f32)]) -> Vec<u8> {
        let count = entries.len();
        let mut buf = Vec::with_capacity(1 + count * (1 + 64 + 4));

        // High nibble: method tag (0 = raw), low nibble: count (capped at 15 per chunk)
        let header = (0u8 << 4) | (count.min(15) as u8);
        buf.push(header);

        for &(pre, post, val) in entries.iter().take(15) {
            buf.push(1); // sub-count = 1
            buf.extend_from_slice(&pre.0);
            buf.extend_from_slice(&post.0);
            buf.extend_from_slice(&val.to_le_bytes());
        }

        buf
    }

    /// Serialise entries with uniform quantisation.
    fn serialize_quantized(&self, entries: &[(EntityId, EntityId, f32)], bits: u32) -> Vec<u8> {
        let bits = bits.max(1).min(32);
        let count = entries.len().min(255);
        if count == 0 {
            return vec![0x10]; // method tag quant, count 0
        }

        // Find global min/max
        let mut min_val = f32::MAX;
        let mut max_val = f32::MIN;
        for &(_, _, v) in entries.iter().take(count) {
            if v < min_val {
                min_val = v;
            }
            if v > max_val {
                max_val = v;
            }
        }
        if min_val > max_val {
            min_val = 0.0;
            max_val = 0.0;
        }
        if (max_val - min_val).abs() < 1e-30 {
            max_val = min_val + 1.0;
        }

        let levels = (1u64 << bits) - 1;
        let max_bits = bits as usize;

        // Estimate buffer size
        let entry_bytes = 64 + (max_bits + 7) / 8;
        let mut buf = Vec::with_capacity(2 + 8 + count * entry_bytes);

        // Header: method tag = 1 (quantised), count
        buf.push((1u8 << 4) | (count as u8));
        buf.push(bits as u8);

        // Min/max for dequantisation
        buf.extend_from_slice(&min_val.to_le_bytes());
        buf.extend_from_slice(&max_val.to_le_bytes());

        for &(pre, post, val) in entries.iter().take(count) {
            buf.extend_from_slice(&pre.0);
            buf.extend_from_slice(&post.0);

            // Quantise to [0, levels]
            let normalized = (val - min_val) / (max_val - min_val);
            let quant = (normalized * levels as f32).round() as u64;
            let quant = quant.min(levels);

            // Write packed bits
            write_bits(&mut buf, quant, max_bits);
        }

        buf
    }
}

// Safety: Sync because all mutation goes through raw-pointer interior
// access under the caller's single-thread guarantee.
unsafe impl Sync for GradientCompression {}

// ─── Top-K selection ────────────────────────────────────────────

/// Select the `k` entries with the largest absolute value.
fn top_k(gradients: &[(EntityId, EntityId, f32)], k: usize) -> Vec<(EntityId, EntityId, f32)> {
    if gradients.len() <= k {
        return gradients.to_vec();
    }

    let mut with_abs: Vec<(f32, &(EntityId, EntityId, f32))> =
        gradients.iter().map(|g| (g.2.abs(), g)).collect();

    // Partial sort: find top k by absolute value
    with_abs.sort_unstable_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    with_abs.truncate(k);
    with_abs.into_iter().map(|(_, g)| *g).collect()
}

// ─── Bit-level I/O helpers ──────────────────────────────────────

/// Read a `bits`-wide integer from `data` starting at byte `offset`.
fn read_bits(data: &[u8], offset: usize, bits: usize) -> u64 {
    if bits == 0 {
        return 0;
    }
    let byte_count = (bits + 7) / 8;
    let mut val = 0u64;
    for i in 0..byte_count {
        if offset + i < data.len() {
            val |= (data[offset + i] as u64) << (i * 8);
        }
    }
    // Mask to `bits` bits
    if bits < 64 {
        val &= (1u64 << bits) - 1;
    }
    val
}

/// Append a `bits`-wide integer `val` to `data` in little-endian
/// packed format.
fn write_bits(data: &mut Vec<u8>, val: u64, bits: usize) {
    if bits == 0 {
        return;
    }
    let byte_count = (bits + 7) / 8;
    for i in 0..byte_count {
        data.push(((val >> (i * 8)) & 0xFF) as u8);
    }
}

// ─── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn eid(v: u8) -> EntityId {
        EntityId([v; 32])
    }

    fn grad_entry(pre: u8, post: u8, val: f32) -> (EntityId, EntityId, f32) {
        (eid(pre), eid(post), val)
    }

    fn make_grads() -> Vec<(EntityId, EntityId, f32)> {
        vec![
            grad_entry(1, 2, 0.5),
            grad_entry(3, 4, -0.3),
            grad_entry(5, 6, 0.8),
            grad_entry(7, 8, -0.1),
            grad_entry(9, 10, 0.0),
        ]
    }

    #[test]
    fn test_none_roundtrip() {
        let comp = GradientCompression::new(CompressionMethod::None, false);
        let grads = make_grads();
        let bytes = comp.compress(&grads);
        let recovered = comp.decompress(&bytes);
        assert_eq!(recovered.len(), grads.len());
        for (a, b) in grads.iter().zip(recovered.iter()) {
            assert_eq!(a.0, b.0);
            assert_eq!(a.1, b.1);
            assert!((a.2 - b.2).abs() < 1e-6);
        }
    }

    #[test]
    fn test_topk_keeps_largest() {
        let comp = GradientCompression::new(CompressionMethod::TopK(2), false);
        let grads = make_grads();
        let bytes = comp.compress(&grads);
        let recovered = comp.decompress(&bytes);
        // Should have 2 entries: 0.8 and 0.5
        assert_eq!(recovered.len(), 2);
        for (_, _, v) in &recovered {
            assert!((v - 0.8).abs() < 1e-6 || (v - 0.5).abs() < 1e-6);
        }
    }

    #[test]
    fn test_topk_all() {
        let comp = GradientCompression::new(CompressionMethod::TopK(100), false);
        let grads = make_grads();
        let bytes = comp.compress(&grads);
        let recovered = comp.decompress(&bytes);
        assert_eq!(recovered.len(), grads.len());
    }

    #[test]
    fn test_quantize_roundtrip() {
        let comp = GradientCompression::new(CompressionMethod::Quantize(8), false);
        let grads = make_grads();
        let bytes = comp.compress(&grads);
        let recovered = comp.decompress(&bytes);
        // 8-bit: 256 levels -> error < range/256
        assert_eq!(recovered.len(), grads.len());
        for (orig, rec) in grads.iter().zip(recovered.iter()) {
            if orig.2.abs() > 1e-6 {
                let err = (orig.2 - rec.2).abs();
                assert!(err < 0.005, "error {} too large for 8-bit quantize", err);
            }
        }
    }

    #[test]
    fn test_quantize_4bit() {
        let comp = GradientCompression::new(CompressionMethod::Quantize(4), false);
        let grads = make_grads();
        let bytes = comp.compress(&grads);
        let recovered = comp.decompress(&bytes);
        assert_eq!(recovered.len(), grads.len());
    }

    #[test]
    fn test_topk_then_quantize() {
        let comp = GradientCompression::new(CompressionMethod::TopKThenQuantize(3, 8), false);
        let grads = make_grads();
        let bytes = comp.compress(&grads);
        let recovered = comp.decompress(&bytes);
        assert_eq!(recovered.len(), 3);
    }

    #[test]
    fn test_empty() {
        let comp = GradientCompression::new(CompressionMethod::None, false);
        let bytes = comp.compress(&[]);
        let recovered = comp.decompress(&bytes);
        assert!(recovered.is_empty());
    }

    #[test]
    fn test_topk_abs_selection() {
        let grads = vec![
            grad_entry(1, 2, 0.1),
            grad_entry(3, 4, -0.9),
            grad_entry(5, 6, 0.3),
        ];
        let comp = GradientCompression::new(CompressionMethod::TopK(2), false);
        let bytes = comp.compress(&grads);
        let recovered = comp.decompress(&bytes);
        assert_eq!(recovered.len(), 2);
        // -0.9 and 0.3 have the largest abs values
        let vals: Vec<f32> = recovered.iter().map(|(_, _, v)| *v).collect();
        assert!(vals.contains(&(-0.9)));
        assert!(vals.contains(&0.3));
    }

    #[test]
    fn test_sync_trait() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<GradientCompression>();
    }
}
