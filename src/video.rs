//! H.264 encoding/decoding and pixel format conversion.

use openh264::encoder::Encoder;
use openh264::formats::{YUVBuffer, YUVSource};

// ============================================================
// H.264 helpers
// ============================================================
pub(crate) struct H264EncoderState {
    pub(crate) encoder: Encoder,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl H264EncoderState {
    pub(crate) fn new(w: u32, h: u32) -> Result<Self, String> {
        // openh264 0.6 takes the frame size from the YUV source on each encode call.
        let enc = Encoder::new().map_err(|e| format!("Encoder: {e}"))?;
        Ok(Self {
            encoder: enc,
            width: w,
            height: h,
        })
    }

    pub(crate) fn encode_rgb(&mut self, rgb: &[u8]) -> Result<Vec<u8>, String> {
        let yuv = rgb_to_yuv420(rgb, self.width, self.height);
        let bits = self
            .encoder
            .encode(&yuv)
            .map_err(|e| format!("encode: {e}"))?;
        Ok(bits.to_vec())
    }
}

/// Converts packed RGB (3 bytes per pixel) to planar YUV 4:2:0.
/// Odd dimensions are padded to even by repeating the last row/column.
pub(crate) fn rgb_to_yuv420(rgb: &[u8], w: u32, h: u32) -> YUVBuffer {
    let wu = w as usize;
    let hu = h as usize;
    let ew = wu + (wu & 1);
    let eh = hu + (hu & 1);
    let mut y_p = vec![0u8; ew * eh];
    let mut u_p = vec![0u8; (ew / 2) * (eh / 2)];
    let mut v_p = vec![0u8; (ew / 2) * (eh / 2)];

    let pixel = |x: usize, y: usize| {
        let px = x.min(wu - 1);
        let py = y.min(hu - 1);
        let i = (py * wu + px) * 3;
        (rgb[i] as f32, rgb[i + 1] as f32, rgb[i + 2] as f32)
    };

    for y in 0..eh {
        for x in 0..ew {
            let (r, g, b) = pixel(x, y);
            y_p[y * ew + x] = (0.299 * r + 0.587 * g + 0.114 * b).clamp(0.0, 255.0) as u8;
        }
    }
    for y in (0..eh).step_by(2) {
        for x in (0..ew).step_by(2) {
            let (mut rs, mut gs, mut bs) = (0.0, 0.0, 0.0);
            for dy in 0..2 {
                for dx in 0..2 {
                    let (r, g, b) = pixel(x + dx, y + dy);
                    rs += r;
                    gs += g;
                    bs += b;
                }
            }
            let (r, g, b) = (rs / 4.0, gs / 4.0, bs / 4.0);
            let u = -0.169 * r - 0.331 * g + 0.5 * b + 128.0;
            let v = 0.5 * r - 0.419 * g - 0.081 * b + 128.0;
            let ui = (y / 2) * (ew / 2) + (x / 2);
            u_p[ui] = u.clamp(0.0, 255.0) as u8;
            v_p[ui] = v.clamp(0.0, 255.0) as u8;
        }
    }

    // YUVBuffer stores the three planes back to back: Y, then U, then V.
    let mut planar = Vec::with_capacity(y_p.len() + u_p.len() + v_p.len());
    planar.extend_from_slice(&y_p);
    planar.extend_from_slice(&u_p);
    planar.extend_from_slice(&v_p);
    YUVBuffer::from_vec(planar, ew, eh)
}

/// Converts any decoded YUV frame to packed RGB, honouring the plane strides
/// (decoded frames may contain row padding).
pub(crate) fn yuv_to_rgb(yuv: &impl YUVSource) -> Vec<u8> {
    let (w, h) = yuv.dimensions();
    let (sy, su, sv) = yuv.strides();
    let (y_p, u_p, v_p) = (yuv.y(), yuv.u(), yuv.v());
    let mut rgb = vec![0u8; w * h * 3];
    for y in 0..h {
        for x in 0..w {
            let yv = y_p[y * sy + x] as f32;
            let ci = (y / 2) * su + (x / 2);
            let uv = u_p[ci] as f32 - 128.0;
            let vv = v_p[(y / 2) * sv + (x / 2)] as f32 - 128.0;
            let r = (yv + 1.402 * vv).clamp(0.0, 255.0) as u8;
            let g = (yv - 0.344 * uv - 0.714 * vv).clamp(0.0, 255.0) as u8;
            let b = (yv + 1.772 * uv).clamp(0.0, 255.0) as u8;
            let o = (y * w + x) * 3;
            rgb[o] = r;
            rgb[o + 1] = g;
            rgb[o + 2] = b;
        }
    }
    rgb
}
