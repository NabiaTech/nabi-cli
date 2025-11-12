# Code Signing Guide

## Quick Start

```bash
# Build and install with automatic signing
just install

# That's it! The binary is now properly signed with your Apple Development certificate
```

## How It Works

The Justfile automatically:
1. Builds the release binary
2. Signs it with your Apple Development certificate
3. Installs to `~/.local/share/nabi/bin/nabi`
4. Clears quarantine attributes
5. Verifies the signature

## Configuration

### Default Behavior
By default, the build system auto-detects your Apple Development certificate:
```bash
just install  # Auto-detects "Apple Development: Francis Troy Kirinhakone (6D3KU8XGDW)"
```

### Custom Signing Identity
Override the signing identity with an environment variable:

```bash
# Use a specific certificate
NABI_SIGNING_IDENTITY="Developer ID Application: Your Name" just install

# Use adhoc signing (for testing)
NABI_SIGNING_IDENTITY="adhoc" just install

# Or use "-" for adhoc
NABI_SIGNING_IDENTITY="-" just install
```

### Make it Permanent
Add to your `.envrc` or shell profile:
```bash
export NABI_SIGNING_IDENTITY="Developer ID Application: Your Name"
```

## Verification

Check the signature:
```bash
# Quick check
codesign -dv ~/.local/share/nabi/bin/nabi

# Detailed verification
codesign -dv --verbose=4 ~/.local/share/nabi/bin/nabi
```

Expected output with Apple Development certificate:
```
Authority=Apple Development: Francis Troy Kirinhakone (6D3KU8XGDW)
Authority=Apple Worldwide Developer Relations Certification Authority
Authority=Apple Root CA
TeamIdentifier=VCK9AX4BSA
```

## Troubleshooting

### Exit Code 137 (SIGKILL)
If you see exit code 137 when running `nabi`, the binary was killed by macOS security:

```bash
# Solution: rebuild and reinstall
just install
```

This happens when:
- Binary loses its signature (macOS updates, etc.)
- Quarantine attributes are present
- Certificate has expired

### No Certificate Found
If you see "No Apple Development certificate found":

1. Check your certificates:
   ```bash
   security find-identity -v -p codesigning
   ```

2. Install Xcode or Command Line Tools:
   ```bash
   xcode-select --install
   ```

3. Use adhoc signing for local development:
   ```bash
   NABI_SIGNING_IDENTITY="adhoc" just install
   ```

### Certificate Not in Keychain
If your certificate exists but isn't found:

1. Open Keychain Access
2. Ensure certificate is in "login" or "System" keychain
3. Verify certificate is valid and not expired

## Available Commands

```bash
just build        # Build only (no signing)
just sign         # Sign the cached binary
just install      # Build, sign, and install
just quick        # Same as install (alias)
just dev          # Install and watch for changes
```

## Why Code Signing Matters

### For macOS Security
- **Prevents SIGKILL**: macOS won't randomly kill signed binaries
- **Better trust**: System recognizes the developer
- **Smooth updates**: Signature persists through macOS updates

### For Distribution
- **Developer ID**: Required for distributing outside App Store
- **Notarization**: Can be notarized for macOS 10.15+
- **Trust chain**: Full certificate chain to Apple Root CA

## Advanced: Developer ID Distribution

For distributing to other users:

1. Get a Developer ID certificate (requires paid Apple Developer account)
2. Sign with Developer ID:
   ```bash
   NABI_SIGNING_IDENTITY="Developer ID Application: Your Name" just install
   ```

3. Notarize the binary:
   ```bash
   # Package as zip
   ditto -c -k --keepParent ~/.local/share/nabi/bin/nabi nabi.zip

   # Submit for notarization
   xcrun notarytool submit nabi.zip \
     --apple-id your-email@example.com \
     --team-id YOUR_TEAM_ID \
     --password YOUR_APP_SPECIFIC_PASSWORD

   # Staple the ticket
   xcrun stapler staple ~/.local/share/nabi/bin/nabi
   ```

## References

- [Apple Code Signing Guide](https://developer.apple.com/documentation/security/code_signing_services)
- [macOS Code Signing](https://developer.apple.com/support/code-signing/)
- [Notarizing macOS Software](https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution)
