#!/bin/bash
# Create Kerberos principals and keytabs via FreeIPA
# Run inside freeipa container AFTER wait-for-ipa.sh
#
# Usage:
#   docker exec hfs-kerb-ipa /scripts/setup-principals.sh
set -e

REALM="HFS.TEST"
KEYTAB_DIR="/keytabs"

echo "==> Setting up Kerberos principals for hfs tests"

# Authenticate as IPA admin
echo "Admin1234" | kinit admin@${REALM}

# Create service principals
ipa service-add "hdfs/namenode-kerb.hfs.test@${REALM}" --force || true
ipa service-add "hdfs/datanode-kerb.hfs.test@${REALM}" --force || true
ipa service-add "hfs/hdfs-client.hfs.test@${REALM}" --force || true

# Create user principal for integration tests
ipa user-add hfs-test --first="HFS" --last="TestUser" --password-expiration=20991231000000Z || true
echo -e "TestPass1234\nTestPass1234" | ipa passwd hfs-test || true

# Export keytabs
mkdir -p ${KEYTAB_DIR}
ipa-getkeytab -s ipa.hfs.test -p "hdfs/namenode-kerb.hfs.test@${REALM}" -k ${KEYTAB_DIR}/hdfs.keytab
ipa-getkeytab -s ipa.hfs.test -p "hfs/hdfs-client.hfs.test@${REALM}" -k ${KEYTAB_DIR}/hfs.keytab

echo "==> Principals and keytabs created in ${KEYTAB_DIR}/"
ls -la ${KEYTAB_DIR}/
