# App

The desktop app around the projects: one process for every open project, one window per project, and the system notifications that bring the developer back. A project window shows, read-only, the Board with what waits for a decision and the Tasks still to finish, the Requirements, the Specs, the History, the list of the agent's jobs with their conversations, General, and Weaver's settings; every change to the Model and to the work goes through the developer's decisions and the agent's jobs. The Requirements and the Specs can show an open Proposal applied over the current ones, and can focus on what a Proposal or a Task concerns. Next to them, the File view (Feature `files`) lets the developer browse the project's files and change them, and the Notes view (Feature `notes`) holds his own notes on the project.

The window keeps its project in sight: it names the project and the branch git is on, and lets the developer switch to another project, open or recent, which opens in its own window or comes to the front. It also keeps in sight how much is left of the usage limits of the model providers and who the developer is for git. From anywhere in the window a palette finds a view, an action, a project, a card, a section of the Requirements or a History entry, and goes to it or runs it exactly as its own control would, with the same confirmations and the same reasons for refusing; it never searches the project's files.

Weaver stays next to the developer's own editor: wherever a file and a line are known, it opens them in the editor chosen in the settings.

The settings belong to the developer, not to a project: they live in the system's configuration folder and apply to every project; a project's files never change them.
