//! Transparent TLS certificate management (generated on first run).

// ============================================================
// Transparent TLS certificate management
// ============================================================
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use directories::ProjectDirs;
use rcgen::{generate_simple_self_signed, CertifiedKey};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::{ClientConfig, ServerConfig};

const FINGERPRINT_LENGTH: usize = 32;

fn config_dir() -> std::io::Result<PathBuf> {
    let dirs = ProjectDirs::from("com", "rustdesk-mini", "RustDesk-Mini").ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "cannot determine config directory",
        )
    })?;
    let dir = dirs.config_dir().to_path_buf();
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn cert_path() -> std::io::Result<PathBuf> {
    Ok(config_dir()?.join("cert.pem"))
}

fn key_path() -> std::io::Result<PathBuf> {
    Ok(config_dir()?.join("key.pem"))
}

/// Ensures a self-signed cert/key pair exists on disk.
/// Generates them silently on first run.
pub fn ensure_certificates() -> std::io::Result<(PathBuf, PathBuf)> {
    let cp = cert_path()?;
    let kp = key_path()?;

    let needs_generation = !cp.exists() || !kp.exists() || !is_valid_pem(&cp, &kp);
    if needs_generation {
        generate_and_write(&cp, &kp)?;
    }
    Ok((cp, kp))
}

fn is_valid_pem(cp: &PathBuf, kp: &PathBuf) -> bool {
    let cert_ok = fs::read_to_string(cp)
        .map(|s| s.contains("BEGIN CERTIFICATE") && s.len() > 100)
        .unwrap_or(false);
    let key_ok = fs::read_to_string(kp)
        .map(|s| s.contains("PRIVATE KEY") && s.len() > 100)
        .unwrap_or(false);
    cert_ok && key_ok
}

fn generate_and_write(cp: &PathBuf, kp: &PathBuf) -> std::io::Result<()> {
    let subject_alt_names = vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
        "rustdesk-mini.local".to_string(),
    ];

    let CertifiedKey { cert, key_pair } =
        generate_simple_self_signed(subject_alt_names).map_err(|e| {
            std::io::Error::other(
                format!("cert generation failed: {e}"),
            )
        })?;

    {
        let mut f = fs::File::create(cp)?;
        f.write_all(cert.pem().as_bytes())?;
        f.sync_all()?;
    }
    {
        let mut opts = fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(kp)?;
        f.write_all(key_pair.serialize_pem().as_bytes())?;
        f.sync_all()?;
    }
    Ok(())
}

pub fn server_tls_config() -> Result<Arc<ServerConfig>, String> {
    let (cp, kp) = ensure_certificates().map_err(|e| e.to_string())?;

    let cert_data = fs::read_to_string(&cp).map_err(|e| format!("read cert: {e}"))?;
    let key_data = fs::read_to_string(&kp).map_err(|e| format!("read key: {e}"))?;

    let certs: Vec<CertificateDer<'static>> =
        rustls_pemfile::certs(&mut std::io::BufReader::new(cert_data.as_bytes()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("parse cert: {e}"))?;

    let key: PrivateKeyDer<'static> =
        rustls_pemfile::private_key(&mut std::io::BufReader::new(key_data.as_bytes()))
            .map_err(|e| format!("parse key: {e}"))?
            .ok_or_else(|| "no private key found".to_string())?;

    let cfg = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| format!("ServerConfig: {e}"))?;

    Ok(Arc::new(cfg))
}

pub fn certificate_fingerprint() -> Result<String, String> {
    let (cp, _) = ensure_certificates().map_err(|e| e.to_string())?;
    let cert_data = fs::read_to_string(cp).map_err(|e| format!("read cert: {e}"))?;
    let cert = rustls_pemfile::certs(&mut std::io::BufReader::new(cert_data.as_bytes()))
        .next()
        .ok_or_else(|| "no certificate found".to_string())?
        .map_err(|e| format!("parse cert: {e}"))?;
    let fingerprint = ring::digest::digest(&ring::digest::SHA256, cert.as_ref());
    Ok(format_fingerprint(fingerprint.as_ref()))
}

fn format_fingerprint(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}

pub(crate) fn parse_fingerprint(input: &str) -> Option<[u8; FINGERPRINT_LENGTH]> {
    let hex: String = input
        .chars()
        .filter(|ch| !ch.is_whitespace() && *ch != ':' && *ch != '-')
        .collect();
    if hex.len() != FINGERPRINT_LENGTH * 2 || !hex.is_ascii() {
        return None;
    }

    let mut fingerprint = [0; FINGERPRINT_LENGTH];
    for (index, byte) in fingerprint.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(fingerprint)
}

pub fn client_tls_config(expected_fingerprint: [u8; FINGERPRINT_LENGTH]) -> Arc<ClientConfig> {
    let config = ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(FingerprintVerifier {
            expected_fingerprint,
        }))
        .with_no_client_auth();
    Arc::new(config)
}

#[derive(Debug)]
struct FingerprintVerifier {
    expected_fingerprint: [u8; FINGERPRINT_LENGTH],
}

impl rustls::client::danger::ServerCertVerifier for FingerprintVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        let actual = ring::digest::digest(&ring::digest::SHA256, end_entity.as_ref());
        ring::constant_time::verify_slices_are_equal(
            actual.as_ref(),
            &self.expected_fingerprint,
        )
        .map_err(|_| rustls::Error::General("host certificate fingerprint mismatch".into()))?;
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &rustls::crypto::aws_lc_rs::default_provider().signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &rustls::crypto::aws_lc_rs::default_provider().signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::aws_lc_rs::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_colon_separated_sha256_fingerprint() {
        let parsed = parse_fingerprint(
            "00:01:02:03:04:05:06:07:08:09:0A:0B:0C:0D:0E:0F:10:11:12:13:14:15:16:17:18:19:1A:1B:1C:1D:1E:1F",
        );
        assert_eq!(parsed, Some(std::array::from_fn(|i| i as u8)));
    }

    #[test]
    fn accepts_fingerprint_with_hyphens_and_lowercase() {
        let parsed = parse_fingerprint(
            "00010203-04050607-08090a0b-0c0d0e0f-10111213-14151617-18191a1b-1c1d1e1f",
        );
        assert_eq!(parsed, Some(std::array::from_fn(|i| i as u8)));
    }

    #[test]
    fn rejects_malformed_fingerprints() {
        assert_eq!(parse_fingerprint("00:01"), None);
        assert_eq!(parse_fingerprint(&"GG".repeat(FINGERPRINT_LENGTH)), None);
        assert_eq!(parse_fingerprint(&"é".repeat(FINGERPRINT_LENGTH)), None);
    }
}
