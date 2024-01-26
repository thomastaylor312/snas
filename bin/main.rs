use std::io::IsTerminal;

use async_nats::jetstream::{kv::Config, stream::StorageType};
use clap::Parser;

use snas::storage::CredStore;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The NATS JetStream domain to connect to
    #[arg(short = 'd', env = "SNAS_JS_DOMAIN")]
    js_domain: Option<String>,

    /// The NATS server to connect to
    #[arg(short = 's', default_value = "127.0.0.1", env = "SNAS_NATS_SERVER")]
    nats_server: String,

    /// The NATS port to connect to
    #[arg(short = 'p', default_value_t = 4222, env = "SNAS_NATS_PORT")]
    nats_port: u16,

    /// The name of the KeyValue bucket to use for storage
    #[arg(short = 'b', default_value = "snas", env = "SNAS_KV_BUCKET")]
    kv_bucket: String,

    // TODO: NATS creds
    /// The admin username to use by default. If this admin user already exists, it will not be
    /// created again
    #[arg(long = "admin-user", default_value = "admin", env = "SNAS_ADMIN_USER")]
    admin_user: String,

    /// The admin password to use by default. If this admin user already exists, it will not
    /// overwrite the current admin password
    #[arg(
        long = "password",
        default_value = "admin",
        env = "SNAS_ADMIN_PASSWORD"
    )]
    admin_password: String,

    // TODO: TLS

    // TODO: Swap this out for an actual parsed IP address
    /// The address and port to listen on for HTTP connections
    #[arg(
        short = 'l',
        default_value = "0.0.0.0:8080",
        env = "SNAS_LISTEN_ADDRESS"
    )]
    listen_address: String,
    // TODO: Do we need some sort of domain/realm thing so we can support multiple groups of users
    // in the future?
    /// Use json formatted logs
    #[arg(short = 'j', long = "json", env = "SNAS_LOG_FORMAT")]
    json_logs: bool,

    /// Only start the NATS API, don't start the socket. Mutually exclusive with --socket-only
    #[arg(
        id = "nats_only",
        long = "nats-only",
        env = "SNAS_NATS_ONLY",
        conflicts_with = "socket_only"
    )]
    nats_only: bool,

    /// Only start the socket, don't start the NATS API. Mutually exclusive with --nats-only
    #[arg(
        id = "socket_only",
        long = "socket-only",
        env = "SNAS_SOCKET_ONLY",
        conflicts_with = "nats_only"
    )]
    socket_only: bool,

    /// The default groups to give to new users, given as a comma delimited list
    #[arg(
        long = "default-groups",
        use_value_delimiter = true,
        env = "SNAS_DEFAULT_GROUPS"
    )]
    default_groups: Option<Vec<String>>,
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

    let client = async_nats::connect(format!("{}:{}", args.nats_server, args.nats_port)).await?;
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

    Ok(())
}
