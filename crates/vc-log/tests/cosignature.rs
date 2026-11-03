//! Witness cosignatures in C2SP tlog-cosignature/v1 form are accepted by verify_note, with the same
//! quorum-by-key rules as plain signatures, and cannot be confused with operator signatures.
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ed25519_dalek::{Signer, SigningKey};
use vc_log::checkpoint::{
    cosig_key_id, cosig_message, note, sign_line, verify_note, Checkpoint, KeyInfo,
};

fn sk(n: u8) -> SigningKey {
    SigningKey::from_bytes(&[n; 32])
}
fn ki(name: &str, n: u8) -> KeyInfo {
    KeyInfo {
        name: name.into(),
        key: sk(n).verifying_key(),
    }
}
fn cosig(name: &str, key: &SigningKey, ts: u64, text: &str) -> String {
    let sig = key.sign(&cosig_message(ts, text));
    let mut raw = cosig_key_id(name, &key.verifying_key()).to_vec();
    raw.extend_from_slice(&ts.to_be_bytes());
    raw.extend_from_slice(&sig.to_bytes());
    format!("\u{2014} {name} {}\n", B64.encode(raw))
}
fn text() -> String {
    Checkpoint {
        origin: "o".into(),
        size: 5,
        root: [9; 32],
    }
    .text()
}

#[test]
fn two_cosignatures_meet_quorum() {
    let t = text();
    let n = note(
        &t,
        &[
            sign_line(&t, "op", &sk(1)),
            cosig("w1", &sk(2), 100, &t),
            cosig("w2", &sk(3), 200, &t),
        ],
    );
    assert!(verify_note(&n, &ki("op", 1), &[ki("w1", 2), ki("w2", 3)], 2).is_ok());
}

#[test]
fn tampered_timestamp_or_text_or_name_fails() {
    let t = text();
    let good = cosig("w1", &sk(2), 100, &t);
    // flip the timestamp inside the raw signature
    let raw = B64
        .decode(good.trim_end().rsplit_once(' ').unwrap().1)
        .unwrap();
    let mut bad = raw.clone();
    bad[11] ^= 1;
    let line = format!("\u{2014} w1 {}\n", B64.encode(bad));
    let n = note(&t, &[sign_line(&t, "op", &sk(1)), line]);
    assert!(verify_note(&n, &ki("op", 1), &[ki("w1", 2)], 1).is_err());
    // cosignature over different text
    let other = Checkpoint {
        origin: "o".into(),
        size: 6,
        root: [9; 32],
    }
    .text();
    let n = note(
        &t,
        &[
            sign_line(&t, "op", &sk(1)),
            cosig("w1", &sk(2), 100, &other),
        ],
    );
    assert!(verify_note(&n, &ki("op", 1), &[ki("w1", 2)], 1).is_err());
    // wrong name for the key
    let n = note(
        &t,
        &[sign_line(&t, "op", &sk(1)), cosig("w1", &sk(2), 100, &t)],
    );
    assert!(verify_note(&n, &ki("op", 1), &[ki("w9", 2)], 1).is_err());
}

#[test]
fn cosignature_is_not_an_operator_signature_and_aliases_do_not_stack() {
    let t = text();
    // operator "signing" in cosignature form must not count as the operator signature
    let n = note(
        &t,
        &[cosig("op", &sk(1), 1, &t), cosig("w1", &sk(2), 1, &t)],
    );
    assert!(verify_note(&n, &ki("op", 1), &[ki("w1", 2)], 1).is_err());
    // one key under two names counts once
    let n = note(
        &t,
        &[
            sign_line(&t, "op", &sk(1)),
            cosig("w1", &sk(2), 1, &t),
            cosig("w1b", &sk(2), 1, &t),
        ],
    );
    assert!(verify_note(&n, &ki("op", 1), &[ki("w1", 2), ki("w1b", 2)], 2).is_err());
    // the same key as both operator and witness never counts
    let n = note(
        &t,
        &[sign_line(&t, "op", &sk(1)), cosig("opw", &sk(1), 1, &t)],
    );
    assert!(verify_note(&n, &ki("op", 1), &[ki("opw", 1)], 1).is_err());
}
