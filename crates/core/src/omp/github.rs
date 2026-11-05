//! omp from its GitHub releases, for when omp's own download channel is blocked
//! (`requirements/tech#omp-setup`, "GitHub download" and "Updates").
//!
//! Some corporate proxies block the site omp's official installer and `omp update` download from
//! (`omp.sh` → build.stencil.so → Cloudflare R2) as file hosting, whatever the certificates. The
//! releases of `can1357/oh-my-pi` carry the same program and usually pass.
//!
//! The download is one script per shell (POSIX `sh` with `curl`; Windows PowerShell 5.1), the same
//! text for the installer's terminal ([`definitions`], called from [`super::terminal`]) and for the
//! background update ([`update_script`], run by [`update`]). The backend has no HTTP client of its
//! own for it. The script:
//!
//! 1. picks the asset of the system and processor from [`ASSETS`];
//! 2. reads `releases/latest` ([`RELEASE_API_URL`], 60 s): `tag_name` and, for the asset with
//!    exactly that name, `browser_download_url`, `size` and `digest`; anything else is "blocked";
//! 3. for an update, stops with `94` unless `tag_name` is semver-newer than omp's version;
//! 4. downloads to `<folder>/.omp.download` (`.omp.download.exe` on Windows), 900 s, and checks, in
//!    this order, each failure deleting the file: status 200 (and no HTML page), size, SHA-256;
//! 5. puts it in place (a Windows `omp.exe` in use is renamed `omp.exe.<unix seconds>.old` first)
//!    and runs `<folder>/<file> --version`, which must exit `0` within 15 s, else the earlier omp
//!    is put back;
//! 6. for an installation only, makes the folder reachable: the user `Path` on Windows, a hint
//!    on macOS and Linux.
//!
//! It never skips a check: no `-k`/`--insecure`, no `-SkipCertificateCheck`. The repository, the API
//! address and the asset names are constants here; what the script reads from the environment is
//! `PI_INSTALL_DIR`, `LOCALAPPDATA`, the processor variables and, from the process that starts it,
//! the certificate variables.
//!
//! | Exit code | Meaning |
//! |---|---|
//! | `0` | installed |
//! | `90` | blocked: an HTTP status other than 200, an HTML page or not the release JSON, or the connection failed or timed out |
//! | `91` | a certificate that could not be verified |
//! | `92` | the size or the SHA-256 differ |
//! | `93` | the placed omp does not answer `--version` |
//! | `94` | the release is not newer than the version given (update only) |
//! | `1` | any other failure |

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::failure::{
    download_failed_reason, DownloadFailure, CERTIFICATE_CODES, CERTIFICATE_PHRASES,
};
use super::lookup::{LookupEnv, Os};
use super::probe::run_update;
use super::run::{limits, Exec};
use super::terminal::{absolute_text, quote, Shell, TerminalError};
use super::version::{parse_omp_version, Version};
use crate::process::timed::Failure;

/// The repository that publishes omp.
pub const REPOSITORY: &str = "can1357/oh-my-pi";

/// The latest release of [`REPOSITORY`], through GitHub's REST API.
pub const RELEASE_API_URL: &str = "https://api.github.com/repos/can1357/oh-my-pi/releases/latest";

/// A file of a release, for one system and processor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Asset {
    /// `darwin`, `linux` or `linux-musl` (macOS and Linux).
    pub system: &'static str,
    /// `x64` or `arm64`.
    pub processor: &'static str,
    /// The exact name of the file in the release.
    pub name: &'static str,
}

/// The assets of macOS and Linux, by the `<system>-<processor>` the script works out.
pub const POSIX_ASSETS: [Asset; 6] = [
    Asset {
        system: "darwin",
        processor: "arm64",
        name: "omp-darwin-arm64",
    },
    Asset {
        system: "darwin",
        processor: "x64",
        name: "omp-darwin-x64",
    },
    Asset {
        system: "linux",
        processor: "x64",
        name: "omp-linux-x64",
    },
    Asset {
        system: "linux",
        processor: "arm64",
        name: "omp-linux-arm64",
    },
    Asset {
        system: "linux-musl",
        processor: "x64",
        name: "omp-linux-musl-x64",
    },
    Asset {
        system: "linux-musl",
        processor: "arm64",
        name: "omp-linux-musl-arm64",
    },
];

/// The assets of Windows, by the value of `PROCESSOR_ARCHITEW6432` (else
/// `PROCESSOR_ARCHITECTURE`).
pub const WINDOWS_ASSETS: [(&str, &str); 2] = [
    ("AMD64", "omp-windows-x64.exe"),
    ("ARM64", "omp-windows-arm64.exe"),
];

/// Where Alpine Linux, which uses musl, says so.
pub const ALPINE_RELEASE: &str = "/etc/alpine-release";

/// The exit codes of the download.
pub mod exit {
    pub const INSTALLED: i32 = 0;
    pub const BLOCKED: i32 = 90;
    pub const CERTIFICATE: i32 = 91;
    pub const DIFFERENT: i32 = 92;
    pub const NOT_RUNNING: i32 = 93;
    pub const NOT_NEWER: i32 = 94;
    pub const OTHER: i32 = 1;
}

/// What the download is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target<'a> {
    /// The installer's terminal: `PI_INSTALL_DIR` or the default folder; the folder is made
    /// reachable.
    Install,
    /// The background update of the omp in `folder`, which has `version`; both already checked
    /// and quoted-safe, see [`update_script`].
    Update { folder: &'a str, version: &'a str },
}

/// What the script reads from outside. The limits and addresses of the real download are the
/// [`Default`]; the tests change them to reach a local server and to wait less.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub api_url: String,
    pub alpine_release: String,
    pub api_secs: u64,
    pub download_secs: u64,
    pub version_secs: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_url: RELEASE_API_URL.to_owned(),
            alpine_release: ALPINE_RELEASE.to_owned(),
            api_secs: 60,
            download_secs: 900,
            version_secs: 15,
        }
    }
}

/// The shell code that defines the download for `shell`, with no call: POSIX defines
/// `omp_github`, which `return`s its exit code; PowerShell defines `Invoke-OmpGithub`, which leaves
/// it in `$global:OmpCode`. Single line, with no double quote in the PowerShell text, so that it
/// passes through the command line of `powershell.exe` and the `do script` of Terminal.
pub fn definitions(shell: Shell, target: Target, config: &Config) -> String {
    match shell {
        Shell::Posix => posix_definitions(target, config),
        Shell::PowerShell => powershell_definitions(target, config),
    }
}

/// The call of the download after [`definitions`].
pub fn call(shell: Shell) -> &'static str {
    match shell {
        Shell::Posix => "omp_github",
        Shell::PowerShell => "Invoke-OmpGithub",
    }
}

/// The script of the background update of the omp in `folder`, which has `version`: the download,
/// then its exit code as the code of the shell. `folder` is the only text inserted from outside
/// the constants, with `version`, and both are checked and quoted.
pub fn update_script(os: Os, folder: &Path, version: &Version) -> Result<String, TerminalError> {
    let folder = absolute_text(os, folder)?;
    let version = version.to_string();
    let target = Target::Update {
        folder,
        version: &version,
    };
    let config = Config::default();
    Ok(match os {
        Os::Windows => format!(
            "{} Invoke-OmpGithub; exit $global:OmpCode",
            definitions(Shell::PowerShell, target, &config)
        ),
        Os::MacOs | Os::Linux => format!(
            "{} omp_github; exit $?",
            definitions(Shell::Posix, target, &config)
        ),
    })
}

/// Joins the lines of a template into one, trimmed, with the placeholders `<<NAME>>` replaced.
fn assemble(template: &str, replacements: &[(&str, String)]) -> String {
    let mut text = template
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    for (name, value) in replacements {
        text = text.replace(&format!("<<{name}>>"), value);
    }
    text
}

