# hfs

HDFS filesystem tool — no JVM required.

```bash
hfs ls /user/hive/warehouse
hfs schema /data/transactions/part-00001.parquet
hfs drift /data/transactions/ --against hive://default.transactions
hfs health
```

> Work in progress — v0.1.0-dev

See [CLAUDE.md](CLAUDE.md) for architecture and [DEV_FLOW.md](DEV_FLOW.md) for development workflow.
