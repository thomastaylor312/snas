use std::ffi::{CStr, CString};
use std::path::Path;
use std::sync::OnceLock;

use libc::c_int;
use pam::constants::{PamFlag, PamResultCode, PAM_DELETE_CRED, PAM_PROMPT_ECHO_OFF};
use pam::conv::Conv;
use pam::module::{PamHandle, PamHooks};
use pam::pam_try;
use snas_lib::api::VerificationResponse;
use snas_lib::clients::{SocketClient, UserClient};
use snas_lib::{SecureString, DEFAULT_SOCKET_PATH};
use tokio::runtime::Runtime;
use tracing::error;

static RUNTIME: OnceLock<(Runtime, SocketClient)> = OnceLock::new();
const USER_INFO: &str = "user_info";

struct PamSocket;
pam::pam_hooks!(PamSocket);

impl PamHooks for PamSocket {
    // Authentication function - validates credentials
    fn sm_authenticate(pamh: &mut PamHandle, _args: Vec<&CStr>, _flags: PamFlag) -> PamResultCode {
        let (runtime, client) = RUNTIME.get_or_init(initialize_runtime);

        // Get PAM conversation handler
        let conv = match pamh.get_item::<Conv>() {
            Ok(Some(conv)) => conv,
            Ok(None) => return PamResultCode::PAM_CONV_ERR,
            Err(err_code) => {
                error!(?err_code, "Could not get pam_conv");
                return err_code;
            }
        };

        // Get username
        let user = match pamh.get_user(None) {
            Ok(u) => u,
            Err(err_code) => {
                error!(?err_code, "Could not get user");
                return err_code;
            }
        };

        // Get password
        let response = match pam_try!(conv.send(PAM_PROMPT_ECHO_OFF, "Password: ")) {
            Some(pass) => match pass.to_str() {
                Ok(p) => SecureString::from(p),
                Err(_) => return PamResultCode::PAM_AUTH_ERR,
            },
            None => return PamResultCode::PAM_AUTH_ERR,
        };

        // Verify credentials
        let (code, res) = match runtime.block_on(client.verify(&user, response)) {
            Ok(res) if res.valid && res.needs_password_reset => {
                (PamResultCode::PAM_NEW_AUTHTOK_REQD, res)
            }
            Ok(res) if res.valid => (PamResultCode::PAM_SUCCESS, res),
            Ok(_) => return PamResultCode::PAM_AUTH_ERR,
            Err(err) => {
                error!(%err, "Error when calling server");
                return PamResultCode::PAM_SYSTEM_ERR;
            }
        };
        if let Err(err) = pamh.set_data(USER_INFO, Box::new(res)) {
            error!(?err, "Could not set user info");
            return PamResultCode::PAM_SYSTEM_ERR;
        }
        code
    }

    // Account management - checks if account is valid
    fn acct_mgmt(pamh: &mut PamHandle, _args: Vec<&CStr>, _flags: PamFlag) -> PamResultCode {
        let user_info = unsafe {
            match pamh.get_data::<VerificationResponse>(USER_INFO) {
                Ok(info) => info,
                Err(_) => return PamResultCode::PAM_USER_UNKNOWN,
            }
        };

        if user_info.valid {
            PamResultCode::PAM_SUCCESS
        } else {
            PamResultCode::PAM_ACCT_EXPIRED
        }
    }