fn posix_definitions(target: Target, config: &Config) -> String {
    let (mode, folder, version) = match target {
        Target::Install => ("install", String::new(), String::new()),
        Target::Update { folder, version } => ("update", folder.to_owned(), version.to_owned()),
    };
    let asset_cases: String = POSIX_ASSETS
        .iter()
        .map(|a| {
            format!(
                "{}-{}) g_asset={};;\n",
                a.system,
                a.processor,
                quote(Shell::Posix, a.name)
            )
        })
        .collect();
    assemble(
        POSIX_TEMPLATE,
        &[
            ("MODE", mode.to_owned()),
            ("DIR", quote(Shell::Posix, &folder)),
            ("VERSION", quote(Shell::Posix, &version)),
            ("API", quote(Shell::Posix, &config.api_url)),
            ("ALPINE", quote(Shell::Posix, &config.alpine_release)),
            (
                "ASSET_CASES",
                asset_cases.lines().collect::<Vec<_>>().join(" "),
            ),
            ("API_SECS", config.api_secs.to_string()),
            ("DOWNLOAD_SECS", config.download_secs.to_string()),
            ("VERSION_SECS", config.version_secs.to_string()),
            (
                "AWK_JSON",
                AWK_JSON
                    .lines()
                    .map(str::trim)
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
            (
                "AWK_SEMVER",
                AWK_SEMVER
                    .lines()
                    .map(str::trim)
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
        ],
    )
}

fn powershell_definitions(target: Target, config: &Config) -> String {
    let (mode, folder, version) = match target {
        Target::Install => ("install", String::new(), String::new()),
        Target::Update { folder, version } => ("update", folder.to_owned(), version.to_owned()),
    };
    let patterns: Vec<String> = CERTIFICATE_CODES
        .iter()
        .chain(CERTIFICATE_PHRASES.iter())
        .chain(["trust relationship"].iter())
        .map(|p| quote(Shell::PowerShell, p))
        .collect();
    let assets: String = WINDOWS_ASSETS
        .iter()
        .map(|(cpu, name)| {
            format!(
                "if ($cpu -eq {}) {{ $asset = {} }};",
                quote(Shell::PowerShell, cpu),
                quote(Shell::PowerShell, name)
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    assemble(
        POWERSHELL_TEMPLATE,
        &[
            ("MODE", quote(Shell::PowerShell, mode)),
            ("DIR", quote(Shell::PowerShell, &folder)),
            ("VERSION", quote(Shell::PowerShell, &version)),
            ("API", quote(Shell::PowerShell, &config.api_url)),
            ("CERTIFICATE_PATTERNS", patterns.join(", ")),
            ("ASSETS", assets),
            ("API_SECS", config.api_secs.to_string()),
            ("DOWNLOAD_SECS", config.download_secs.to_string()),
            ("VERSION_MS", (config.version_secs * 1000).to_string()),
        ],
    )
}

/// The functions of the POSIX download. Every statement ends with `;` and the text has no
/// backslash pair, so that it survives being joined into one line and typed in any shell.
const POSIX_TEMPLATE: &str = r#"
g_mode=<<MODE>>; g_dir=<<DIR>>; g_version=<<VERSION>>; g_api=<<API>>; g_work=''; g_dl='';
g_say() { printf '%s\n' "$*" >&2; };
g_clean() { if [ -n "$g_work" ]; then rm -rf "$g_work"; fi; if [ -n "$g_dl" ]; then rm -f "$g_dl"; fi; };
g_curl_code() { case "$1" in 60|77) return 91;; 5|6|7|18|22|28|35|47|52|55|56) return 90;; *) return 1;; esac; };
g_find_asset() {
g_sys=$(uname -s 2>/dev/null); g_cpu=$(uname -m 2>/dev/null); g_key=''; g_asset='';
case "$g_sys" in
Darwin) if [ "$(sysctl -n hw.optional.arm64 2>/dev/null)" = 1 ]; then g_key=darwin-arm64; else g_key=darwin-x64; fi;;
Linux)
case "$g_cpu" in x86_64|amd64) g_arch=x64;; aarch64|arm64) g_arch=arm64;; *) g_arch='';; esac;
if [ -n "$g_arch" ]; then
g_libc=linux;
if [ -e <<ALPINE>> ] || ldd --version 2>&1 | grep -qi musl; then g_libc=linux-musl; fi;
g_key=$g_libc-$g_arch;
fi;;
esac;
case "$g_key" in
<<ASSET_CASES>>
*) g_asset='';;
esac;
[ -n "$g_asset" ];
};
g_check() {
"$1" --version >"$g_work/version.out" 2>&1 & g_pid=$!;
( sleep <<VERSION_SECS>>; kill -9 "$g_pid" ) >/dev/null 2>&1 & g_dog=$!;
wait "$g_pid"; g_r=$?;
kill "$g_dog" >/dev/null 2>&1;
return "$g_r";
};
omp_github_run() {
g_work=$(mktemp -d "${TMPDIR:-/tmp}/omp-github.XXXXXX") || { g_say "Could not create a temporary folder."; return 1; };
if ! g_find_asset; then g_say "omp has no GitHub build for $g_sys $g_cpu"; return 1; fi;
if [ "$g_mode" = install ]; then
g_dir=${PI_INSTALL_DIR:-};
if [ -z "$g_dir" ]; then
if [ -z "${HOME:-}" ]; then g_say "Could not find the folder for omp: HOME is not set."; return 1; fi;
g_dir=$HOME/.local/bin;
fi;
fi;
mkdir -p "$g_dir" || { g_say "Could not create $g_dir"; return 1; };
g_dl=$g_dir/.omp.download; g_target=$g_dir/omp;
g_say "Latest omp release: $g_api";
g_status=$(curl -sS -L --connect-timeout 10 --max-time <<API_SECS>> -H 'Accept: application/vnd.github+json' -o "$g_work/release.json" -w '%{http_code}' "$g_api" 2>"$g_work/curl.err"); g_c=$?;
if [ "$g_c" -ne 0 ]; then g_curl_code "$g_c"; g_r=$?; g_say "Could not read $g_api (curl exit $g_c): $(cat "$g_work/curl.err")"; return "$g_r"; fi;
if [ "$g_status" != 200 ]; then g_say "Could not read $g_api: HTTP status $g_status"; return 90; fi;
g_info=$(awk -v asset="$g_asset" '<<AWK_JSON>>' "$g_work/release.json");
g_ok=$(printf '%s\n' "$g_info" | sed -n 1p); g_tag=$(printf '%s\n' "$g_info" | sed -n 2p);
g_url=$(printf '%s\n' "$g_info" | sed -n 3p); g_size=$(printf '%s\n' "$g_info" | sed -n 4p); g_digest=$(printf '%s\n' "$g_info" | sed -n 5p);
if [ "$g_ok" != ok ] || [ -z "$g_tag" ]; then g_say "The answer of $g_api is not the release JSON of omp."; return 90; fi;
g_say "GitHub release $g_tag, asset $g_asset";
if [ "$g_mode" = update ]; then
if ! T=$g_tag C=$g_version awk '<<AWK_SEMVER>>'; then g_say "omp $g_version is not older than the GitHub release $g_tag."; return 94; fi;
fi;
g_sum=${g_digest#sha256:};
if [ -z "$g_url" ] || [ "$g_sum" = "$g_digest" ] || ! printf '%s' "$g_sum" | grep -Eq '^[0-9a-fA-F]{64}$' || ! printf '%s' "$g_size" | grep -Eq '^[0-9]+$'; then g_say "The release has no usable $g_asset (download address, size and sha256 digest)."; return 90; fi;
rm -f "$g_dl";
g_say "Downloading $g_url";
g_out=$(curl -fL -sS --connect-timeout 10 --max-time <<DOWNLOAD_SECS>> -o "$g_dl" -w '%{http_code} %{content_type}' "$g_url" 2>"$g_work/curl.err"); g_c=$?;
if [ "$g_c" -ne 0 ]; then
g_curl_code "$g_c"; g_r=$?; if [ "$g_c" -eq 18 ]; then g_r=92; fi;
rm -f "$g_dl"; g_say "Could not download $g_url (curl exit $g_c, HTTP status ${g_out%% *}): $(cat "$g_work/curl.err")"; return "$g_r";
fi;
g_status=${g_out%% *}; g_type=${g_out#* };
if [ "$g_status" != 200 ]; then rm -f "$g_dl"; g_say "Could not download $g_url: HTTP status $g_status"; return 90; fi;
case "$g_type" in text/html*) rm -f "$g_dl"; g_say "$g_url answered an HTML page, not the file."; return 90;; esac;
g_real=$(wc -c < "$g_dl" | tr -d ' ');
if [ "$g_real" != "$g_size" ]; then rm -f "$g_dl"; g_say "The downloaded file has $g_real bytes, GitHub publishes $g_size."; return 92; fi;
if command -v sha256sum >/dev/null 2>&1; then g_hash=$(sha256sum "$g_dl" 2>/dev/null | awk '{ print $1 }'); else g_hash=$(shasum -a 256 "$g_dl" 2>/dev/null | awk '{ print $1 }'); fi;
g_hash=$(printf '%s' "$g_hash" | tr 'A-F' 'a-f'); g_want=$(printf '%s' "$g_sum" | tr 'A-F' 'a-f');
if [ -z "$g_hash" ]; then rm -f "$g_dl"; g_say "Could not compute the SHA-256 of the download (no sha256sum or shasum)."; return 1; fi;
if [ "$g_hash" != "$g_want" ]; then rm -f "$g_dl"; g_say "The SHA-256 of the download ($g_hash) is not the one GitHub publishes ($g_want)."; return 92; fi;
chmod 755 "$g_dl" || { rm -f "$g_dl"; g_say "Could not make the download executable."; return 1; };
g_had=0;
if [ -e "$g_target" ]; then g_had=1; cp -p "$g_target" "$g_work/previous" || { rm -f "$g_dl"; g_say "Could not keep the earlier omp aside."; return 1; }; fi;
mv -f "$g_dl" "$g_target" || { rm -f "$g_dl"; g_say "Could not move the download to $g_target."; return 1; };
if ! g_check "$g_target"; then
if [ "$g_had" -eq 1 ]; then cp -p "$g_work/previous" "$g_dl" && mv -f "$g_dl" "$g_target"; else rm -f "$g_target"; fi;
g_say "The omp from GitHub does not answer --version; the earlier omp stays in place."; return 93;
fi;
g_say "omp $g_tag is in $g_target";
if [ "$g_mode" = install ]; then
case ":$PATH:" in *":$g_dir:"*) ;; *) g_say "Add $g_dir to your PATH to use omp in your own terminal.";; esac;
fi;
return 0;
};
omp_github() { omp_github_run; g_final=$?; g_clean; return "$g_final"; };
"#;

/// Reads the release JSON of the file named by `asset` and prints `ok`, `tag_name` and the
/// `browser_download_url`, `size` and `digest` of that asset, one per line (empty when missing); it
/// prints nothing when the text is not a JSON object. A small tokenizer: no `jq` is assumed. No
/// quote or backslash of its own, so that it can sit in single quotes: they are built from codes.
const AWK_JSON: &str = r#"
function put(v,  p, d) { p = ""; for (d = 1; d <= depth; d++) { p = p (d > 1 ? "/" : "") (kind[d] == "o" ? key[d] : idx[d]); } val[p] = v; }
{ text = text $0 "\n"; }
END {
q = sprintf("%c", 34); bs = sprintf("%c", 92); n = length(text); i = 1; depth = 0; bad = 0;
while (i <= n && !bad) {
c = substr(text, i, 1);
if (c == " " || c == "\t" || c == "\n" || c == "\r") { i++; continue; }
if (depth == 0 && c != "{") { bad = 1; break; }
if (c == "{") { depth++; kind[depth] = "o"; key[depth] = ""; wantkey[depth] = 1; i++; continue; }
if (c == "[") { depth++; kind[depth] = "a"; idx[depth] = 0; i++; continue; }
if (c == "}" || c == "]") { depth--; i++; if (depth == 0) { break; } continue; }
if (c == ",") { if (kind[depth] == "o") { wantkey[depth] = 1; } else { idx[depth]++; } i++; continue; }
if (c == ":") { wantkey[depth] = 0; i++; continue; }
if (c == q) {
s = ""; i++;
while (i <= n) {
c = substr(text, i, 1);
if (c == q) { break; }
if (c == bs) {
i++; c = substr(text, i, 1);
if (c == "n") { c = "\n"; } else if (c == "t") { c = "\t"; } else if (c == "r") { c = ""; }
else if (c == "u") { hex = substr(text, i + 1, 4); i += 4; c = (hex == "0026") ? "&" : "?"; }
}
s = s c; i++;
}
if (i > n) { bad = 1; break; }
i++;
if (kind[depth] == "o" && wantkey[depth]) { key[depth] = s; } else { put(s); }
continue;
}
j = i;
while (j <= n) { c = substr(text, j, 1); if (c == "," || c == "}" || c == "]" || c == " " || c == "\n" || c == "\t" || c == "\r") { break; } j++; }
put(substr(text, i, j - i)); i = j;
}
if (bad || depth != 0) { exit; }
for (k in val) { if (val[k] == asset && index(k, "assets/") == 1 && substr(k, length(k) - 4) == "/name") { b = substr(k, 1, length(k) - 4); url = val[b "browser_download_url"]; size = val[b "size"]; digest = val[b "digest"]; } }
print "ok"; print val["tag_name"]; print url; print size; print digest;
}
"#;

/// Exits `0` when the version in `T` (`tag_name`, a leading `v` ignored) is semver-newer than the
/// one in `C`, `1` when it is not and `2` when either is not a version: the order of
/// [`Version`](super::version::Version).
const AWK_SEMVER: &str = r#"
function parse(v, o,  k, core, pre, parts) {
sub(/^v/, "", v); sub(/[+].*$/, "", v); pre = ""; core = v; k = index(v, "-");
if (k > 0) { core = substr(v, 1, k - 1); pre = substr(v, k + 1); }
if (core !~ /^[0-9]+[.][0-9]+[.][0-9]+$/) { return 0; }
if (k > 0 && pre !~ /^[0-9A-Za-z.-]+$/) { return 0; }
split(core, parts, "."); o[1] = parts[1] + 0; o[2] = parts[2] + 0; o[3] = parts[3] + 0; o[4] = pre; return 1;
}
function numeric(s) { return s ~ /^[0-9]+$/; }
BEGIN {
if (!parse(ENVIRON["T"], a) || !parse(ENVIRON["C"], b)) { exit 2; }
for (i = 1; i <= 3; i++) { if (a[i] > b[i]) { exit 0; } if (a[i] < b[i]) { exit 1; } }
if (a[4] == b[4]) { exit 1; }
if (a[4] == "") { exit 0; }
if (b[4] == "") { exit 1; }
na = split(a[4], x, "."); nb = split(b[4], y, ".");
for (i = 1; i <= na && i <= nb; i++) {
if (numeric(x[i]) && numeric(y[i])) { if (x[i] + 0 != y[i] + 0) { exit ((x[i] + 0 > y[i] + 0) ? 0 : 1); } }
else if (numeric(x[i])) { exit 1; }
else if (numeric(y[i])) { exit 0; }
else if (x[i] != y[i]) { exit ((x[i] > y[i]) ? 0 : 1); }
}
exit ((na > nb) ? 0 : 1);
}
"#;

