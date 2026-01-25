**Zed discord presence** is an extension for [Zed](https://zed.dev) that adds support for [Discord Rich Presence](https://discord.com/developers/docs/rich-presence/how-to) using LSP

Using LSP is a workaround for now (yeah, it's a bit hacky) but once Zed has proper extension support, I'll rewrite it.

### Requirements

[rust](https://rust-lang.org) is required for installing this extension. \
The easiest way to get [rust](https://rust-lang.org) is by using [rustup](https://rustup.rs)

### How to install?

Since [zed-industries/extensions#1217](https://github.com/zed-industries/extensions/pull/1217) has been merged, you can simply download the extension in <kbd>zed: extensions</kbd>.
Don't forget to give at least a ⭐ if you like this project :D

<details>
<summary>Dev installation</summary>

1. Clone this repository
2. <kbd>CTRL</kbd> + <kbd>SHIFT</kbd> + <kbd>P</kbd> and select <kbd>zed: install dev extension</kbd>
3. Choose the directory where you cloned this repository
4. Enjoy :)

</details>

### WSL Guide

If you're using Zed on Windows with WSL, this extension runs within WSL and therefore can't access Discord's Windows IPC socket. You can create a bridge that forwards connections from WSL to Windows.

<details>
<summary>WSL Configuration</summary>

1. On Windows, download [npiperelay](https://github.com/jstarks/npiperelay/releases), extract the `.zip`, and place `npiperelay.exe` in a directory such as `C:/npiperelay`.
2. On WSL, install socat: `yay -S socat` for Arch, `sudo apt-get install socat` for Ubuntu/Debian.
3. Run `echo $XDG_RUNTIME_DIR` and confirm it returns a path (usually `/run/user/1000`). This is where the Discord IPC socket will be created. If it's empty, enable systemd in WSL by adding the following to `/etc/wsl.conf` and restarting WSL:

```ini
[boot]
systemd=true
```

4. Create a bridge script, such as `~/scripts/discord-ipc-bridge.sh`:

```sh
#!/bin/bash
SOCKET_PATH="${XDG_RUNTIME_DIR}/discord-ipc-0"

# Deletes the existing socket file if it exists
rm -f "$SOCKET_PATH"

socat UNIX-LISTEN:"$SOCKET_PATH",fork \
  EXEC:"/mnt/c/npiperelay/npiperelay.exe -ep -s //./pipe/discord-ipc-0",nofork
```

5. Make the script executable: `chmod +x ~/scripts/discord-ipc-bridge.sh`
6. Run the script in the background: `~/scripts/discord-ipc-bridge.sh &`
7. Open Zed. The presence should now display.

To start the bridge automatically, add this to your `.bashrc` or `.zshrc`:

```sh
if ! pgrep -f "discord-ipc-bridge" > /dev/null; then
  ~/scripts/discord-ipc-bridge.sh &
fi
```

If the presence stops displaying, restart the bridge:

```sh
pkill -f "discord-ipc-bridge" && ~/scripts/discord-ipc-bridge.sh &
```

</details>

## How to configure?

You can configure state, details and git integration by changing Discord Presence LSP settings. This can be done in <kbd>zed: open settings</kbd> with following configuration:

### Application ID

The `application_id` is required for the rich presence to work. It should be kept as is unless you have a specific reason to change it.

```jsonc
"application_id": "1263505205522337886"
```

### Base Icons URL

The `base_icons_url` is the base URL for all language icons. This URL points to the location where the icons are stored.

```jsonc
"base_icons_url": "https://raw.githubusercontent.com/xhyrom/zed-discord-presence/main/assets/icons/"
```

### State

The `state` option allows you to set the state message displayed in Discord. You can use [placeholders](#available-placeholders) like `{filename}` and `{line_number}` which will be replaced with the current file name and line number.

```jsonc
"state": "Working on {filename}:{line_number}"
```

### Details

The `details` option allows you to set the details message displayed in Discord. You can use [placeholders](#available-placeholders) like `{workspace}` which will be replaced with the current workspace name.

```jsonc
"details": "In {workspace}"
```

### Large Image

The `large_image` option specifies the URL for the large image displayed in Discord. You can use [placeholders](#available-placeholders) like `{base_icons_url}` and `{language}` which will be replaced accordingly.
The `:lo` modifier is used to convert the language name to lowercase.

```jsonc
"large_image": "{base_icons_url}/{language:lo}.png"
```

### Large Text

The `large_text` option specifies the text displayed when hovering over the large image. You can use [placeholders](#available-placeholders) like `{language}` with the `:u` modifier to capitalize the first letter.

```jsonc
"large_text": "{language:u}"
```

### Small Image

The `small_image` option specifies the URL for the small image displayed in Discord. You can use [placeholders](#available-placeholders) here as well.

```jsonc
"small_image": "{base_icons_url}/zed.png"
```

### Small Text

The `small_text` option specifies the text displayed when hovering over the small image. You can use [placeholders](#available-placeholders) here as well.

```jsonc
"small_text": "Zed"
```

### Idle Settings

The `idle` settings configure the behavior when you are inactive.

The `timeout` specifies the idle timeout in seconds (300 seconds = 5 minutes).

The `action` determines what happens when you go idle:

- `change_activity` changes the activity to idle with the specified details
- `clear_activity` hides the activity

The `state`, `details`, `large_image`, `large_text`, `small_image`, and `small_text` options specify the messages and images to display when idle. All of these can use [placeholders](#available-placeholders).

```jsonc
"idle": {
  "timeout": 300,
  "action": "change_activity",
  "state": "Idling",
  "details": "In Zed",
  "large_image": "{base_icons_url}/zed.png",
  "large_text": "Zed",
  "small_image": "{base_icons_url}/idle.png",
  "small_text": "Idle"
}
```

### Rules

The `rules` option allows you to disable presence in specific workspaces. The `mode` can be set to `blacklist`
or `whitelist`, and the `paths` array should contain the absolute paths to apply the rule to.

```jsonc
"rules": {
  "mode": "blacklist",
  "paths": ["absolute path"]
}
```

### Git Integration

The `git_integration` option enables or disables Git integration. When enabled, the extension
will display a button to open the Git repository.

```jsonc
"git_integration": true
```

### Per-Language Configuration

The `languages` field allows you to override the default activity settings for specific languages.
Each key must be the **language name in lowercase**, and the value is an object containing options `state`, `details`, `large_image`, `large_text`, `small_image` and `small_text`. All of these can use [placeholders](#available-placeholders).

```jsonc
"languages": {
  "rust": {
    "state": "Hacking on {filename}",
    "details": "Rustacean at work",
    "large_image": "{base_icons_url}/rust.png",
    "large_text": "RUST !!!!",
    "small_image": "{base_icons_url}/zed.png",
    "small_text": "Zed"
  },
  "python": {
    // haha i'm cool
    "large_image": "{base_icons_url}/c.png",
    "large_text": "C"
  }
}
```

If a language is not specified in the `languages` map, the default top-level `activity` settings will be used instead.

### Example Configuration

```jsonc
{
  "lsp": {
    "discord_presence": {
      "initialization_options": {
        // Application ID for the rich presence (don't touch it unless you know what you're doing)
        "application_id": "1263505205522337886",
        // Base URL for all language icons
        "base_icons_url": "https://raw.githubusercontent.com/xhyrom/zed-discord-presence/main/assets/icons/",

        "state": "Working on {filename}",
        "details": "In {workspace}",
        // URL for the large image
        "large_image": "{base_icons_url}/{language:lo}.png", // :lo lowercase the language name
        "large_text": "{language:u}", // :u capitalizes the first letter
        // URL for the small image
        "small_image": "{base_icons_url}/zed.png",
        "small_text": "Zed",

        // Idle settings - when you're inactive
        "idle": {
          "timeout": 300, // Idle timeout in seconds (300 seconds = 5 minutes)

          // Action to take when idle
          // `change_activity` - changes the activity to idle with the following details
          // `clear_activity` - clears the activity (hides it)
          "action": "change_activity",

          "state": "Idling",
          "details": "In Zed",
          "large_image": "{base_icons_url}/zed.png",
          "large_text": "Zed",
          "small_image": "{base_icons_url}/idle.png",
          "small_text": "Idle",
        },

        // Rules to disable presence in specific workspaces
        "rules": {
          "mode": "blacklist", // Can also be "whitelist"
          "paths": ["absolute path"],
        },

        "git_integration": true,

        // Per-language overrides
        "languages": {
          "rust": {
            "state": "Hacking on {filename}",
            "details": "Rustacean at work",
          },
        },
      },
    },
  },
}
```

You can also set any option to `null` to unset it, except for `base_icons_url`, `rules`, and `git_integration`.

### Available Placeholders

You can use the following placeholders in your configuration:

- `{filename}` - Current file name (e.g., "main.rs")
- `{workspace}` - Current workspace name (e.g., "my-project")
- `{language}` - Programming language (e.g., "rust")
- `{base_icons_url}` - Base URL for icons (from configuration)
- `{relative_file_path}` - File path relative to workspace root (e.g., "src/main.rs")
- `{folder_and_file}` - Parent directory and file name (e.g., "src/main.rs")
- `{directory_name}` - Name of parent directory (e.g., "src")
- `{full_directory_name}` - Full path of parent directory (e.g., "/home/user/project/src")

- `{line_number}` - Current line number (e.g., "42")

> [!WARNING]  
> The line number might not always be accurate or update instantly due to LSP limitations. Updates usually happen on file edits, saves, or specific cursor interactions.

- `{git_branch}` - Current git branch name (e.g., "main")
- `{file_size}` - Current file size (e.g., "1.2 KB")

Modifiers can be applied to any placeholder except `{line_number}`:

- `:u` - Capitalizes the first letter (e.g., `{language:u}` → "Rust")
- `:lo` - Converts to lowercase (e.g., `{language:lo}` → "rust")

Example: `"Working on {filename} in {directory_name:u}"`
