# System Certificates

Whether omp trusts the certificate authorities the developer's system trusts, one for the whole app. It is a setting of Weaver, off by default. While it is on, Weaver exports those authorities, together with the public root authorities, into one bundle for the developer, outside every project, and every omp process Weaver starts afterwards is told to trust that bundle: jobs, General, the questions Weaver asks omp about itself, omp's update and logout, the background download of omp from GitHub, and the system terminals of omp's installer and logins. While it is off, or while no bundle exists, those processes start exactly as without it. A change reaches only the processes started after it.

An export never blocks the app, a window or a job, and ends within a time limit; a failed export keeps the earlier bundle in use. Each export, successful or not, makes Weaver look at omp again.

Wherever Weaver shows why omp failed, an error about a server's certificate or an HTTP refusal is explained in plain words: a corporate proxy that inspects HTTPS traffic is the likely cause and, while the setting is off, turning it on may fix it. No error text names a product; only the notice of the automatic turn-on names Zscaler.

## States

### Off (initial)
The setting is off. Nothing is exported, and a bundle left from earlier stays on disk unused.
- `desktop.APP_STARTED` → Ready: when the setting is on and the bundle was written less than a day ago.
- `desktop.APP_STARTED` → Exporting: when the setting is on and the bundle is missing or was written more than a day ago; Weaver's first look at omp waits for this export to end.
- `desktop.APP_STARTED` → Exporting: only on macOS and Windows, when the setting is off, Zscaler is installed and Weaver never turned the setting on by itself. Weaver turns it on, remembers that it did, and tells the developer once, with a system notification and a banner in every open window, both leading to Settings; Weaver's first look at omp waits for this export to end. If the setting cannot be written, nothing is remembered or shown, and the next start tries again. Looking for Zscaler never delays the start, and a failure to look shows nothing.
- `developer.CHANGE_SETTINGS` → Exporting: only when the developer turns the setting on.

### On
The setting is on. omp processes started from now on receive the bundle while one exists.
- `developer.CHANGE_SETTINGS` → Off: only when the developer turns the setting off; omp processes started afterwards get no certificate variables, and an export still running is ignored when it ends. Weaver never turns the setting on by itself again.

#### Exporting (initial)
Weaver reads the authorities the system trusts and writes the bundle whole, replacing the earlier one only when it succeeds. omp processes started meanwhile receive the earlier bundle, or none. A second request to export while one runs starts nothing.
- `desktop.CERTIFICATES_EXPORTED` → Ready: Weaver says how many certificates the bundle holds and when it was written.
- `desktop.CERTIFICATES_EXPORT_FAILED` → Failed: the earlier bundle stays in use, if one exists. When the export started by itself (at the app's start, after Weaver turned the setting on, or daily), a banner in every open window says it failed, with a way to open Settings; when the developer asked for it, only Settings says so.

#### Ready
The bundle exists and was written by the last export.
- `developer.REFRESH_SYSTEM_CERTIFICATES` → Exporting
- `time.CERTIFICATES_REFRESH_DUE` → Exporting

#### Failed
The last export failed. Weaver says why, and whether an earlier bundle is still in use.
- `developer.REFRESH_SYSTEM_CERTIFICATES` → Exporting
- `time.CERTIFICATES_REFRESH_DUE` → Exporting

## Events

### AUTO_ENABLED
Emitted when Weaver turns the setting on by itself because Zscaler is installed.

### EXPORT_ENDED
Emitted when an export ends, successful or not, while the setting is on.
