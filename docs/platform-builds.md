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

## Android interaction check

Build for an x86_64 emulator with the matching Rust target:

```sh
rustup target add x86_64-linux-android
cargo apk build -p creamui-android-demo --target x86_64-linux-android
adb install --no-incremental -r target/debug/apk/main.apk
adb shell am start -n dev.creamui.demo/android.app.NativeActivity
adb logcat -s creamui
```

On an API 36 emulator with Vulkan enabled, open Menu → Pickers → Choose a file.
The prompt should focus its path input and remain above the keyboard. Check
that an empty path and a missing `.png` path show errors, Cancel preserves the
selection, and Android Back hides the keyboard without reopening it. The
content viewport should return to its full height after keyboard dismissal.
Use Ctrl+A to replace the path and Ctrl+C/Ctrl+V to check clipboard shortcuts.

For a readable file owned by the debug application:

```sh
adb shell run-as dev.creamui.demo mkdir -p files
adb shell run-as dev.creamui.demo touch files/asset.png
```

Enter `/data/user/0/dev.creamui.demo/files/asset.png` and press Enter or Open.
The prompt should close and the showcase should display that path. The
extension filter checks the filename, not image decoding. Repeat opening and
cancellation, and use Tab to verify focus stays within the prompt. Logcat
reports prompt opening, validation, acceptance, and dismissal. Use an explicit
`adb -s` device selector when more than one device is connected.

## Android presentation and lifecycle checks

The [Android lifecycle fixture](../crates/render/tests/android/README.md)
checks CPU and GPU output, partial frame updates, repeated suspension,
orientation changes, preserved application state, input after resume, and
rejection of additional windows. It runs independently of the interactive
showcase and includes reproducible APK and pixel-check commands.

Both renderers drop their native surfaces on suspension. Resume keeps the
reactive tree and window handle, recreates the presenter, refreshes the scale
and viewport, and forces a full frame. Android provides one activity surface:
additional top-level window and native popup requests log a warning without
calling their ready callbacks. Use hosted overlays for secondary views.
Closing the activity window exits by default. With `AppBuilder::keep_running()`,
a later window request can create a replacement after the old window closes.

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
