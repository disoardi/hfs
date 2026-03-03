# Comandi

## Sintassi generale

```bash
hfs [OPZIONI GLOBALI] <COMANDO> [ARGOMENTI]
```

### Opzioni globali

| Opzione | Default | Descrizione |
|---------|---------|-------------|
| `--namenode <URI>` | da `core-site.xml` o `HFS_NAMENODE` | URI NameNode: `hdfs://host:8020` o `http://host:9870` |
| `--backend <rpc\|webhdfs\|auto>` | `auto` | Forza il backend di connessione |
| `--output <text\|json>` | `text` | Formato output |
| `--show-backend` | — | Stampa il backend selezionato su stderr |

---

## Comandi filesystem

### `ls` — Lista file e directory

```bash
hfs ls <path>
hfs ls --output json <path>
hfs ls --human-readable <path>
```

**Esempio:**
```
hfs ls /dati/warehouse/

┌──────┬──────┬────────┬────────────┬──────┬────────────┬────────────────────────────────┐
│ Perm │ Repl │ Owner  │ Group      │ Size │ Modified   │ Path                           │
╞══════╪══════╪════════╪════════════╪══════╪════════════╪════════════════════════════════╡
│ d755 │ -    │ hdfs   │ supergroup │ 0    │ 1706745600 │ /dati/warehouse/transactions   │
│ -644 │ 3    │ hdfs   │ supergroup │ 128M │ 1706832000 │ /dati/warehouse/sales.parquet  │
└──────┴──────┴────────┴────────────┴──────┴────────────┴────────────────────────────────┘
```

**Confronto con HDFS Java:**

| Scenario | `hdfs dfs -ls` | `hfs ls` |
|----------|---------------|----------|
| Directory da 2M file | OOM — lista in RAM | Stream riga per riga |
| Startup singolo comando | 4–8 secondi | ~50ms |

---

### `stat` — Statistiche di un file o directory

```bash
hfs stat <path>
hfs stat --output json <path>
```

**Esempio:**
```
hfs stat /dati/warehouse/sales.parquet

Path:          /dati/warehouse/sales.parquet
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

### `du` — Utilizzo disco

```bash
hfs du <path>
hfs du --human-readable <path>
hfs du --output json <path>
```

**Esempio:**
```
hfs du --human-readable /dati/warehouse/

Path:          /dati/warehouse/
Files:         1247
Directories:   83
Size (raw):    45.2 GB
Space used:    135.6 GB
```

!!! note
    "Space used" include le repliche (es. 3 repliche × size = 3× la dimensione raw).

---

### `health` — Salute del cluster

```bash
hfs health
hfs health --output json
```

**Esempio:**
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

---

## Comandi schema

!!! tip "Sprint Day 3"
    I comandi `schema`, `stats`, `rowcount` saranno implementati nel Day 3.

---

## Comandi cluster avanzati

!!! tip "Sprint Day 4-5"
    I comandi `blocks`, `replicas`, `small-files`, `drift` saranno implementati nel Day 4-5.

