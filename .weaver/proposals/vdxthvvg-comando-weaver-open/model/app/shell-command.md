# Shell command

The `weaver` command in the developer's own terminal, with which he opens a folder in the running Weaver or shows its welcome window. Weaver already keeps its own copy of the command line for the agent, renewed at every start; installing makes that copy reachable from the terminals the developer opens afterwards, and removing undoes exactly what the installation did. Weaver never installs or removes it by itself. The state is what Weaver finds on the computer each time the settings show it, so a change made outside Weaver shows there too; an update of Weaver keeps an installed command working.

## States

### Not installed (initial)
The developer's terminal does not reach Weaver's command line through Weaver's installation.
- `developer.INSTALL_COMMAND` → Installed: Weaver makes its command line reachable from the terminals opened afterwards and says where; on macOS the system may first ask for an administrator password; on Linux, when the folder used may not be among the places the terminal looks for commands, Weaver warns.
- `developer.INSTALL_COMMAND` → Not installed: when something that is not Weaver's command already takes its place; nothing is touched and Weaver says which file is there.
- `developer.INSTALL_COMMAND` → Not installed: when the developer cancels the system's request for an administrator password; nothing changes and nothing is shown.
- `developer.INSTALL_COMMAND` → Not installed: when the installation fails; nothing is left behind and Weaver says why.

### Installed
The developer's terminal reaches Weaver's command line through Weaver's installation.
- `developer.REMOVE_COMMAND` → Not installed: Weaver undoes what its installation made; terminals opened afterwards no longer find the command through it.
- `developer.REMOVE_COMMAND` → Not installed: when what takes its place is no longer Weaver's command; nothing is touched and Weaver says which file is there.
- `developer.REMOVE_COMMAND` → Installed: when the developer cancels the system's request for an administrator password; nothing changes and nothing is shown.
- `developer.REMOVE_COMMAND` → Installed: when the removal fails; nothing changes and Weaver says why.
