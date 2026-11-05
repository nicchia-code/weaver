# Desktop

The developer's operating system around Weaver: it starts the app, delivers the clicks on Weaver's system notifications, tells when a window comes to the foreground and holds the certificate authorities the system trusts.

## Events

### APP_STARTED
Weaver starts, at login or because the developer launched it.

### NOTIFICATION_CLICKED
The developer clicked a system notification Weaver showed.

### WINDOW_FOCUSED
A project window came back to the foreground.

### CERTIFICATES_EXPORTED
The certificate authorities the system trusts were read within the time limit and Weaver wrote them, with the public root authorities, into the bundle.

### CERTIFICATES_EXPORT_FAILED
The certificate authorities could not be read, none was found, the time limit passed, or the bundle could not be written, with the reason.
