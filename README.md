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

```json
{
  "lsp": {
    "discord_presence": {
      "initialization_options": {
        // Base url for all language icons
        "base_icons_url": "https://raw.githubusercontent.com/xhyrom/zed-discord-presence/main/assets/icons/",

        "state": "Working on {filename}",
        "details": "In {workspace}",
        // URL for large image
        "large_image": "{base_icons_url}/{language}.png",
        "large_text": "{language:u}", // :u makes first letter upper-case
        // URL for small image
        "small_image": "{base_icons_url}/zed.png",
        "small_text": "Zed",

        // Rules - disable presence in some workspaces
        "rules": {
          "mode": "blacklist", // or whitelist
          "paths": [
            "absolute path"
          ]
        },

        "git_integration": true
      }
    }
  }
}
```

You can also use `null` to unset the option. Possible for everything except `base_icons_url`, `rules` and `git_integration`
