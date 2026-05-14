# Ente Monorepo - Analysis & Improvement Opportunities

## 1. Repository Overview

**Project**: Fully open-source, end-to-end encrypted cloud platform
**Products**: Ente Photos, Ente Auth (2FA), Ente Locker
**Total open issues**: 514
**Stack**: Go server (Gin), Flutter mobile, Next.js/React web, Electron desktop, Rust crypto, Go CLI

## 2. Issue Triage Analysis

### 2.1 Label Usage (Inadequate)
The repo has labels but they are **severely underused**:
- No `bug`, `enhancement`, or `feature` labels applied to any open issues
- Only 514 issues with consistent platform labels (`--mobile`, `--desktop`, `--web`, `--server`, `--cli`)
- No severity labels (`critical`, `major`, `minor`)
- No priority labels (`p0`, `p1`, `p2`)
- No `wontfix`, `not-planned`, `duplicate`, or `invalid` labels applied anywhere
- **Action**: Implement proper label taxonomy: bug/feature/enhancement/question, priority tiers, severity tiers, and actively label issues

### 2.2 Issue Distribution by Platform

| Platform | Count (approx) |
|----------|---------------|
| Desktop | ~150 |
| Mobile | ~120 |
| Web | ~25 |
| CLI | ~10 |
| Server | ~8 |
| Self-hosting | ~15 |

## 3. Recurring Bug Categories (Problems to Fix)

### 3.1 Desktop App Stability (Critical)
**Most impactful problems**:
- **Linux AppImage/Deb/Flatpak launch failures**: #1509, #4100, #4752, #6705, #6976, #6600, #9738 — recurring white screen / crash on startup across distros
- **Windows black screen / blank window**: #6029, #4547, #4587 — app opens but renders nothing
- **AppImage sandbox issues**: #4313, #2552 — won't launch on NixOS, systemd-less environments
- **Segfaults on Fedora**: #5634, #8957 — libgallium / SIGSEGV
- **Wayland problems**: #9657, #6010, #7531, #1881 — freeze / black screen / blurry HiDPI
- **Electron OOM crashes**: #9954 — V8 OOM during video processing
- **Suspected root cause**: Electron version compatibility with OS graphics stacks, missing sandbox permissions, lack of CI testing across distros