    // Credential management - handles group assignments
    fn sm_setcred(pamh: &mut PamHandle, _args: Vec<&CStr>, flag: PamFlag) -> PamResultCode {
        tracing::debug!("beginning set credentials");

        let user = match pamh.get_user(None) {
            Ok(u) => u,
            Err(err_code) => return err_code,
        };

        // Get user info from storage
        let user_info = unsafe {
            match pamh.get_data::<VerificationResponse>(USER_INFO) {
                Ok(info) => info,
                Err(err) => {
                    error!(?err, "Could not get user info from session");
                    return PamResultCode::PAM_SYSTEM_ERR;
                }
            }
        };

        let user_c = match CString::new(user.clone()) {
            Ok(s) => s,
            Err(_) => {
                error!("Invalid username");
                return PamResultCode::PAM_USER_UNKNOWN;
            }
        };

        // Create home directory if it doesn't exist
        let homedir = Path::new("/home").join(&user);
        if let Err(err) = std::fs::create_dir_all(&homedir) {
            error!(%err, "Could not create home directory");
            return PamResultCode::PAM_SYSTEM_ERR;
        }

        // Get user's system info
        let pwd = unsafe {
            let pwd_ptr = libc::getpwnam(user_c.as_ptr());
            if pwd_ptr.is_null() {
                return PamResultCode::PAM_USER_UNKNOWN;
            }
            *pwd_ptr
        };

        if let Err(err) = std::os::unix::fs::chown(homedir, Some(pwd.pw_uid), Some(pwd.pw_gid)) {
            error!(%err, "Could not change ownership of home directory");
            return PamResultCode::PAM_SYSTEM_ERR;
        }

        if flag == PAM_DELETE_CRED {
            // Clear supplementary groups on session end
            if unsafe { libc::setgroups(0, std::ptr::null()) } != 0 {
                return PamResultCode::PAM_SYSTEM_ERR;
            }
        } else {
            // Set up groups for user
            let mut group_ids = Vec::new();

            for group_name in &user_info.groups {
                let group_c = match CString::new(group_name.as_str()) {
                    Ok(s) => s,
                    Err(_) => continue,
                };

                let grp = unsafe {
                    let grp_ptr = libc::getgrnam(group_c.as_ptr());
                    if grp_ptr.is_null() {
                        // Try to create group if it doesn't exist
                        // NOTE: This is a bit of a hack, but I was too lazy to figure out how to
                        // use libc to modify groups
                        let result = libc::system(
                            CString::new(format!("groupadd {}", group_name))
                                .unwrap()
                                .as_ptr(),
                        );
                        if result != 0 {
                            error!("Group {} not found and cannot be created", group_name);
                            return PamResultCode::PAM_SYSTEM_ERR;
                        }
                        // Try getting the group again after creation
                        let grp_ptr = libc::getgrnam(group_c.as_ptr());
                        if grp_ptr.is_null() {
                            error!(
                                "Group {} creation succeeded but group still not found",
                                group_name
                            );
                            return PamResultCode::PAM_SYSTEM_ERR;
                        }
                    }
                    *grp_ptr
                };

                group_ids.push(grp.gr_gid);
            }

            let ngroups: c_int = match group_ids.len().try_into() {
                Ok(ngroups) => ngroups,
                Err(_) => {
                    error!("Too many groups");
                    return PamResultCode::PAM_SYSTEM_ERR;
                }
            };
            if unsafe { libc::setgroups(ngroups, group_ids.as_ptr()) } != 0 {
                return PamResultCode::PAM_SYSTEM_ERR;
            }
        }

        PamResultCode::PAM_SUCCESS
    }

    // Password change functionality
    fn sm_chauthtok(pamh: &mut PamHandle, _args: Vec<&CStr>, _flags: PamFlag) -> PamResultCode {
        let (runtime, client) = RUNTIME.get_or_init(initialize_runtime);

        let conv = match pamh.get_item::<Conv>() {
            Ok(Some(conv)) => conv,
            Ok(None) => return PamResultCode::PAM_CONV_ERR,
            Err(err_code) => return err_code,
        };

        let user = match pamh.get_user(None) {
            Ok(u) => u,
            Err(err_code) => return err_code,
        };

        // Get current password
        let old_pass = match pam_try!(conv.send(PAM_PROMPT_ECHO_OFF, "Current Password: ")) {
            Some(pass) => match pass.to_str() {
                Ok(p) => SecureString::from(p),
                Err(_) => return PamResultCode::PAM_AUTHTOK_ERR,
            },
            None => return PamResultCode::PAM_AUTHTOK_ERR,
        };

        // Get new password
        let new_pass = match pam_try!(conv.send(PAM_PROMPT_ECHO_OFF, "New Password: ")) {
            Some(pass) => match pass.to_str() {
                Ok(p) => SecureString::from(p),
                Err(_) => return PamResultCode::PAM_AUTHTOK_ERR,
            },
            None => return PamResultCode::PAM_AUTHTOK_ERR,
        };

        // Verify new password
        let verify_pass = match pam_try!(conv.send(PAM_PROMPT_ECHO_OFF, "Verify Password: ")) {
            Some(pass) => match pass.to_str() {
                Ok(p) => SecureString::from(p),
                Err(_) => return PamResultCode::PAM_AUTHTOK_ERR,
            },
            None => return PamResultCode::PAM_AUTHTOK_ERR,
        };

        if new_pass != verify_pass {
            return PamResultCode::PAM_AUTHTOK_ERR;
        }

        match runtime.block_on(client.change_password(&user, old_pass, new_pass)) {
            Ok(_) => PamResultCode::PAM_SUCCESS,
            Err(_) => PamResultCode::PAM_AUTHTOK_ERR,
        }
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
