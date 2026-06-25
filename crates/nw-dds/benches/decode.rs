//! Throughput benchmarks for the DDS/BCn decode path.
//!
//! Each case builds a minimal single-mip DDS in memory (header + a
//! deterministic pseudo-random payload of the correct block-packed size) and
//! times [`nw_dds::decode_top_mip`], which dispatches to the BCn/plain decoder.
//! Random bytes decode to garbage pixels but exercise the decoder identically
//! for timing purposes.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

const DDS_FILE_HEADER_LEN: usize = 128;
const DDS_HEADER_SIZE: u32 = 124;
const DDS_PIXEL_FORMAT_SIZE: u32 = 32;
const DDPF_FOUR_CC: u32 = 0x4;
const DDPF_RGB: u32 = 0x40;
const DDPF_ALPHA_PIXELS: u32 = 0x1;

/// A deterministic, allocation-free pseudo-random byte stream (xorshift) so the
/// decoder always sees the same garbage-but-valid input across runs.
fn fill_pseudo_random(buf: &mut [u8]) {
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    for byte in buf.iter_mut() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        *byte = (state >> 24) as u8;
    }
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

/// Build a single-mip four-CC DDS with `payload` appended after the header.
fn four_cc_dds(four_cc: [u8; 4], width: u32, height: u32, payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0u8; DDS_FILE_HEADER_LEN];
    bytes[0..4].copy_from_slice(b"DDS ");
    put_u32(&mut bytes, 4, DDS_HEADER_SIZE);
    put_u32(&mut bytes, 8, 0x1 | 0x2 | 0x4 | 0x1000);
    put_u32(&mut bytes, 12, height);
    put_u32(&mut bytes, 16, width);
    put_u32(&mut bytes, 28, 1);
    put_u32(&mut bytes, 76, DDS_PIXEL_FORMAT_SIZE);
    put_u32(&mut bytes, 80, DDPF_FOUR_CC);
    bytes[84..88].copy_from_slice(&four_cc);
    put_u32(&mut bytes, 108, 0x1000);
    bytes.extend_from_slice(payload);
    bytes
}

/// Build a single-mip DX10 DDS for `dxgi_format` with `payload` appended.
fn dx10_dds(dxgi_format: u32, width: u32, height: u32, payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0u8; DDS_FILE_HEADER_LEN];
    bytes[0..4].copy_from_slice(b"DDS ");
    put_u32(&mut bytes, 4, DDS_HEADER_SIZE);
    put_u32(&mut bytes, 8, 0x1 | 0x2 | 0x4 | 0x1000);
    put_u32(&mut bytes, 12, height);
    put_u32(&mut bytes, 16, width);
    put_u32(&mut bytes, 28, 1);
    put_u32(&mut bytes, 76, DDS_PIXEL_FORMAT_SIZE);
    put_u32(&mut bytes, 80, DDPF_FOUR_CC);
    bytes[84..88].copy_from_slice(b"DX10");
    put_u32(&mut bytes, 108, 0x1000);
    // DX10 header (20 bytes): dxgi_format, dim=2D(3), misc=0, array_size=1, misc2=0.
    let mut dx10 = Vec::with_capacity(20);
    dx10.extend_from_slice(&dxgi_format.to_le_bytes());
    dx10.extend_from_slice(&3u32.to_le_bytes());
    dx10.extend_from_slice(&0u32.to_le_bytes());
    dx10.extend_from_slice(&1u32.to_le_bytes());
    dx10.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&dx10);
    bytes.extend_from_slice(payload);
    bytes
}

/// Build a single-mip plain RGBA8 DDS with `payload` appended.
fn rgba8_dds(width: u32, height: u32, payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0u8; DDS_FILE_HEADER_LEN];
    bytes[0..4].copy_from_slice(b"DDS ");
    put_u32(&mut bytes, 4, DDS_HEADER_SIZE);
    put_u32(&mut bytes, 8, 0x1 | 0x2 | 0x4 | 0x1000);
    put_u32(&mut bytes, 12, height);
    put_u32(&mut bytes, 16, width);
    put_u32(&mut bytes, 28, 1);
    put_u32(&mut bytes, 76, DDS_PIXEL_FORMAT_SIZE);
    put_u32(&mut bytes, 80, DDPF_RGB | DDPF_ALPHA_PIXELS);
    put_u32(&mut bytes, 88, 32);
    put_u32(&mut bytes, 92, 0x0000_00ff);
    put_u32(&mut bytes, 96, 0x0000_ff00);
    put_u32(&mut bytes, 100, 0x00ff_0000);
    put_u32(&mut bytes, 104, 0xff00_0000);
    put_u32(&mut bytes, 108, 0x1000);
    bytes.extend_from_slice(payload);
    bytes
}

/// Block-packed payload size: `block_bytes` per 4x4 block.
fn block_payload(width: u32, height: u32, block_bytes: usize) -> Vec<u8> {
    let blocks = (width as usize).div_ceil(4) * (height as usize).div_ceil(4);
    let mut buf = vec![0u8; blocks * block_bytes];
    fill_pseudo_random(&mut buf);
    buf
}

fn rgba_payload(width: u32, height: u32) -> Vec<u8> {
    let mut buf = vec![0u8; width as usize * height as usize * 4];
    fill_pseudo_random(&mut buf);
    buf
}

type Builder = dyn Fn(u32, u32) -> Vec<u8>;

fn bench_decode(c: &mut Criterion) {
    let sizes = [512u32, 1024u32];
    let cases: &[(&str, &Builder)] = &[
        ("bc1", &|w, h| four_cc_dds(*b"DXT1", w, h, &block_payload(w, h, 8))),
        ("bc3", &|w, h| four_cc_dds(*b"DXT5", w, h, &block_payload(w, h, 16))),
        ("bc7", &|w, h| dx10_dds(98, w, h, &block_payload(w, h, 16))),
        ("rgba8", &|w, h| rgba8_dds(w, h, &rgba_payload(w, h))),
    ];

    let mut group = c.benchmark_group("decode_top_mip");
    for &dim in &sizes {
        for (name, build) in cases {
            let dds = build(dim, dim);
            group.throughput(Throughput::Elements(u64::from(dim) * u64::from(dim)));
            group.bench_with_input(
                BenchmarkId::new(*name, format!("{dim}x{dim}")),
                &dds,
                |b, dds| {
                    b.iter(|| {
                        let img = nw_dds::decode_top_mip(black_box(dds), &[]).unwrap();
                        black_box(img.rgba.len())
                    });
                },
            );
        }
    }
    group.finish();
}

criterion_group!(benches, bench_decode);
criterion_main!(benches);
