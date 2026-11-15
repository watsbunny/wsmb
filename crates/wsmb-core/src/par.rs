//! Tiny row-parallel helper (std only, no rayon).

/// Run `f(row_index, row_slice)` for every row and return the filled buffer.
///
/// Rows are handed out in contiguous bands, one band per worker thread. The
/// buffer is interleaved with `channels` values per pixel.
pub fn rows_out<F>(width: usize, channels: usize, height: usize, f: F) -> Vec<f32>
where
    F: Fn(usize, &mut [f32]) + Sync,
{
    let mut out = vec![0.0f32; width * channels * height];
    if width == 0 || height == 0 || channels == 0 {
        return out;
    }

    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .clamp(1, height);
    let rows_per = (height + threads - 1) / threads;
    let stride = width * channels;

    std::thread::scope(|scope| {
        let mut rest: &mut [f32] = &mut out;
        let mut y0 = 0usize;
        while y0 < height {
            let rows = rows_per.min(height - y0);
            let (chunk, tail) = rest.split_at_mut(rows * stride);
            rest = tail;
            let body = &f;
            scope.spawn(move || {
                for r in 0..rows {
                    let s = r * stride;
                    body(y0 + r, &mut chunk[s..s + stride]);
                }
            });
            y0 += rows;
        }
    });

    out
}
