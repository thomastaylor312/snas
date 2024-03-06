use std::{os::unix::fs::PermissionsExt, path::Path};

use serde::Serialize;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::time::error::Elapsed;
use tokio::time::Duration;
use tracing::{error, instrument, trace, warn};

use crate::api::{
    GenericResponse, PasswordChangeRequest, VerificationRequest, VerificationResponse,
};
use crate::error::HandleError;
use crate::handlers::Handlers;
use crate::{REQUEST_IDENTIFIER, RESPONSE_IDENTIFIER, TERMINATOR};

const MISBEHAVING_LIMIT: usize = 2048;

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
            let handler = SocketHandler {
                stream: BufReader::new(stream),
                handlers: self.handlers.clone(),
            };
            tokio::spawn(async move {
                if let Err(e) = handler.handle().await {
                    error!("Error handling socket connection: {}", e);
                }
            });
        }
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

struct SocketHandler {
    stream: BufReader<UnixStream>,
    handlers: Handlers,
}

impl SocketHandler {
    #[instrument(level = "trace", skip(self))]
    async fn handle(mut self) -> anyhow::Result<()> {
        loop {
            let (method, body) = match parse_incoming(&mut self.stream).await {
                Ok(v) => v,
                Err(ParseError::ConnectionClosed) => {
                    if let Err(e) = self.stream.shutdown().await {
                        // This isn't fatal, but we should log
                        error!(err= %e, "Error shutting down socket cleanly");
                    }
                    return Ok(());
                }
                Err(ParseError::BadRequest(e)) => {
                    // We have to consume the leftover here if we have a malformed request, so we
                    // have a nested block here
                    match consume_leftover(&mut self.stream).await {
                        // We don't return bad request from consume_leftover, so we can continue on
                        Ok(_) | Err(ParseError::BadRequest(_)) => (),
                        Err(ParseError::ConnectionClosed) => {
                            if let Err(e) = self.stream.shutdown().await {
                                // This isn't fatal, but we should log
                                error!(err= %e, "Error shutting down socket cleanly");
                            }
                            return Ok(());
                        }
                        Err(ParseError::Error(e)) => {
                            if let Err(e) = self.stream.shutdown().await {
                                // This isn't fatal, but we should log
                                error!(err= %e, "Error shutting down socket cleanly");
                            }
                            return Err(e);
                        }
                    }
                    self.send_error(e).await;
                    continue;
                }
                Err(ParseError::Error(e)) => {
                    if let Err(e) = self.stream.shutdown().await {
                        // This isn't fatal, but we should log
                        error!(err= %e, "Error shutting down socket cleanly");
                    }
                    return Err(e);
                }
            };

            trace!(%method, len=%body.len(), "Received request");
            match method.as_str() {
                "verify" => {
                    self.handle_verify(body).await;
                }
                "change_password" => {
                    self.handle_change_password(body).await;
                }
                _ => {
                    self.send_error(format!("Unknown method {method}")).await;
                }
            }
        }
    }

    async fn send_error(&mut self, message: impl ToString) {
        // This is more allocations, but results in cleaner code. Highly doubt we need to optimize
        // here
        let mut data = RESPONSE_IDENTIFIER.to_vec();
        if let Err(e) =
            serde_json::to_writer(&mut data, &GenericResponse::new(false, message.to_string()))
        {
            error!(err = %e, "Error serializing error response");
            return;
        };
        data.extend_from_slice(TERMINATOR);
        if let Err(e) = self.stream.write_all(&data).await {
            error!(err = %e, "Error sending error response");
        }
    }

    async fn send_response<T: Serialize>(&mut self, response: T) {
        // This is more allocations, but results in cleaner code. Highly doubt we need to optimize
        // here
        let mut data = RESPONSE_IDENTIFIER.to_vec();
        if let Err(e) = serde_json::to_writer(&mut data, &response) {
            error!(err = %e, "Error serializing response");
            return;
        };
        data.extend_from_slice(TERMINATOR);
        if let Err(e) = self.stream.write_all(&data).await {
            error!(err = %e, "Error sending response");
        }
    }

