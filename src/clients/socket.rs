use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{de::DeserializeOwned, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Interest};
use tokio::net::UnixStream;
use tokio::sync::Mutex;
use tracing::{instrument, trace};

use crate::api::{
    GenericResponse, PasswordChangeRequest, VerificationRequest, VerificationResponse,
};
use crate::clients::UserClient;
use crate::{SecureString, REQUEST_IDENTIFIER, RESPONSE_IDENTIFIER, TERMINATOR};

/// A client for communicating with the SNAS user API over a unix socket. It will automatically try
/// to reconnect the socket if it is disconnected.
///
/// This client is not clonable as it cannot support multiple writes or reads simultaneously.
/// However, you can use [`try_clone`](Self::try_clone) to open a new socket
pub struct SocketClient {
    // This needs to be in a mutex to properly implement the `UserClient` trait which doesn't have
    // the ability to do a `mut self`
    //
    // NOTE: There isn't a way to call shutdown on cleanup as it is async. So this isn't a great
    // cleanup when it is automatically dropped
    socket: Mutex<tokio::net::UnixStream>,
    socket_path: PathBuf,
}

impl SocketClient {
    /// Creates a new socket client from a socket path
    pub async fn new(socket_path: impl AsRef<Path>) -> anyhow::Result<Self> {
        Ok(Self {
            socket: Mutex::new(UnixStream::connect(&socket_path).await?),
            socket_path: socket_path.as_ref().to_owned(),
        })
    }

    /// Attempts to clone this client, returning a new client if successful. This will be a
    /// completely different socket connection and does not share any resources with the original.
    pub async fn try_clone(&self) -> anyhow::Result<Self> {
        Ok(Self {
            socket: Mutex::new(UnixStream::connect(&self.socket_path).await?),
            socket_path: self.socket_path.clone(),
        })
    }

    /// Cleanly shutdown the socket. This is necessary because there is no async drop yet.
    pub async fn shutdown(self) -> anyhow::Result<()> {
        let mut socket = self.socket.into_inner();
        socket.shutdown().await.map_err(Into::into)
    }

    #[instrument(level = "debug", skip(self, data))]
    async fn send_request<Req: Serialize, Resp: DeserializeOwned>(
        &self,
        method: &str,
        data: Req,
    ) -> anyhow::Result<GenericResponse<Resp>> {
        let mut buf = Vec::new();
        buf.extend_from_slice(REQUEST_IDENTIFIER);
        buf.extend_from_slice(method.as_bytes());
        buf.push(b'\n');
        serde_json::to_writer(&mut buf, &data)?;
        buf.push(b'\r');
        buf.extend_from_slice(TERMINATOR);

        trace!(len = %buf.len(), "Sending request");
        let mut socket = self.socket.lock().await;

        socket.write_all(&buf).await?;
        socket.flush().await?;

        let data = parse_response(&mut socket).await?;

        serde_json::from_slice(&data).map_err(Into::into)
    }

    /// Helper that reconnects the client if the connection is closed (only for write)
    async fn reconnect(&self) -> anyhow::Result<()> {
        let mut socket = self.socket.lock().await;
        let can_write = match socket.ready(Interest::WRITABLE).await {
            Ok(ready) if ready.is_writable() => true,
            Ok(ready) if ready.is_write_closed() => {
                trace!("Socket is write closed, reconnecting");
                false
            }
            // Any other ready state shouldn't occur
            Ok(ready) => {
                trace!(
                    "Socket is in an unexpected ready state: {:?}, reconnecting",
                    ready
                );
                false
            }
            Err(e)
                if matches!(
                    e.kind(),
                    ErrorKind::ConnectionReset
                        | ErrorKind::BrokenPipe
                        | ErrorKind::NotConnected
                        | ErrorKind::UnexpectedEof
                        | ErrorKind::ConnectionAborted
                        | ErrorKind::Interrupted
                        | ErrorKind::TimedOut,
                ) =>
            {
                trace!(err = %e, "Socket connection errored, reconnecting");
                false
            }
            Err(e) => {
                return Err(e.into());
            }
        };
        if !can_write {
            *socket = UnixStream::connect(&self.socket_path).await?;
        }

        Ok(())
    }
}

impl UserClient for SocketClient {
    async fn verify(
        &self,
        username: &str,
        password: SecureString,
    ) -> anyhow::Result<VerificationResponse> {
        self.reconnect().await?;
        let resp = self
            .send_request(
                "verify",
                VerificationRequest {
                    username: username.to_owned(),
                    password,
                },
            )
            .await?;
        resp.into_result_required()
            .context("Error while verifying user")
    }

    async fn change_password(
        &self,
        username: &str,
        old_password: SecureString,
        new_password: SecureString,
    ) -> anyhow::Result<()> {
        self.reconnect().await?;
        let resp = self
            .send_request(
                "change_password",
                PasswordChangeRequest {
                    username: username.to_owned(),
                    old_password,
                    new_password,
                },
            )
            .await?;
        resp.into_result_empty()
            .context("Error while changing password")
    }
}

async fn parse_response(stream: &mut UnixStream) -> anyhow::Result<Vec<u8>> {
    let mut reader = BufReader::new(stream);
    let mut buf = [0u8; RESPONSE_IDENTIFIER.len()];
    reader.read_exact(&mut buf).await?;
    let mut data = Vec::new();
    let read = reader.read_until(b'\r', &mut data).await?;
    trace!(num_bytes = read, "Read response");
    if read == 0 {
        return Err(anyhow::anyhow!("Socket closed"));
    }
    match data.pop() {
        Some(b'\r') => (),
        _ => {
            anyhow::bail!("Got malformed response");
        }
    }
    let mut buf = [0u8; TERMINATOR.len()];
    reader.read_exact(&mut buf).await?;
    if buf != TERMINATOR {
        anyhow::bail!("Got malformed response");
    }
    Ok(data)
}
