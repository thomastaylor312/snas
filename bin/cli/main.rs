use std::collections::BTreeSet;
use std::path::PathBuf;

use anyhow::Context;
use async_nats::ConnectOptions;
use clap::{Parser, Subcommand};
use snas_lib::clients::NatsClient;
use snas_lib::SecureString;

#[derive(Parser, Debug)]
#[command(author, version, about = "SNAS admin CLI", long_about = None)]
struct Cli {
    /// NATS server host
    #[arg(
        long = "nats-server",
        default_value = "127.0.0.1",
        env = "SNAS_NATS_SERVER"
    )]
    nats_server: String,

    /// NATS server port
    #[arg(long = "nats-port", default_value_t = 4222, env = "SNAS_NATS_PORT")]
    nats_port: u16,

    /// NATS credentials file (mutually exclusive with username/password)
    #[arg(long = "creds", env = "SNAS_NATS_CREDS", conflicts_with_all = ["nats_username", "nats_password"])]
    creds: Option<PathBuf>,

    /// NATS username (requires --nats-password)
    #[arg(
        long = "nats-username",
        env = "SNAS_NATS_USER",
        requires = "nats_password",
        conflicts_with = "creds"
    )]
    nats_username: Option<String>,

    /// NATS password (requires --nats-username)
    #[arg(
        long = "nats-password",
        env = "SNAS_NATS_PASSWORD",
        requires = "nats_username",
        conflicts_with = "creds"
    )]
    nats_password: Option<String>,

    /// Optional CA cert for NATS TLS
    #[arg(long = "nats-ca-cert", env = "SNAS_NATS_CA_CERT")]
    nats_ca_cert: Option<PathBuf>,

    /// Optional JetStream domain
    #[arg(long = "js-domain", env = "SNAS_JS_DOMAIN")]
    _js_domain: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Administrative actions
    Admin {
        #[command(subcommand)]
        command: AdminCmd,
    },
}

#[derive(Subcommand, Debug)]
enum AdminCmd {
    /// Add a user
    AddUser {
        /// Username
        #[arg(long)]
        username: String,
        /// Password
        #[arg(long)]
        password: String,
        /// Group memberships (can repeat)
        #[arg(long = "group")]
        groups: Vec<String>,
        /// Force password change on first login
        #[arg(long = "force-reset", default_value_t = false)]
        force_reset: bool,
        /// Optional user topic prefix for user/admin APIs
        #[arg(long = "user-topic-prefix")]
        user_topic_prefix: Option<String>,
        /// Optional admin topic prefix for admin APIs
        #[arg(long = "admin-topic-prefix")]
        admin_topic_prefix: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let nc = get_nats_client(
        format!("{}:{}", cli.nats_server, cli.nats_port),
        cli.creds,
        cli.nats_username,
        cli.nats_password,
        cli.nats_ca_cert,
    )
    .await?;

    match cli.command {
        Commands::Admin { command: admin } => match admin {
            AdminCmd::AddUser {
                username,
                password,
                groups,
                force_reset,
                user_topic_prefix,
                admin_topic_prefix,
            } => {
                use snas_lib::clients::AdminClient;
                let client =
                    NatsClient::new_with_prefix(nc, user_topic_prefix, admin_topic_prefix)?;
                let groups: BTreeSet<String> = groups.into_iter().collect();
                client
                    .add_user(&username, SecureString::from(password), groups, force_reset)
                    .await
                    .context("failed to add user")?;
                println!("User {} added", username);
            }
        },
    }

    Ok(())
}

async fn get_nats_client(
    nats_addr: String,
    creds: Option<PathBuf>,
    username: Option<String>,
    password: Option<String>,
    ca_cert: Option<PathBuf>,
) -> anyhow::Result<async_nats::Client> {
    let mut opts = ConnectOptions::new();
    if let Some(cert) = ca_cert {
        opts = opts.add_root_certificates(cert)
    }

    if let Some(creds_file) = creds {
        opts = opts
            .credentials_file(creds_file)
            .await
            .context("Unable to open credentials file")?;
    } else if let (Some(user), Some(pass)) = (username, password) {
        opts = opts.user_and_password(user, pass);
    }

    opts.connect(nats_addr)
        .await
        .context("Unable to connect to NATS")
}