    async fn handle_verify(&mut self, data: Vec<u8>) {
        let req: VerificationRequest = match serde_json::from_slice(&data) {
            Ok(r) => r,
            Err(e) => {
                self.send_error(format!("Error parsing verification request: {}", e))
                    .await;
                return;
            }
        };
        // TODO(thomastaylor312): This is essentially a copy paste of what we do in NATS, but with a
        // different way to send back the response. Might be worth abstracting this out later
        match self.handlers.verify(&req.username, req.password).await {
            Ok(r) => {
                self.send_response(GenericResponse {
                    success: true,
                    message: "Verification succeeded".to_string(),
                    response: Some(r),
                })
                .await;
            }
            Err(HandleError::InvalidCredentials) => {
                self.send_response(GenericResponse {
                    success: true,
                    message: "Verification failed".to_string(),
                    response: Some(VerificationResponse {
                        valid: false,
                        message: HandleError::InvalidCredentials.to_string(),
                        needs_password_reset: false,
                        groups: Default::default(),
                    }),
                })
                .await;
            }
            Err(HandleError::PasswordResetExpired) => {
                self.send_response(GenericResponse {
                    success: true,
                    message: "Verification failed".to_string(),
                    response: Some(VerificationResponse {
                        valid: false,
                        message: HandleError::PasswordResetExpired.to_string(),
                        needs_password_reset: true,
                        groups: Default::default(),
                    }),
                })
                .await;
            }
            Err(err) => {
                self.send_error(format!("verification failed: {}", err))
                    .await;
            }
        }
    }

    async fn handle_change_password(&mut self, data: Vec<u8>) {
        let req: PasswordChangeRequest = match serde_json::from_slice(&data) {
            Ok(r) => r,
            Err(e) => {
                self.send_error(format!("Error parsing password change request: {}", e))
                    .await;
                return;
            }
        };

        // TODO(thomastaylor312): This is essentially a copy paste of what we do in NATS, but with a
        // different way to send back the response. Might be worth abstracting this out later
        match self
            .handlers
            .change_password(&req.username, req.old_password, req.new_password)
            .await
        {
            Ok(_) => {
                self.send_response(GenericResponse::new(true, "password changed".to_string()))
                    .await;
            }
            Err(err) => {
                self.send_error(format!("password change failed: {}", err))
                    .await;
            }
        }
    }
}

#[derive(Debug)]
enum ParseError {
    ConnectionClosed,
    BadRequest(anyhow::Error),
    Error(anyhow::Error),
}
/// Helper function that handles an io read future and handles errors and logging. Returns true if
/// the handle function should exit or if an error occurred, the error can be returned via try
async fn perform_read<F>(fut: F) -> Result<(), ParseError>
where
    F: std::future::Future<Output = std::io::Result<usize>>,
{
    match fut.await {
        Ok(1..) => Ok(()),
        Ok(0) => {
            trace!("Client disconnected");
            Err(ParseError::ConnectionClosed)
        }
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
            warn!("Client disconnected due to unexpected EOF");
            Err(ParseError::ConnectionClosed)
        }
        // Anything else probably means the socket is no longer valid so return the error
        Err(e) => {
            error!("Error reading from socket: {}", e);
            Err(ParseError::Error(anyhow::anyhow!(
                "Error reading from socket: {e}"
            )))
        }
    }
}

async fn consume_leftover(stream: &mut BufReader<UnixStream>) -> Result<(), ParseError> {
    // HACK ALERT: For some reason, when we are cleaning up data, particularly large amounts of
    // garbage data, the buffer on the buf reader will not fill. Don't ask me why. So what we do
    // instead is we manually consume all of the buffer, returning the amount of data we consumed.
    // Then we read as much data as possible _directly_ from the socket (see a little further down).
    // This should ensure that the buffer is empty, the socket is empty, and we can continue.
    let total_read = if !stream.buffer().is_empty() {
        let consumed = stream.buffer().len();
        // Consume the entire buffer
        stream.consume(consumed);
        consumed
    } else {
        0
    };
    if total_read >= MISBEHAVING_LIMIT {
        // We hit our limit so return without reading anything else
        return Err(ParseError::Error(anyhow::anyhow!(
            "Aborting connection due to too much garbage data"
        )));
    }
    // Check how much data is left in the buffer. If it's greater than 2048 (2kb) including the
    // amount we consumed from the buffer, something is probably wrong or the client is misbehaving
    let remaining = MISBEHAVING_LIMIT - total_read;
    let mut buf = vec![0u8; remaining];
    match tokio::time::timeout(Duration::from_millis(300), stream.get_mut().read(&mut buf)).await {
        Ok(Ok(0)) => {
            trace!("Timed out waiting for leftover data from client");
            Err(ParseError::ConnectionClosed)
        }
        Ok(Ok(n)) if n == remaining => {
            // We filled the buffer so just return a shutdown
            Err(ParseError::Error(anyhow::anyhow!(
                "Aborting connection due to too much garbage data"
            )))
        }
        Ok(Ok(n)) => {
            // We cleared out the rest of the data, so we can return
            trace!(len = n, "Read leftover data from client");
            Ok(())
        }
        Ok(Err(e)) => Err(ParseError::Error(e.into())),
        Err(_) => {
            trace!("Timed out waiting for leftover data from client");
            Ok(())
        }
    }
}

