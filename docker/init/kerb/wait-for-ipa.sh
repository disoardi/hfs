#!/bin/bash
# Wait for FreeIPA to be fully initialized before proceeding
set -e

echo "Waiting for FreeIPA to be ready..."
MAX_RETRIES=30
RETRY_INTERVAL=15

for i in $(seq 1 $MAX_RETRIES); do
    if ipactl status 2>/dev/null | grep -q "ipa.service - RUNNING"; then
        echo "FreeIPA is ready!"
        exit 0
    fi
    echo "  Attempt $i/$MAX_RETRIES — not ready yet, waiting ${RETRY_INTERVAL}s..."
    sleep $RETRY_INTERVAL
done

echo "ERROR: FreeIPA did not start within $((MAX_RETRIES * RETRY_INTERVAL)) seconds"
exit 1
