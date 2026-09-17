#!/bin/bash
# Signs a freshly linked binary with a stable local identity, then runs it.
#
# Cargo invokes this as the `runner` for macOS targets (see .cargo/config.toml), so it sits in the
# one place that always sees the binary after linking and before launch.
#
# Why it has to exist: a debug build is ad-hoc signed, and an ad-hoc signature is just a hash of the
# code. The Keychain ACL that "Always Allow" writes pins the approved program by that identity, so
# every rebuild looks like a different program asking for the SQLCipher key and the prompt comes
# back. Signing with a certificate instead gives every build the same authority, and the approval
# sticks. Release builds go through the bundler and are unaffected by this.
#
# Signing is best-effort on purpose: a machine without the certificate should still be able to build
# and run, it just keeps getting prompted. A broken signing step must never block development.
#
# The desktop binary and example binaries are signed; test binaries are not. Cargo routes every test
# binary through this runner too, and none of them ask the Keychain for anything, so signing all of
# them would only add seconds to each test run. An example that opens the database does need it, or
# it asks for the key again on every rebuild.
set -uo pipefail

IDENTITY="${OPENHISTORY_SIGNING_IDENTITY:-OpenHistory Dev Signing}"
binary="$1"
shift

if { [[ "$(basename "$binary")" == "openhistory-desktop" ]] || [[ "$binary" == */examples/* ]]; } \
    && [[ "$(uname -s)" == "Darwin" ]] \
    && command -v codesign >/dev/null 2>&1; then
    if ! codesign --force --sign "$IDENTITY" --preserve-metadata=entitlements "$binary" 2>/dev/null; then
        echo "warning: could not sign $binary as '$IDENTITY'; the Keychain will keep prompting" >&2
        echo "warning: create the certificate with scripts/create-signing-cert.sh" >&2
    fi
fi

exec "$binary" "$@"
