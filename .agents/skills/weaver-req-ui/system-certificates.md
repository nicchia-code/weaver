# System Certificates

## system-certificates

- **Switch:** title `System Certificates`, a shadcn `Switch` (Radix `@radix-ui/react-switch`), the first boolean setting of the Settings view; saved at once like the other settings. Description (≤160 characters, from the settings schema): `omp trusts the certificates your system trusts, for proxies that inspect HTTPS. Replaces NODE_EXTRA_CA_CERTS, SSL_CERT_FILE and similar variables.`
- **Places:** one component, last in the `omp` group of Settings, and under the omp status box wherever that box shows outside Settings (Get Started and the welcome window). Every place shows the same value and state, and a change in one shows in all at once.
- **Status line:** only while the switch is on, one line of secondary text under the description, an icon on the left and `Refresh` at the right end:
  - exporting: `Loader` and `Exporting system certificates…`;
  - ready: `CircleCheck` (success) and `<N> certificates, exported <date and time>.`;
  - failed with an earlier bundle: `TriangleAlert` (warning) and `Could not export the system certificates (<reason>). Using the ones exported <date and time>.`;
  - failed without a bundle: `TriangleAlert` (warning) and `Could not export the system certificates (<reason>). omp gets no certificates from Weaver.`
  - `<date and time>` is when the bundle in use was written: `HH:MM` today, else `D MMM HH:MM`. `<reason>` is one of those of `requirements/tech#system-certificates` and `no certificate authority found`.
  - The line is the switch's description for assistive technology (`aria-describedby`) and its changes are announced (`aria-live="polite"`). `Loader` is static under reduced motion.
- **Refresh:** `outline` small button, `RefreshCw` icon, text `Refresh`, accessible name `Refresh system certificates`. Disabled while exporting, with the tooltip `Exporting system certificates…`. Below ~300 px it wraps under the text, left-aligned; nothing overlaps.
- **Turn-on notice:** the system notification has the title `System Certificates turned on` and the message `Weaver turned on System Certificates because Zscaler is installed: omp and the agent's commands now trust the certificates your system trusts.`
  - The same text shows in a banner at the top of every window, project and welcome, including windows opened later in the same run, with `Open Settings` (project windows only; it shows the Settings view) and an icon-only `X` (accessible name and tooltip `Close`).
  - Closing it in one window, or clicking `Open Settings`, closes it in every window. It never shows again after Weaver quits.
- **Export failure banner:** after a failed automatic export, a banner at the top of every window, warning colour with `TriangleAlert`, with the text of the failed status line, `Open Settings` (project windows only) and `X`. A newer failure replaces it; a successful export or turning the setting off removes it; closing it in one window closes it in every window.
- **Proxy explanations,** English, with no final full stop so that each place can embed them; nothing else is added to them:
  - certificate, setting off: `omp could not verify a server's certificate; a corporate proxy that inspects HTTPS traffic is the usual cause, and turning on System Certificates in Weaver's settings may fix it`;
  - certificate, setting on: `omp could not verify a server's certificate, even with System Certificates on; a corporate proxy that inspects HTTPS traffic is the usual cause`;
  - HTTP, setting off: `omp received an HTTP <code> error; a corporate proxy that inspects HTTPS traffic may be blocking the request, and turning on System Certificates in Weaver's settings may fix it`;
  - HTTP, setting on: `omp received an HTTP <code> error; a corporate proxy that inspects HTTPS traffic may be blocking the request`.
- **Where they replace the reason:** the Missing text of the omp status box (with a final `.`), the `(<reason>)` of a failed update in Missing and Ready, `Could not log out of <name>: <reason>.`, the line saying why the model list is unavailable in Settings, a job's `Failed: <reason>.` (exit, start, opening and turn failures), Chat's `omp stopped: <reason>.` and its failed-turn line, and `Not delivered: <reason>.`. `Show details`, where it exists, keeps omp's own lines.
- **Failed installer,** in the Missing row of the omp status box, by the exit code of `requirements/tech#omp-setup`:
  - `90`: `The omp installer failed (exit code 90): the download of omp was blocked on both of its download sites, probably by a corporate proxy that filters downloads. See the terminal.`
  - `91`: `The omp installer failed (exit code 91): omp could not verify a server's certificate; a corporate proxy that inspects HTTPS traffic is the usual cause. See the terminal.`, followed with the setting off by ` Turning on System Certificates in Weaver's settings may fix it.`
  - `92`: `The omp installer failed (exit code 92): the downloaded omp was incomplete or altered, so it was not installed; a proxy may have changed it. See the terminal.`
  - `93`: `The omp installer failed (exit code 93): the downloaded omp does not start. See the terminal.`
  - any other code: `The omp installer failed (exit code <N>); see the terminal. A corporate proxy that inspects HTTPS traffic is a common cause.`, followed with the setting off by ` Turning on System Certificates in Weaver's settings may fix it.`
- **Failed download of an update,** as the `(<reason>)` of a failed update in Missing and Ready, with no final full stop:
  - `90`: `the download was blocked, probably by a corporate proxy that filters downloads`;
  - `91`: the certificate explanation above, for the setting as it is then;
  - `92`: `the downloaded omp was incomplete or altered`;
  - `93`: `the downloaded omp did not start`;
  - any other code: `the download of omp failed (exit code <N>)`; past the time limit: `the download of omp timed out`.
- No error text names a product; only the turn-on notice and its banner name Zscaler.
