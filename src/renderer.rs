use crate::doom::Frame;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::Write;

const THUMBNAIL_WIDTH: u32 = 160;
const THUMBNAIL_HEIGHT: u32 = 100;
const PALETTE_COLORS: usize = 64;

// --- Frame Scaling ---

pub fn scale_frame(frame: &Frame, new_width: u32, new_height: u32) -> Frame {
    let mut pixels = vec![0u32; (new_width * new_height) as usize];
    let sx = frame.width as f64 / new_width as f64;
    let sy = frame.height as f64 / new_height as f64;

    for y in 0..new_height {
        for x in 0..new_width {
            let src_x = (x as f64 * sx).min((frame.width - 1) as f64) as u32;
            let src_y = (y as f64 * sy).min((frame.height - 1) as f64) as u32;
            pixels[(y * new_width + x) as usize] =
                frame.pixels[(src_y * frame.width + src_x) as usize];
        }
    }

    Frame {
        width: new_width,
        height: new_height,
        pixels,
    }
}

// --- PNG Renderer (palette mode for minimal size) ---

/// Quantize an RGB pixel to a 6-bit index (2 bits per channel, 64 colors)
fn quantize(r: u8, g: u8, b: u8) -> u8 {
    ((r >> 6) << 4) | ((g >> 6) << 2) | (b >> 6)
}

/// Build the 64-entry RGB palette (4 levels per channel: 0, 85, 170, 255)
fn build_palette() -> Vec<u8> {
    let mut pal = Vec::with_capacity(PALETTE_COLORS * 3);
    for i in 0..PALETTE_COLORS as u8 {
        pal.push(((i >> 4) & 3) * 85);
        pal.push(((i >> 2) & 3) * 85);
        pal.push((i & 3) * 85);
    }
    pal
}

pub fn render_png(frame: &Frame) -> Vec<u8> {
    // Thumbnail palette PNG - small enough for low tokens, big enough for Claude to see enemies
    let small = scale_frame(frame, THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT);
    let width = small.width;
    let height = small.height;

    let mut png = vec![137, 80, 78, 71, 13, 10, 26, 10]; // PNG signature

    // IHDR - palette mode (color type 3), 8-bit depth
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(3); // color type: indexed (palette)
    ihdr.push(0); // compression
    ihdr.push(0); // filter
    ihdr.push(0); // interlace
    png.extend(png_chunk(b"IHDR", &ihdr));

    // PLTE - 64-color palette
    png.extend(png_chunk(b"PLTE", &build_palette()));

    // IDAT - indexed pixel data (1 byte per pixel + filter byte per row)
    let row_bytes = 1 + width as usize;
    let mut raw = vec![0u8; height as usize * row_bytes];

    for y in 0..height as usize {
        raw[y * row_bytes] = 0; // filter: none
        for x in 0..width as usize {
            let pixel = small.pixels[y * width as usize + x];
            let r = ((pixel >> 16) & 0xFF) as u8;
            let g = ((pixel >> 8) & 0xFF) as u8;
            let b = (pixel & 0xFF) as u8;
            raw[y * row_bytes + 1 + x] = quantize(r, g, b);
        }
    }

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&raw).unwrap();
    let compressed = encoder.finish().unwrap();

    png.extend(png_chunk(b"IDAT", &compressed));
    png.extend(png_chunk(b"IEND", &[]));

    png
}

fn png_chunk(chunk_type: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut chunk = Vec::with_capacity(12 + data.len());
    chunk.extend_from_slice(&(data.len() as u32).to_be_bytes());
    chunk.extend_from_slice(chunk_type);
    chunk.extend_from_slice(data);

    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(chunk_type);
    crc_input.extend_from_slice(data);
    chunk.extend_from_slice(&crc32(&crc_input).to_be_bytes());

    chunk
}

// Compile-time CRC32 table
const CRC_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut n = 0;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xEDB88320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        table[n] = c;
        n += 1;
    }
    table
};

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc = (crc >> 8) ^ CRC_TABLE[((crc ^ byte as u32) & 0xFF) as usize];
    }
    crc ^ 0xFFFF_FFFF
}

/// Render a full-resolution RGB PNG (no palette quantization)
pub fn render_png_full(frame: &Frame) -> Vec<u8> {
    let width = frame.width;
    let height = frame.height;

    let mut png = vec![137, 80, 78, 71, 13, 10, 26, 10];

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(2); // color type: RGB
    ihdr.push(0);
    ihdr.push(0);
    ihdr.push(0);
    png.extend(png_chunk(b"IHDR", &ihdr));

    let row_bytes = 1 + (width as usize) * 3;
    let mut raw = vec![0u8; (height as usize) * row_bytes];
    for y in 0..height as usize {
        raw[y * row_bytes] = 0;
        for x in 0..width as usize {
            let pixel = frame.pixels[y * width as usize + x];
            let off = y * row_bytes + 1 + x * 3;
            raw[off] = ((pixel >> 16) & 0xFF) as u8;
            raw[off + 1] = ((pixel >> 8) & 0xFF) as u8;
            raw[off + 2] = (pixel & 0xFF) as u8;
        }
    }

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&raw).unwrap();
    let compressed = encoder.finish().unwrap();

    png.extend(png_chunk(b"IDAT", &compressed));
    png.extend(png_chunk(b"IEND", &[]));
    png
}

// --- Base64 Encoder ---

const B64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(data: &[u8]) -> String {
    let mut result = String::with_capacity(data.len().div_ceil(3) * 4);

    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(B64_CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(B64_CHARS[((triple >> 12) & 0x3F) as usize] as char);
        result.push(if chunk.len() > 1 {
            B64_CHARS[((triple >> 6) & 0x3F) as usize] as char
        } else {
            '='
        });
        result.push(if chunk.len() > 2 {
            B64_CHARS[(triple & 0x3F) as usize] as char
        } else {
            '='
        });
    }

    result
}
