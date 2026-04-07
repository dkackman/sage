#!/bin/bash
# Build and sign macOS app for passkey (WebAuthn PRF) development.
#
# PREREQUISITES:
# 1. Register App ID com.rigidnetwork.sage at:
#    https://developer.apple.com/account/resources/identifiers/list
#    Enable the "Associated Domains" capability.
#
# 2. Create a Mac Development provisioning profile at:
#    https://developer.apple.com/account/resources/profiles/list
#    - Type: macOS App Development
#    - App ID: com.rigidnetwork.sage
#    - Include your development certificate and Mac device
#    Download and place it at: sage/embedded.provisionprofile
#
# 3. Update src-tauri/Entitlements.plist and src/hooks/usePasskey.ts so that
#    the associated domain and RP_ID constant match your actual domain.
#
# Run this script from the sage root directory.

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

ENTITLEMENTS="src-tauri/Entitlements.plist"
PROVISIONING_PROFILE="embedded.provisionprofile"
BUNDLE_ID="net.kackman.webauthn.example"
APP_NAME="Sage"

# Check prerequisites
if [ ! -f "$PROVISIONING_PROFILE" ]; then
    echo "ERROR: Provisioning profile not found at $PROVISIONING_PROFILE"
    echo ""
    echo "Create a Mac Development profile at:"
    echo "  https://developer.apple.com/account/resources/profiles/list"
    echo "  - Select 'macOS App Development'"
    echo "  - App ID: $BUNDLE_ID"
    echo "  - Include your certificate and this Mac"
    echo "Download and save as: $SCRIPT_DIR/$PROVISIONING_PROFILE"
    exit 1
fi

if [ ! -f "$ENTITLEMENTS" ]; then
    echo "ERROR: Entitlements file not found at $ENTITLEMENTS"
    exit 1
fi

# Find signing identity
IDENTITY=$(security find-identity -v -p codesigning | grep "Apple Development" | head -1 | sed 's/.*"\(.*\)".*/\1/')
if [ -z "$IDENTITY" ]; then
    echo "ERROR: No Apple Development signing identity found."
    echo "Install a development certificate from https://developer.apple.com/account/resources/certificates/list"
    exit 1
fi

echo "=== Building Sage for macOS passkey development ==="
echo "Bundle ID:        $BUNDLE_ID"
echo "Signing Identity: $IDENTITY"
echo ""

# Point bindgen at the macOS SDK so it doesn't pick up the Android NDK clang.
# Without this, aws-lc-sys fails with "'stdlib.h' file not found".
export SDKROOT=$(xcrun --show-sdk-path)
export BINDGEN_EXTRA_CLANG_ARGS="-isysroot $SDKROOT"
echo "SDK Root: $SDKROOT"
echo ""

# Extract and compile translations before building
echo "Step 1: Extracting and compiling translations..."
pnpm extract && pnpm compile

# Build
echo "Step 2: Building app bundle (debug)..."
pnpm tauri build --debug --bundles app

BUNDLE_PATH="target/debug/bundle/macos/${APP_NAME}.app"
if [ ! -d "$BUNDLE_PATH" ]; then
    echo "ERROR: Bundle not found at $BUNDLE_PATH"
    exit 1
fi
echo "Bundle: $BUNDLE_PATH"

# Embed provisioning profile
echo ""
echo "Step 3: Embedding provisioning profile..."
cp "$PROVISIONING_PROFILE" "$BUNDLE_PATH/Contents/embedded.provisionprofile"

# Re-sign with entitlements
echo ""
echo "Step 4: Removing existing signature..."
codesign --remove-signature "$BUNDLE_PATH" 2>/dev/null || true

echo ""
echo "Step 5: Re-signing inner frameworks and dylibs..."
find "$BUNDLE_PATH/Contents/Frameworks" \( -name "*.dylib" -o -name "*.framework" -o -name "*.app" \) \
    | sort -r \
    | while read -r item; do
        codesign --force --sign "$IDENTITY" --timestamp "$item" 2>/dev/null || true
    done

echo ""
echo "Step 6: Signing bundle with passkey entitlements..."
codesign --force \
    --sign "$IDENTITY" \
    --entitlements "$ENTITLEMENTS" \
    --options runtime \
    --timestamp \
    "$BUNDLE_PATH"

# Verify
echo ""
echo "Step 7: Verifying..."
codesign -dv --verbose=4 "$BUNDLE_PATH" 2>&1 | head -20
echo ""
codesign -d --entitlements - "$BUNDLE_PATH" 2>&1

echo ""
echo "=== Done ==="
echo "Launch with:"
echo "  $SCRIPT_DIR/$BUNDLE_PATH/Contents/MacOS/sage-tauri"
echo ""