async fn parse_incoming(
    stream: &mut BufReader<UnixStream>,
) -> Result<(String, Vec<u8>), ParseError> {
    let mut buf = [0u8; REQUEST_IDENTIFIER.len()];
    // We don't timeout here because this is where we block waiting for a new request
    perform_read(stream.read_exact(&mut buf)).await?;
    if buf != REQUEST_IDENTIFIER {
        return Err(ParseError::BadRequest(anyhow::anyhow!(
            "Invalid request identifier: {}",
            String::from_utf8_lossy(&buf)
        )));
    }
    let mut method = Vec::new();
    match tokio::time::timeout(
        Duration::from_millis(500),
        perform_read(stream.read_until(b'\n', &mut method)),
    )
    .await
    {
        Ok(Ok(_)) => (),
        Ok(Err(e)) => {
            return Err(e);
        }
        Err(Elapsed { .. }) => {
            return Err(ParseError::BadRequest(anyhow::anyhow!(
                "Timed out reading method"
            )));
        }
    }
    match method.pop() {
        Some(b'\n') => (),
        Some(_) => {
            return Err(ParseError::BadRequest(anyhow::anyhow!(
                "Method does not end in newline"
            )));
        }
        None => {
            return Err(ParseError::BadRequest(anyhow::anyhow!("Method was empty")));
        }
    }
    let method = match String::from_utf8(method) {
        Ok(m) => m,
        Err(e) => {
            return Err(ParseError::BadRequest(anyhow::anyhow!(
                "Error parsing method as string: {e}"
            )));
        }
    };

    let mut body = Vec::new();
    match tokio::time::timeout(
        Duration::from_millis(500),
        perform_read(stream.read_until(b'\r', &mut body)),
    )
    .await
    {
        Ok(Ok(_)) => (),
        Ok(Err(e)) => {
            return Err(e);
        }
        Err(Elapsed { .. }) => {
            return Err(ParseError::BadRequest(anyhow::anyhow!(
                "Timed out reading body"
            )));
        }
    }
    // Trim off the trailing \r that comes with the body
    match body.pop() {
        Some(b'\r') => (),
        _ => {
            return Err(ParseError::BadRequest(anyhow::anyhow!(
                "Body does not end with carriage return"
            )));
        }
    }
    // Now make sure we've read to the end of the body
    let mut buf = [0u8; TERMINATOR.len()];
    match tokio::time::timeout(
        Duration::from_millis(500),
        perform_read(stream.read_exact(&mut buf)),
    )
    .await
    {
        Ok(Ok(_)) => (),
        Ok(Err(e)) => {
            return Err(e);
        }
        Err(Elapsed { .. }) => {
            return Err(ParseError::BadRequest(anyhow::anyhow!(
                "Timed out reading terminator"
            )));
        }
    }
    if buf != TERMINATOR {
        return Err(ParseError::BadRequest(anyhow::anyhow!(
            "Invalid terminator"
        )));
    }
    Ok((method, body))
}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(serde::Serialize, serde::Deserialize, PartialEq, Eq, Debug)]
    struct TestBody {
        foo: String,
        bar: u32,
    }

    #[tokio::test]
    async fn test_protocol() {
        let (mut client, server) = UnixStream::pair().unwrap();
        let mut server = BufReader::new(server);

        // Test a good request first
        let mut req = REQUEST_IDENTIFIER.to_vec();

        let test_body = TestBody {
            foo: "hello".to_string(),
            bar: 123,
        };
        req.extend_from_slice("coolmethod\n".as_bytes());
        let body = serde_json::to_vec(&test_body).unwrap();
        req.extend(body);
        req.push(b'\r');
        req.extend_from_slice(TERMINATOR);

        client.write_all(&req).await.unwrap();
        let (method, body) = parse_incoming(&mut server)
            .await
            .expect("Should be able to parse incoming body");
        assert_eq!(method, "coolmethod", "Method should be correct");
        let body: TestBody = serde_json::from_slice(&body).unwrap();
        assert_eq!(body, test_body, "Body should be correct");

        // Do the same request to make sure it works with a second request
        client.write_all(&req).await.unwrap();
        let (method, body) = parse_incoming(&mut server)
            .await
            .expect("Should be able to parse incoming body");
        assert_eq!(method, "coolmethod", "Method should be correct");
        let body: TestBody = serde_json::from_slice(&body).unwrap();
        assert_eq!(body, test_body, "Body should be correct");

        // Test closing the connection
        client.shutdown().await.unwrap();
        let err = parse_incoming(&mut server)
            .await
            .expect_err("Should error with EOF");
        assert!(
            matches!(err, ParseError::ConnectionClosed),
            "Error should be connection closed, got {err:?}"
        );
    }

    #[tokio::test]
    async fn test_protocol_bad_requests() {
        let (mut client, server) = UnixStream::pair().unwrap();
        let mut server = BufReader::new(server);

        // Write some garbage and make sure we get the right error
        client.write_all(b"garbage").await.unwrap();
        let err = parse_incoming(&mut server)
            .await
            .expect_err("Should error with garbage request");
        assert!(
            matches!(err, ParseError::BadRequest(_)),
            "Should error with garbage request, got {err:?}"
        );

        consume_leftover(&mut server)
            .await
            .expect("Should be able to clean out extra data");

        // Test an interrupted request
        client.write_all(REQUEST_IDENTIFIER).await.unwrap();
        let err = parse_incoming(&mut server)
            .await
            .expect_err("Should error with interrupted request");
        assert!(
            matches!(err, ParseError::BadRequest(_)),
            "Should error with interrupted request, got {err:?}",
        );

        consume_leftover(&mut server)
            .await
            .expect("Should be able to clean out extra data");

        // Test interrupted after method
        client.write_all(REQUEST_IDENTIFIER).await.unwrap();
        client.write_all(b"coolmethod\n").await.unwrap();
        let err = parse_incoming(&mut server)
            .await
            .expect_err("Should error with interrupted request");
        assert!(
            matches!(err, ParseError::BadRequest(_)),
            "Should error with interrupted request, got {err:?}"
        );

        consume_leftover(&mut server)
            .await
            .expect("Should be able to clean out extra data");

        // Test interrupted after body
        client.write_all(REQUEST_IDENTIFIER).await.unwrap();
        client.write_all(b"coolmethod\n").await.unwrap();
        client
            .write_all(b"{\"foo\": \"hello\", \"bar\": 123}\r")
            .await
            .unwrap();
        let err = parse_incoming(&mut server)
            .await
            .expect_err("Should error with interrupted request");
        assert!(
            matches!(err, ParseError::BadRequest(_)),
            "Should error with interrupted request, got {err:?}"
        );

        consume_leftover(&mut server)
            .await
            .expect("Should be able to clean out extra data");

        // Test garbage terminator
        client.write_all(REQUEST_IDENTIFIER).await.unwrap();
        client.write_all(b"coolmethod\n").await.unwrap();
        client
            .write_all(b"{\"foo\": \"hello\", \"bar\": 123}\r")
            .await
            .unwrap();
        client.write_all(b"garbage").await.unwrap();
        let err = parse_incoming(&mut server)
            .await
            .expect_err("Should error with garbage terminator");
        assert!(
            matches!(err, ParseError::BadRequest(_)),
            "Should error with garbage terminator, got {err:?}"
        );

        consume_leftover(&mut server)
            .await
            .expect("Should be able to clean out extra data");

        // Just for safety sake, let's do a valid request and make sure it can read it properly

        client.write_all(REQUEST_IDENTIFIER).await.unwrap();
        client.write_all(b"coolmethod\n").await.unwrap();
        client
            .write_all(b"{\"foo\": \"hello\", \"bar\": 123}\r")
            .await
            .unwrap();
        client.write_all(TERMINATOR).await.unwrap();
        let (method, _) = parse_incoming(&mut server)
            .await
            .expect("Should be able to handle subsequent valid request");
        assert_eq!(method, "coolmethod", "Method should be correct");

        // Test non-string method
        client.write_all(REQUEST_IDENTIFIER).await.unwrap();
        client.write_all(&[99, 111, 255]).await.unwrap();
        client.write_u8(b'\n').await.unwrap();
        client
            .write_all(b"{\"foo\": \"hello\", \"bar\": 123}\r")
            .await
            .unwrap();
        client.write_all(TERMINATOR).await.unwrap();
        let err = parse_incoming(&mut server)
            .await
            .expect_err("Should error with non-string method");
        assert!(
            matches!(err, ParseError::BadRequest(_)),
            "Should error with non-string method, got {err:?}"
        );

        // Create a vec of 3000 random bytes to make sure we error on too much data
        let mut body = Vec::new();
        body.resize(3000, 12u8);

        client.write_all(&body).await.unwrap();

        let err = consume_leftover(&mut server)
            .await
            .expect_err("Should error on consume leftover with too much data");
        assert!(
            matches!(err, ParseError::Error(_)),
            "Should error with the correct type, got {err:?}"
        )
    }
}