/// The functions of the PowerShell 5.1 download. No double quote anywhere, every statement ends
/// with `;` and the whole is one line, so that it passes through `powershell.exe -Command`.
const POWERSHELL_TEMPLATE: &str = r#"
$ProgressPreference = 'SilentlyContinue';
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12;
$global:OmpMode = <<MODE>>; $global:OmpDir = <<DIR>>; $global:OmpVersion = <<VERSION>>; $global:OmpCode = 1; $global:OmpDownload = $null;
function Write-OmpLine([string]$m) { [Console]::Error.WriteLine($m) };
function Stop-Omp([int]$c, [string]$m) { Write-OmpLine $m; $global:OmpCode = $c };
function Get-OmpFailureCode($e) {
$m = [string]$e.Exception.Message; $i = $e.Exception.InnerException;
while ($i) { $m += ' ' + $i.Message; $i = $i.InnerException };
foreach ($p in @(<<CERTIFICATE_PATTERNS>>)) { if ($m.IndexOf($p, [StringComparison]::OrdinalIgnoreCase) -ge 0) { return 91 } };
if ($e.Exception -is [System.Net.WebException]) { return 90 };
return 1
};
function Get-OmpFailureText($e) {
$m = [string]$e.Exception.Message;
if ($e.Exception.Response) { $m += ' (HTTP status ' + [int]$e.Exception.Response.StatusCode + ')' };
return $m
};
function Test-OmpNewer([string]$Tag, [string]$Current) {
$re = '^v?([0-9]+)[.]([0-9]+)[.]([0-9]+)(-([0-9A-Za-z.-]+))?([+].*)?$';
$x = [regex]::Match($Tag, $re); $y = [regex]::Match($Current, $re);
if (-not $x.Success -or -not $y.Success) { return $false };
for ($i = 1; $i -le 3; $i++) { $p = [int64]$x.Groups[$i].Value; $q = [int64]$y.Groups[$i].Value; if ($p -gt $q) { return $true }; if ($p -lt $q) { return $false } };
$s = $x.Groups[5].Value; $t = $y.Groups[5].Value;
if ($s -ceq $t) { return $false };
if ($s -eq '') { return $true };
if ($t -eq '') { return $false };
$m = $s.Split('.'); $n = $t.Split('.');
for ($i = 0; $i -lt [Math]::Min($m.Length, $n.Length); $i++) {
$u = $m[$i]; $v = $n[$i]; $nu = $u -match '^[0-9]+$'; $nv = $v -match '^[0-9]+$';
if ($nu -and $nv) { if ([int64]$u -ne [int64]$v) { return ([int64]$u -gt [int64]$v) } }
elseif ($nu) { return $false }
elseif ($nv) { return $true }
elseif ($u -cne $v) { return ([string]::CompareOrdinal($u, $v) -gt 0) }
};
return ($m.Length -gt $n.Length)
};
function Invoke-OmpGithubSteps {
$api = <<API>>;
$asset = $null; $cpu = $env:PROCESSOR_ARCHITEW6432; if (-not $cpu) { $cpu = $env:PROCESSOR_ARCHITECTURE };
<<ASSETS>>
if (-not $asset) { Stop-Omp 1 ('omp has no GitHub build for Windows ' + $cpu); return };
if ($global:OmpMode -eq 'install') {
$dir = $env:PI_INSTALL_DIR;
if (-not $dir) { if (-not $env:LOCALAPPDATA) { Stop-Omp 1 'Could not find the folder for omp: LOCALAPPDATA is not set.'; return }; $dir = Join-Path $env:LOCALAPPDATA 'omp' };
} else { $dir = $global:OmpDir };
[void](New-Item -ItemType Directory -Force -Path $dir);
Get-ChildItem -LiteralPath $dir -Filter 'omp.exe.*.old' -Force -ErrorAction SilentlyContinue | ForEach-Object { Remove-Item -LiteralPath $_.FullName -Force -ErrorAction SilentlyContinue };
$dl = Join-Path $dir '.omp.download.exe'; $target = Join-Path $dir 'omp.exe'; $global:OmpDownload = $dl;
Write-OmpLine ('Latest omp release: ' + $api);
try { $r = Invoke-WebRequest -UseBasicParsing -Uri $api -Headers @{ Accept = 'application/vnd.github+json' } -TimeoutSec <<API_SECS>> } catch { Stop-Omp (Get-OmpFailureCode $_) ('Could not read ' + $api + ': ' + (Get-OmpFailureText $_)); return };
if ($r.StatusCode -ne 200) { Stop-Omp 90 ('Could not read ' + $api + ': HTTP status ' + $r.StatusCode); return };
$body = $r.Content; if ($body -is [byte[]]) { $body = [Text.Encoding]::UTF8.GetString($body) };
$j = $null; try { $j = $body | ConvertFrom-Json } catch { $j = $null };
if (-not $j -or -not $j.tag_name) { Stop-Omp 90 ('The answer of ' + $api + ' is not the release JSON of omp.'); return };
$tag = [string]$j.tag_name;
Write-OmpLine ('GitHub release ' + $tag + ', asset ' + $asset);
if ($global:OmpMode -eq 'update') { if (-not (Test-OmpNewer $tag $global:OmpVersion)) { Stop-Omp 94 ('omp ' + $global:OmpVersion + ' is not older than the GitHub release ' + $tag + '.'); return } };
$a = $null; foreach ($f in @($j.assets)) { if ($f.name -ceq $asset) { $a = $f; break } };
$sum = ''; $size = 0L; $url = '';
if ($a) { $url = [string]$a.browser_download_url; if (([string]$a.digest) -match '^sha256:([0-9a-fA-F]{64})$') { $sum = $Matches[1] }; if (-not [int64]::TryParse([string]$a.size, [ref]$size)) { $sum = '' } };
if (-not $url -or -not $sum) { Stop-Omp 90 ('The release has no usable ' + $asset + ' (download address, size and sha256 digest).'); return };
Write-OmpLine ('Downloading ' + $url);
try { $resp = Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $dl -PassThru -TimeoutSec <<DOWNLOAD_SECS>> } catch { Stop-Omp (Get-OmpFailureCode $_) ('Could not download ' + $url + ': ' + (Get-OmpFailureText $_)); return };
if ($resp.StatusCode -ne 200) { Stop-Omp 90 ('Could not download ' + $url + ': HTTP status ' + $resp.StatusCode); return };
if ([string]$resp.Headers['Content-Type'] -like 'text/html*') { Stop-Omp 90 ($url + ' answered an HTML page, not the file.'); return };
$real = (Get-Item -LiteralPath $dl).Length;
if ($real -ne $size) { Stop-Omp 92 ('The downloaded file has ' + $real + ' bytes, GitHub publishes ' + $size + '.'); return };
$hash = (Get-FileHash -LiteralPath $dl -Algorithm SHA256).Hash;
if ($hash -ne $sum) { Stop-Omp 92 ('The SHA-256 of the download (' + $hash + ') is not the one GitHub publishes (' + $sum + ').'); return };
$old = $null;
if (Test-Path -LiteralPath $target) { $old = $target + '.' + [DateTimeOffset]::UtcNow.ToUnixTimeSeconds() + '.old'; Move-Item -LiteralPath $target -Destination $old -Force };
try { Move-Item -LiteralPath $dl -Destination $target -Force } catch { if ($old) { Move-Item -LiteralPath $old -Destination $target -Force }; throw };
$ok = $false;
try {
$psi = New-Object System.Diagnostics.ProcessStartInfo; $psi.FileName = $target; $psi.Arguments = '--version'; $psi.UseShellExecute = $false; $psi.CreateNoWindow = $true; $psi.RedirectStandardOutput = $true; $psi.RedirectStandardError = $true;
$proc = [System.Diagnostics.Process]::Start($psi); $so = $proc.StandardOutput.ReadToEndAsync(); $se = $proc.StandardError.ReadToEndAsync();
if ($proc.WaitForExit(<<VERSION_MS>>)) { $ok = ($proc.ExitCode -eq 0) } else { try { $proc.Kill() } catch { } }
} catch { $ok = $false };
if (-not $ok) {
Remove-Item -LiteralPath $target -Force -ErrorAction SilentlyContinue;
if ($old) { Move-Item -LiteralPath $old -Destination $target -Force };
Stop-Omp 93 'The omp from GitHub does not answer --version; the earlier omp stays in place.'; return
};
Write-OmpLine ('omp ' + $tag + ' is in ' + $target);
if ($global:OmpMode -eq 'install') {
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User'); $has = $false;
foreach ($e in ([string]$userPath).Split(';')) { if ($e.Trim().TrimEnd('\') -ieq $dir.TrimEnd('\')) { $has = $true } };
if (-not $has) { if ($userPath) { $newPath = $userPath.TrimEnd(';') + ';' + $dir } else { $newPath = $dir }; [Environment]::SetEnvironmentVariable('Path', $newPath, 'User'); Write-OmpLine ('Added ' + $dir + ' to your user Path; open a new terminal to use omp.') }
};
$global:OmpCode = 0
};
function Invoke-OmpGithub {
$global:OmpCode = 1; $global:OmpDownload = $null; $ErrorActionPreference = 'Stop';
try { Invoke-OmpGithubSteps } catch { Stop-Omp (Get-OmpFailureCode $_) ('The download of omp failed: ' + (Get-OmpFailureText $_)) } finally { if ($global:OmpDownload) { Remove-Item -LiteralPath $global:OmpDownload -Force -ErrorAction SilentlyContinue } }
};
"#;

// --- the update ---------------------------------------------------------------------------------

/// How the background download ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadOutcome {
    /// `0`: omp was put in place.
    Installed,
    /// `94`: the release is not newer than omp.
    NotNewer,
    /// Any other code, or the time limit.
    Failed(DownloadFailure),
}

/// What the background download did, for the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadRun {
    /// The `tag_name` of the release, when the script got that far.
    pub tag: Option<String>,
    /// The asset it chose, when it got that far.
    pub asset: Option<String>,
    pub outcome: DownloadOutcome,
    /// The exit code; `None` when the time limit ended it.
    pub exit_code: Option<i32>,
    pub elapsed: Duration,
    /// The reason shown to the developer when it failed (`None` when it did not).
    pub reason: Option<String>,
}

/// What [`update`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateRun {
    /// `Ok` is `omp.UPDATE_ENDED`; `Err(reason)` is `omp.UPDATE_FAILED`.
    pub result: Result<(), String>,
    /// The download from GitHub, when it ran.
    pub download: Option<DownloadRun>,
}

/// The folder the download may replace omp in, when it may: `ompPath` is empty, and the omp found,
/// with its symlinks resolved, sits directly in `PI_INSTALL_DIR` or the default folder of the
/// official installer (not bun's folder, not a package manager's), and on Windows has no `.bunx`
/// file next to it. That folder, with its symlinks resolved.
pub fn fallback_folder(setting: &str, omp: &Path, env: &LookupEnv) -> Option<PathBuf> {
    if !setting.trim().is_empty() {
        return None;
    }
    let resolved = dunce::canonicalize(omp).ok()?;
    let folder = resolved.parent()?;
    let official = env
        .install_dirs()
        .iter()
        .filter_map(|dir| dunce::canonicalize(dir).ok())
        .any(|dir| dir == folder);
    if !official {
        return None;
    }
    if env.os == Os::Windows && resolved.with_extension("bunx").exists() {
        return None;
    }
    Some(folder.to_path_buf())
}

/// `omp update` and, when it fails and [`fallback_folder`] allows it, the download of the latest
/// GitHub release in its place, hidden, with a limit of 15 min and the certificate variables of
/// `exec`. `94` (the release is not newer) leaves the reason of `omp update`; any other code, or
/// the limit, gives the reason of the download.
pub fn update(omp: &Path, setting: &str, env: &LookupEnv, exec: &dyn Exec) -> UpdateRun {
    let reason = match run_update(omp, exec) {
        Ok(()) => {
            return UpdateRun {
                result: Ok(()),
                download: None,
            }
        }
        Err(reason) => reason,
    };
    let not_run = |reason| UpdateRun {
        result: Err(reason),
        download: None,
    };
    let Some(folder) = fallback_folder(setting, omp, env) else {
        return not_run(reason);
    };
    // An omp whose version cannot be read counts as older than any release.
    let version = exec
        .run(omp, &["--version"], limits::VERSION)
        .ok()
        .and_then(|output| parse_omp_version(&output.stdout))
        .or_else(|| Version::parse("0.0.0"))
        .expect("0.0.0 is a version");
    let Ok((program, args)) = background_command(env.os, &folder, &version) else {
        return not_run(reason);
    };
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let started = Instant::now();
    let ran = exec.run(&program, &args, limits::GITHUB_UPDATE);
    let elapsed = started.elapsed();
    let (exit_code, stderr) = match &ran {
        Ok(output) => (Some(exit::INSTALLED), output.stderr.as_str()),
        Err(Failure::Exit { code, stderr }) => (Some(code.unwrap_or(exit::OTHER)), stderr.as_str()),
        Err(Failure::NotStarted) => (Some(exit::OTHER), ""),
        Err(Failure::TimedOut { .. }) => (None, ""),
    };
    let (tag, asset) = parse_release_line(stderr);
    let outcome = match exit_code {
        Some(exit::INSTALLED) => DownloadOutcome::Installed,
        Some(exit::NOT_NEWER) => DownloadOutcome::NotNewer,
        Some(code) => DownloadOutcome::Failed(DownloadFailure::Exit(code)),
        None => DownloadOutcome::Failed(DownloadFailure::TimedOut),
    };
    let (result, reason) = match outcome {
        DownloadOutcome::Installed => (Ok(()), None),
        DownloadOutcome::NotNewer => (Err(reason), None),
        DownloadOutcome::Failed(failure) => {
            let text = download_failed_reason(failure, exec.system_certificates_on());
            (Err(text.clone()), Some(text))
        }
    };
    UpdateRun {
        result,
        download: Some(DownloadRun {
            tag,
            asset,
            outcome,
            exit_code,
            elapsed,
            reason,
        }),
    }
}

