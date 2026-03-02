#!/bin/bash
# Load test fixtures into HDFS
# Run inside hdfs-client container:
#   docker exec hfs-test-client /load-fixtures.sh
set -e

HDFS_NAMENODE="${HDFS_NAMENODE:-hdfs://namenode:8020}"

echo "==> Loading hfs test fixtures into HDFS"
echo "    Namenode: $HDFS_NAMENODE"

# Create directory structure
hdfs dfs -mkdir -p /test-data/parquet/
hdfs dfs -mkdir -p /test-data/avro/
hdfs dfs -mkdir -p /test-data/mixed/
hdfs dfs -mkdir -p /test-data/schema-drift/v1/
hdfs dfs -mkdir -p /test-data/schema-drift/v2/
hdfs dfs -mkdir -p /test-data/small-files/

# Upload Parquet fixtures
if [ -f /fixtures/small.parquet ]; then
    hdfs dfs -put -f /fixtures/small.parquet /test-data/parquet/small.parquet
    echo "  Uploaded: small.parquet"
fi

# Create many small files (for small-files test)
echo "  Creating small files..."
for i in $(seq 1 20); do
    echo "small file $i content for testing" | hdfs dfs -put - /test-data/small-files/file_$(printf "%03d" $i).txt
done

echo "==> Fixtures loaded. Directory listing:"
hdfs dfs -ls -R /test-data/
