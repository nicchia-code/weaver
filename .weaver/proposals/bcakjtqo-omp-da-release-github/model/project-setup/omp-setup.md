# omp setup

Whether omp is ready for Weaver to start it, one for the whole app. Weaver looks for omp where the settings say, or else in the places its official installer uses, and asks its version and its usable models. A job created while omp is not ready waits; no job and no General conversation starts until it is. A result that comes back for a place of omp that is no longer the current one is ignored. Every omp process, download of omp and system terminal started here receives System Certificates while it is on, and a failure about a certificate or an HTTP refusal is explained as System Certificates says.

When omp's own channel fails, because a corporate proxy blocks the site it downloads from, Weaver falls back on omp's releases published on GitHub, which carry the same program: for an installation and for an update alike, with no setting to choose. A file from GitHub replaces omp only when its size and checksum match those GitHub publishes and it answers once in place.

## States

### Checking (initial)
Weaver asks omp about itself, when the app starts and whenever something may have changed it. At the app's start, while System Certificates exports its bundle, Weaver waits for that export to end before asking, and says so.
- `omp.READY` → Ready
- `omp.MISSING` → Missing
- `omp.OUTDATED` → Updating: Weaver runs omp's own update.
- `omp.NO_PROVIDER` → Not connected

### Missing
omp cannot be found or does not answer. Weaver offers to install it. After a failed installation, Weaver says so with its exit code and what it means until the developer starts the installation again or omp is found.
- `developer.INSTALL_OMP` → Installing: Weaver opens the system terminal, which downloads omp's official installer for the developer's system and runs it; when the installer fails, the same terminal downloads omp for the developer's system and processor from its latest GitHub release instead, checks it, puts it in the folder where the official installer would and, on Windows, adds that folder to the developer's own path when missing. The developer sees everything that happens.
- `developer.CHANGE_SETTINGS` → Checking: only when the place of omp changed or System Certificates was turned off.
- `system-certificates.EXPORT_ENDED` → Checking

### Installing
The official installer runs in the system terminal, followed by the download from GitHub when it fails. When the terminal ends, it leaves the exit code of the installation where Weaver can read it; a terminal closed before the end leaves none. Weaver looks at omp, and at that exit code, again every few seconds.
- `time.RECHECK_DUE` → Installing: Weaver asks omp about itself again.
- `omp.MISSING` → Installing: the installation has not finished yet, or finished well and omp is not found yet.
- `omp.INSTALL_FAILED` → Missing: omp is still not found; Weaver shows the exit code with its cause: the download blocked, probably by a corporate proxy; a server's certificate that could not be verified, with, while System Certificates is off, that turning it on may fix it; a file from GitHub that is incomplete or altered, which Weaver did not install; an omp from GitHub that does not start. For any other cause Weaver says that a corporate proxy that inspects HTTPS traffic is a common cause and, while System Certificates is off, that turning it on may fix it.
- `omp.READY` → Ready
- `omp.OUTDATED` → Updating
- `omp.NO_PROVIDER` → Not connected
- `developer.INSTALL_OMP` → Installing: opens the installation again, forgetting the exit code of the earlier one.
- `developer.CHANGE_SETTINGS` → Checking: only when the place of omp changed.

### Updating
omp updates itself. When omp's own update fails and omp is a file in one of its installer's folders, not installed through a package manager nor chosen in the settings, Weaver downloads omp's latest GitHub release in the background, with no terminal, only when it is newer than omp, checks it and puts it in omp's place; on Windows the file in use is moved aside first, so that running jobs continue. Jobs created meanwhile wait for the update to end.
- `omp.UPDATE_ENDED` → Checking
- `omp.UPDATE_FAILED` → Ready: when omp still answers with the minimum version and a usable model; the failure is shown, the one of the download from GitHub when it ran, and never blocks a job.
- `omp.UPDATE_FAILED` → Missing: when omp is still older than the minimum; it counts as missing and Weaver offers its installation, with the failure, the one of the download from GitHub when it ran.

### Not connected
omp answers but has no usable model. Weaver lists the providers omp can log in to and offers to connect one.
- `developer.LOG_IN_PROVIDER` → Logging in: Weaver opens the system terminal with omp's login for that provider.
- `developer.CHANGE_SETTINGS` → Checking: only when the place of omp changed or System Certificates was turned off.
- `system-certificates.EXPORT_ENDED` → Checking

### Logging in
omp's login for a provider runs in the system terminal; Weaver never reads what the developer types there and looks at omp again every few seconds.
- `time.RECHECK_DUE` → Logging in: Weaver asks omp about its models again.
- `omp.NO_PROVIDER` → Logging in: the login has not finished yet.
- `omp.READY` → Ready
- `omp.MISSING` → Missing
- `developer.LOG_IN_PROVIDER` → Logging in: opens the login again, for the same or another provider.

### Ready
omp answers with the minimum version and at least one usable model. Weaver shows its version, its models and which providers are connected.
- `developer.LOG_IN_PROVIDER` → Logging in
- `developer.LOG_OUT_PROVIDER` → Checking: omp forgets only the credentials it stored for that provider; if the provider is still connected afterwards, Weaver says that other credentials of the developer's environment still connect it.
- `developer.CHANGE_SETTINGS` → Checking: only when the place of omp changed or System Certificates was turned off.
- `system-certificates.EXPORT_ENDED` → Checking
- `time.UPDATE_CHECK_DUE` → Ready: Weaver asks omp in the background whether a newer version exists, never waiting for the answer to start a job.
- `omp.UPDATE_AVAILABLE` → Updating: the update runs before the next job starts; jobs already working continue.
- `omp.MISSING` → Missing: omp disappeared, for example because it was uninstalled.