/// The hidden child process of the background download: `/bin/sh -c <script>` on macOS and Linux,
/// `powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command <script>` on
/// Windows (started with `CREATE_NO_WINDOW` by [`crate::process::command`]), with the script of
/// [`update_script`] for `folder` and `version`.
pub fn background_command(
    os: Os,
    folder: &Path,
    version: &Version,
) -> Result<(PathBuf, Vec<String>), TerminalError> {
    let script = update_script(os, folder, version)?;
    Ok(match os {
        Os::Windows => (
            powershell_path(),
            [
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
            ]
            .into_iter()
            .map(str::to_owned)
            .chain([script])
            .collect(),
        ),
        Os::MacOs | Os::Linux => (PathBuf::from("/bin/sh"), vec!["-c".to_owned(), script]),
    })
}

/// The Windows PowerShell 5.1 of this machine, by its full path.
fn powershell_path() -> PathBuf {
    let root =
        std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
    root.join("System32")
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe")
}

/// The tag and the asset of the line `GitHub release <tag>, asset <name>` that the script prints.
fn parse_release_line(stderr: &str) -> (Option<String>, Option<String>) {
    stderr
        .lines()
        .find_map(|line| line.trim().strip_prefix("GitHub release "))
        .and_then(|rest| rest.split_once(", asset "))
        .map_or((None, None), |(tag, asset)| {
            (Some(tag.trim().to_owned()), Some(asset.trim().to_owned()))
        })
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::omp::failure::download_failed_reason;
    use crate::omp::run::Certificates;
    use crate::process::timed::Output;

    // --- the scripts as text ---------------------------------------------------------------------

    fn posix(target: Target) -> String {
        definitions(Shell::Posix, target, &Config::default())
    }

    fn powershell(target: Target) -> String {
        definitions(Shell::PowerShell, target, &Config::default())
    }

    #[test]
    fn the_release_line_gives_the_tag_and_the_asset() {
        assert_eq!(
            parse_release_line(
                "Latest omp release: x\nGitHub release v18.16.0, asset omp-linux-x64\nboom"
            ),
            (
                Some("v18.16.0".to_owned()),
                Some("omp-linux-x64".to_owned())
            )
        );
        assert_eq!(parse_release_line("Could not read it"), (None, None));
    }

    #[test]
    fn the_repository_the_address_and_the_assets_are_constants() {
        assert_eq!(REPOSITORY, "can1357/oh-my-pi");
        assert_eq!(
            RELEASE_API_URL,
            "https://api.github.com/repos/can1357/oh-my-pi/releases/latest"
        );
        let names: Vec<&str> = POSIX_ASSETS
            .iter()
            .map(|a| a.name)
            .chain(WINDOWS_ASSETS.iter().map(|(_, name)| *name))
            .collect();
        assert_eq!(
            names,
            [
                "omp-darwin-arm64",
                "omp-darwin-x64",
                "omp-linux-x64",
                "omp-linux-arm64",
                "omp-linux-musl-x64",
                "omp-linux-musl-arm64",
                "omp-windows-x64.exe",
                "omp-windows-arm64.exe",
            ]
        );
        let sh = posix(Target::Install);
        let ps = powershell(Target::Install);
        assert!(sh.contains(&format!("g_api='{RELEASE_API_URL}'")));
        assert!(ps.contains(&format!("$api = '{RELEASE_API_URL}'")));
        for name in &names[..6] {
            assert!(sh.contains(&format!("g_asset='{name}'")), "{name}");
        }
        for (_, name) in WINDOWS_ASSETS {
            assert!(ps.contains(&format!("$asset = '{name}'")), "{name}");
        }
    }

    #[test]
    fn no_script_skips_a_check_or_asks_for_more_than_the_constants() {
        let folder = Path::new("/opt/omp");
        let version = Version::parse("18.15.0").unwrap();
        for text in [
            posix(Target::Install),
            powershell(Target::Install),
            update_script(Os::Linux, folder, &version).unwrap(),
            update_script(Os::Windows, Path::new(r"C:\omp"), &version).unwrap(),
        ] {
            for forbidden in [
                " -k ",
                "--insecure",
                "-SkipCertificateCheck",
                "-sk ",
                "| sh",
                "| bash",
                "iex",
                "Invoke-Expression",
            ] {
                assert!(!text.contains(forbidden), "{forbidden}");
            }
            assert!(!text.contains('\n'), "one line");
        }
        // The environment it reads is `PI_INSTALL_DIR`, `LOCALAPPDATA`, `HOME`, `TMPDIR`, `PATH`
        // and the processor variables.
        let ps = powershell(Target::Install);
        let mut variables: Vec<&str> = ps
            .match_indices("$env:")
            .map(|(at, _)| {
                let rest = &ps[at + 5..];
                &rest[..rest
                    .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                    .unwrap()]
            })
            .collect();
        variables.sort_unstable();
        variables.dedup();
        assert_eq!(
            variables,
            [
                "LOCALAPPDATA",
                "PI_INSTALL_DIR",
                "PROCESSOR_ARCHITECTURE",
                "PROCESSOR_ARCHITEW6432"
            ]
        );
    }

    #[test]
    fn the_texts_survive_any_shell_and_the_command_line_of_powershell() {
        let version = Version::parse("18.15.0").unwrap();
        // Apart from the folder, no `\\` or `\'`, which fish reads inside single quotes.
        let plain = update_script(Os::MacOs, Path::new("/opt/omp"), &version).unwrap();
        assert!(!plain.contains("\\\\") && !plain.contains("\\'"), "{plain}");
        let sh = update_script(Os::MacOs, Path::new("/o'neil/bin"), &version).unwrap();
        assert!(sh.starts_with("g_mode=update; g_dir='/o'\\''neil/bin'; g_version='18.15.0';"));
        assert!(sh.ends_with(" omp_github; exit $?"));
        let ps = update_script(Os::Windows, Path::new(r"C:\o'neil\bin"), &version).unwrap();
        assert!(
            !ps.contains('"'),
            "powershell.exe cannot carry a double quote"
        );
        assert!(ps.contains(r"$global:OmpDir = 'C:\o''neil\bin';"), "{ps}");
        assert!(ps.contains("$global:OmpVersion = '18.15.0';"));
        assert!(ps.ends_with(" Invoke-OmpGithub; exit $global:OmpCode"));
        // Only an absolute, safe folder is inserted.
        assert_eq!(
            update_script(Os::Linux, Path::new("bin"), &version),
            Err(TerminalError::NotAbsolute)
        );
        assert_eq!(
            update_script(Os::Windows, Path::new("C:\\a\"b"), &version),
            Err(TerminalError::UnsafePath)
        );
        assert_eq!(
            update_script(Os::Linux, Path::new("/a\nb"), &version),
            Err(TerminalError::UnsafePath)
        );
    }

    // --- Windows: the text of the PowerShell script ----------------------------------------------

    fn position(text: &str, needle: &str) -> usize {
        text.find(needle)
            .unwrap_or_else(|| panic!("missing in the script: {needle}"))
    }

    #[test]
    fn windows_picks_the_asset_from_the_processor_variables() {
        let ps = powershell(Target::Install);
        assert!(ps.starts_with("$ProgressPreference = 'SilentlyContinue';"));
        assert!(ps.contains(
            "$cpu = $env:PROCESSOR_ARCHITEW6432; if (-not $cpu) { $cpu = $env:PROCESSOR_ARCHITECTURE };"
        ));
        assert!(ps.contains("if ($cpu -eq 'AMD64') { $asset = 'omp-windows-x64.exe' };"));
        assert!(ps.contains("if ($cpu -eq 'ARM64') { $asset = 'omp-windows-arm64.exe' };"));
        assert!(ps.contains("'omp has no GitHub build for Windows ' + $cpu"));
        assert!(ps.contains("Stop-Omp 1 ('omp has no GitHub build"));
    }

    #[test]
    fn windows_follows_the_steps_in_order_with_the_limits_of_the_requirements() {
        let ps = powershell(Target::Install);
        let steps = [
            "'omp.exe.*.old'",
            "-Headers @{ Accept = 'application/vnd.github+json' } -TimeoutSec 60",
            "$j.tag_name",
            "-Uri $url -OutFile $dl -PassThru -TimeoutSec 900",
            "-like 'text/html*'",
            "(Get-Item -LiteralPath $dl).Length",
            "Get-FileHash -LiteralPath $dl -Algorithm SHA256",
            "Move-Item -LiteralPath $target -Destination $old -Force",
            "Move-Item -LiteralPath $dl -Destination $target -Force",
            "WaitForExit(15000)",
            "Stop-Omp 93 'The omp from GitHub does not answer --version",
            "GetEnvironmentVariable('Path', 'User')",
        ];
        let at: Vec<usize> = steps.iter().map(|s| position(&ps, s)).collect();
        assert!(at.windows(2).all(|w| w[0] < w[1]), "{at:?}");
        assert!(ps.contains("'.omp.download.exe'"));
        assert!(ps.contains("'omp.exe'"));
        assert!(
            ps.contains("$target + '.' + [DateTimeOffset]::UtcNow.ToUnixTimeSeconds() + '.old'")
        );
        assert!(ps.contains("-Uri $api -Headers"));
        assert!(ps.contains("-UseBasicParsing"));
        // Any error leaves no `.omp.download.exe` behind.
        assert!(ps.contains(
            "finally { if ($global:OmpDownload) { Remove-Item -LiteralPath $global:OmpDownload"
        ));
    }

    #[test]
    fn windows_gives_each_failure_its_exit_code() {
        let ps = powershell(Target::Install);
        for needle in [
            "Stop-Omp 90 ('The answer of ",
            "Stop-Omp 90 ('Could not download ",
            "Stop-Omp 92 ('The downloaded file has ",
            "Stop-Omp 92 ('The SHA-256 of the download (",
            "Stop-Omp 93 'The omp from GitHub does not answer --version",
            "Stop-Omp 94 ('omp ' + $global:OmpVersion + ' is not older than",
            "$global:OmpCode = 0",
            "if ($e.Exception -is [System.Net.WebException]) { return 90 };",
        ] {
            assert!(ps.contains(needle), "{needle}");
        }
        // The certificate errors of the rules, and the one of Windows, give `91`.
        let certificate_check = position(&ps, "return 91");
        let web_check = position(&ps, "return 90 };");
        assert!(certificate_check < web_check);
        for pattern in CERTIFICATE_CODES
            .iter()
            .chain(CERTIFICATE_PHRASES.iter())
            .chain(["trust relationship"].iter())
        {
            assert!(ps.contains(&format!("'{pattern}'")), "{pattern}");
        }
    }

    #[test]
    fn windows_adds_the_folder_to_the_user_path_once_and_only_for_an_installation() {
        let ps = powershell(Target::Install);
        // Compared ignoring case (`-ieq`) and a final `\`, on both sides.
        assert!(ps.contains(
            "foreach ($e in ([string]$userPath).Split(';')) { if ($e.Trim().TrimEnd('\\') -ieq $dir.TrimEnd('\\')) { $has = $true } };"
        ));
        assert!(ps.contains("if (-not $has) {"));
        assert!(ps.contains("[Environment]::SetEnvironmentVariable('Path', $newPath, 'User')"));
        assert!(ps.contains("$newPath = $userPath.TrimEnd(';') + ';' + $dir"));
        // The step is inside the installation's branch, after the omp was verified.
        let guard = position(&ps, "if ($global:OmpMode -eq 'install') { $userPath");
        assert!(guard > position(&ps, "Write-OmpLine ('omp ' + $tag + ' is in '"));
        // The user's `Path` is never printed: the message names only the folder.
        assert!(ps.contains("Write-OmpLine ('Added ' + $dir + ' to your user Path;"));
        assert!(!ps.contains("Write-OmpLine $userPath") && !ps.contains("$userPath +"));
    }

    #[test]
    fn windows_updates_only_when_the_release_is_newer_than_the_version_given() {
        let version = Version::parse("18.15.0").unwrap();
        let script = update_script(
            Os::Windows,
            Path::new(r"C:\Users\u\AppData\Local\omp"),
            &version,
        )
        .unwrap();
        assert!(script.contains("$global:OmpMode = 'update';"));
        let newer = position(&script, "if (-not (Test-OmpNewer $tag $global:OmpVersion))");
        // `94` comes before the asset is looked for and before any download.
        assert!(newer < position(&script, "$a = $null; foreach ($f in @($j.assets))"));
        assert!(newer < position(&script, "-OutFile $dl"));
        // The folder is the one given, not `PI_INSTALL_DIR`; no `Path` change.
        assert!(script.contains("} else { $dir = $global:OmpDir };"));
    }

    // --- macOS and Linux: the script against a local server --------------------------------------

    #[cfg(unix)]
    mod run_posix {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::os::unix::fs::PermissionsExt;
        use std::process::Command as Process;
        use std::time::Duration;

        use sha2::{Digest, Sha256};

        use super::*;

        const GOOD: &[u8] = b"#!/bin/sh\necho omp/99.0.0\n";
        const OLD: &str = "#!/bin/sh\necho omp/1.0.0\n";

        // The local server.

        struct Reply {
            status: u16,
            content_type: &'static str,
            body: Vec<u8>,
            /// Written in `Content-Length` instead of the length of `body`: a transfer cut short.
            declared_length: Option<usize>,
            delay: Duration,
        }

        impl Reply {
            fn new(status: u16, content_type: &'static str, body: impl Into<Vec<u8>>) -> Self {
                Self {
                    status,
                    content_type,
                    body: body.into(),
                    declared_length: None,
                    delay: Duration::ZERO,
                }
            }
        }

        struct Server {
            base: String,
            hits: Arc<Mutex<Vec<String>>>,
        }

        fn serve(route: impl Fn(&str, &str) -> Reply + Send + Sync + 'static) -> Server {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let hits = Arc::new(Mutex::new(Vec::new()));
            let route = Arc::new(route);
            let (seen, address) = (Arc::clone(&hits), base.clone());
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    let Ok(mut stream) = stream else { continue };
                    let (route, seen, address) =
                        (Arc::clone(&route), Arc::clone(&seen), address.clone());
                    std::thread::spawn(move || {
                        let mut request = Vec::new();
                        let mut buffer = [0u8; 1024];
                        while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                            match stream.read(&mut buffer) {
                                Ok(0) | Err(_) => return,
                                Ok(n) => request.extend_from_slice(&buffer[..n]),
                            }
                        }
                        let text = String::from_utf8_lossy(&request).into_owned();
                        let path = text.split_whitespace().nth(1).unwrap_or("/").to_owned();
                        seen.lock().unwrap().push(path.clone());
                        let reply = route(&address, &path);
                        std::thread::sleep(reply.delay);
                        let length = reply.declared_length.unwrap_or(reply.body.len());
                        let head = format!(
                            "HTTP/1.1 {} Reply\r\nContent-Type: {}\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n",
                            reply.status, reply.content_type
                        );
                        let _ = stream.write_all(head.as_bytes());
                        let _ = stream.write_all(&reply.body);
                    });
                }
            });
            Server { base, hits }
        }

        // What the server answers.

        #[derive(Clone, Copy, PartialEq)]
        enum Digest_ {
            Right,
            Wrong,
            Missing,
            Malformed,
        }

        #[derive(Clone)]
        struct Scenario {
            tag: &'static str,
            /// The answer of the API, instead of the release JSON.
            release: Option<(u16, &'static str, &'static str)>,
            listed: bool,
            digest: Digest_,
            size_delta: i64,
            binary: Vec<u8>,
            binary_reply: Option<(u16, &'static str)>,
            declared_length: Option<usize>,
            release_delay: Duration,
            binary_delay: Duration,
            compact: bool,
        }

        impl Default for Scenario {
            fn default() -> Self {
                Self {
                    tag: "v99.0.0",
                    release: None,
                    listed: true,
                    digest: Digest_::Right,
                    size_delta: 0,
                    binary: GOOD.to_vec(),
                    binary_reply: None,
                    declared_length: None,
                    release_delay: Duration::ZERO,
                    binary_delay: Duration::ZERO,
                    compact: false,
                }
            }
        }

        fn hex(bytes: &[u8]) -> String {
            format!("{:x}", Sha256::digest(bytes))
        }

        impl Scenario {
            fn release_json(&self, base: &str) -> String {
                let size = i64::try_from(self.binary.len()).unwrap() + self.size_delta;
                let mut entry = serde_json::json!({
                    "url": format!("{base}/api/asset"),
                    "id": 7,
                    "name": "omp-linux-x64",
                    "uploader": { "login": "bot", "type": "User", "site_admin": false },
                    "content_type": "application/octet-stream",
                    "size": size,
                    "download_count": 3,
                    "browser_download_url": format!("{base}/omp.bin"),
                });
                match self.digest {
                    Digest_::Right => {
                        entry["digest"] = format!("sha256:{}", hex(&self.binary)).into()
                    }
                    Digest_::Wrong => {
                        entry["digest"] = format!("sha256:{}", hex(b"something else")).into();
                    }
                    Digest_::Malformed => entry["digest"] = "sha256:1234".into(),
                    Digest_::Missing => {}
                }
                let decoy = serde_json::json!({
                    "name": "omp-windows-x64.exe",
                    "size": 5,
                    "digest": format!("sha256:{}", "0".repeat(64)),
                    "browser_download_url": format!("{base}/decoy.bin"),
                });
                let assets = if self.listed {
                    vec![decoy, entry]
                } else {
                    vec![decoy]
                };
                let release = serde_json::json!({
                    "tag_name": self.tag,
                    "name": self.tag,
                    // Text that looks like JSON, with escaped quotes and a backslash.
                    "body": "Notes {\"name\": \"omp-linux-x64\", \"size\": 1}, [1, 2] \\ done",
                    "assets": assets,
                });
                if self.compact {
                    serde_json::to_string(&release).unwrap()
                } else {
                    serde_json::to_string_pretty(&release).unwrap()
                }
            }

            fn server(&self) -> Server {
                let scenario = self.clone();
                serve(move |base, path| match path {
                    "/release" => {
                        let mut reply = match scenario.release {
                            Some((status, kind, body)) => Reply::new(status, kind, body),
                            None => Reply::new(
                                200,
                                "application/json; charset=utf-8",
                                scenario.release_json(base),
                            ),
                        };
                        reply.delay = scenario.release_delay;
                        reply
                    }
                    "/omp.bin" => {
                        let (status, kind) = scenario
                            .binary_reply
                            .unwrap_or((200, "application/octet-stream"));
                        let mut reply = Reply::new(status, kind, scenario.binary.clone());
                        reply.declared_length = scenario.declared_length;
                        reply.delay = scenario.binary_delay;
                        reply
                    }
                    _ => Reply::new(404, "text/plain", "not found"),
                })
            }
        }

        // The system the script runs on.

        struct Rig {
            root: tempfile::TempDir,
            stubs: PathBuf,
            tmp: PathBuf,
            dir: PathBuf,
        }

        struct Attempt {
            code: i32,
            stderr: String,
            hits: Vec<String>,
        }

        #[derive(Clone, Copy)]
        enum Mode {
            Install,
            Update,
        }

        impl Rig {
            fn new() -> Self {
                let root = tempfile::tempdir().unwrap();
                let base = root.path();
                let rig = Self {
                    stubs: base.join("stubs"),
                    tmp: base.join("tmp dir"),
                    dir: base.join("omp folder"),
                    root,
                };
                std::fs::create_dir_all(&rig.stubs).unwrap();
                std::fs::create_dir_all(&rig.tmp).unwrap();
                rig.system("Linux", "x86_64");
                rig
            }

            fn stub(&self, name: &str, body: &str) {
                let path = self.stubs.join(name);
                std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            }

            /// What `uname -s` and `uname -m` say.
            fn system(&self, kernel: &str, processor: &str) {
                self.stub(
                    "uname",
                    &format!("case \"$1\" in -s) echo {kernel};; -m) echo {processor};; esac"),
                );
            }

            fn put_omp(&self, text: &str) -> PathBuf {
                std::fs::create_dir_all(&self.dir).unwrap();
                let path = self.dir.join("omp");
                std::fs::write(&path, text).unwrap();
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
                path
            }

            fn config(&self, server: &Server) -> Config {
                Config {
                    api_url: format!("{}/release", server.base),
                    alpine_release: self.root.path().join("no-alpine").display().to_string(),
                    api_secs: 10,
                    download_secs: 10,
                    version_secs: 5,
                }
            }

            /// Runs `script` as the terminal would (`sh -c`) with the stubs in front of PATH.
            fn run(
                &self,
                script: &str,
                install_dir: Option<&Path>,
                path: &[&Path],
            ) -> (i32, String) {
                let mut folders: Vec<PathBuf> = vec![self.stubs.clone()];
                folders.extend(path.iter().map(|p| p.to_path_buf()));
                folders.extend([PathBuf::from("/usr/bin"), PathBuf::from("/bin")]);
                let mut process = Process::new("/bin/sh");
                process
                    .args(["-c", script])
                    .env_clear()
                    .env("PATH", std::env::join_paths(folders).unwrap())
                    .env("TMPDIR", &self.tmp)
                    .env("HOME", self.root.path().join("home"));
                if let Some(dir) = install_dir {
                    process.env("PI_INSTALL_DIR", dir);
                }
                let output = process.output().unwrap();
                (
                    output.status.code().unwrap(),
                    String::from_utf8_lossy(&output.stderr).into_owned(),
                )
            }

            fn attempt_with(
                &self,
                scenario: &Scenario,
                mode: Mode,
                tweak: impl FnOnce(&mut Config),
            ) -> Attempt {
                let server = scenario.server();
                let mut config = self.config(&server);
                tweak(&mut config);
                let folder = self.dir.display().to_string();
                let target = match mode {
                    Mode::Install => Target::Install,
                    Mode::Update => Target::Update {
                        folder: &folder,
                        version: "1.0.0",
                    },
                };
                let script = format!(
                    "{} omp_github; exit $?",
                    definitions(Shell::Posix, target, &config)
                );
                let (code, stderr) = self.run(&script, Some(&self.dir), &[]);
                let hits = server.hits.lock().unwrap().clone();
                Attempt { code, stderr, hits }
            }

            fn attempt(&self, scenario: &Scenario, mode: Mode) -> Attempt {
                self.attempt_with(scenario, mode, |_| {})
            }

            /// No file of the download is left behind.
            fn assert_clean(&self) {
                assert!(!self.dir.join(".omp.download").exists());
                let left: Vec<_> = std::fs::read_dir(&self.tmp).unwrap().collect();
                assert!(left.is_empty(), "{left:?}");
            }

            fn omp_text(&self) -> Option<String> {
                std::fs::read_to_string(self.dir.join("omp")).ok()
            }
        }

        // The asset.

        /// The asset the script picks, and what `uname` said, for a system made of stubs.
        fn asset_of(rig: &Rig, alpine: bool) -> (bool, String) {
            let mut config = Config::default();
            let alpine_file = rig.root.path().join("alpine-release");
            config.alpine_release = alpine_file.display().to_string();
            if alpine {
                std::fs::write(&alpine_file, "3.20\n").unwrap();
            }
            let script = format!(
                "{} g_find_asset; g_found=$?; printf '%s|%s|%s' \"$g_found\" \"$g_asset\" \"$g_sys $g_cpu\"",
                definitions(Shell::Posix, Target::Install, &config)
            );
            let mut process = Process::new("/bin/sh");
            process
                .args(["-c", &script])
                .env_clear()
                .env("PATH", format!("{}:/usr/bin:/bin", rig.stubs.display()));
            let output = process.output().unwrap();
            let text = String::from_utf8_lossy(&output.stdout).into_owned();
            let (found, rest) = text.split_once('|').unwrap();
            (found == "0", rest.to_owned())
        }

        fn rig_for(kernel: &str, processor: &str, sysctl: Option<&str>, ldd: Option<&str>) -> Rig {
            let rig = Rig::new();
            rig.system(kernel, processor);
            if let Some(answer) = sysctl {
                rig.stub("sysctl", &format!("echo {answer}"));
            }
            if let Some(text) = ldd {
                rig.stub("ldd", &format!("echo '{text}' >&2"));
            }
            rig
        }

        #[test]
        fn linux_picks_the_asset_from_uname() {
            for (processor, asset) in [
                ("x86_64", "omp-linux-x64"),
                ("amd64", "omp-linux-x64"),
                ("aarch64", "omp-linux-arm64"),
                ("arm64", "omp-linux-arm64"),
            ] {
                let rig = rig_for("Linux", processor, None, Some("ldd (GNU libc) 2.39"));
                assert_eq!(
                    asset_of(&rig, false),
                    (true, format!("{asset}|Linux {processor}")),
                    "{processor}"
                );
            }
        }

        #[test]
        fn linux_with_musl_takes_the_musl_asset() {
            // `ldd --version` mentions musl, ignoring case.
            for (processor, asset) in [
                ("x86_64", "omp-linux-musl-x64"),
                ("aarch64", "omp-linux-musl-arm64"),
            ] {
                let rig = rig_for(
                    "Linux",
                    processor,
                    None,
                    Some("musl libc (x86_64) Version 1.2.5"),
                );
                assert_eq!(
                    asset_of(&rig, false).1,
                    format!("{asset}|Linux {processor}")
                );
                let rig = rig_for("Linux", processor, None, Some("MUSL libc"));
                assert_eq!(
                    asset_of(&rig, false).1,
                    format!("{asset}|Linux {processor}")
                );
            }
            // `/etc/alpine-release` exists, whatever `ldd` says (or when there is no `ldd`).
            let rig = rig_for("Linux", "x86_64", None, Some("ldd (GNU libc) 2.39"));
            assert_eq!(asset_of(&rig, true).1, "omp-linux-musl-x64|Linux x86_64");
        }

        #[test]
        fn macos_asks_sysctl_so_rosetta_gets_the_native_asset() {
            // An arm64 Mac, and an x86_64 process under Rosetta on it: `hw.optional.arm64` is 1.
            for processor in ["arm64", "x86_64"] {
                let rig = rig_for("Darwin", processor, Some("1"), None);
                assert_eq!(
                    asset_of(&rig, false),
                    (true, format!("omp-darwin-arm64|Darwin {processor}"))
                );
            }
            // An Intel Mac: the key does not exist or is 0.
            for answer in ["0", "''"] {
                let rig = rig_for("Darwin", "x86_64", Some(answer), None);
                assert_eq!(asset_of(&rig, false).1, "omp-darwin-x64|Darwin x86_64");
            }
            let rig = rig_for("Darwin", "x86_64", None, None);
            rig.stub("sysctl", "exit 1");
            assert_eq!(asset_of(&rig, false).1, "omp-darwin-x64|Darwin x86_64");
        }

        #[test]
        fn an_unknown_processor_or_system_has_no_build_and_exits_with_1() {
            for (kernel, processor) in [
                ("Linux", "riscv64"),
                ("Linux", "i686"),
                ("FreeBSD", "amd64"),
            ] {
                let rig = rig_for(kernel, processor, None, None);
                assert!(!asset_of(&rig, false).0, "{kernel} {processor}");
                let attempt = rig.attempt(&Scenario::default(), Mode::Install);
                assert_eq!(attempt.code, exit::OTHER, "{kernel} {processor}");
                assert!(
                    attempt
                        .stderr
                        .contains(&format!("omp has no GitHub build for {kernel} {processor}")),
                    "{}",
                    attempt.stderr
                );
                // It stopped before asking GitHub for anything.
                assert!(attempt.hits.is_empty());
                rig.assert_clean();
            }
        }

        // The download.

        #[test]
        fn an_installation_puts_omp_in_the_folder_checks_it_and_hints_at_the_path() {
            let rig = Rig::new();
            let attempt = rig.attempt(&Scenario::default(), Mode::Install);
            assert_eq!(attempt.code, exit::INSTALLED, "{}", attempt.stderr);
            assert_eq!(attempt.hits, ["/release", "/omp.bin"]);
            assert_eq!(rig.omp_text().unwrap().as_bytes(), GOOD);
            let mode = std::fs::metadata(rig.dir.join("omp"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o755);
            rig.assert_clean();
            assert!(
                attempt
                    .stderr
                    .contains("GitHub release v99.0.0, asset omp-linux-x64"),
                "{}",
                attempt.stderr
            );
            // PATH does not hold the folder: the hint says how to add it, and no file of the
            // shell is touched (HOME is a folder that does not exist).
            let hint = format!(
                "Add {} to your PATH to use omp in your own terminal.",
                rig.dir.display()
            );
            assert!(attempt.stderr.contains(&hint), "{}", attempt.stderr);
            assert!(!rig.root.path().join("home").exists());
        }

        #[test]
        fn the_hint_is_not_printed_when_the_folder_is_in_the_path() {
            let rig = Rig::new();
            let server = Scenario::default().server();
            let script = format!(
                "{} omp_github; exit $?",
                definitions(Shell::Posix, Target::Install, &rig.config(&server))
            );
            let (code, stderr) = rig.run(&script, Some(&rig.dir), &[rig.dir.as_path()]);
            assert_eq!(code, 0, "{stderr}");
            assert!(!stderr.contains("to your PATH"), "{stderr}");
        }

        #[test]
        fn an_installation_without_pi_install_dir_uses_local_bin_of_the_home() {
            let rig = Rig::new();
            let server = Scenario::default().server();
            let script = format!(
                "{} omp_github; exit $?",
                definitions(Shell::Posix, Target::Install, &rig.config(&server))
            );
            let (code, stderr) = rig.run(&script, None, &[]);
            assert_eq!(code, 0, "{stderr}");
            let installed = rig.root.path().join("home/.local/bin/omp");
            assert_eq!(std::fs::read(installed).unwrap(), GOOD);
            // `PI_INSTALL_DIR` set to nothing is the same as not set.
            assert!(!rig.dir.exists());
        }

        #[test]
        fn an_update_replaces_the_omp_of_the_folder_when_the_release_is_newer() {
            for compact in [false, true] {
                let rig = Rig::new();
                rig.put_omp(OLD);
                let scenario = Scenario {
                    compact,
                    ..Scenario::default()
                };
                let attempt = rig.attempt(&scenario, Mode::Update);
                assert_eq!(attempt.code, 0, "{}", attempt.stderr);
                assert_eq!(rig.omp_text().unwrap().as_bytes(), GOOD);
                rig.assert_clean();
                // An update leaves the PATH alone.
                assert!(!attempt.stderr.contains("PATH"), "{}", attempt.stderr);
            }
        }

        #[test]
        fn an_update_exits_94_without_downloading_when_the_release_is_not_newer() {
            for tag in ["v1.0.0", "1.0.0", "v0.9.9", "v1.0.0-rc.1", "v1.0.0+build"] {
                let rig = Rig::new();
                rig.put_omp(OLD);
                let attempt = rig.attempt(
                    &Scenario {
                        tag,
                        ..Scenario::default()
                    },
                    Mode::Update,
                );
                assert_eq!(attempt.code, exit::NOT_NEWER, "{tag}: {}", attempt.stderr);
                assert_eq!(attempt.hits, ["/release"], "{tag}");
                assert_eq!(rig.omp_text().as_deref(), Some(OLD));
                rig.assert_clean();
            }
            // A tag that is not a version is not newer either.
            let rig = Rig::new();
            rig.put_omp(OLD);
            let attempt = rig.attempt(
                &Scenario {
                    tag: "nightly",
                    ..Scenario::default()
                },
                Mode::Update,
            );
            assert_eq!(attempt.code, exit::NOT_NEWER);
        }

        #[test]
        fn an_installation_has_no_version_to_compare() {
            // Installing the very version that is "installed" is fine: there is none to compare.
            let rig = Rig::new();
            let attempt = rig.attempt(
                &Scenario {
                    tag: "v0.0.1",
                    ..Scenario::default()
                },
                Mode::Install,
            );
            assert_eq!(attempt.code, 0, "{}", attempt.stderr);
        }

        #[test]
        fn the_semver_order_of_the_script_is_the_one_of_the_version_type() {
            let pairs = [
                ("v2.0.0", "1.9.9"),
                ("1.2.3", "1.2.3"),
                ("1.2.4", "1.2.3"),
                ("1.10.0", "1.9.0"),
                ("2.0.0", "2.0.0-rc.1"),
                ("2.0.0-rc.1", "2.0.0"),
                ("2.0.0-rc.2", "2.0.0-rc.1"),
                ("2.0.0-rc.10", "2.0.0-rc.9"),
                ("2.0.0-rc.1", "2.0.0-rc"),
                ("2.0.0-rc", "2.0.0-rc.1"),
                ("2.0.0-beta", "2.0.0-alpha"),
                ("2.0.0-1", "2.0.0-alpha"),
                ("2.0.0-alpha", "2.0.0-1"),
                ("1.0.0+a", "1.0.0+b"),
                ("18.16.0", "18.15.0"),
            ];
            let program = AWK_SEMVER
                .lines()
                .map(str::trim)
                .collect::<Vec<_>>()
                .join(" ");
            let code = |tag: &str, current: &str| {
                Process::new("awk")
                    .arg(&program)
                    .env("T", tag)
                    .env("C", current)
                    .status()
                    .unwrap()
                    .code()
                    .unwrap()
            };
            for (tag, current) in pairs {
                let expected =
                    i32::from(Version::parse(tag).unwrap() <= Version::parse(current).unwrap());
                assert_eq!(code(tag, current), expected, "{tag} against {current}");
            }
            // Not versions: `2`.
            for (tag, current) in [
                ("nightly", "1.0.0"),
                ("1.0.0", "x"),
                ("1.0", "1.0.0"),
                ("", "1.0.0"),
            ] {
                assert_eq!(code(tag, current), 2, "{tag} against {current}");
            }
        }

        /// A failure of the download: its exit code, and nothing of the earlier omp lost.
        fn assert_fails(scenario: &Scenario, expected: i32, text: &str) {
            for mode in [Mode::Update, Mode::Install] {
                let rig = Rig::new();
                let earlier = matches!(mode, Mode::Update).then(|| rig.put_omp(OLD));
                let attempt = rig.attempt(scenario, mode);
                assert_eq!(attempt.code, expected, "{}", attempt.stderr);
                assert!(attempt.stderr.contains(text), "{text}: {}", attempt.stderr);
                // The earlier omp is still there and still the same; nothing else was made.
                assert_eq!(
                    rig.omp_text().as_deref(),
                    earlier.map(|_| OLD),
                    "the earlier omp"
                );
                rig.assert_clean();
            }
        }

        #[test]
        fn a_status_other_than_200_from_the_api_is_blocked() {
            for status in [403, 404, 407, 500, 502] {
                assert_fails(
                    &Scenario {
                        release: Some((status, "application/json", "{\"message\":\"no\"}")),
                        ..Scenario::default()
                    },
                    exit::BLOCKED,
                    "/release",
                );
            }
        }

        #[test]
        fn an_html_page_or_something_else_for_the_release_is_blocked() {
            for body in [
                "<html><body>Blocked by your administrator</body></html>",
                "",
                "[]",
                "{\"tag_name\":",
                "plain text",
                "\"tag_name\"",
            ] {
                assert_fails(
                    &Scenario {
                        release: Some((200, "text/html", body)),
                        ..Scenario::default()
                    },
                    exit::BLOCKED,
                    "is not the release JSON",
                );
            }
        }

        #[test]
        fn a_release_without_the_asset_or_its_digest_is_blocked() {
            assert_fails(
                &Scenario {
                    listed: false,
                    ..Scenario::default()
                },
                exit::BLOCKED,
                "has no usable omp-linux-x64",
            );
            for digest in [Digest_::Missing, Digest_::Malformed] {
                assert_fails(
                    &Scenario {
                        digest,
                        ..Scenario::default()
                    },
                    exit::BLOCKED,
                    "has no usable omp-linux-x64",
                );
            }
        }

        #[test]
        fn a_status_other_than_200_or_an_html_page_for_the_file_is_blocked() {
            for status in [403, 404, 500] {
                assert_fails(
                    &Scenario {
                        binary_reply: Some((status, "text/html")),
                        ..Scenario::default()
                    },
                    exit::BLOCKED,
                    "curl exit 22",
                );
            }
            assert_fails(
                &Scenario {
                    binary_reply: Some((200, "text/html; charset=utf-8")),
                    binary: b"<html>Blocked</html>".to_vec(),
                    ..Scenario::default()
                },
                exit::BLOCKED,
                "answered an HTML page",
            );
        }

        #[test]
        fn a_file_of_another_size_is_not_installed() {
            for size_delta in [1, -1, 1000] {
                assert_fails(
                    &Scenario {
                        size_delta,
                        ..Scenario::default()
                    },
                    exit::DIFFERENT,
                    "GitHub publishes",
                );
            }
            // A transfer cut short, which curl reports itself.
            assert_fails(
                &Scenario {
                    declared_length: Some(GOOD.len() + 100),
                    ..Scenario::default()
                },
                exit::DIFFERENT,
                "curl exit 18",
            );
        }

        #[test]
        fn a_file_with_another_sha256_is_not_installed() {
            assert_fails(
                &Scenario {
                    digest: Digest_::Wrong,
                    ..Scenario::default()
                },
                exit::DIFFERENT,
                "is not the one GitHub publishes",
            );
            // The same size and a different content.
            let mut altered = GOOD.to_vec();
            altered[12] = b'X';
            let rig = Rig::new();
            let server = serve({
                let altered = altered.clone();
                move |base, path| {
                    let honest = Scenario::default();
                    match path {
                        "/release" => {
                            Reply::new(200, "application/json", honest.release_json(base))
                        }
                        _ => Reply::new(200, "application/octet-stream", altered.clone()),
                    }
                }
            });
            let script = format!(
                "{} omp_github; exit $?",
                definitions(Shell::Posix, Target::Install, &rig.config(&server))
            );
            let (code, stderr) = rig.run(&script, Some(&rig.dir), &[]);
            assert_eq!(code, exit::DIFFERENT, "{stderr}");
            assert!(rig.omp_text().is_none());
            rig.assert_clean();
        }

        #[test]
        fn an_omp_that_does_not_answer_version_is_not_kept() {
            for binary in [&b"#!/bin/sh\nexit 3\n"[..], &b"not a program at all"[..]] {
                assert_fails(
                    &Scenario {
                        binary: binary.to_vec(),
                        ..Scenario::default()
                    },
                    exit::NOT_RUNNING,
                    "does not answer --version",
                );
            }
        }

        #[test]
        fn the_earlier_omp_is_put_back_working_when_the_new_one_does_not_start() {
            let rig = Rig::new();
            let earlier = rig.put_omp(OLD);
            let before = std::fs::metadata(&earlier).unwrap().permissions().mode();
            let attempt = rig.attempt(
                &Scenario {
                    binary: b"#!/bin/sh\nexit 3\n".to_vec(),
                    ..Scenario::default()
                },
                Mode::Update,
            );
            assert_eq!(attempt.code, exit::NOT_RUNNING);
            let after = std::fs::metadata(&earlier).unwrap().permissions().mode();
            assert_eq!(before & 0o777, after & 0o777);
            let version = Process::new(&earlier).arg("--version").output().unwrap();
            assert_eq!(String::from_utf8_lossy(&version.stdout), "omp/1.0.0\n");
            rig.assert_clean();
        }

        #[test]
        fn an_omp_that_hangs_on_version_fails_after_its_limit() {
            let rig = Rig::new();
            rig.put_omp(OLD);
            let started = std::time::Instant::now();
            let attempt = rig.attempt_with(
                &Scenario {
                    binary: b"#!/bin/sh\nexec sleep 30\n".to_vec(),
                    ..Scenario::default()
                },
                Mode::Update,
                |config| config.version_secs = 1,
            );
            assert_eq!(attempt.code, exit::NOT_RUNNING, "{}", attempt.stderr);
            assert!(started.elapsed() < Duration::from_secs(20));
            assert_eq!(rig.omp_text().as_deref(), Some(OLD));
            rig.assert_clean();
        }

        #[test]
        fn a_slow_api_or_download_is_blocked_after_its_limit() {
            let slow_api = Scenario {
                release_delay: Duration::from_secs(4),
                ..Scenario::default()
            };
            let rig = Rig::new();
            rig.put_omp(OLD);
            let attempt = rig.attempt_with(&slow_api, Mode::Update, |config| config.api_secs = 1);
            assert_eq!(attempt.code, exit::BLOCKED, "{}", attempt.stderr);
            assert!(
                attempt.stderr.contains("curl exit 28"),
                "{}",
                attempt.stderr
            );
            assert_eq!(rig.omp_text().as_deref(), Some(OLD));
            rig.assert_clean();

            let slow_file = Scenario {
                binary_delay: Duration::from_secs(4),
                ..Scenario::default()
            };
            let rig = Rig::new();
            rig.put_omp(OLD);
            let attempt =
                rig.attempt_with(&slow_file, Mode::Update, |config| config.download_secs = 1);
            assert_eq!(attempt.code, exit::BLOCKED, "{}", attempt.stderr);
            assert!(
                attempt.stderr.contains("curl exit 28"),
                "{}",
                attempt.stderr
            );
            assert_eq!(rig.omp_text().as_deref(), Some(OLD));
            rig.assert_clean();
        }

        #[test]
        fn an_unreachable_host_is_blocked_and_a_certificate_error_is_91() {
            for (curl_exit, expected) in [
                (6, exit::BLOCKED),
                (7, exit::BLOCKED),
                (28, exit::BLOCKED),
                (35, exit::BLOCKED),
                (60, exit::CERTIFICATE),
                (77, exit::CERTIFICATE),
                (2, exit::OTHER),
                (23, exit::OTHER),
            ] {
                let rig = Rig::new();
                rig.stub(
                    "curl",
                    &format!("echo 'curl: ({curl_exit}) it failed' >&2; exit {curl_exit}"),
                );
                let server = Scenario::default().server();
                let script = format!(
                    "{} omp_github; exit $?",
                    definitions(Shell::Posix, Target::Install, &rig.config(&server))
                );
                let (code, stderr) = rig.run(&script, Some(&rig.dir), &[]);
                assert_eq!(code, expected, "curl {curl_exit}: {stderr}");
                // The step, the address and the tool's message are printed.
                assert!(stderr.contains("Could not read"), "{stderr}");
                assert!(
                    stderr.contains(&format!("curl: ({curl_exit}) it failed")),
                    "{stderr}"
                );
                assert!(stderr.contains("/release"), "{stderr}");
                rig.assert_clean();
            }
        }

        #[test]
        fn the_script_never_turns_a_check_off_when_it_runs() {
            // The calls of curl, as the stub saw them.
            let rig = Rig::new();
            let calls = rig.root.path().join("calls");
            rig.stub(
                "curl",
                &format!("printf '%s\\n' \"$*\" >> '{}'; exit 6", calls.display()),
            );
            let server = Scenario::default().server();
            let script = format!(
                "{} omp_github; exit $?",
                definitions(Shell::Posix, Target::Install, &rig.config(&server))
            );
            rig.run(&script, Some(&rig.dir), &[]);
            let seen = std::fs::read_to_string(calls).unwrap();
            assert!(
                seen.contains("-H Accept: application/vnd.github+json"),
                "{seen}"
            );
            assert!(seen.contains("--max-time 10"), "{seen}");
            assert!(!seen.contains("--insecure") && !seen.split_whitespace().any(|w| w == "-k"));
        }

        #[test]
        fn the_real_limits_are_the_ones_of_the_requirements() {
            let sh = posix(Target::Install);
            assert!(sh.contains(
                "--connect-timeout 10 --max-time 60 -H 'Accept: application/vnd.github+json'"
            ));
            assert!(sh.contains("curl -fL -sS --connect-timeout 10 --max-time 900 -o"));
            assert!(sh.contains("( sleep 15; kill -9"));
            assert!(sh.contains("g_dl=$g_dir/.omp.download;"));
            assert!(sh.contains("sha256sum") && sh.contains("shasum -a 256"));
        }
    }

    // --- the update: when it runs and what it gives ----------------------------------------------

    type Answer = Result<Output, Failure>;

    /// An [`Exec`] that answers `omp update`, `omp --version` and the script apart, and records
    /// what it was asked to run.
    struct Scripted {
        update: Answer,
        version: Answer,
        script: Answer,
        certificates_on: bool,
        calls: Mutex<Vec<(PathBuf, Vec<String>, Duration)>>,
    }

    impl Scripted {
        fn new(script: Answer) -> Self {
            Self {
                update: Err(Failure::Exit {
                    code: Some(1),
                    stderr: "could not fetch\nupdate failed\n".to_owned(),
                }),
                version: Ok(Output {
                    stdout: "omp/18.15.0\n".to_owned(),
                    stderr: String::new(),
                }),
                script,
                certificates_on: false,
                calls: Mutex::new(Vec::new()),
            }
        }

        fn exit(code: i32, stderr: &str) -> Answer {
            Err(Failure::Exit {
                code: Some(code),
                stderr: stderr.to_owned(),
            })
        }

        /// What ran besides `omp update` and `omp --version`.
        fn scripts(&self) -> Vec<(PathBuf, Vec<String>, Duration)> {
            self.calls
                .lock()
                .unwrap()
                .iter()
                .filter(|(_, args, _)| args != &["update"] && args != &["--version"])
                .cloned()
                .collect()
        }
    }

    impl Exec for Scripted {
        fn run(&self, program: &Path, args: &[&str], limit: Duration) -> Answer {
            let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
            self.calls
                .lock()
                .unwrap()
                .push((program.to_path_buf(), args.clone(), limit));
            if args == ["update"] {
                self.update.clone()
            } else if args == ["--version"] {
                self.version.clone()
            } else {
                self.script.clone()
            }
        }

        fn system_certificates_on(&self) -> bool {
            self.certificates_on
        }
    }

    /// A system with omp in the default folder of the installer, `PI_INSTALL_DIR` unset.
    struct Home {
        _root: tempfile::TempDir,
        env: LookupEnv,
        omp: PathBuf,
    }

    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "x").unwrap();
    }

    fn home(os: Os) -> Home {
        let root = tempfile::tempdir().unwrap();
        let base = dunce::canonicalize(root.path()).unwrap();
        let env = LookupEnv {
            os,
            path: std::ffi::OsString::new(),
            pi_install_dir: None,
            bun_install: None,
            home: Some(base.join("home")),
            local_app_data: Some(base.join("local")),
        };
        let omp = match os {
            Os::Windows => base.join("local").join("omp").join("omp.exe"),
            _ => base.join("home").join(".local").join("bin").join("omp"),
        };
        touch(&omp);
        Home {
            _root: root,
            env,
            omp,
        }
    }

    #[test]
    fn omp_update_that_works_never_runs_the_download() {
        let home = home(Os::Linux);
        let exec = Scripted {
            update: Ok(Output {
                stdout: String::new(),
                stderr: String::new(),
            }),
            ..Scripted::new(Scripted::exit(0, ""))
        };
        let run = update(&home.omp, "", &home.env, &exec);
        assert_eq!(
            run,
            UpdateRun {
                result: Ok(()),
                download: None
            }
        );
        assert!(exec.scripts().is_empty());
        assert_eq!(exec.calls.lock().unwrap().len(), 1);
    }

    // The tests that reach the download need the folder of `home(Os::Linux)`, a real temporary
    // folder, to be absolute for `/bin/sh`: only on a Unix host.
    #[test]
    #[cfg(unix)]
    fn after_a_failed_omp_update_the_download_runs_hidden_for_the_folder_with_omps_version() {
        let home = home(Os::Linux);
        let exec = Scripted::new(Ok(Output {
            stdout: String::new(),
            stderr: "GitHub release v18.16.0, asset omp-linux-x64\n".to_owned(),
        }));
        let run = update(&home.omp, "", &home.env, &exec);
        assert_eq!(run.result, Ok(()));
        let download = run.download.unwrap();
        assert_eq!(download.outcome, DownloadOutcome::Installed);
        assert_eq!(download.exit_code, Some(0));
        assert_eq!(download.tag.as_deref(), Some("v18.16.0"));
        assert_eq!(download.asset.as_deref(), Some("omp-linux-x64"));
        assert_eq!(download.reason, None);

        let scripts = exec.scripts();
        assert_eq!(scripts.len(), 1);
        let (program, args, limit) = &scripts[0];
        assert_eq!(program, Path::new("/bin/sh"));
        assert_eq!(args[0], "-c");
        assert_eq!(args.len(), 2);
        assert_eq!(*limit, Duration::from_secs(15 * 60));
        let folder = home.omp.parent().unwrap().display().to_string();
        assert!(args[1].starts_with(&format!(
            "g_mode=update; g_dir='{folder}'; g_version='18.15.0';"
        )));
        assert!(args[1].ends_with(" omp_github; exit $?"));
        // `omp update` ran first, then omp's version was read.
        let all = exec.calls.lock().unwrap().clone();
        assert_eq!(all[0].1, ["update"]);
        assert_eq!(all[1].1, ["--version"]);
    }

    #[test]
    fn on_windows_the_download_runs_in_a_hidden_powershell() {
        let version = Version::parse("18.15.0").unwrap();
        let folder = Path::new(r"C:\Users\u\AppData\Local\omp");
        let (program, args) = background_command(Os::Windows, folder, &version).unwrap();
        assert!(program.ends_with("powershell.exe"), "{program:?}");
        assert_eq!(
            args[..5],
            [
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command"
            ]
        );
        assert_eq!(args.len(), 6);
        assert!(args[5].contains(r"$global:OmpDir = 'C:\Users\u\AppData\Local\omp';"));
        assert!(args[5].contains("$global:OmpMode = 'update';"));
        assert!(args[5].ends_with(" Invoke-OmpGithub; exit $global:OmpCode"));
        // The others: `/bin/sh -c <script>`.
        let (program, args) =
            background_command(Os::MacOs, Path::new("/Users/u/.local/bin"), &version).unwrap();
        assert_eq!(program, Path::new("/bin/sh"));
        assert_eq!(args[0], "-c");
        assert_eq!(args.len(), 2);
        // A folder that is not absolute on the system is not inserted.
        assert_eq!(
            background_command(Os::Windows, Path::new("/Users/u"), &version),
            Err(TerminalError::NotAbsolute)
        );
    }

    #[test]
    #[cfg(unix)]
    fn the_exit_code_decides_between_update_ended_and_update_failed_and_its_reason() {
        let home = home(Os::Linux);
        let own = "update failed".to_owned();
        // `0`: UPDATE_ENDED. `94`: UPDATE_FAILED with the reason of `omp update`.
        let ended = update(
            &home.omp,
            "",
            &home.env,
            &Scripted::new(Ok(Output {
                stdout: String::new(),
                stderr: String::new(),
            })),
        );
        assert_eq!(ended.result, Ok(()));
        let not_newer = update(
            &home.omp,
            "",
            &home.env,
            &Scripted::new(Scripted::exit(94, "omp 18.15.0 is not older")),
        );
        assert_eq!(not_newer.result, Err(own.clone()));
        let download = not_newer.download.unwrap();
        assert_eq!(download.outcome, DownloadOutcome::NotNewer);
        assert_eq!(download.exit_code, Some(94));
        assert_eq!(download.reason, None);
        // Any other code: the reason of the download for that code, for the setting as it is.
        for certificates_on in [false, true] {
            for code in [90, 91, 92, 93, 1, 2, 137] {
                let mut exec = Scripted::new(Scripted::exit(code, "boom"));
                exec.certificates_on = certificates_on;
                let run = update(&home.omp, "", &home.env, &exec);
                let reason = download_failed_reason(DownloadFailure::Exit(code), certificates_on);
                assert_eq!(run.result, Err(reason.clone()), "{code}");
                let download = run.download.unwrap();
                assert_eq!(
                    download.outcome,
                    DownloadOutcome::Failed(DownloadFailure::Exit(code))
                );
                assert_eq!(download.reason, Some(reason));
            }
        }
        // The time limit.
        let run = update(
            &home.omp,
            "",
            &home.env,
            &Scripted::new(Err(Failure::TimedOut {
                limit: Duration::from_secs(900),
            })),
        );
        assert_eq!(
            run.result,
            Err(download_failed_reason(DownloadFailure::TimedOut, false))
        );
        let download = run.download.unwrap();
        assert_eq!(
            download.outcome,
            DownloadOutcome::Failed(DownloadFailure::TimedOut)
        );
        assert_eq!(download.exit_code, None);
        // A shell that did not start, or was ended by a signal, is a failure of any other kind.
        for answer in [
            Err(Failure::NotStarted),
            Err(Failure::Exit {
                code: None,
                stderr: String::new(),
            }),
        ] {
            let run = update(&home.omp, "", &home.env, &Scripted::new(answer));
            assert_eq!(
                run.result,
                Err(download_failed_reason(DownloadFailure::Exit(1), false))
            );
        }
    }

    #[test]
    #[cfg(unix)]
    fn an_omp_whose_version_cannot_be_read_is_older_than_any_release() {
        let home = home(Os::Linux);
        let mut exec = Scripted::new(Scripted::exit(0, ""));
        exec.version = Ok(Output {
            stdout: "garbage\n".to_owned(),
            stderr: String::new(),
        });
        let _ = update(&home.omp, "", &home.env, &exec);
        assert!(exec.scripts()[0].1[1].contains("g_version='0.0.0';"));
    }

    #[test]
    #[cfg(unix)]
    fn the_download_is_tried_only_for_an_omp_in_the_folder_of_its_installer() {
        let failing = || Scripted::new(Scripted::exit(0, ""));
        let own = Err("update failed".to_owned());

        // `ompPath` set (even to the same file): never.
        let home1 = home(Os::Linux);
        let setting = home1.omp.display().to_string();
        let exec = failing();
        assert_eq!(update(&home1.omp, &setting, &home1.env, &exec).result, own);
        assert_eq!(
            update(&home1.omp, "  ", &home1.env, &failing()).result,
            Ok(())
        );
        assert!(exec.scripts().is_empty());

        // Bun's folder, a package manager's folder: never.
        let mut other = home(Os::Linux);
        let bun = other
            .env
            .home
            .clone()
            .unwrap()
            .join(".bun")
            .join("bin")
            .join("omp");
        touch(&bun);
        let exec = failing();
        assert_eq!(update(&bun, "", &other.env, &exec).result, own);
        let brew = other
            .env
            .home
            .clone()
            .unwrap()
            .join("homebrew")
            .join("bin")
            .join("omp");
        touch(&brew);
        assert_eq!(update(&brew, "", &other.env, &exec).result, own);
        // A subfolder of the installer's folder is not "directly in" it.
        let nested = other.omp.parent().unwrap().join("nested").join("omp");
        touch(&nested);
        assert_eq!(update(&nested, "", &other.env, &exec).result, own);
        assert!(exec.scripts().is_empty());

        // `PI_INSTALL_DIR` is the installer's folder too; the default one then is as well.
        let custom = other.env.home.clone().unwrap().join("custom").join("omp");
        touch(&custom);
        other.env.pi_install_dir = Some(custom.parent().unwrap().to_path_buf());
        let exec = failing();
        assert_eq!(update(&custom, "", &other.env, &exec).result, Ok(()));
        assert_eq!(update(&other.omp, "", &other.env, &exec).result, Ok(()));
        assert_eq!(exec.scripts().len(), 2);
        let folders: Vec<String> = exec
            .scripts()
            .iter()
            .map(|(_, args, _)| args[1].split('\'').nth(1).unwrap().to_owned())
            .collect();
        assert_eq!(
            folders,
            [
                custom.parent().unwrap().display().to_string(),
                other.omp.parent().unwrap().display().to_string()
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_resolved_before_the_folder_is_compared() {
        let failing = || Scripted::new(Scripted::exit(0, ""));
        // A link in the installer's folder to a file elsewhere: that file is not the installer's.
        let home = home(Os::Linux);
        let elsewhere = home.env.home.clone().unwrap().join("opt").join("omp");
        touch(&elsewhere);
        std::fs::remove_file(&home.omp).unwrap();
        std::os::unix::fs::symlink(&elsewhere, &home.omp).unwrap();
        let exec = failing();
        assert!(update(&home.omp, "", &home.env, &exec).result.is_err());
        assert!(exec.scripts().is_empty());
        // A link elsewhere to the file in the installer's folder: it is.
        let real = home.env.home.clone().unwrap().join("real").join("omp");
        touch(&real);
        let mut env = home.env.clone();
        env.pi_install_dir = Some(real.parent().unwrap().to_path_buf());
        let link = home.env.home.clone().unwrap().join("links").join("omp");
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let exec = failing();
        assert_eq!(update(&link, "", &env, &exec).result, Ok(()));
        assert!(exec.scripts()[0].1[1]
            .contains(&format!("g_dir='{}'", real.parent().unwrap().display())));
    }

    #[test]
    fn on_windows_a_bunx_file_next_to_omp_keeps_the_download_away() {
        let home = home(Os::Windows);
        assert_eq!(
            fallback_folder("", &home.omp, &home.env),
            Some(home.omp.parent().unwrap().to_path_buf())
        );
        touch(&home.omp.with_extension("bunx"));
        assert_eq!(fallback_folder("", &home.omp, &home.env), None);
        // Elsewhere a `.bunx` file means nothing.
        let unix = home_unix_with_bunx();
        assert!(fallback_folder("", &unix.omp, &unix.env).is_some());
    }

    fn home_unix_with_bunx() -> Home {
        let home = home(Os::Linux);
        touch(&home.omp.with_extension("bunx"));
        home
    }

    #[test]
    fn an_omp_that_cannot_be_resolved_gets_no_download() {
        let home = home(Os::Linux);
        let gone = home.omp.with_file_name("gone");
        let exec = Scripted::new(Scripted::exit(0, ""));
        assert!(update(&gone, "", &home.env, &exec).result.is_err());
        assert!(exec.scripts().is_empty());
    }

    #[test]
    fn only_the_installers_folders_are_listed_and_bun_comes_after_them_for_the_lookup() {
        let home = home(Os::Linux);
        let mut env = home.env.clone();
        env.pi_install_dir = Some(PathBuf::from("/pi"));
        let home_dir = env.home.clone().unwrap();
        assert_eq!(
            env.install_dirs(),
            [PathBuf::from("/pi"), home_dir.join(".local").join("bin")]
        );
        assert_eq!(
            env.installer_dirs(),
            [
                PathBuf::from("/pi"),
                home_dir.join(".local").join("bin"),
                home_dir.join(".bun").join("bin")
            ]
        );
    }

    /// The certificate variables reach the background download: the real runner, a `curl` that
    /// is a script in Weaver's own folder, and an omp that fails to update.
    #[cfg(unix)]
    #[test]
    fn the_background_download_gets_the_certificate_variables() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().unwrap();
        let base = dunce::canonicalize(root.path()).unwrap();
        let local_bin = base.join("local-bin");
        let folder = base.join("installer");
        std::fs::create_dir_all(&local_bin).unwrap();
        std::fs::create_dir_all(&folder).unwrap();
        let seen = base.join("seen");
        let curl = local_bin.join("curl");
        std::fs::write(
            &curl,
            format!(
                "#!/bin/sh\nprintf '%s|%s|%s\\n' \"$SSL_CERT_FILE\" \"$CURL_CA_BUNDLE\" \"$PI_REQUEST_COMPRESSION\" >> '{}'\nexit 60\n",
                seen.display()
            ),
        )
        .unwrap();
        let omp = folder.join("omp");
        std::fs::write(
            &omp,
            "#!/bin/sh\ncase \"$1\" in update) echo 'could not fetch' >&2; exit 1;; --version) echo omp/18.15.0;; esac\n",
        )
        .unwrap();
        for file in [&curl, &omp] {
            std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let env = LookupEnv {
            os: Os::current(),
            path: std::ffi::OsString::new(),
            pi_install_dir: Some(folder.clone()),
            bun_install: None,
            home: Some(base.join("home")),
            local_app_data: None,
        };
        let certificates = |enabled: bool| -> crate::omp::run::CertificateSource {
            Arc::new(move || Certificates {
                enabled,
                variables: if enabled {
                    vec![
                        ("SSL_CERT_FILE", "/bundle.pem".into()),
                        ("CURL_CA_BUNDLE", "/bundle.pem".into()),
                        ("PI_REQUEST_COMPRESSION", "0".into()),
                    ]
                } else {
                    Vec::new()
                },
            })
        };
        let runner = |enabled: bool| {
            crate::omp::run::SystemExec::new(local_bin.clone(), None, "/usr/bin:/bin".into())
                .with_certificates(certificates(enabled))
        };

        let run = update(&omp, "", &env, &runner(true));
        // curl said `60`: a certificate the bundle did not cover, `91`, explained for "on".
        assert_eq!(
            run.result,
            Err(download_failed_reason(DownloadFailure::Exit(91), true))
        );
        assert_eq!(
            std::fs::read_to_string(&seen).unwrap(),
            "/bundle.pem|/bundle.pem|0\n"
        );
        // With the setting off no variable is added, and the reason says so.
        let run = update(&omp, "", &env, &runner(false));
        assert_eq!(
            run.result,
            Err(download_failed_reason(DownloadFailure::Exit(91), false))
        );
    }
}
