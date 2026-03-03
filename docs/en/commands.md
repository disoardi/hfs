# Commands

## General syntax

```bash
hfs [GLOBAL OPTIONS] <COMMAND> [ARGUMENTS]
```

### Global options

| Option | Default | Description |
|--------|---------|-------------|
| `--namenode <URI>` | from `core-site.xml` or `HFS_NAMENODE` | NameNode URI: `hdfs://host:8020` or `http://host:9870` |
| `--backend <rpc\|webhdfs\|auto>` | `auto` | Force connection backend |
| `--output <text\|json>` | `text` | Output format |
| `--show-backend` | — | Print selected backend to stderr |

---

## Filesystem commands

### `ls` — List files and directories

```bash
hfs ls <path>
hfs ls --output json <path>
hfs ls --human-readable <path>
```

**Example:**
```
hfs ls /data/warehouse/

┌──────┬──────┬────────┬────────────┬──────┬────────────┬────────────────────────────────┐
│ Perm │ Repl │ Owner  │ Group      │ Size │ Modified   │ Path                           │
╞══════╪══════╪════════╪════════════╪══════╪════════════╪════════════════════════════════╡
│ d755 │ -    │ hdfs   │ supergroup │ 0    │ 1706745600 │ /data/warehouse/transactions   │
│ -644 │ 3    │ hdfs   │ supergroup │ 128M │ 1706832000 │ /data/warehouse/sales.parquet  │
└──────┴──────┴────────┴────────────┴──────┴────────────┴────────────────────────────────┘
```

**Comparison with Java HDFS client:**

| Scenario | `hdfs dfs -ls` | `hfs ls` |
|----------|---------------|----------|
| Directory with 2M files | OOM — full list in RAM | Stream row by row |
| Startup for single command | 4–8 seconds | ~50ms |

---

### `stat` — File or directory statistics

```bash
hfs stat <path>
hfs stat --output json <path>
```

**Example:**
```
hfs stat /data/warehouse/sales.parquet

Path:          /data/warehouse/sales.parquet
Type:          file
Size:          128.0 MB (134217728 bytes)
Replication:   3
Block size:    128.0 MB
Owner:         hdfs
Group:         supergroup
Permission:    644
Modified:      1706832000
Accessed:      1706832001
```

---

### `du` — Disk usage

```bash
hfs du <path>
hfs du --human-readable <path>
hfs du --output json <path>
```

**Example:**
```
hfs du --human-readable /data/warehouse/

Path:          /data/warehouse/
Files:         1247
Directories:   83
Size (raw):    45.2 GB
Space used:    135.6 GB
```

!!! note
    "Space used" includes replicas (e.g. 3 replicas × size = 3× the raw size).

---

### `health` — Cluster health dashboard

```bash
hfs health
hfs health --output json
```

**Example:**
```
hfs health

Cluster status:        OK
Live datanodes:        2
Dead datanodes:        0
Stale datanodes:       0
Under-replicated:      0
Corrupt blocks:        0
Capacity (total):      1.1 TB
Capacity (used):       352.0 KB
Capacity (free):       909.9 GB
```

The `health` command reads cluster metrics from the NameNode JMX endpoint — no data scan required.

**Status levels:**

| Status | Condition |
|--------|-----------|
| `OK` | No dead nodes, no corrupt blocks, no under-replicated blocks |
| `WARNING` | Under-replicated blocks or stale nodes present |
| `DEGRADED` | Dead nodes or corrupt blocks detected |

---

## Schema commands

!!! tip "Sprint Day 3"
    The `schema`, `stats`, and `rowcount` commands will be implemented on Day 3.

---

## Advanced cluster commands

!!! tip "Sprint Day 4-5"
    The `blocks`, `replicas`, `small-files`, and `drift` commands will be implemented on Days 4-5.
