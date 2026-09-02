use crate::{db::Database, metrics::Metrics};
use anyhow::Result;
use std::sync::Arc;
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpListener};

pub async fn serve(host: String, port: u16, metrics: Arc<Metrics>, db: Database) -> Result<()> {
    let listener = TcpListener::bind((host.as_str(), port)).await?;
    tracing::info!(%host, %port, "health server listening");
    loop {
        let (mut stream, _) = listener.accept().await?;
        let metrics = metrics.clone();
        let db = db.clone();
        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            let n = match stream.read(&mut buf).await { Ok(n) => n, Err(_) => return };
            let req = String::from_utf8_lossy(&buf[..n]);
            let path = req.lines().next().and_then(|l| l.split_whitespace().nth(1)).unwrap_or("/");
            let (status, content_type, body) = match path {
                "/health" => ("200 OK", "application/json", "{\"status\":\"ok\"}".to_string()),
                "/ready" if metrics.ready() => ("200 OK", "application/json", "{\"ready\":true}".to_string()),
                "/ready" => ("503 Service Unavailable", "application/json", "{\"ready\":false}".to_string()),
                "/metrics" => ("200 OK", "text/plain; version=0.0.4", metrics.prometheus()),
                "/stats" => {
                    match db.db_stats().await {
                        Ok(s) => ("200 OK", "application/json", format!(
                            "{{\"uptime_seconds\":{},\"senryus\":{},\"muted_channels\":{},\"opt_outs\":{},\"database_connected\":{}}}",
                            metrics.uptime_seconds(), s.senryu_count, s.muted_channel_count, s.opt_out_count, s.connected)),
                        Err(_) => ("500 Internal Server Error", "application/json", "{\"error\":\"database\"}".to_string()),
                    }
                }
                _ => ("404 Not Found", "text/plain; charset=utf-8", "not found".to_string()),
            };
            let response = format!("HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.as_bytes().len());
            let _ = stream.write_all(response.as_bytes()).await;
        });
    }
}
