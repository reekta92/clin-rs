# Backup Dashboard

The Backup dashboard provides a Git-based version control interface for your notes vault. It allows you to track changes, stage files, commit updates, and view your vault's history directly within the TUI.

---

## Overview

The Backup view (`ViewMode::Backup`) is designed to keep your notes synchronized and versioned. It integrates directly with Git to provide a familiar workflow for managing your vault's state.

**Source:** `src/backup/` — modules: `app`, `git_ops`, `input`, `render`, `state`, `worker`

---

## Interface Layout

The dashboard is divided into three main sections:

### 1. Status Section
Located at the top-left, this section shows the current state of your vault using explicit Git-style staging workflows:
- **Staged Changes**: Files ready to be committed. Stage files individually or all at once.
- **Unstaged Changes**: Modified files not yet staged.
- **Untracked Files**: New files not yet tracked by Git.

### 2. History Section
Located at the bottom-left, this section lists the commit history of your vault. Selecting a commit allows you to see its details and the changes it introduced.

### 3. Diff Preview Pane
Located on the right side, the preview pane shows the diff for the currently selected file in the Status section or the changes in the selected commit from the History section. It uses standard Git diff formatting to highlight additions and removals.

---

## Core Interactions

### Staging and Committing
- **Stage file**: Select a file from unstaged or untracked changes and press the stage shortcut to move it to staged changes.
- **Unstage file**: Select a file from staged changes and press the unstage shortcut to return it to unstaged changes.
- **Stage all changes**: Use a single action to stage all modified and untracked files at once.
- **Pull from remote**: Fetch and merge the latest changes from the configured remote repository.
- **Commit Mode**: Pressing the commit shortcut opens an input mode where you can enter a commit message. Confirming the message creates a new commit with the staged changes.

### Settings and Automation
The Backup view includes a settings popup (`EditSettings`) for configuring automation and remote synchronization:
- **Auto-backup on Save**: Automatically create a commit whenever a note is saved.
- **Auto-backup on Quit**: Ensure all changes are committed when exiting the application.
- **Auto-push**: Automatically push commits to a remote repository.
- **Remote Sync**: Configure the `remote_url` and `remote_name` (default: `origin`) for synchronizing with platforms like GitHub or GitLab.

---

## Configuration

Backup settings can be configured in your `clin.toml` under the `[backup]` section:

```toml
[backup]
enabled = true
backup_on_save = true
backup_on_quit = true
auto_push = false
remote_url = "https://github.com/user/my-notes.git"
remote_name = "origin"
```

For a full list of options, see the [Configuration Reference](CONFIG_REFERENCE.md).
