//! Sparse tensor storage using Compressed Sparse Row (CSR) format.
//!
//! The `CSRMatrix` type provides memory-efficient storage and
//! arithmetic for the large, sparse weight matrices that arise in
//! Hebbian neural networks.  All operations are designed to work
//! with three-value-address tuples `(pre, post, weight)` from the
//! synapse map.
//!
//! # Format
//!
//! ```text
//! values:       [w₀, w₁, w₂, …, wₙ₋₁]
//! col_indices:  [c₀, c₁, c₂, …, cₙ₋₁]   — column index per value
//! row_ptr:      [r₀, r₁, …, rₘ]          — start index of each row
//! shape:        (rows, cols)
//! ```
//!
//! Row `i` occupies `values[row_ptr[i] .. row_ptr[i+1]]`.
//! Empty rows have `row_ptr[i] == row_ptr[i+1]`.

// ─── CSRMatrix ──────────────────────────────────────────────────

/// Compressed Sparse Row matrix.
///
/// Stores a sparse `rows × cols` matrix where zero entries are not
/// stored explicitly.  Suitable for weight matrices where each row
/// corresponds to a post-synaptic neuron and each column to a
/// pre-synaptic neuron.
#[derive(Debug, Clone)]
pub struct CSRMatrix {
    /// Non-zero values stored in row-major order.
    pub values: Vec<f32>,
    /// Column index for each value.
    pub col_indices: Vec<u32>,
    /// Row pointers: `row_ptr[i]` is the start of row `i`;
    /// `row_ptr[row_ptr.len() - 1] == values.len()`.
    pub row_ptr: Vec<usize>,
    /// Matrix dimensions `(rows, cols)`.
    pub shape: (usize, usize),
}

impl CSRMatrix {
    /// Create an empty CSR matrix with the given shape.
    ///
    /// All rows are initially empty (no non-zero entries).
    pub fn new(rows: usize, cols: usize) -> Self {
        CSRMatrix {
            values: Vec::new(),
            col_indices: Vec::new(),
            row_ptr: vec![0; rows + 1],
            shape: (rows, cols),
        }
    }

    /// Build a CSR matrix from a dense 2-D slice.
    ///
    /// Every `f32` value is checked; exactly-zero entries are omitted.
    ///
    /// # Panics
    ///
    /// Panics if `dense` is empty or if rows have inconsistent lengths.
    pub fn from_dense(dense: &[Vec<f32>]) -> Self {
        assert!(!dense.is_empty(), "dense matrix must have at least one row");
        let rows = dense.len();
        let cols = dense[0].len();
        let mut values = Vec::new();
        let mut col_indices = Vec::new();
        let mut row_ptr = Vec::with_capacity(rows + 1);
        row_ptr.push(0);

        for row in dense {
            assert_eq!(row.len(), cols, "all rows must have the same length");
            for (j, &v) in row.iter().enumerate() {
                if v != 0.0 {
                    values.push(v);
                    col_indices.push(j as u32);
                }
            }
            row_ptr.push(values.len());
        }

        CSRMatrix {
            values,
            col_indices,
            row_ptr,
            shape: (rows, cols),
        }
    }

    /// Compute `y = W · x` (matrix–vector product).
    ///
    /// `W` is `self` (shape `(rows, cols)`), `x` is a slice of length
    /// `cols`.  Returns a `Vec<f32>` of length `rows`.
    ///
    /// # Panics
    ///
    /// Panics if `x.len() != self.shape.1`.
    pub fn matmul(&self, vec: &[f32]) -> Vec<f32> {
        assert_eq!(
            vec.len(),
            self.shape.1,
            "vector length {} does not match matrix cols {}",
            vec.len(),
            self.shape.1
        );
        let rows = self.shape.0;
        let mut result = vec![0.0_f32; rows];
        for i in 0..rows {
            let start = self.row_ptr[i];
            let end = self.row_ptr[i + 1];
            let mut sum = 0.0_f32;
            for idx in start..end {
                let col = self.col_indices[idx] as usize;
                sum += self.values[idx] * vec[col];
            }
            result[i] = sum;
        }
        result
    }

