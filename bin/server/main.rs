use std::{io::IsTerminal, path::PathBuf};

use anyhow::Context;
use async_nats::{
    jetstream::{kv::Config, stream::StorageType},
    ConnectOptions,
};
use clap::Parser;
use futures::future::{pending, Either};
use tracing::error;

use snas_lib::{
    handlers::Handlers,
    servers::{
        nats::{admin::NatsAdminServer, user::NatsUserServer},
        socket::SocketUserServer,
    },
    storage::CredStore,
    DEFAULT_SOCKET_PATH,
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The NATS JetStream domain to connect to
    #[arg(short = 'd', env = "SNAS_JS_DOMAIN")]
    js_domain: Option<String>,

    /// The NATS server to connect to
    #[arg(
        short = 's',
        long = "nats-server",
        default_value = "127.0.0.1",
        env = "SNAS_NATS_SERVER"
    )]
    nats_server: String,

    /// The NATS port to connect to
    #[arg(
        short = 'p',
        long = "nats-port",
        default_value_t = 4222,
        env = "SNAS_NATS_PORT"
    )]
    nats_port: u16,

    /// The name of the KeyValue bucket to use for storage
    #[arg(
        short = 'b',
        long = "kv-bucket",
        default_value = "snas",
        env = "SNAS_KV_BUCKET"
    )]
    kv_bucket: String,

    /// The creds file to use for authenticating to NATS. This is the preferred option
    #[arg(id = "creds", short = 'c', long = "creds", env = "SNAS_NATS_CREDS", conflicts_with_all = ["username", "password"])]
    creds: Option<PathBuf>,

    /// The username to use to authenticate with NATS. Using this option also requires the
    /// `--password` flag and is mutually exclusive with the `--creds` flag
    #[arg(
        id = "username",
        long = "username",
        env = "SNAS_NATS_USER",
        requires = "password",
        conflicts_with = "creds"
    )]
    nats_username: Option<String>,

    /// The password to use to authenticate with NATS. Using this option also requires the
    /// `--username` flag and is mutually exclusive with `--creds`
    #[arg(
        id = "password",
        long = "password",
        env = "SNAS_NATS_PASSWORD",
        requires = "username",
        conflicts_with = "creds"
    )]
    nats_password: Option<String>,

    /// A path to a Certificate Authority certificate for cases when you are using an internal
    /// signing authority
    #[arg(long = "ca-cert", env = "SNAS_NATS_CA_CERT")]
    nats_ca_cert: Option<PathBuf>,

    // TODO: Do we need some sort of domain/realm thing so we can support multiple groups of users
    // in the future?
    /// Use json formatted logs
    #[arg(short = 'j', long = "json", env = "SNAS_LOG_FORMAT")]
    json_logs: bool,

    /// Listen on the admin NATS API topics. By default this is off as listening to this on a host
    /// with a leaf node could allow anonymous access to the admin API
    #[arg(
        id = "admin_nats",
        long = "admin-nats",
        env = "SNAS_ADMIN_NATS",
        default_value_t = false
    )]
    admin_nats: bool,

    /// An optional topic prefix to use for the admin NATS API. If this is not provided, the default
    /// `snas.admin` will be used. Requires the `--admin-nats` flag to be set
    #[arg(
        id = "admin_nats_topic_prefix",
        long = "admin-nats-topic-prefix",
        env = "SNAS_ADMIN_NATS_TOPIC_PREFIX",
        requires = "admin_nats"
    )]
    admin_nats_topic_prefix: Option<String>,

    /// Listen on the user NATS API topics. By default this is off as listening to this on a host
    /// with a leaf node could allow anonymous access to the user API
    #[arg(
        id = "user_nats",
        long = "user-nats",
        env = "SNAS_USER_NATS",
        default_value_t = false
    )]
    user_nats: bool,

    /// An optional topic prefix to use for the user NATS API. If this is not provided, the default
    /// `snas.user` will be used. Requires the `--user-nats` flag to be set
    #[arg(
        id = "user_nats_topic_prefix",
        long = "user-nats-topic-prefix",
        env = "SNAS_USER_NATS_TOPIC_PREFIX",
        requires = "user_nats"
    )]
    user_nats_topic_prefix: Option<String>,

    /// Whether or not to enable the user socket. This is required if the admin and user NATS
    /// servers are not enabled
    #[cfg(unix)]
    #[arg(
        long = "user-socket",
        env = "SNAS_USER_SOCKET",
        default_value_t = false,
        required_unless_present_any = ["admin_nats", "user_nats"],
    )]
    user_socket: bool,
    /// The path to the socket file to use for the user API. This should exist in a directory that
    /// is only accessible to root or other super admins so as to not be abused
    // TODO(thomastaylor312): Use named pipes on Windows instead as UDS support isn't in the
    // standard library or Tokio yet (and it might take a bit)
    #[cfg(unix)]
    #[arg(
        long = "socket-file",
        env = "SNAS_SOCKET_FILE",
        default_value = DEFAULT_SOCKET_PATH,
    )]
    socket_file: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let builder = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal());
    if args.json_logs {
        builder.json().init();
    } else {
        builder.pretty().init();
    }

    let client = get_nats_client(
        format!("{}:{}", args.nats_server, args.nats_port),
        args.creds,
        args.nats_username,
        args.nats_password,
        args.nats_ca_cert,
    )
    .await?;
    tracing::info!("Successfully connected to NATS server");
    let js = if let Some(domain) = args.js_domain {
        async_nats::jetstream::with_domain(client.clone(), domain)
    } else {
        async_nats::jetstream::new(client.clone())
    };
    let bucket = match js.get_key_value(&args.kv_bucket).await {
        Ok(b) => b,
        // There isn't an error that says whether or not the bucket exists, so we have to just
        // assume the error means it doesn't exist. Just to be sure we use create rather than get or
        // create so we don't swallow any connection errors
        Err(e) => {
            tracing::warn!(err = %e, "KV bucket doesn't exist, creating it. It is highly recommended that you create your own bucket with proper replication settings for use in production");
            js.create_key_value(Config {
                bucket: args.kv_bucket,
                description: "Bucket for storing SNAS data".to_string(),
                history: 4,
                storage: StorageType::File,
                ..Default::default()
            })
            .await?
        }
    };
    tracing::info!("Successfully connected to bucket");
    let store = CredStore::new(bucket).await?;

    let handlers = Handlers::new(store);

    let nats_user_server = if args.user_nats {
        Either::Left(
            NatsUserServer::new(
                handlers.clone(),
                client.clone(),
                args.user_nats_topic_prefix,
            )
            .await?
            .run(),
        )
    } else {
        Either::Right(pending::<anyhow::Result<()>>())
    };

    let nats_admin_server = if args.admin_nats {
        Either::Left(
            NatsAdminServer::new(
                handlers.clone(),
                client.clone(),
                args.admin_nats_topic_prefix,
            )
            .await?
            .run(),
        )
    } else {
        Either::Right(pending::<anyhow::Result<()>>())
    };

    let socket_server = if args.user_socket {
        Either::Left(
            SocketUserServer::new(handlers.clone(), args.socket_file)
                .await?
                .run(),
        )
    } else {
        Either::Right(pending::<anyhow::Result<()>>())
    };

    if let Err(err) = futures::try_join!(nats_user_server, nats_admin_server, socket_server) {
        error!(%err, "An error occurred, shutting down");
        return Err(err);
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

    // We don't need to check if multiple creds are set as the CLI validates that for us
    if let Some(creds_file) = creds {
        opts = opts
            .credentials_file(creds_file)
            .await
            .context("Unable to open credentials file")?;
    } else if let (Some(user), Some(pass)) = (username, password) {
        // Same thing here around validating that both username and password are set. No need to
        // error as the CLI checks that both are set
        opts = opts.user_and_password(user, pass);
    }

    opts.connect(nats_addr)
        .await
        .context("Unable to connect to NATS")
}
