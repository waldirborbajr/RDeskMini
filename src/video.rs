//! H.264 encoding/decoding and pixel format conversion.

use openh264::encoder::{Encoder, EncoderConfig};

// ============================================================
// H.264 helpers
// ============================================================
struct H264EncoderState {
    pub(crate) encoder: Encoder,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl H264EncoderState {
    pub(crate) fn new(w: u32, h: u32) -> Result<Self, String> {
        let cfg = EncoderConfig::new(w, h);
        let enc = Encoder::with_config(cfg).map_err(|e| format!("Encoder: {e}"))?;
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

pub(crate) fn rgb_to_yuv420(rgb: &[u8], w: u32, h: u32) -> openh264::formats::YUVBuffer {
    let wu = w as usize;
    let hu = h as usize;
    let mut y_p = vec![0u8; wu * hu];
    let mut u_p = vec![0u8; (wu / 2) * (hu / 2)];
    let mut v_p = vec![0u8; (wu / 2) * (hu / 2)];

    for y in 0..hu {
        for x in 0..wu {
            let i = (y * wu + x) * 3;
            let r = rgb[i] as f32;
            let g = rgb[i + 1] as f32;
            let b = rgb[i + 2] as f32;
            y_p[y * wu + x] = (0.299 * r + 0.587 * g + 0.114 * b).clamp(0.0, 255.0) as u8;
        }
    }
    for y in (0..hu).step_by(2) {
        for x in (0..wu).step_by(2) {
            let mut rs = 0.0;
            let mut gs = 0.0;
            let mut bs = 0.0;
            for dy in 0..2 {
                for dx in 0..2 {
                    let px = (x + dx).min(wu - 1);
                    let py = (y + dy).min(hu - 1);
                    let i = (py * wu + px) * 3;
                    rs += rgb[i] as f32;
                    gs += rgb[i + 1] as f32;
                    bs += rgb[i + 2] as f32;
                }
            }
            let r = rs / 4.0;
            let g = gs / 4.0;
            let b = bs / 4.0;
            let u = -0.169 * r - 0.331 * g + 0.5 * b + 128.0;
            let v = 0.5 * r - 0.419 * g - 0.081 * b + 128.0;
            let ui = (y / 2) * (wu / 2) + (x / 2);
            u_p[ui] = u.clamp(0.0, 255.0) as u8;
            v_p[ui] = v.clamp(0.0, 255.0) as u8;
        }
    }
    openh264::formats::YUVBuffer::new_from_yuv(y_p, u_p, v_p)
}

pub(crate) fn yuv_to_rgb(yuv: &openh264::formats::YUVBuffer) -> Vec<u8> {
    let w = yuv.width() as usize;
    let h = yuv.height() as usize;
    let y_p = yuv.y_plane();
    let u_p = yuv.u_plane();
    let v_p = yuv.v_plane();
    let mut rgb = vec![0u8; w * h * 3];
    for y in 0..h {
        for x in 0..w {
            let yi = y * w + x;
            let ci = (y / 2) * (w / 2) + (x / 2);
            let yv = y_p[yi] as f32;
            let uv = u_p[ci] as f32 - 128.0;
            let vv = v_p[ci] as f32 - 128.0;
            let r = (yv + 1.402 * vv).clamp(0.0, 255.0) as u8;
            let g = (yv - 0.344 * uv - 0.714 * vv).clamp(0.0, 255.0) as u8;
            let b = (yv + 1.772 * uv).clamp(0.0, 255.0) as u8;
            let o = yi * 3;
            rgb[o] = r;
            rgb[o + 1] = g;
            rgb[o + 2] = b;
        }
    }
    rgb
}
