# hfs — HDFS CLI without JVM

**hfs** is a command-line tool written in Rust to interact with HDFS clusters
without requiring a Java Virtual Machine or installed Hadoop client.

## Why hfs?

The typical HDFS debug workflow requires 4-6 separate JVM commands, each with
4-8 seconds of startup time. With `hfs`, all the necessary information arrives in less than 200ms.

## Quick start

```bash
# List files
hfs ls /data/warehouse/

# Parquet file schema (reads only ~8KB, no matter the file size)
hfs schema /data/warehouse/part-00001.parquet

# Cluster health
hfs health

# Schema drift vs Hive table
hfs drift /data/warehouse/ --against hive://default.transactions
```

See the [Commands](commands.md) section for full documentation.
