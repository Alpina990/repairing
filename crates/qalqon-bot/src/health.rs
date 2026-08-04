use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use qalqon_core::ModerationStore;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::timeout,
};

pub async fn bind(addr: SocketAddr) -> Result<TcpListener> {
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("health server {addr} manziliga bind bo'lmadi"))?;
    tracing::info!(%addr, "health server ishga tushdi");
    Ok(listener)
}

pub async fn serve(listener: TcpListener, store: Arc<dyn ModerationStore>) -> Result<()> {
    loop {
        let (socket, _) = listener.accept().await?;
        let store = Arc::clone(&store);
        tokio::spawn(async move {
            if let Err(error) = handle(socket, store).await {
                tracing::debug!(%error, "health request bajarilmadi");
            }
        });
    }
}

async fn handle(mut socket: TcpStream, store: Arc<dyn ModerationStore>) -> Result<()> {
    let mut buffer = [0_u8; 1024];
    let size = timeout(Duration::from_secs(2), socket.read(&mut buffer))
        .await
        .context("health request timeout")??;
    let request = std::str::from_utf8(&buffer[..size]).unwrap_or_default();
    let path = request.split_whitespace().nth(1).unwrap_or("/");

    let (status, body) = match path {
        "/healthz" => ("200 OK", "ok"),
        "/readyz" if store.healthcheck().await.is_ok() => ("200 OK", "ready"),
        "/readyz" => ("503 Service Unavailable", "database unavailable"),
        _ => ("404 Not Found", "not found"),
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    socket.write_all(response.as_bytes()).await?;
    socket.shutdown().await?;
    Ok(())
}

pub async fn probe(addr: SocketAddr) -> Result<()> {
    let connect_addr = SocketAddr::new(loopback_for(addr.ip()), addr.port());
    let mut stream = timeout(Duration::from_secs(3), TcpStream::connect(connect_addr))
        .await
        .context("health server connection timeout")??;
    stream
        .write_all(b"GET /readyz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await?;
    let mut response = Vec::with_capacity(256);
    timeout(Duration::from_secs(3), stream.read_to_end(&mut response))
        .await
        .context("health response timeout")??;
    if !response.starts_with(b"HTTP/1.1 200") {
        bail!("readiness probe 200 qaytarmadi");
    }
    Ok(())
}

fn loopback_for(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::LOCALHOST),
    }
}

#[cfg(test)]
mod tests {
    use qalqon_storage::PgModerationStore;

    use super::*;

    #[test]
    fn unspecified_addresses_become_loopback() {
        assert_eq!(
            loopback_for("0.0.0.0".parse().expect("valid IP")),
            "127.0.0.1".parse::<IpAddr>().expect("valid IP")
        );
        assert_eq!(
            loopback_for("::".parse().expect("valid IP")),
            "::1".parse::<IpAddr>().expect("valid IP")
        );
    }

    #[tokio::test]
    async fn readiness_roundtrip_with_postgres() {
        let Ok(database_url) = std::env::var("TEST_DATABASE_URL") else {
            eprintln!("TEST_DATABASE_URL yo'q: health integration testi skip qilindi");
            return;
        };
        let store: Arc<dyn ModerationStore> = Arc::new(
            PgModerationStore::connect(&database_url, 2)
                .await
                .expect("test PostgreSQL must be reachable"),
        );
        let listener = bind("127.0.0.1:0".parse().expect("valid address"))
            .await
            .expect("bind health listener");
        let address = listener.local_addr().expect("health listener address");
        let task = tokio::spawn(serve(listener, store));

        probe(address).await.expect("readiness probe must pass");
        task.abort();
        let result = task.await.expect_err("aborted task returns join error");
        assert!(result.is_cancelled());
    }
}
