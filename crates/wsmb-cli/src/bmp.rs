//! Just enough BMP to move frames in and out of the dev CLI without pulling in
//! any dependency. Reads 24/32 bpp BI_RGB and BI_BITFIELDS, writes 32 bpp.

use std::fs;
use std::io;
use std::path::Path;

pub struct Bitmap {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

fn bad(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_string())
}

#[inline]
fn u16le(d: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([d[o], d[o + 1]])
}

#[inline]
fn u32le(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

#[inline]
fn i32le(d: &[u8], o: usize) -> i32 {
    i32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

pub fn read(path: &Path) -> io::Result<Bitmap> {
    let data = fs::read(path)?;
    if data.len() < 54 || &data[0..2] != b"BM" {
        return Err(bad("not a BMP file"));
    }

    let offset = u32le(&data, 10) as usize;
    let header_size = u32le(&data, 14) as usize;
    if header_size < 40 {
        return Err(bad("unsupported BMP header (need BITMAPINFOHEADER)"));
    }

    let width = i32le(&data, 18);
    let height_raw = i32le(&data, 22);
    let bpp = u16le(&data, 28) as usize;
    let compression = u32le(&data, 30);

    if width <= 0 || height_raw == 0 {
        return Err(bad("bad BMP dimensions"));
    }
    if !(bpp == 24 || bpp == 32) {
        return Err(bad("only 24 and 32 bpp BMP are supported"));
    }
    if compression != 0 && compression != 3 {
        return Err(bad("only uncompressed BMP is supported"));
    }

    let top_down = height_raw < 0;
    let width = width as usize;
    let height = height_raw.unsigned_abs() as usize;

    let stride = ((width * bpp + 31) / 32) * 4;
    let need = offset
        .checked_add(stride * height)
        .ok_or_else(|| bad("BMP size overflow"))?;
    if need > data.len() {
        return Err(bad("truncated BMP"));
    }

    let mut rgba = vec![255u8; width * height * 4];
    let bytes_pp = bpp / 8;
    for y in 0..height {
        let src_row = if top_down { y } else { height - 1 - y };
        let row_start = offset + src_row * stride;
        for x in 0..width {
            let s = row_start + x * bytes_pp;
            let d = (y * width + x) * 4;
            rgba[d] = data[s + 2];
            rgba[d + 1] = data[s + 1];
            rgba[d + 2] = data[s];
            rgba[d + 3] = if bpp == 32 { data[s + 3] } else { 255 };
        }
    }

    Ok(Bitmap {
        width,
        height,
        rgba,
    })
}

pub fn write(path: &Path, width: usize, height: usize, rgba: &[u8]) -> io::Result<()> {
    if rgba.len() != width * height * 4 {
        return Err(bad("rgba buffer size mismatch"));
    }
    let image_size = width * height * 4;
    let file_size = 54 + image_size;
    let mut out = Vec::with_capacity(file_size);

    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(file_size as u32).to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&54u32.to_le_bytes());

    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(width as i32).to_le_bytes());
    out.extend_from_slice(&(-(height as i32)).to_le_bytes()); // top-down
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    out.extend_from_slice(&(image_size as u32).to_le_bytes());
    out.extend_from_slice(&2835i32.to_le_bytes());
    out.extend_from_slice(&2835i32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());

    for px in rgba.chunks_exact(4) {
        out.push(px[2]);
        out.push(px[1]);
        out.push(px[0]);
        out.push(px[3]);
    }

    fs::write(path, out)
}
