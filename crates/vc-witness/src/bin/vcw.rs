//! `vcw`: run an independent witness for a VeriChain log.
//!   vcw keygen <key-file>                         create a witness key, print its public key (hex)
//!   vcw run --log URL --origin O --operator-key HEX --key-file F --name N --state-dir D --token T [--every SECS]
//! The key file holds 32 secret bytes as hex (mode 0600). Without --every, one round runs and exits (0 on success).
use ed25519_dalek::SigningKey;
use std::io::Read;
use vc_log::checkpoint::KeyInfo;
use vc_witness::{client, FileState, Witness};

fn rand32() -> Result<[u8; 32], String> {
    let mut b = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut b))
        .map_err(|e| e.to_string())?;
    Ok(b)
}

fn main() -> std::process::ExitCode {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match run(&a) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("vcw: {e}");
            std::process::ExitCode::from(1)
        }
    }
}

fn opt<'a>(a: &'a [String], k: &str) -> Option<&'a str> {
    a.iter()
        .position(|x| x == k)
        .and_then(|i| a.get(i + 1))
        .map(String::as_str)
}

fn run(a: &[String]) -> Result<(), String> {
    match a.first().map(String::as_str) {
        Some("keygen") if a.len() == 2 => {
            let p = std::path::Path::new(&a[1]);
            if p.exists() {
                return Err("key file already exists; refusing to overwrite".into());
            }
            let b = rand32()?;
            std::fs::write(p, hex::encode(b)).map_err(|e| e.to_string())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
            }
            println!("{}", hex::encode(SigningKey::from_bytes(&b).verifying_key().to_bytes()));
            Ok(())
        }
        Some("run") => {
            let need = |k: &str| opt(a, k).ok_or(format!("missing {k}"));
            let kb: [u8; 32] = hex::decode(std::fs::read_to_string(need("--key-file")?).map_err(|e| e.to_string())?.trim())
                .map_err(|e| e.to_string())?
                .try_into()
                .map_err(|_| "bad key file")?;
            let opk: [u8; 32] = hex::decode(need("--operator-key")?).map_err(|e| e.to_string())?.try_into().map_err(|_| "bad operator key")?;
            let origin = need("--origin")?;
            let store = FileState::new(need("--state-dir")?).map_err(|e| e.to_string())?;
            let mut w = Witness::new(need("--name")?, SigningKey::from_bytes(&kb), store).map_err(|e| e.to_string())?;
            w.add_log(origin, KeyInfo { name: "operator".into(), key: ed25519_dalek::VerifyingKey::from_bytes(&opk).map_err(|e| e.to_string())? });
            let every: Option<u64> = opt(a, "--every").and_then(|s| s.parse().ok());
            loop {
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_secs();
                match client::run_once(&mut w, need("--log")?, origin, need("--token")?, now) {
                    Ok(n) => println!("cosigned {origin} at size {n}"),
                    Err(e) if every.is_some() => eprintln!("vcw: {e}"),
                    Err(e) => return Err(e.to_string()),
                }
                match every {
                    Some(s) => std::thread::sleep(std::time::Duration::from_secs(s.max(5))),
                    None => return Ok(()),
                }
            }
        }
        _ => Err("usage: vcw keygen <key-file> | vcw run --log URL --origin O --operator-key HEX --key-file F --name N --state-dir D --token T [--every SECS]".into()),
    }
}
