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

The `state` option allows you to set the state message displayed in Discord. The placeholder `{filename}` will be replaced with the current file name.

```jsonc
"state": "Working on {filename}"
```

### Details

The `details` option allows you to set the details message displayed in Discord. The placeholder `{workspace}` will be replaced with the current workspace name.

```jsonc
"details": "In {workspace}"
```

### Large Image

The `large_image` option specifies the URL for the large image displayed in Discord. The placeholders `{base_icons_url}` and `{language}` will be replaced accordingly.
The `:lo` modifier is used to convert the language name to lowercase.

```jsonc
"large_image": "{base_icons_url}/{language:lo}.png"
```

### Large Text

The `large_text` option specifies the text displayed when hovering over the large image. The `:u` modifier capitalizes the first letter of the language name.

```jsonc
"large_text": "{language:u}"
```

### Small Image

The `small_image` option specifies the URL for the small image displayed in Discord.

```jsonc
"small_image": "{base_icons_url}/zed.png"
```

### Small Text

The `small_text` option specifies the text displayed when hovering over the small image.

```jsonc
"small_text": "Zed"
```

### Idle Settings

The `idle` settings configure the behavior when you are inactive.

The `timeout` specifies the idle timeout in seconds (300 seconds = 5 minutes).

The `action` determines what happens when you go idle:

- `change_activity` changes the activity to idle with the specified details
- `clear_activity` hides the activity

The `state`, `details`, `large_image`, `large_text`, `small_image`, and `small_text` options specify the messages and images to display when idle.

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
      },
    },
  },
}
```

You can also set any option to `null` to unset it, except for `base_icons_url`, `rules`, and `git_integration`.
