//! `vc-server`: hosted VeriChain log. Env: VC_DATA (default ./vc-data), VC_HOST (origin host, default localhost),
//! VC_ADDR (default 127.0.0.1:8080), VC_ADMIN_TOKEN (required, >= 24 chars),
//! VC_COLLECT=1 (start the public market-data collector at boot; also controllable from the console). Put TLS in front (Caddy/nginx/cloud LB).
fn main() -> std::process::ExitCode {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("runtime");
    rt.block_on(async {
        let env = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
        let Ok(tok) = std::env::var("VC_ADMIN_TOKEN") else {
            eprintln!("VC_ADMIN_TOKEN is required (at least 24 characters)");
            return std::process::ExitCode::from(2);
        };
        let app = match vc_server::App::open(
            env("VC_DATA", "vc-data").into(),
            &env("VC_HOST", "localhost"),
            &tok,
        ) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("startup failed: {e}");
                return std::process::ExitCode::from(2);
            }
        };
        println!("operator public key: {}", app.operator_public_key());
        if env("VC_COLLECT", "0") == "1" {
            match app.start_collector_default() {
                Ok(()) => println!("market data collector started (VC_COLLECT=1)"),
                Err(e) => eprintln!("collector not started: {e}"),
            }
        }
        let addr = env("VC_ADDR", "127.0.0.1:8080");
        let l = match tokio::net::TcpListener::bind(&addr).await {
            Ok(l) => l,
            Err(e) => {
                eprintln!("cannot bind {addr}: {e}");
                return std::process::ExitCode::from(2);
            }
        };
        println!("vc-server listening on {addr}");
        let _ = axum::serve(l, vc_server::router(app))
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await;
        std::process::ExitCode::SUCCESS
    })
}
