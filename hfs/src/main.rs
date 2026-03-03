// hfs — HDFS CLI without JVM
// See CLAUDE.md before modifying this file.

use anyhow::Result;
use clap::{Parser, Subcommand};
use comfy_table::{presets::UTF8_FULL_CONDENSED, Table};
use hfs_core::{HdfsClient, HdfsConfig, WebHdfsClient};

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

    // Build config from env/file, then apply CLI override
    let mut config = HdfsConfig::load()?;
    if let Some(ref nn) = cli.namenode {
        if nn.starts_with("http://") || nn.starts_with("https://") {
            config.webhdfs_url = Some(nn.clone());
        } else {
            config.namenode_uri = nn.clone();
        }
    }
    if cli.backend != "auto" {
        config.preferred_backend = cli.backend.clone();
    }

    // For now, always use WebHDFS. Builder with auto-detection comes in Day 2.
    let webhdfs_url = config.effective_webhdfs_url();
    let client = WebHdfsClient::new(&webhdfs_url);

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
            cmd_ls(&client, path, *human_readable, &cli.output).await?;
        }
        Commands::Stat { path } => {
            cmd_stat(&client, path, &cli.output).await?;
        }
        Commands::Du {
            path,
            summary: _,
            human_readable,
        } => {
            cmd_du(&client, path, *human_readable, &cli.output).await?;
        }
        Commands::Health => {
            cmd_health(&client, &cli.output).await?;
        }
        Commands::Capabilities => {
            println!("hfs v0.1.0-dev");
            println!("Backend: {}", client.backend_name());
            // Kerberos feature is in hfs-core, not the binary crate
            println!("Kerberos: disabled (build with --features kerberos for RPC Kerberos)");
            println!("WebHDFS URL: {}", webhdfs_url);
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
    client: &WebHdfsClient,
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
        } else {
            e.replication.to_string()
        };
        let mtime = format_mtime(e.modification_time);
        table.add_row([&perm, &repl, &e.owner, &e.group, &size, &mtime, &e.path]);
    }

    println!("{}", table);
    Ok(())
}

async fn cmd_stat(client: &WebHdfsClient, path: &str, output_format: &str) -> Result<()> {
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
    println!("Replication:   {}", s.replication);
    println!("Block size:    {}", format_size(s.block_size));
    println!("Owner:         {}", s.owner);
    println!("Group:         {}", s.group);
    println!("Permission:    {}", s.permission);
    println!("Modified:      {}", format_mtime(s.modification_time));
    println!("Accessed:      {}", format_mtime(s.access_time));

    Ok(())
}

async fn cmd_du(
    client: &WebHdfsClient,
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

async fn cmd_health(client: &WebHdfsClient, output_format: &str) -> Result<()> {
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

/// Format a HDFS modification timestamp (milliseconds since epoch) as UTC string.
fn format_mtime(ms: u64) -> String {
    if ms == 0 {
        return "-".to_string();
    }
    let secs = ms / 1000;
    // Simple ISO-ish format: YYYY-MM-DD HH:MM
    // Use std::time to avoid date library dependency
    use std::time::{Duration, UNIX_EPOCH};
    let dt = UNIX_EPOCH + Duration::from_secs(secs);
    // Convert to a simple timestamp — for full date formatting we'd use chrono
    // For now, print the Unix timestamp as seconds
    let _ = dt;
    format!("{}", secs)
}
