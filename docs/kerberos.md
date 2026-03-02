# Kerberos

!!! tip "Stub"
    Questa pagina verrà completata durante lo sprint Kerberos.

## Prerequisiti

- `hfs` compilato con `--features kerberos`
- `kinit` eseguito con un keytab valido prima di usare `hfs`
- Cluster HDFS configurato con `hadoop.security.authentication = kerberos`

## Autenticazione

```bash
# Ottieni ticket Kerberos
kinit -kt /path/to/your.keytab user@REALM.COM

# Verifica ticket
klist

# Usa hfs normalmente — rileva Kerberos da core-site.xml automaticamente
hfs ls /data/warehouse/
```

## WebHDFS con Kerberos (SPNEGO)

WebHDFS supporta autenticazione SPNEGO via HTTP Negotiate.
Funziona anche senza `--features kerberos` se `kinit` è già stato eseguito.

```bash
# WebHDFS SPNEGO — non richiede --features kerberos
hfs --backend webhdfs ls /data/
```
