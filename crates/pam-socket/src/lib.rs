use std::ffi::CStr;
use std::sync::OnceLock;

use pam::constants::{PamFlag, PamResultCode, PAM_PROMPT_ECHO_OFF};
use pam::conv::Conv;
use pam::module::{PamHandle, PamHooks};
use pam::pam_try;
use snas::clients::{SocketClient, UserClient};
use snas::{SecureString, DEFAULT_SOCKET_PATH};
use tokio::runtime::Runtime;
use tracing::error;

static RUNTIME: OnceLock<(Runtime, SocketClient)> = OnceLock::new();

struct PamSocket;
pam::pam_hooks!(PamSocket);

impl PamHooks for PamSocket {
    fn sm_authenticate(pamh: &mut PamHandle, _args: Vec<&CStr>, _flags: PamFlag) -> PamResultCode {
        let (runtime, client) = RUNTIME.get_or_init(initialize_runtime);
        // Initialize PAM conversation
        let conv = match pamh.get_item::<Conv>() {
            Ok(Some(conv)) => conv,
            Ok(None) => {
                return PamResultCode::PAM_CONV_ERR;
            }
            Err(err_code) => {
                error!(?err_code, "Could not get pam_conv");
                return err_code;
            }
        };

        let user = match pamh.get_user(None) {
            Ok(u) => u,
            Err(err_code) => {
                error!(?err_code, "Could not get user");
                return err_code;
            }
        };

        // Ask for input
        let response = pam_try!(conv.send(PAM_PROMPT_ECHO_OFF, "Password: ")).map(|cs| cs.to_str());

        match response {
            Some(Ok(password)) => {
                let password: SecureString = password.into();

                match runtime.block_on(client.verify(&user, password)) {
                    // TODO: Maybe use set_item to store a group list?
                    Ok(res) if res.valid => PamResultCode::PAM_SUCCESS,
                    Ok(_res) => PamResultCode::PAM_AUTH_ERR,
                    Err(err) => {
                        error!(%err, "Error when calling server");
                        PamResultCode::PAM_SYSTEM_ERR
                    }
                }
            }
            None | Some(Err(_)) => PamResultCode::PAM_AUTH_ERR,
        }
    }

    fn sm_setcred(_pamh: &mut PamHandle, _args: Vec<&CStr>, _flags: PamFlag) -> PamResultCode {
        println!("Set credentials.");
        // I think when we this is called, we create any missing groups and then assign them as well as creating a home directory if needed. When called with close we should delete groups but not the directory
        PamResultCode::PAM_SUCCESS
    }

    fn acct_mgmt(_pamh: &mut PamHandle, _args: Vec<&CStr>, _flags: PamFlag) -> PamResultCode {
        println!("Account management.");
        PamResultCode::PAM_SUCCESS
    }
    fn sm_chauthtok(_pamh: &mut PamHandle, _args: Vec<&CStr>, _flags: PamFlag) -> PamResultCode {
        todo!("Implement change password")
    }
}

fn initialize_runtime() -> (Runtime, SocketClient) {
    // Purposefully ignoring the error as that means the subscriber was already created
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .try_init();
    // We have to panic here because get_or_try_init is unstable for a `OnceLock`
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Unable to initialize async runtime");
    let client = runtime
        .block_on(SocketClient::new(
            std::env::var("SNAS_PAM_SOCKET_PATH")
                .unwrap_or_else(|_| DEFAULT_SOCKET_PATH.to_string()),
        ))
        .expect("Unable to create socket client");
    (runtime, client)
}
