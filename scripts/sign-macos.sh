#!/bin/sh
# Signs a macOS binary with the Developer ID and notarizes it, in place.
# Reads the certificate and the App Store Connect key from the environment;
# see docs/adr/0029-releases-come-from-release-please.md.
set -eu

binary=$1
identifier=$2
: "${MACOS_CERTIFICATE:?}" "${MACOS_CERTIFICATE_PASSWORD:?}" "${APPLE_TEAM_ID:?}"
: "${APPLE_API_KEY:?}" "${APPLE_API_KEY_ID:?}" "${APPLE_API_ISSUER:?}"

work=$(mktemp -d)
keychain=$work/signing.keychain-db
password=$(openssl rand -base64 24)
searched=$(security list-keychains -d user | tr -d '"')

cleanup() {
	# shellcheck disable=SC2086 # one keychain path per word
	security list-keychains -d user -s $searched
	security delete-keychain "$keychain" 2>/dev/null || true
	rm -rf "$work"
}
trap cleanup EXIT

security create-keychain -p "$password" "$keychain"
security set-keychain-settings -lut 3600 "$keychain"
security unlock-keychain -p "$password" "$keychain"
printf %s "$MACOS_CERTIFICATE" | base64 --decode >"$work/certificate.p12"
security import "$work/certificate.p12" -k "$keychain" \
	-P "$MACOS_CERTIFICATE_PASSWORD" -T /usr/bin/codesign >/dev/null
security set-key-partition-list -S apple-tool:,apple: -s -k "$password" \
	"$keychain" >/dev/null
# codesign builds the chain to Apple's root only from searched keychains.
# shellcheck disable=SC2086
security list-keychains -d user -s "$keychain" $searched

codesign --force --sign "Developer ID Application" --keychain "$keychain" \
	--identifier "$identifier" --options runtime --timestamp "$binary"
codesign --verify --strict --verbose=2 "$binary"
codesign --display --verbose=2 "$binary" 2>&1 | grep -qx "TeamIdentifier=$APPLE_TEAM_ID"

# notarytool takes a zip, not a tar.gz; a bare binary cannot be stapled,
# so Gatekeeper looks the ticket up online.
ditto -c -k "$binary" "$work/postal.zip"
printf %s "$APPLE_API_KEY" >"$work/key.p8"
result=$(xcrun notarytool submit "$work/postal.zip" --wait --timeout 30m \
	--key "$work/key.p8" --key-id "$APPLE_API_KEY_ID" --issuer "$APPLE_API_ISSUER" \
	--output-format json)
echo "$result"
id=$(printf %s "$result" | plutil -extract id raw -o - -)
status=$(printf %s "$result" | plutil -extract status raw -o - -)
if [ "$status" != Accepted ]; then
	xcrun notarytool log "$id" \
		--key "$work/key.p8" --key-id "$APPLE_API_KEY_ID" --issuer "$APPLE_API_ISSUER"
	exit 1
fi