    /// Compute `y = Wᵀ · x` (transpose matrix–vector product).
    ///
    /// `Wᵀ` has shape `(cols, rows)`.
    /// `x` is a slice of length `rows`.
    /// Returns a `Vec<f32>` of length `cols`.
    ///
    /// # Panics
    ///
    /// Panics if `x.len() != self.shape.0`.
    pub fn matmul_transpose(&self, vec: &[f32]) -> Vec<f32> {
        assert_eq!(
            vec.len(),
            self.shape.0,
            "vector length {} does not match matrix rows {}",
            vec.len(),
            self.shape.0
        );
        let cols = self.shape.1;
        let mut result = vec![0.0_f32; cols];
        for i in 0..self.shape.0 {
            let wi = vec[i];
            if wi == 0.0 {
                continue;
            }
            let start = self.row_ptr[i];
            let end = self.row_ptr[i + 1];
            for idx in start..end {
                let col = self.col_indices[idx] as usize;
                result[col] += self.values[idx] * wi;
            }
        }
        result
    }

    /// Sparse outer-product Hebbian update.
    ///
    /// Performs `W += η · pre ⊗ post`, i.e. `W[i][j] += η · pre[j] · post[i]`,
    /// but only for entries where both `pre[j]` and `post[i]` are non-zero.
    ///
    /// This is a sparse-friendly implementation: it iterates over
    /// non-zero entries of `pre` and `post` and touches only affected
    /// rows/columns.
    ///
    /// # Panics
    ///
    /// Panics if `pre.len() != cols` or `post.len() != rows`.
    pub fn outer_update(&mut self, pre: &[f32], post: &[f32]) {
        assert_eq!(pre.len(), self.shape.1, "pre (input) length mismatch");
        assert_eq!(post.len(), self.shape.0, "post (output) length mismatch");

        // Collect (col, row, delta) triplets for non-zero outer contributions.
        let mut updates: Vec<(u32, usize, f32)> = Vec::new();
        for (j, &pj) in pre.iter().enumerate() {
            if pj == 0.0 {
                continue;
            }
            for (i, &pi) in post.iter().enumerate() {
                let delta = pj * pi;
                if delta != 0.0 {
                    updates.push((j as u32, i, delta));
                }
            }
        }

        // Apply updates row-by-row.
        for (col, row, delta) in updates {
            let start = self.row_ptr[row];
            let end = self.row_ptr[row + 1];
            let col_idx = col;

            // Search for existing column in this row.
            let pos = self.col_indices[start..end]
                .binary_search(&col_idx)
                .map(|idx| start + idx)
                .ok();

            match pos {
                Some(idx) => {
                    self.values[idx] += delta;
                }
                None => {
                    // Insert new entry, keeping sorted order.
                    let insert_pos = self.col_indices[start..end]
                        .binary_search(&col_idx)
                        .unwrap_or_else(|e| start + e);
                    self.values.insert(insert_pos, delta);
                    self.col_indices.insert(insert_pos, col_idx);
                    // Shift row pointers for rows below.
                    for rp in self.row_ptr[row + 1..].iter_mut() {
                        *rp += 1;
                    }
                }
            }
        }
    }

    /// Remove entries whose absolute value falls below `threshold`.
    ///
    /// This compacts the storage in-place.  After pruning, empty rows
    /// have matching `row_ptr` entries.
    pub fn prune(&mut self, threshold: f32) {
        let mut new_values = Vec::new();
        let mut new_col_indices = Vec::new();
        let mut new_row_ptr = Vec::with_capacity(self.row_ptr.len());
        new_row_ptr.push(0);

        for i in 0..self.shape.0 {
            let start = self.row_ptr[i];
            let end = self.row_ptr[i + 1];
            for idx in start..end {
                if self.values[idx].abs() >= threshold {
                    new_values.push(self.values[idx]);
                    new_col_indices.push(self.col_indices[idx]);
                }
            }
            new_row_ptr.push(new_values.len());
        }

        self.values = new_values;
        self.col_indices = new_col_indices;
        self.row_ptr = new_row_ptr;
    }

    /// Fraction of non-zero entries: `nnz / (rows × cols)`.
    ///
    /// Returns `0.0` for a zero-sized matrix.
    pub fn density(&self) -> f32 {
        let total = self.shape.0 * self.shape.1;
        if total == 0 {
            return 0.0;
        }
        self.values.len() as f32 / total as f32
    }

