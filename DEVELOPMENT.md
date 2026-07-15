# Development Guide

This document covers the local development workflow for kendra, especially the difference between:

- a locally built binary from this repository
- a Homebrew-installed binary from `kendra-to/tap`

These can coexist on the same machine, and if your shell resolves the wrong one you can easily think you are testing one distribution path while actually running the other.

## Binary Sources

There are two common ways `kendra` ends up on your `PATH` during development.

### 1. Local development binary

Build from source:

```bash
cargo build --release -p kendra-cli
```

The built binary is:

```bash
target/release/kendra
```

In many local setups, this is then exposed via a symlink:

```bash
~/.local/bin/kendra -> /path/to/kendra/target/release/kendra
```

In this repository, the release binary is often symlinked into `~/.local/bin/kendra` for convenience during development.

### 2. Homebrew-installed binary

Install from the public tap:

```bash
brew install kendra-to/tap/kendra
```

On Apple Silicon macOS, Homebrew commonly installs:

```bash
/opt/homebrew/bin/kendra
```

That path is typically a symlink into the Homebrew Cellar, for example:

```bash
/opt/homebrew/bin/kendra -> ../Cellar/kendra/0.1.1/bin/kendra
```

## Which Binary Am I Running?

Always check before debugging installation issues.

```bash
which kendra
ls -l "$(which kendra)"
kendra --version
```

If your shell prints:

```bash
/Users/<you>/.local/bin/kendra
```

then you are running the local development binary, not the Homebrew one.

If it prints:

```bash
/opt/homebrew/bin/kendra
```

then you are running the Homebrew-installed binary.

## Why This Matters

A local symlink in `~/.local/bin` can take precedence over `/opt/homebrew/bin` depending on your `PATH` order.

That means all of the following can look correct while still testing the wrong binary:

- `brew install kendra-to/tap/kendra`
- `kendra --version`
- launching `kendra` from a fresh shell

If `~/.local/bin` appears earlier in `PATH`, your shell will keep using the local build.

## Inspect Your PATH

```bash
echo $PATH
print -l ${(s/:/)PATH}
```

Look for whether:

- `~/.local/bin`
- `/opt/homebrew/bin`

appears first.

## Local Development Workflow

### Build the binary

```bash
cargo build --release -p kendra-cli
```

### Point your shell at the local build

If you want the development binary to be your default:

```bash
ln -sf /Users/nghibui/codes/kendra/target/release/kendra ~/.local/bin/kendra
hash -r
which kendra
```

Adjust the repository path if your checkout lives elsewhere.

### Verify the local binary

```bash
which kendra
ls -l "$(which kendra)"
kendra --version
```

## Homebrew Workflow

### Install from the tap

```bash
brew tap kendra-to/tap
brew install kendra-to/tap/kendra
```

### Verify the Homebrew binary

```bash
which kendra
ls -l "$(which kendra)"
brew info kendra-to/tap/kendra
kendra --version
```

## Clean Homebrew Install Test

Use this flow when you want to test the public Homebrew installation from a clean state and avoid accidentally running the development binary.

Exact command sequence:

```bash
rm -f ~/.local/bin/kendra
hash -r
brew uninstall kendra
brew untap kendra-to/tap
brew tap kendra-to/tap
brew install kendra-to/tap/kendra
which kendra
kendra --version
```

### 1. Remove the local development symlink

```bash
rm -f ~/.local/bin/kendra
hash -r
```

### 2. Confirm the shell no longer resolves the local binary

```bash
which kendra
```

Expected outcomes:

- no `kendra` found yet
- or `/opt/homebrew/bin/kendra` if Homebrew still has it installed

### 3. Remove the existing Homebrew installation

```bash
brew uninstall kendra
```

### 4. Untap and retap the formula repository

```bash
brew untap kendra-to/tap
brew tap kendra-to/tap
```

Note: `brew untap kendra-to/tap` will fail if `kendra` is still installed. Uninstall first.

### 5. Install again from scratch

```bash
brew install kendra-to/tap/kendra
```

### 6. Verify what is being executed

```bash
which kendra
ls -l "$(which kendra)"
kendra --version
```

## Switching Back To The Local Development Binary

After a Homebrew test, restore the local symlink if you want your shell to prefer the repo build again.

Exact command sequence:

```bash
ln -sf /Users/nghibui/codes/kendra/target/release/kendra ~/.local/bin/kendra
hash -r
which kendra
```

That is required if you removed `~/.local/bin/kendra` for testing and want to use the local development binary again.

## Running The Repository Build/Test Workflow

Common commands:

```bash
cargo build --workspace
cargo test --workspace --lib --tests
cargo check --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all
```

Release binary build:

```bash
cargo build --release -p kendra-cli
```

Real smoke test:

```bash
echo "hello" | kendra -p "hello"
```

## Release And Homebrew Notes

The release process builds archives and installers, then publishes a Homebrew formula into:

```bash
kendra-to/homebrew-tap
```

The formula users install is:

```bash
Formula/kendra.rb
```

### Common failure mode: stale local tap state

If Homebrew reports an invalid ref while auto-updating the tap, reset the tap locally:

```bash
brew uninstall kendra
brew untap kendra-to/tap
brew tap kendra-to/tap
brew install kendra-to/tap/kendra
```

### Common failure mode: installed formula but wrong binary on PATH

If `brew install` succeeds but running `kendra` still behaves like your local checkout, check:

```bash
which kendra
ls -l "$(which kendra)"
```

If it points to `~/.local/bin/kendra`, remove or rename that symlink before testing Homebrew.

## Useful Debug Commands

Check which formula is installed:

```bash
brew list --versions kendra
```

Inspect Homebrew metadata:

```bash
brew info kendra-to/tap/kendra
```

Inspect the binary resolution:

```bash
which kendra
type -a kendra
```

Inspect symlinks:

```bash
ls -l ~/.local/bin/kendra
ls -l /opt/homebrew/bin/kendra
```

## Recommendation

When validating release packaging, do not trust `kendra --version` alone.

Always check all three:

```bash
which kendra
ls -l "$(which kendra)"
kendra --version
```

That avoids confusing:

- a repo build
- a stale shell hash entry
- a Homebrew install
- an old symlink on `PATH`

