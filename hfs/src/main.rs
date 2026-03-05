// hfs — HDFS CLI without JVM
// See CLAUDE.md before modifying this file.

use anyhow::Result;
use clap::{Parser, Subcommand};
use comfy_table::{presets::UTF8_FULL_CONDENSED, Table};
use hfs_core::{HdfsClient, HdfsClientBuilder, HdfsConfig};

#[derive(Parser)]
#[command(
    name = "hfs",
    about = "HDFS filesystem tool — no JVM required",
    version = "0.1.0",
    long_about = None,
)]
struct Cli {
    /// Namenode URI or WebHDFS URL (e.g. http://namenode:9870)
    /// Default: read from core-site.xml or HFS_NAMENODE env var
    #[arg(long, global = true)]
    namenode: Option<String>,

    /// Force backend: rpc | webhdfs | auto (default)
    #[arg(long, global = true, default_value = "auto")]
    backend: String,

    /// Output format: text | json
    #[arg(long, global = true, default_value = "text")]
    output: String,

    /// Print which backend was selected (to stderr)
    #[arg(long, global = true)]
    show_backend: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List files and directories
    Ls {
        path: String,
        #[arg(short = 'l')]
        long: bool,
        #[arg(short = 'R')]
        recursive: bool,
        #[arg(long)]
        human_readable: bool,
    },
    /// File or directory statistics
    Stat { path: String },
    /// Disk usage
    Du {
        path: String,
        #[arg(short = 's')]
        summary: bool,
        #[arg(long)]
        human_readable: bool,
    },
    /// Find files
    Find {
        path: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        mtime: Option<String>, // e.g. "-1" = last 24h, "+7" = older than 7 days
        #[arg(long)]
        size: Option<String>, // e.g. "+100M", "-1G"
        #[arg(long = "type")]
        file_type: Option<String>, // f | d
    },
    /// Block locations for a file
    Blocks { path: String },
    /// Under-replicated files
    Replicas { path: String },
    /// Files with corrupt blocks
    Corrupt {
        #[arg(default_value = "/")]
        path: String,
    },
    /// Download file from HDFS
    Get {
        hdfs_path: String,
        local_path: String,
    },
    /// Upload file to HDFS
    Put {
        local_path: String,
        hdfs_path: String,
    },
    /// Stream file to stdout
    Cat { path: String },
    /// Cluster health dashboard
    Health,
    /// Find directories with many small files
    SmallFiles {
        path: String,
        #[arg(long, default_value = "134217728")] // 128MB
        threshold: u64,
    },
    /// Schema of Parquet/Avro/ORC files
    Schema {
        path: String,
        /// Compare with Hive table (e.g. hive://database.table)
        #[arg(long)]
        against: Option<String>,
    },
    /// Column statistics from Parquet footer
    Stats { path: String },
    /// Row count from footer (no data scan)
    Rowcount { path: String },
    /// Last N arrived files by mtime
    LastArrival {
        path: String,
        #[arg(short = 'n', default_value = "10")]
        count: usize,
    },
    /// Detect schema drift between files and a Hive table
    Drift {
        path: String,
        #[arg(long)]
        against: String, // e.g. "hive://default.transactions"
    },
    /// Show client capability map (backends, Kerberos, ...)
    Capabilities,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Build config from env/file, then apply CLI overrides.
    let mut config = HdfsConfig::load()?;
    if let Some(ref nn) = cli.namenode {
        if nn.starts_with("http://") || nn.starts_with("https://") {
            // Explicit WebHDFS URL: http://host:9870 or https://host:9870
            config.webhdfs_url = Some(nn.clone());
        } else if nn.starts_with("hdfs://") {
            // Explicit RPC URI: hdfs://host:8020
            config.namenode_uri = nn.clone();
        } else {
            // Bare host:port — infer intent from port number.
            // 9870 (Hadoop 3.x WebHDFS) or 50070 (Hadoop 2.x/HDP WebHDFS) → HTTP.
            // 8020/8021 (HDFS RPC) or unknown port → set namenode_uri so auto can probe RPC first.
            let port = nn.split(':').last().and_then(|p| p.parse::<u16>().ok());
            match port {
                Some(9870) | Some(50070) => {
                    config.webhdfs_url = Some(format!("http://{}", nn));
                }
                _ => {
                    // RPC port (8020, 8021) or unrecognised — let auto-detect probe RPC first,
                    // then fall back to WebHDFS on the default port (9870).
                    config.namenode_uri = format!("hdfs://{}", nn);
                }
            }
        }
    }
    if cli.backend != "auto" {
        config.preferred_backend = cli.backend.clone();
    }

    // Auto-select backend: tries RPC first, falls back to WebHDFS.
    let client = HdfsClientBuilder::build(&config).await;

    if cli.show_backend {
        eprintln!("[backend: {}]", client.backend_name());
    }

    match &cli.command {
        Commands::Ls {
            path,
            long: _,
            recursive: _,
            human_readable,
        } => {
            cmd_ls(client.as_ref(), path, *human_readable, &cli.output).await?;
        }
        Commands::Stat { path } => {
            cmd_stat(client.as_ref(), path, &cli.output).await?;
        }
        Commands::Du {
            path,
            summary: _,
            human_readable,
        } => {
            cmd_du(client.as_ref(), path, *human_readable, &cli.output).await?;
        }
        Commands::Health => {
            cmd_health(client.as_ref(), &cli.output).await?;
        }
        Commands::Capabilities => {
            println!("hfs v0.1.0-dev");
            println!("Backend: {}", client.backend_name());
            println!("Kerberos: disabled (build with --features kerberos for RPC Kerberos)");
            println!("WebHDFS URL: {}", config.effective_webhdfs_url());
            println!("NameNode URI: {}", config.namenode_uri);
        }
        _ => {
            eprintln!("Command not yet implemented. Sprint plan: see CLAUDE.md");
            std::process::exit(1);
        }
    }

