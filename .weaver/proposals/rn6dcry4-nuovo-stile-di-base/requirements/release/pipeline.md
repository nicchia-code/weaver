---
description: "Use when changing CI workflows, packaging, release artifacts, update signing or the app's update check."
---
# Build and release

## General

### Repository

- The code lives in the GitHub repository `nicchia-code/weaver`. Work lands on `dev`; `main` holds what is released. Both are protected: changes to `main` come from `dev` through a pull request whose `ci` checks passed.
- The version lives in `src-tauri/tauri.conf.json` (`version`). `src-tauri/Cargo.toml`, `crates/core/Cargo.toml`, `crates/cli/Cargo.toml` and `ui/package.json` carry the same version; `ci` fails when they differ.
- Toolchains are pinned: Rust in `rust-toolchain.toml` (exact version, with `rustfmt` and `clippy`), Node in `.nvmrc` (exact version) and `ui/package.json` `engines`, pnpm through `packageManager`. Dependencies install from the committed lockfiles without updating them (`cargo --locked`, `pnpm install --frozen-lockfile`).

### App icon

- Weaver has one app icon, its logo (`requirements/ui#window`, **Logo**): a cream (`#FBF6EE`) rounded square crossed by a horizontal Sky thread and by four diagonal threads, Peach, Lavender, Rose and Sage, that weave a "W". It is the icon of the bundles on every platform, of the macOS Dock, of the Windows and Linux taskbar, of the tray icon (`requirements/tech#window`), of the system notifications (`requirements/ui#notification`) and of the drag-out preview of assets (`requirements/tech#asset-view`). The interface shows the same drawing.
- Its master is the vector drawing `src-tauri/icons/source.svg`, a square canvas in which the rounded square takes 896/1024 of each side, centred, and everything outside it is fully transparent, with no white left around the shape. The interface uses the same SVG, never a raster copy; there is no other master and `source.png` is no longer kept.
- Every file listed in `bundle.icon` of `src-tauri/tauri.conf.json` (`32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.icns`, `icon.ico`) and `src-tauri/icons/icon.png` is generated from the master by plain downscaling (`tauri icon`), with no separate drawing for small sizes. Outputs the app does not use (Windows Store, Android, iOS) are not kept in the repository.

### Workflow `ci`

- File `.github/workflows/ci.yml`. Triggers: push to `dev`, pull requests to `dev`, push to `main`; never pull requests to `main`. A pull request from `dev` to `main` gets its checks from the run of the push to `dev` on the same commit, and the run of the push to `main` is the one that tests the merge commit before `release`.
- Matrix `fail-fast: false`, every entry required: `macos-latest` (arm64), `windows-latest` (x64), `ubuntu-22.04` (x64). Windows is x64 only. No job of `ci` has `continue-on-error`: a failure on any system fails the run, on `dev` as on `main`, also while only some systems are released.
- On Linux the Tauri 2 system packages are installed first (`libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `patchelf`).
- Steps per platform: install, build the frontend, `cargo test --workspace --locked --no-fail-fast`, Vitest, build the app in release mode with the bundled `weaver-cli`. `--no-fail-fast` runs the tests of every crate even when an earlier crate fails.
- On Windows every executable of the `weaver` package, the app and its test binaries, embeds the same application manifest, kept in `src-tauri` and passed to the MSVC linker by `src-tauri/build.rs` (`/MANIFEST:EMBED`, `/MANIFESTINPUT`) only when the target is Windows MSVC. Its content is tauri-build's default: the dependency on `Microsoft.Windows.Common-Controls` 6.0.0.0. tauri-build's own manifest is turned off (`WindowsAttributes::new_without_app_manifest()`), so the app carries one manifest only.
- Smoke step on every platform, with the `weaver-cli` binary just built and a fresh temporary folder as `--root`: `weaver-cli --help`, `weaver-cli project new --input '{}'`, `weaver-cli model check`; each must exit 0.
- End-to-end tests with tauri-driver run on Linux and Windows only (macOS has no WebDriver for its webview):
  - Linux: `webkit2gtk-driver` (WebKitWebDriver) under `xvfb-run`;
  - Windows: `msedgedriver` matching the runner's installed Edge version;
  - against the release build of the app, never a dev server.
- Job timeouts: 40 minutes for build and test jobs, the same on every system, 15 minutes for every other job.
- Caches (Cargo registry and `target`, the JS package store) are keyed by OS, toolchain version and lockfile hash.

### Workflow `release`

- File `.github/workflows/release.yml`. It runs on `workflow_run` of `ci` completed on branch `main` with conclusion `success`, and works on that run's `head_sha`; it never runs for `dev` or pull requests and never builds a commit whose tests failed.
- First job (15 min): reads `version` from `src-tauri/tauri.conf.json` and checks `refs/tags/v<version>` on `origin`. When the tag exists, the workflow ends successfully after a `::notice::` saying the release of `v<version>` is skipped because the tag exists; nothing is built.
- Otherwise it creates a draft GitHub Release named `Weaver v<version>` for tag `v<version>` targeting `head_sha` (a draft does not create the tag).
- Build jobs (25 min each) use `tauri-apps/tauri-action` and upload to that draft; file names are the ones Tauri produces for product name `Weaver`:

  | Runner | Target | Artifacts |
  |---|---|---|
  | `macos-latest` | `universal-apple-darwin` | `.dmg`; updater `.app.tar.gz` + `.sig` |
  | `windows-latest` | `x86_64-pc-windows-msvc` | NSIS `-setup.exe` + `.sig` (no MSI) |
  | `ubuntu-22.04` | `x86_64-unknown-linux-gnu` | `.AppImage` + `.sig`, `.deb` |

- The `weaver-cli` binary is built for the same target as the app and bundled as a sidecar under that name, never as `weaver`, the name of the app's own executable; on macOS the arm64 and x64 builds are joined with `lipo` into one universal binary.
- The updater manifest `latest.json` (platforms `darwin-aarch64`, `darwin-x86_64`, `windows-x86_64`, `linux-x86_64`) is attached to the release.
- A final job publishes the draft, which creates tag `v<version>` on `head_sha`, only when every build job succeeded. When any build job failed or was cancelled, the draft is deleted and no tag is created.
- The workflow never moves, deletes or recreates a tag and never edits a published release.
- Release notes list, per platform, the exact file name to download and the steps to open an unsigned build.

### Signing

- Updater artifacts are signed with the Tauri updater key: secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; the public key is in `tauri.conf.json` (`plugins.updater.pubkey`); `bundle.createUpdaterArtifacts` is `true`. The private key is also kept outside GitHub, since losing it stops updates of every installed copy.
- OS code signing and notarisation are not done yet: macOS builds are not signed or notarised, Windows builds have no Authenticode signature. They come with a later Proposal.
- The README explains how to open unsigned builds: macOS, right-click **Open** or System Settings → Privacy & Security → **Open Anyway**; Windows SmartScreen, **More info** → **Run anyway**; Linux, `chmod +x` the AppImage.

### Updates of Weaver

- Weaver uses the Tauri updater plugin with endpoint `https://github.com/nicchia-code/weaver/releases/latest/download/latest.json`.
- It checks at start and then at most once every 24 hours while running; the time of the last check is stored in the settings. A failed check is logged and retried at the next due time, never shown as an error.
- A found update shows its version and notes and is installed only when the developer chooses **Install and restart**; **Later** hides it until the next check. Install and restart goes through the same path as quitting Weaver, including its confirmation when jobs are working.
- An update whose signature does not verify is refused and logged.