### 3.2 Mobile App Performance (Critical)
- **Low framerate on Android** (#8873, 27 comments) — frame rate locked at 30fps on many devices
- **iOS crashes** (#8496, 12 comments) — "crashes all of the time" (iOS 12.5+)
- **iOS background sync broken** (#7127, 16 comments) — background synchronization does not work
- **OOM on "On Device" tab** (#9416) — full-resolution bitmaps loaded without downsampling
- **Upload reliability**: #9607 crashes on massive upload, #10055 temp file cleanup fails, #7167 hangs on large Google Takeout

### 3.3 Video Playback Issues
- **Desktop**: MOV files not playing (#9608), black screen (#3744, #3902), reverse orientation (#5210), landscape/portrait issue (#1749)
- **Mobile**: Brightness/gamma wrong (#2994), video loading hangs (#6651, #6652), crash on specific videos (#6470)
- **Firefox**: Can't play videos (#2581)
- **AV1**: No thumbnails (#6852)
- **High-bitrate**: GoPro footage problems (#3647)

### 3.4 Auth App Specific
- **QR code scanning bugs**: #9086 (black screen after scan), #2768 (can't scan large QR, 37 comments)
- **Search issues**: #1296 (Windows search intermittent, 22 comments), #4388 (freeze on search)
- **Duplicate entries**: #6903, #4786
- **Import issues**: Aegis (#7282, #5188, #5324), Google Auth (#5048, #2380), 2FAS (#5418)
- **Icon not changing**: #10388, #8972, #8561, #5836
- **Window management**: #4793 (double width), #8164 (won't close), #2061 (inside taskbar), #1414 (behind taskbar), #9286 (can't close before biometric unlock)
- **Tray icon missing/broken**: #4608, #7815, #8165, #4360, #3518
- **Autofill issues**: #4823 (iOS URI missing), #1303 (Bitwarden conflict)

### 3.5 Self-Hosting Pain Points
- **Docker Compose problems**: #9720 (build failures, 8 comments), #9192 (MinIO archived), #4056 (password can't change)
- **SMTP issues**: #9513 (can't use port 25/TLS)
- **Custom domain**: #7107 (hijack risk), #7079 (IDN not supported), #8994, #7834
- **Family features**: #7709 (can't enable when self-hosted)
- **Build failures**: #7773 (works with Docker but not Podman)
- **PostgreSQL version mismatch**: #7657

### 3.6 CLI Issues
- **Keyring problems**: #7493, #722 — completion/crash with keyring
- **D-Bus dependency on server**: #1328 (7 comments) — CLI fails on server without dbus
- **Export crashes**: #9288 (panic: float64 not int8), #7193 (404 errors), #8495 (decoder crypto failed)
- **Temp file cleanup**: #6551 (fills up /tmp/)
- **Dockerfile-x86 bug**: #6579 (apt-get on Alpine)
- **Windows issues**: #9064 (missing attributes)

### 3.7 Web App
- **Mobile browser broken**: #10398 — "Not working on browsers of mobile phones"
- **Self-hosted album links**: #10286 (can't open by Ente app)
- **Trip layout bugs**: #10038 (incorrect timeline grouping), #10037 (weird location names)
- **Upload to shared album**: #9417, #8256 — can't upload to albums shared to you
- **Video playback**: #2581 (Firefox), #1749 (thumbnail aspect ratio)

## 4. Feature Gaps & Improvement Opportunities

### 4.1 Missing Features
- **Native desktop packaging**: No Snap/Flatpak for Photos desktop, incomplete Flatpak for Auth
- **HEIF/HEIC export**: #6949 — Windows exports Live Photos as JPG+MOV instead of HEIC+MOV
- **Multi-album upload**: #10274 — can't upload to multiple albums at once
- **HDR photo support**: #4498, #8494 — iOS HDR / Lightroom HDR not recognized
- **AVIF rendering**: #4725 — pixelated
- **ML on Desktop**: #4087 — can't complete ML (face/magic search) but Android can
- **Watch folders broken**: #8558 (empty after restart), #4848 (changed file not rescanned), #8134 (re-upload large subset)
- **Export limitations**: #10045 (duplicate downloads), #9220 (metadata JSON not used on reimport), #3769 (not working)
- **Album sorting**: #9958 — mobile "Add to album" lacks sorting

### 4.2 Security & Privacy Concerns
- **iCloud backup leak**: #9774 — TOTP seeds exposed to Apple via iCloud Backup
- **iCloud temp file**: #2742 — import leaves source unencrypted in /tmp
- **Windows Smart App Control**: #10184 — unsigned DLL flagged
- **Auth code in email subject**: #5471 — verification code should not be in Subject
- **Orphaned ML blobs**: #9281 — disabling ML leaves encrypted data on server

### 4.3 Accessibility
- **Screen readers**: #4686 — Ente Auth Windows not accessible
- **Keyboard navigation**: #1182 (can't tab between fields), #1929 (tab skips tags), #2619 (Home/End keys ignored)
- **Font size**: #2005 — codes not visible with non-default font size

### 4.4 Architectural Improvements
- **Rust crypto consolidation**: The codebase has 3 crypto implementations:
  1. `flutter_sodium` (libsodium bindings) for mobile
  2. `rust/core/` (pure Rust: xsalsa20poly1305, x25519-dalek, argon2) for Rust targets
  3. `web/packages/wasm/` (Rust compiled to WASM) for web
  4. Server-side Go uses `golang.org/x/crypto` + `ente-io/go-srp`
  - **Opportunity**: Consolidate to single Rust crypto core and use WASM everywhere
- **Flutter ↔ Rust ↔ UniFFI**: Currently uses `flutter_rust_bridge` v2.12.0. Several Rust crates (`accounts`, `contacts`, `ensu/`, `e2e/`, `cli/`) are excluded from the workspace — unclear dependency management
- **Go server structure**: Single `ente/` package with 47 files — suggests domain boundaries could be cleaner
- **SRP protocol**: Custom non-standard SRP (#7529) — document it properly

### 4.5 Testing & CI
- **Flutter analyze fails**: Multiple PRs mention `flutter analyze` is "blocked by existing baseline errors on main" (#10386, #10375, #10382)
  - **Action**: Fix baseline errors, enforce zero-warning policy
- **No comprehensive E2E tests** visible in the repo
- **Desktop CI**: No cross-distro testing (AppImage tested only on Ubuntu)
- **Mobile CI**: Integration tests exist but unclear if run in CI

### 4.6 Documentation Gaps
- **Self-hosting**: Quickstart script broken (#6324), shell script bugs (#7701)
- **SRP flow**: Undocumented custom protocol (#7529)
- **API documentation**: No formal API spec (OpenAPI)

## 5. Most Impactful Improvements (Priority Order)

| Priority | Area | What | Why |
|----------|------|------|-----|
| **P0** | Desktop Linux | Fix startup crashes (white screen, segfault, AppImage/sandbox) | Affects all Linux users across all distros |
| **P0** | Mobile Android | Fix 30fps lock on Fairphone/OnePlus | 27 comments, affects core UX |
| **P0** | Mobile iOS | Fix background sync | 16 comments, core feature broken on iOS |
| **P1** | CI | Fix `flutter analyze` baseline errors | Blocks all mobile contributions |
| **P1** | Auth | Fix QR scanning (black screen + large QR) | 37+7 comments, core auth feature |
| **P1** | Desktop Auth | Fix search on Windows | 22 comments, daily driver issue |
| **P1** | Self-hosting | Fix Docker Compose builds | 8+ comments, blocks new users |
| **P2** | Issue triage | Add labels, prioritize, close stale | No label hygiene = hard to contribute |
| **P2** | CLI | Fix keyring, D-Bus, temp cleanup | Multiple crash reports |
| **P2** | Video | Fix MOV playback, AV1 thumbnails, orientation | Broad impact across platforms |
| **P3** | Edge | Consolidate crypto implementations | Reduce maintenance burden |
| **P3** | Edge | Enable Flatpak/Snap for desktop Photos | Linux distribution gap |

## 6. Code Health Observations

### 6.1 Dependencies
- **Go server**: Uses Gin (stable), but `aws-sdk-go` v1 (v2 is current) — migrate risk
- **Flutter**: `flutter_secure_storage` pinned at 9.0.0 due to known bug (#870)
- **Rust**: Pinned SRP crate to a specific commit — no semantic versioning
- **Media-kit**: Fork maintained by ente — high maintenance burden

### 6.2 Workspace Structure
- `rust/Cargo.toml` has 15 excluded members but only 2 actual workspace members
- Many Rust crates appear orphaned (no CI, no clear consumer)
- `web/` yarn workspaces and `mobile/` melos workspaces are well-organized

### 6.3 Potential Dead Code
- `rust/e2e/`, `rust/accounts/`, `rust/contacts/` — not in workspace, unclear if actively used
- `web/apps/legacy/`, `web/apps/paste/`, `web/apps/albums/` — unclear purpose
- `mobile/packages/` has many packages — some may be abandoned