    Ok(())
}

// ─── Command implementations ──────────────────────────────────────────────────

async fn cmd_ls(
    client: &dyn HdfsClient,
    path: &str,
    human_readable: bool,
    output_format: &str,
) -> Result<()> {
    let entries = client.list(path).await?;

    if output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL_CONDENSED);
    table.set_header(["Perm", "Repl", "Owner", "Group", "Size", "Modified", "Path"]);

    for e in &entries {
        let size = if human_readable {
            format_size(e.length)
        } else {
            e.length.to_string()
        };
        let perm = format!("{}{}", if e.is_dir { 'd' } else { '-' }, &e.permission);
        let repl = if e.is_dir {
            "-".to_string()
        } else if e.replication > 0 {
            e.replication.to_string()
        } else {
            "-".to_string() // RPC backend: replication not exposed by hdfs-native v0.9
        };
        let mtime = format_mtime(e.modification_time);
        table.add_row([&perm, &repl, &e.owner, &e.group, &size, &mtime, &e.path]);
    }

    println!("{}", table);
    Ok(())
}

async fn cmd_stat(client: &dyn HdfsClient, path: &str, output_format: &str) -> Result<()> {
    let s = client.stat(path).await?;

    if output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&s)?);
        return Ok(());
    }

    println!("Path:          {}", s.path);
    println!(
        "Type:          {}",
        if s.is_dir { "directory" } else { "file" }
    );
    println!(
        "Size:          {} ({} bytes)",
        format_size(s.length),
        s.length
    );
    if s.replication > 0 {
        println!("Replication:   {}", s.replication);
    }
    if s.block_size > 0 {
        println!("Block size:    {}", format_size(s.block_size));
    }
    println!("Owner:         {}", s.owner);
    println!("Group:         {}", s.group);
    println!("Permission:    {}", s.permission);
    println!("Modified:      {}", format_mtime(s.modification_time));
    println!("Accessed:      {}", format_mtime(s.access_time));

    Ok(())
}

async fn cmd_du(
    client: &dyn HdfsClient,
    path: &str,
    human_readable: bool,
    output_format: &str,
) -> Result<()> {
    let cs = client.content_summary(path).await?;

    if output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&cs)?);
        return Ok(());
    }

    let space = if human_readable {
        format_size(cs.space_consumed)
    } else {
        cs.space_consumed.to_string()
    };
    let length = if human_readable {
        format_size(cs.length)
    } else {
        cs.length.to_string()
    };

    println!("Path:          {}", path);
    println!("Files:         {}", cs.file_count);
    println!("Directories:   {}", cs.directory_count);
    println!("Size (raw):    {}", length);
    println!("Space used:    {}", space);
    if cs.quota > 0 {
        println!("Quota:         {}", cs.quota);
    }
    if cs.space_quota > 0 {
        println!(
            "Space quota:   {}",
            if human_readable {
                format_size(cs.space_quota as u64)
            } else {
                cs.space_quota.to_string()
            }
        );
    }

    Ok(())
}

async fn cmd_health(client: &dyn HdfsClient, output_format: &str) -> Result<()> {
    let h = client.health().await?;

    if output_format == "json" {
        println!("{}", serde_json::to_string_pretty(&h)?);
        return Ok(());
    }

    let status = if h.corrupt_blocks > 0 || h.dead_datanodes > 0 {
        "DEGRADED"
    } else if h.under_replicated_blocks > 0 || h.stale_datanodes > 0 {
        "WARNING"
    } else {
        "OK"
    };

    println!("Cluster status:        {}", status);
    println!("Live datanodes:        {}", h.live_datanodes);
    println!("Dead datanodes:        {}", h.dead_datanodes);
    println!("Stale datanodes:       {}", h.stale_datanodes);
    println!("Under-replicated:      {}", h.under_replicated_blocks);
    println!("Corrupt blocks:        {}", h.corrupt_blocks);
    if h.capacity_total_bytes > 0 {
        println!(
            "Capacity (total):      {}",
            format_size(h.capacity_total_bytes)
        );
        println!(
            "Capacity (used):       {}",
            format_size(h.capacity_used_bytes)
        );
        println!(
            "Capacity (free):       {}",
            format_size(h.capacity_remaining_bytes)
        );
    }
    if let Some(ref ha) = h.namenode_ha_state {
        println!("HA state:              {}", ha);
    }

    Ok(())
}

// ─── Formatting helpers ───────────────────────────────────────────────────────

/// Format a byte count as human-readable (KB, MB, GB, TB).
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    const TB: u64 = 1024 * GB;

    if bytes >= TB {
        format!("{:.1} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Format a HDFS modification timestamp (milliseconds since epoch) as a UTC string.
fn format_mtime(ms: u64) -> String {
    use std::time::{Duration, UNIX_EPOCH};
    if ms == 0 {
        return "-".to_string();
    }
    let secs = ms / 1000;
    // chrono is not a dependency; use a manual calculation for YYYY-MM-DD HH:MM UTC.
    let dt = UNIX_EPOCH + Duration::from_secs(secs);
    // Fallback: print as Unix seconds until chrono is added.
    let _ = dt;
    format!("{}", secs)
}
