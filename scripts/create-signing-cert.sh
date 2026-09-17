#!/bin/bash
# Creates the local code signing certificate that scripts/sign-and-run.sh uses.
#
# Run this once per machine. It is idempotent: if the identity can already sign, it changes nothing.
#
# This certificate is self-signed and exists only so every local build carries the same signing
# authority. That is what makes the Keychain's "Always Allow" for the database key survive a rebuild,
# since the ACL pins the approved program by its signature and an ad-hoc signature changes every time
# the code does. It grants no distribution ability whatsoever: shipping to other machines needs a real
# Apple Developer ID, because Gatekeeper and notarization check who signed, not merely that someone did.
set -euo pipefail

IDENTITY="${OPENHISTORY_SIGNING_IDENTITY:-OpenHistory Dev Signing}"
KEYCHAIN="${HOME}/Library/Keychains/login.keychain-db"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "This certificate only matters on macOS; nothing to do." >&2
    exit 0
fi

# `security find-identity` reports self-signed certificates as invalid because nothing vouches for
# them, yet codesign accepts them. So the only honest test of "is this usable" is to use it.
probe="$(mktemp)"
cp /bin/echo "$probe"
if codesign --force --sign "$IDENTITY" "$probe" >/dev/null 2>&1; then
    rm -f "$probe"
    echo "'$IDENTITY' can already sign. Nothing to do."
    exit 0
fi
rm -f "$probe"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# LibreSSL ships with macOS and does not take -addext, so the extensions go through a config file.
# Code Signing usage is not decorative: codesign refuses a certificate without it.
cat > "$work/openssl.cnf" <<EOF
[ req ]
distinguished_name = dn
prompt             = no
[ dn ]
CN = ${IDENTITY}
[ codesign ]
basicConstraints     = critical,CA:FALSE
keyUsage             = critical,digitalSignature
extendedKeyUsage     = critical,codeSigning
subjectKeyIdentifier = hash
EOF

openssl req -x509 -newkey rsa:2048 -nodes -days 3650 \
    -keyout "$work/key.pem" -out "$work/cert.pem" \
    -config "$work/openssl.cnf" -extensions codesign >/dev/null 2>&1

openssl pkcs12 -export -inkey "$work/key.pem" -in "$work/cert.pem" \
    -out "$work/bundle.p12" -passout pass: -name "$IDENTITY" >/dev/null 2>&1

# -T lets codesign use the private key. Without it macOS asks for the login password on every build
# instead of every launch, which trades one prompt for another.
security import "$work/bundle.p12" -k "$KEYCHAIN" -P "" -T /usr/bin/codesign >/dev/null

echo "Created '$IDENTITY' in the login keychain."
echo
echo "The first build may still ask once for permission to use the signing key. Approving that with"
echo "\"Always Allow\" holds, because it is granted to /usr/bin/codesign, whose signature does not"
echo "change when your code does."
