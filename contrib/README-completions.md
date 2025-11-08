# Dynamic Tmux Completions for nabi

The `nabi-completions-dynamic.zsh` file provides runtime-aware tab completion for tmux commands by querying live tmux sessions, windows, and panes.

## Installation

The dynamic completion file is automatically injected into the static `_nabi` completion file during `make install`. No manual sourcing is required - it's integrated into the generated completion script.

## Features

### `nabi tmux send-prompt <TAB>`
- Completes with all available panes in format `session:window.pane` or `session:window`
- Queries live tmux sessions at completion time
- **Performance**: Uses 2-second TTL cache for fast repeated completions
- **XDG Compliant**: Cache stored in `$XDG_CACHE_HOME/nabi/` or `~/.cache/nabi/`

### `nabi tmux list windows <TAB>`
- Completes with all available tmux session names

### `nabi tmux list panes <TAB>`
- First `<TAB>`: Completes with all available tmux session names
- Second `<TAB>`: After selecting a session, completes with window numbers for that session

## How It Works

The dynamic completion hooks into zsh's completion system by overriding the `_nabi` completion function. When completing `nabi tmux send-prompt`, it:

1. Detects the completion context using `$CURRENT` and `words` array
2. Calls `tmux list-panes -a -s` directly (optimized, bypasses `nabi` CLI overhead)
3. Extracts unique `session:window` pairs and full `session:window.pane` targets
4. Uses string manipulation (faster than regex) to process results
5. Caches results for 2 seconds to improve performance on repeated completions
6. Provides the results as completion options

### Performance Optimizations

- **Direct tmux calls**: Bypasses `nabi tmux list` overhead, calls `tmux` directly
- **Single query**: Uses `tmux list-panes -a -s` to fetch all panes in one call
- **String manipulation**: Uses `${pane%.*}` instead of regex for extraction
- **2-second cache**: Reduces completion time from ~1s to ~0.1s on cached hits
- **Conditional debug logging**: Only logs when `_NABI_DEBUG=1` is set

### Cache Location

Cache files are stored in XDG-compliant locations:
- **Cache data**: `$XDG_CACHE_HOME/nabi/tmux-completion-cache.txt` (or `~/.cache/nabi/`)
- **Cache timestamp**: `$XDG_CACHE_HOME/nabi/tmux-completion-cache-time.txt`
- **Debug log**: `$XDG_CACHE_HOME/nabi/completion-debug.log` (when `_NABI_DEBUG=1`)

Cache has a 2-second TTL to balance freshness and performance.

## Troubleshooting

If completions don't work:

1. **Check installation**: Verify completions are installed:
   ```bash
   ls -la ~/.cache/zsh/completions/_nabi
   ```

2. **Check tmux**: Make sure you have tmux sessions running:
   ```bash
   tmux list-sessions
   ```

3. **Clear cache**: If completions seem stale, clear the cache:
   ```bash
   rm ~/.cache/nabi/tmux-completion-cache*
   ```

4. **Enable debug logging**: Set debug flag and check logs:
   ```zsh
   export _NABI_DEBUG=1
   # Try completion, then check:
   tail -f ~/.cache/nabi/completion-debug.log
   ```

5. **Test manually**: Try calling the completion functions directly:
   ```zsh
   _nabi_get_cached_targets
   ```
