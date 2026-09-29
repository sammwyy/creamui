# Platform builds and Android signing

CreamUI desktop applications are ordinary Rust binaries. Android applications
use `cargo-apk` to package a NativeActivity library into an APK.

## Windows desktop application

Build Windows applications on a Windows machine with Rust's MSVC toolchain.
Install [Rust with `rustup`](https://www.rust-lang.org/tools/install); it will
prompt for the Visual Studio C++ Build Tools when they are needed. The `Desktop
development with C++` workload and a Windows SDK provide the native linker and
libraries.

Create a normal Rust binary:

```powershell
cargo new --bin hello-creamui
Set-Location hello-creamui
```

Add CreamUI to `Cargo.toml`:

```toml
[dependencies]
creamui = { version = "0.1", features = ["jsx"] }
```

Use the code from the [getting started guide](getting-started.md) in
`src/main.rs`, then run or package it:

```powershell
cargo run
cargo build --release
```

The release executable is `target\release\hello-creamui.exe`. The default
CreamUI features support the native desktop window path, so no Windows-specific
feature is required for a regular application. Build on Windows instead of
cross-compiling from Linux; Rust does not support Linux-to-MSVC cross-compilation
as a standard workflow.

## Android debug APK

The Android demo requires a JDK, Android SDK command-line tools, platform
tools, Build Tools, an NDK, the Rust Android target, and `cargo-apk`. Set
`JAVA_HOME`, `ANDROID_HOME`, and `ANDROID_NDK_HOME`; also include `cargo-apk`
and Android platform tools in `PATH`. Android documents the command-line SDK
setup in its [SDK Manager guide](https://developer.android.com/tools/sdkmanager).

```sh
rustup target add aarch64-linux-android
cargo install cargo-apk
cargo apk build --package creamui-android-demo --target aarch64-linux-android
```

This creates a debug APK signed with the automatic debug keystore at
`target/debug/apk/main.apk`. It is suitable for local device testing, not
distribution.

## Android release APK

Android requires APKs to be signed. `cargo-apk` creates a debug keystore only
for debug builds; a release build deliberately fails until it is given a private
release keystore and password.

Create a keystore once, then back it up securely. The suggested `.keys/`
directory is ignored by Git in this repository.

```sh
mkdir -p .keys
keytool -genkeypair -v \
  -keystore .keys/creamui-release.p12 \
  -storetype PKCS12 \
  -alias creamui \
  -keyalg RSA -keysize 4096 -validity 10000
```

`keytool` prompts for the keystore password and certificate details. Do not put
the password in `Cargo.toml`, a shell profile, source code, or Git. The same
signing key must be retained for every update of an application package.

Pass the keystore to `cargo-apk` through temporary environment variables. On
Bash:

```sh
export CARGO_APK_RELEASE_KEYSTORE="$PWD/.keys/creamui-release.p12"
read -rs -p 'Keystore password: ' CARGO_APK_RELEASE_KEYSTORE_PASSWORD
echo
export CARGO_APK_RELEASE_KEYSTORE_PASSWORD
cargo apk build --release --package creamui-android-demo --target aarch64-linux-android
```

On PowerShell:

```powershell
$env:CARGO_APK_RELEASE_KEYSTORE = (Resolve-Path '.keys/creamui-release.p12').Path
$keystorePassword = Read-Host 'Keystore password' -AsSecureString
$credential = [System.Management.Automation.PSCredential]::new('unused', $keystorePassword)
$env:CARGO_APK_RELEASE_KEYSTORE_PASSWORD = $credential.GetNetworkCredential().Password
cargo apk build --release --package creamui-android-demo --target aarch64-linux-android
Remove-Variable keystorePassword, credential
```

The output is `target/release/apk/main.apk`. Clear the password from the current
PowerShell session when finished:

```powershell
Remove-Item Env:CARGO_APK_RELEASE_KEYSTORE_PASSWORD
```

`cargo-apk` also accepts a `[package.metadata.android.signing.release]` table,
which is why it reports that name in its error. Environment variables take
precedence and keep credentials out of the tracked manifest, so they are the
recommended approach for local development and CI secrets.

For Play distribution, use a separate upload key where appropriate and keep the
private key in secure backup or secret storage. See Android's
[app-signing guidance](https://developer.android.com/studio/publish/app-signing)
before publishing.
