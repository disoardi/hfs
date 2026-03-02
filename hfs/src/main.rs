// hfs — HDFS CLI
// Leggi CLAUDE.md prima di modificare questo file.

use clap::{Parser, Subcommand};
use anyhow::Result;

#[derive(Parser)]
#[command(
    name = "hfs",
    about = "HDFS filesystem tool — no JVM required",
    version = "0.1.0",
    long_about = None,
)]
struct Cli {
    /// Namenode URI (es. hdfs://namenode:8020)
    /// Default: letto da core-site.xml o HFS_NAMENODE env var
    #[arg(long, global = true)]
    namenode: Option<String>,

    /// Forza backend: rpc | webhdfs | auto (default)
    #[arg(long, global = true, default_value = "auto")]
    backend: String,

    /// Output format: text | json | csv
    #[arg(long, global = true, default_value = "text")]
    output: String,

    /// Mostra quale backend è stato selezionato
    #[arg(long, global = true)]
    show_backend: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Lista file e directory
    Ls {
        path: String,
        #[arg(short = 'l')]
        long: bool,
        #[arg(short = 'R')]
        recursive: bool,
        #[arg(short = 'h')]
        human_readable: bool,
    },
    /// Statistiche di un file o directory
    Stat { path: String },
    /// Utilizzo disco
    Du {
        path: String,
        #[arg(short = 's')]
        summary: bool,
        #[arg(short = 'h')]
        human_readable: bool,
    },
    /// Cerca file
    Find {
        path: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        mtime: Option<String>,   // es. "-1" = ultime 24h, "+7" = più di 7 giorni fa
        #[arg(long)]
        size: Option<String>,    // es. "+100M", "-1G"
        #[arg(long = "type")]
        file_type: Option<String>,  // f | d
    },
    /// Informazioni sui blocchi di un file
    Blocks { path: String },
    /// File sotto-replicati
    Replicas { path: String },
    /// File con blocchi corrotti
    Corrupt {
        #[arg(default_value = "/")]
        path: String,
    },
    /// Scarica file da HDFS
    Get { hdfs_path: String, local_path: String },
    /// Carica file su HDFS
    Put { local_path: String, hdfs_path: String },
    /// Stampa file su stdout
    Cat { path: String },
    /// Stato di salute del cluster
    Health,
    /// Rileva directory con small files
    SmallFiles {
        path: String,
        #[arg(long, default_value = "134217728")]  // 128MB
        threshold: u64,
    },
    /// Schema di file Parquet/Avro/ORC
    Schema {
        path: String,
        /// Confronta con tabella Hive (es. hive://database.table)
        #[arg(long)]
        against: Option<String>,
    },
    /// Statistiche di colonna da footer Parquet
    Stats { path: String },
    /// Row count stimato dal footer (senza data scan)
    Rowcount { path: String },
    /// Ultime N file arrivate per mtime
    LastArrival {
        path: String,
        #[arg(short = 'n', default_value = "10")]
        count: usize,
    },
    /// Rileva schema drift tra file e tabella Hive
    Drift {
        path: String,
        #[arg(long)]
        against: String,   // es. "hive://default.transactions"
    },
    /// Mostra capability map del client (backend disponibili, Kerberos, ...)
    Capabilities,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // TODO: costruire HdfsClient da config + flag CLI
    // let config = HdfsConfig::load()?.merge_cli(&cli);
    // let client = HdfsClientBuilder::new(config).build().await?;

    match &cli.command {
        Commands::Ls { path, long, recursive, human_readable } => {
            println!("TODO: ls {} (long={}, recursive={}, human={})", path, long, recursive, human_readable);
        }
        Commands::Stat { path } => {
            println!("TODO: stat {}", path);
        }
        Commands::Du { path, summary, human_readable } => {
            println!("TODO: du {} (summary={}, human={})", path, summary, human_readable);
        }
        Commands::Schema { path, against } => {
            println!("TODO: schema {} (against={:?})", path, against);
        }
        Commands::Health => {
            println!("TODO: health check");
        }
        Commands::Capabilities => {
            println!("hfs v0.1.0-dev");
            println!("Backend: auto (RPC → WebHDFS fallback)");
            println!("Kerberos: {}", if cfg!(feature = "kerberos") { "enabled" } else { "disabled" });
        }
        _ => {
            println!("Command not yet implemented. See CLAUDE.md for sprint plan.");
        }
    }

    Ok(())
}
