# Comandi

!!! tip "Stub"
    Questa pagina verrà completata durante lo sviluppo.
    Ogni giorno di sprint aggiunge la documentazione dei comandi implementati.

## Sintassi generale

```bash
hfs [OPZIONI GLOBALI] <COMANDO> [ARGOMENTI]
```

### Opzioni globali

| Opzione | Default | Descrizione |
|---------|---------|-------------|
| `--namenode <URI>` | da `core-site.xml` | URI NameNode: `hdfs://host:8020` o `http://host:9870` |
| `--backend <rpc\|webhdfs\|auto>` | `auto` | Forza il backend di connessione |
| `--user <nome>` | utente corrente | Utente HDFS |
| `--json` | — | Output in formato JSON |
| `--show-backend` | — | Mostra il backend selezionato |
| `-v, --verbose` | — | Output dettagliato |

## Comandi filesystem

I comandi saranno documentati durante lo sprint di sviluppo.

## Comandi schema

I comandi saranno documentati durante lo sprint di sviluppo.

## Comandi cluster

I comandi saranno documentati durante lo sprint di sviluppo.
