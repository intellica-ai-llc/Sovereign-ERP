//! Minimal WebAuthn (passkey) verification for ES256 credentials: registration with attestation "none"
//! (the browser hands us the SPKI public key) and assertions. Only what approvals need; no attestation trust.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64U, Engine};
use p256::ecdsa::{signature::Verifier, Signature, VerifyingKey};
use p256::pkcs8::DecodePublicKey;
use serde_json::Value;
use sha2::{Digest, Sha256};

const FLAG_UP: u8 = 0x01;
const FLAG_UV: u8 = 0x04;

pub fn parse_spki(der: &[u8]) -> Result<VerifyingKey, &'static str> {
    VerifyingKey::from_public_key_der(der).map_err(|_| "invalid P-256 public key")
}

fn client_data_ok(cd: &[u8], typ: &str, challenge: &str, origin: &str) -> Result<(), &'static str> {
    let v: Value = serde_json::from_slice(cd).map_err(|_| "clientDataJSON is not JSON")?;
    if v["type"] != typ {
        return Err("wrong ceremony type");
    }
    if v["challenge"].as_str() != Some(challenge) {
        return Err("challenge mismatch");
    }
    if v["origin"].as_str() != Some(origin) {
        return Err("origin mismatch");
    }
    if v.get("crossOrigin") == Some(&Value::Bool(true)) {
        return Err("cross-origin ceremony refused");
    }
    Ok(())
}

/// Registration: the ceremony must carry our challenge and origin; the key is taken as given (attestation none).
pub fn verify_registration(
    client_data_json: &[u8],
    challenge_b64u: &str,
    origin: &str,
) -> Result<(), &'static str> {
    client_data_ok(client_data_json, "webauthn.create", challenge_b64u, origin)
}

/// Assertion: returns the authenticator's new signature counter.
#[allow(clippy::too_many_arguments)]
pub fn verify_assertion(
    rp_id: &str,
    origin: &str,
    challenge_b64u: &str,
    key: &VerifyingKey,
    stored_count: u32,
    authenticator_data: &[u8],
    client_data_json: &[u8],
    signature_der: &[u8],
) -> Result<u32, &'static str> {
    client_data_ok(client_data_json, "webauthn.get", challenge_b64u, origin)?;
    if authenticator_data.len() < 37 {
        return Err("authenticator data too short");
    }
    if authenticator_data[..32] != Sha256::digest(rp_id.as_bytes())[..] {
        return Err("rpIdHash mismatch");
    }
    let flags = authenticator_data[32];
    if flags & FLAG_UP == 0 || flags & FLAG_UV == 0 {
        return Err("user presence and user verification are both required");
    }
    let count = u32::from_be_bytes(authenticator_data[33..37].try_into().unwrap());
    if (count != 0 || stored_count != 0) && count <= stored_count {
        return Err("signature counter did not increase (cloned authenticator?)");
    }
    let mut msg = authenticator_data.to_vec();
    msg.extend_from_slice(&Sha256::digest(client_data_json));
    let sig = Signature::from_der(signature_der).map_err(|_| "malformed signature")?;
    key.verify(&msg, &sig).map_err(|_| "signature invalid")?;
    Ok(count)
}

pub fn b64u(b: &[u8]) -> String {
    B64U.encode(b)
}
pub fn unb64u(s: &str) -> Option<Vec<u8>> {
    B64U.decode(s).ok()
}
