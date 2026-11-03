//! `vc-witnessd`. Env: WITNESS_DIR (default ./witness-data), WITNESS_NAME (required, signed-note key name),
//! WITNESS_ADDR (default 127.0.0.1:8090). The witness key is created on first start in `<dir>/witness.key`;
//! its public key is printed so log operators can register it. Put TLS in front.
use ed25519_dalek::SigningKey;
use std::io::Read;
fn main() -> std::process::ExitCode {
    let env = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
    let Ok(name) = std::env::var("WITNESS_NAME") else {
        eprintln!("WITNESS_NAME is required");
        return std::process::ExitCode::from(2);
    };
    let dir: std::path::PathBuf = env("WITNESS_DIR", "witness-data").into();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("cannot create {}: {e}", dir.display());
        return std::process::ExitCode::from(2);
    }
    let kp = dir.join("witness.key");
    let sk = match std::fs::read_to_string(&kp) {
        Ok(s) => match hex::decode(s.trim())
            .ok()
            .and_then(|b| <[u8; 32]>::try_from(b).ok())
        {
            Some(b) => SigningKey::from_bytes(&b),
            None => {
                eprintln!("{} is not a 32-byte hex key", kp.display());
                return std::process::ExitCode::from(2);
            }
        },
        Err(_) => {
            let mut b = [0u8; 32];
            if std::fs::File::open("/dev/urandom")
                .and_then(|mut f| f.read_exact(&mut b))
                .is_err()
                || std::fs::write(&kp, hex::encode(b)).is_err()
            {
                eprintln!("cannot create witness key");
                return std::process::ExitCode::from(2);
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&kp, std::fs::Permissions::from_mode(0o600));
            }
            SigningKey::from_bytes(&b)
        }
    };
    println!(
        "witness {name} public key: {}",
        hex::encode(sk.verifying_key().to_bytes())
    );
    let w = match vc_witnessd::open(dir, &name, sk) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("startup failed: {e}");
            return std::process::ExitCode::from(2);
        }
    };
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("runtime");
    rt.block_on(async {
        let addr = env("WITNESS_ADDR", "127.0.0.1:8090");
        let Ok(l) = tokio::net::TcpListener::bind(&addr).await else {
            eprintln!("cannot bind {addr}");
            return std::process::ExitCode::from(2);
        };
        println!("vc-witnessd listening on {addr}");
        let _ = axum::serve(l, vc_witnessd::router(w))
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await;
        std::process::ExitCode::SUCCESS
    })
}
