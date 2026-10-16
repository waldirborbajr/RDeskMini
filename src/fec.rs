//! Forward error correction (Reed-Solomon) for UDP video datagrams.

use reed_solomon_erasure::galois_8::ReedSolomon;

use crate::protocol::{FEC_DATA_SHARDS, FEC_PARITY_SHARDS, FEC_SHARD_SIZE};

// ============================================================
// FEC helpers
// ============================================================
struct FecCodec {
    pub(crate) rs: ReedSolomon,
}

impl FecCodec {
    pub(crate) fn new() -> Result<Self, String> {
        let rs = ReedSolomon::new(FEC_DATA_SHARDS, FEC_PARITY_SHARDS)
            .map_err(|e| format!("ReedSolomon::new: {e}"))?;
        Ok(Self { rs })
    }

    pub(crate) fn encode(&self, payload: &[u8]) -> Result<Vec<Vec<u8>>, String> {
        let mut shards: Vec<Vec<u8>> = Vec::with_capacity(FEC_DATA_SHARDS + FEC_PARITY_SHARDS);
        for i in 0..FEC_DATA_SHARDS {
            let start = i * FEC_SHARD_SIZE;
            let end = ((i + 1) * FEC_SHARD_SIZE).min(payload.len());
            let mut shard = vec![0u8; FEC_SHARD_SIZE];
            if start < payload.len() {
                let chunk = &payload[start..end];
                shard[..chunk.len()].copy_from_slice(chunk);
            }
            shards.push(shard);
        }
        for _ in 0..FEC_PARITY_SHARDS {
            shards.push(vec![0u8; FEC_SHARD_SIZE]);
        }
        self.rs
            .encode(&mut shards)
            .map_err(|e| format!("FEC encode: {e}"))?;
        Ok(shards)
    }

    pub(crate) fn decode(
        &self,
        present: Vec<(usize, Vec<u8>)>,
        original_len: usize,
    ) -> Result<Vec<u8>, String> {
        if present.len() < FEC_DATA_SHARDS {
            return Err(format!(
                "Not enough shards: {} < {}",
                present.len(),
                FEC_DATA_SHARDS
            ));
        }
        let mut shards: Vec<Option<Vec<u8>>> = vec![None; FEC_DATA_SHARDS + FEC_PARITY_SHARDS];
        for (i, d) in present {
            if i < shards.len() {
                shards[i] = Some(d);
            }
        }
        self.rs
            .reconstruct(&mut shards)
            .map_err(|e| format!("FEC reconstruct: {e}"))?;
        let mut out = Vec::with_capacity(original_len);
        for s in shards.iter().take(FEC_DATA_SHARDS) {
            if let Some(sd) = s {
                out.extend_from_slice(sd);
            }
        }
        out.truncate(original_len);
        Ok(out)
    }
}

pub(crate) fn encode_udp_datagram(
    block_id: u32,
    shard_idx: u16,
    total: u16,
    orig_len: u32,
    shard: &[u8],
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(12 + shard.len());
    buf.extend_from_slice(&block_id.to_be_bytes());
    buf.extend_from_slice(&shard_idx.to_be_bytes());
    buf.extend_from_slice(&total.to_be_bytes());
    buf.extend_from_slice(&orig_len.to_be_bytes());
    buf.extend_from_slice(shard);
    buf
}

pub(crate) fn decode_udp_datagram(data: &[u8]) -> Option<(u32, u16, u16, u32, &[u8])> {
    if data.len() < 12 {
        return None;
    }
    let b = u32::from_be_bytes(data[0..4].try_into().ok()?);
    let si = u16::from_be_bytes(data[4..6].try_into().ok()?);
    let t = u16::from_be_bytes(data[6..8].try_into().ok()?);
    let ol = u32::from_be_bytes(data[8..12].try_into().ok()?);
    Some((b, si, t, ol, &data[12..]))
}
