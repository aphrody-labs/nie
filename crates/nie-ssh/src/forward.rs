//! Local port forwarding (`-L`).

use crate::connect::Connection;
use crate::{Error, Result};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

/// A `-L [bind:]port:host:port` forward.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Forward {
    pub bind: SocketAddr,
    pub host: String,
    pub port: u16,
}

impl Forward {
    /// Parses `[bind:]port:host:port` (`bind` defaults to 127.0.0.1; `[v6]` brackets accepted).
    pub fn parse(spec: &str) -> Result<Self> {
        let invalid = || Error::new(format!("invalid forward {spec:?}: use [bind:]port:host:port"));
        let mut parts: Vec<String> = Vec::new();
        let mut rest = spec;
        while !rest.is_empty() {
            if let Some(bracketed) = rest.strip_prefix('[') {
                let (inside, after) = bracketed.split_once(']').ok_or_else(invalid)?;
                parts.push(inside.to_owned());
                rest = after.strip_prefix(':').unwrap_or(after);
            } else {
                let (head, after) = rest.split_once(':').unwrap_or((rest, ""));
                parts.push(head.to_owned());
                rest = after;
            }
        }
        let (bind_host, listen, host, port) = match parts.as_slice() {
            [listen, host, port] => ("127.0.0.1", listen, host, port),
            [bind, listen, host, port] => (bind.as_str(), listen, host, port),
            _ => return Err(invalid()),
        };
        let bind_host = if bind_host == "localhost" || bind_host == "*" {
            "127.0.0.1"
        } else {
            bind_host
        };
        let listen: u16 = listen.parse().map_err(|_| invalid())?;
        let port: u16 = port.parse().map_err(|_| invalid())?;
        let ip = bind_host.parse().map_err(|_| invalid())?;
        if host.is_empty() {
            return Err(invalid());
        }
        Ok(Self {
            bind: SocketAddr::new(ip, listen),
            host: host.clone(),
            port,
        })
    }
}

impl Connection {
    /// Binds the forward and relays every accepted connection through the SSH session until the
    /// transport closes. Returns the bound address through `on_bound` before it starts relaying.
    pub async fn forward_local(
        self: Arc<Self>,
        forward: &Forward,
        on_bound: impl FnOnce(SocketAddr),
    ) -> Result<()> {
        let listener = TcpListener::bind(forward.bind)
            .await
            .map_err(|e| Error::new(format!("binding {}: {e}", forward.bind)))?;
        on_bound(listener.local_addr().map_err(|e| Error::new(e.to_string()))?);
        loop {
            if self.is_closed() {
                return Err(Error::new("ssh connection closed"));
            }
            let (mut local, peer) = tokio::select! {
                accepted = listener.accept() => accepted.map_err(|e| Error::new(format!("accept: {e}")))?,
                () = tokio::time::sleep(std::time::Duration::from_secs(1)) => continue,
            };
            let connection = self.clone();
            let (host, port) = (forward.host.clone(), forward.port);
            tokio::spawn(async move {
                let channel = connection
                    .handle()
                    .channel_open_direct_tcpip(host, u32::from(port), peer.ip().to_string(), u32::from(peer.port()))
                    .await;
                if let Ok(channel) = channel {
                    let mut remote = channel.into_stream();
                    let _ = tokio::io::copy_bidirectional(&mut local, &mut remote).await;
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_specs() {
        let f = Forward::parse("8080:example.org:80").unwrap();
        assert_eq!(f.bind, "127.0.0.1:8080".parse().unwrap());
        assert_eq!((f.host.as_str(), f.port), ("example.org", 80));
        let f = Forward::parse("0.0.0.0:2222:10.0.0.5:22").unwrap();
        assert_eq!(f.bind, "0.0.0.0:2222".parse().unwrap());
        let f = Forward::parse("[::1]:9000:db:5432").unwrap();
        assert_eq!(f.bind, "[::1]:9000".parse().unwrap());
        assert!(Forward::parse("80").is_err());
        assert!(Forward::parse("x:host:80").is_err());
    }
}
