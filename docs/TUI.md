# TUI Reference

BTC Miner features a k9s-style terminal UI with vim keybindings, log filtering, and a help overlay.

## Quick Reference

Press `?` at any time to see the help overlay with all available shortcuts.

## General

| Key           | Action                          |
| ------------- | ------------------------------- |
| `?`           | Toggle help overlay             |
| `:`           | Enter command mode              |
| `Ctrl+C`      | Quit immediately                |
| `p`           | Pause/Resume mining             |
| `Space`       | Pause/Resume mining             |
| `l`           | Focus logs (full screen)        |
| `Shift+L`     | Toggle Bitcoin Core logs        |

## Navigation

| Key           | Action                          |
| ------------- | ------------------------------- |
| `j` / `↓`     | Scroll down                     |
| `k` / `↑`     | Scroll up                       |
| `g`           | Go to top                       |
| `G`           | Go to bottom                    |
| `PgUp`        | Page up                         |
| `PgDn`        | Page down                       |
| `Home`        | Go to top                       |
| `End`         | Go to bottom                    |

## Focused Logs Mode

Press `l` to enter focused logs mode (full screen logs).

| Key           | Action                          |
| ------------- | ------------------------------- |
| `Esc`         | Unfocus (return to dashboard)   |
| `s`           | Toggle autoscroll               |
| `Shift+C`     | Clear logs                      |
| `Shift+L`     | Switch between Miner/Core logs  |

### Time Filters

Filter logs by time (k9s-style):

| Key | Filter         |
| --- | -------------- |
| `0` | Tail (last 100)|
| `1` | Last 1 minute  |
| `2` | Last 5 minutes |
| `3` | Last 15 minutes|
| `4` | Last 30 minutes|
| `5` | Last 1 hour    |
| `6` | All logs       |

The active filter is shown in the title bar: `Logs (123) [5m]`

## Command Mode

Press `:` to enter command mode (vim-style).

| Command  | Action          |
| -------- | --------------- |
| `:q`     | Quit            |
| `:quit`  | Quit            |
| `:clear` | Clear all logs  |

Press `Esc` to cancel command mode.

## Status Indicators

The header shows real-time status:

```
BTC Miner: v0.1.0
Mode:      True Solo (direct)
Status:    Connected (8 peers)
Compute:   24 CPU (SHA-NI) + 3 GPU(s)
Web:       http://localhost:8080
```

- **Mode**: Solo, Pool (ckpool/braiins/ocean), or SV2
- **Status**: Connected/Syncing/Disconnected with peer count
- **Compute**: CPU threads with SIMD level + GPU count

## Focused Logs Shortcut Bar

When logs are focused, a k9s-style shortcut bar appears at the bottom:

```
<0> tail <1> 1m <2> 5m <3> 15m <4> 30m <5> 1h <6> all
<esc> Unfocus  <Shift+c> Clear  <s> AutoScroll  <Shift+l> Core    Autoscroll:On
```

The active time filter is highlighted in yellow.
