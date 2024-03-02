use std::{os::unix::fs::PermissionsExt, path::Path};

use tokio::net::UnixListener;

use crate::handlers::Handlers;

pub struct UserSocket {
    handlers: Handlers,
    socket: UnixListener,
}

impl UserSocket {
    pub async fn new(handlers: Handlers, socket_path: impl AsRef<Path>) -> anyhow::Result<Self> {
        Ok(Self {
            handlers,
            socket: get_socket(socket_path).await?,
        })
    }

    pub async fn run(self) -> anyhow::Result<()> {
        loop {
            let (stream, _) = self.socket.accept().await?;
        }
        todo!("Handle socket connection")
    }
}

async fn get_socket(socket_path: impl AsRef<Path>) -> anyhow::Result<UnixListener> {
    match tokio::fs::remove_file(&socket_path).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    let socket = UnixListener::bind(&socket_path)?;
    // Make sure this is only accessible by the current user
    let mut perms = tokio::fs::metadata(&socket_path).await?.permissions();
    perms.set_mode(0o700);
    tokio::fs::set_permissions(socket_path, perms).await?;
    Ok(socket)
}