    /// Convert back to a dense `Vec<Vec<f32>>`.
    pub fn to_dense(&self) -> Vec<Vec<f32>> {
        let (rows, cols) = self.shape;
        let mut dense = vec![vec![0.0_f32; cols]; rows];
        for i in 0..rows {
            let start = self.row_ptr[i];
            let end = self.row_ptr[i + 1];
            for idx in start..end {
                let col = self.col_indices[idx] as usize;
                dense[i][col] = self.values[idx];
            }
        }
        dense
    }

    /// Number of non-zero entries.
    pub fn nnz(&self) -> usize {
        self.values.len()
    }

    /// Shape of the matrix.
    pub fn shape(&self) -> (usize, usize) {
        self.shape
    }
}

// ─── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_matrix() {
        let m = CSRMatrix::new(3, 4);
        assert_eq!(m.nnz(), 0);
        assert_eq!(m.shape(), (3, 4));
        assert!((m.density() - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_from_dense_and_to_dense() {
        let dense = vec![
            vec![1.0, 0.0, 2.0],
            vec![0.0, 3.0, 0.0],
            vec![4.0, 5.0, 6.0],
        ];
        let m = CSRMatrix::from_dense(&dense);
        assert_eq!(m.nnz(), 6);
        assert_eq!(m.to_dense(), dense);
    }

    #[test]
    fn test_matmul_identity() {
        let dense = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];
        let m = CSRMatrix::from_dense(&dense);
        let x = vec![2.0, 3.0, 4.0];
        let y = m.matmul(&x);
        assert!((y[0] - 2.0).abs() < 1e-6);
        assert!((y[1] - 3.0).abs() < 1e-6);
        assert!((y[2] - 4.0).abs() < 1e-6);
    }

    #[test]
    fn test_matmul_general() {
        let dense = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let m = CSRMatrix::from_dense(&dense);
        let x = vec![5.0, 6.0];
        let y = m.matmul(&x);
        assert!((y[0] - 17.0).abs() < 1e-6); // 1*5 + 2*6
        assert!((y[1] - 39.0).abs() < 1e-6); // 3*5 + 4*6
    }

    #[test]
    fn test_matmul_transpose() {
        let dense = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let m = CSRMatrix::from_dense(&dense);
        let x = vec![5.0, 6.0];
        let y = m.matmul_transpose(&x);
        // W^T = [[1,3],[2,4]], W^T·x = [1*5+3*6, 2*5+4*6] = [23, 34]
        assert!((y[0] - 23.0).abs() < 1e-6);
        assert!((y[1] - 34.0).abs() < 1e-6);
    }

    #[test]
    fn test_outer_update() {
        let mut m = CSRMatrix::new(2, 3);
        let pre = vec![1.0, 0.0, 2.0];
        let post = vec![3.0, 4.0];
        m.outer_update(&pre, &post);
        // Row 0: col 0 += 3, col 2 += 6
        // Row 1: col 0 += 4, col 2 += 8
        let dense = m.to_dense();
        assert!((dense[0][0] - 3.0).abs() < 1e-6);
        assert!((dense[0][2] - 6.0).abs() < 1e-6);
        assert!((dense[1][0] - 4.0).abs() < 1e-6);
        assert!((dense[1][2] - 8.0).abs() < 1e-6);
    }

    #[test]
    fn test_prune() {
        let dense = vec![vec![1.0, 0.01, 0.5], vec![0.001, 0.0, 2.0]];
        let mut m = CSRMatrix::from_dense(&dense);
        m.prune(0.1);
        let pruned = m.to_dense();
        assert!((pruned[0][0] - 1.0).abs() < 1e-6);
        assert!((pruned[0][2] - 0.5).abs() < 1e-6);
        assert!((pruned[1][2] - 2.0).abs() < 1e-6);
        assert!((pruned[0][1] - 0.0).abs() < 1e-6); // pruned
        assert!((pruned[1][0] - 0.0).abs() < 1e-6); // pruned
    }

    #[test]
    fn test_density() {
        let dense = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let m = CSRMatrix::from_dense(&dense);
        assert!((m.density() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_outer_update_idempotent() {
        let mut m = CSRMatrix::new(2, 2);
        let pre = vec![1.0, 1.0];
        let post = vec![1.0, 1.0];
        m.outer_update(&pre, &post);
        let nnz1 = m.nnz();
        m.outer_update(&pre, &post);
        let nnz2 = m.nnz();
        // No new entries created on second call (all columns already exist)
        assert_eq!(nnz1, nnz2);
    }
}
