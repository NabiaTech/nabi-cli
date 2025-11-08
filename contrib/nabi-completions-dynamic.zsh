#compdef nabi
# Dynamic Tmux Completion Enhancement for nabi
#
# This file adds runtime-aware completion to nabi CLI by querying tmux and nabi itself.
# It's loaded after the base _nabi completion to provide:
# - Session names for `nabi tmux send-prompt` and `nabi tmux list windows/panes`
# - Window lists for context-aware selection
# - Pane targets in full session:window.pane format
#
# This hooks into the clap-generated completions by overriding the _nabi function
# to add dynamic completions for specific arguments.

# Debug log file
_NABI_COMPLETION_DEBUG_LOG="${HOME}/.nabi/cache/completion-debug.log"

# Debug logging function
_nabi_debug_log() {
    mkdir -p "${HOME}/.nabi/cache" 2>/dev/null
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" >> "$_NABI_COMPLETION_DEBUG_LOG"
}

# Define completion function for pane argument
# This will be called when zsh completes the pane argument
_nabi_tmux_send_prompt_pane_completion() {
    _nabi_debug_log "_nabi_tmux_send_prompt_pane_completion called"
    _nabi_tmux_send_prompt_pane
}

# Hook into the main _nabi completion function to override pane argument
if (( $+functions[_nabi] )); then
    # Store original _nabi function
    if (( ! $+functions[_nabi_original] )); then
        functions[_nabi_original]=$functions[_nabi]
    fi

    # Override _nabi to replace :pane: with our completion function
    _nabi() {
        _nabi_debug_log "_nabi called: context=$curcontext, CURRENT=$CURRENT, words=${words[@]}"

        # Check if we're completing pane argument for send-prompt
        if [[ "${words[1]}" == "nabi" ]] && \
           [[ "${words[2]}" == "tmux" ]] && \
           [[ "${words[3]}" == "send-prompt" ]] && \
           [[ $CURRENT -eq 4 ]]; then
            _nabi_debug_log "Detected send-prompt pane completion, calling our function"
            _nabi_tmux_send_prompt_pane
            return 0
        fi

        # For other cases, call original
        _nabi_original "$@"
    }
fi

# Cache file for tmux data (1 second TTL)
_NABI_TMUX_CACHE="${HOME}/.nabi/cache/tmux-completion-cache.txt"
_NABI_TMUX_CACHE_TIME="${HOME}/.nabi/cache/tmux-completion-cache-time.txt"

# Check if cache is still valid (1 second TTL for fast updates)
_nabi_cache_valid() {
    [[ -f "$_NABI_TMUX_CACHE_TIME" ]] && \
    [[ -f "$_NABI_TMUX_CACHE" ]] && \
    (( $(date +%s) - $(cat "$_NABI_TMUX_CACHE_TIME" 2>/dev/null || echo 0) < 1 ))
}

# Helper function to get tmux sessions (direct tmux call, much faster)
_nabi_get_tmux_sessions() {
    local -a sessions
    # Use tmux directly instead of nabi CLI for speed (0.018s vs 2.4s)
    sessions=($(tmux list-sessions -F '#S' 2>/dev/null))
    echo "${sessions[@]}"
}

# Helper function to get windows for a session (direct tmux call)
_nabi_get_tmux_windows() {
    local session="$1"
    if [[ -z "$session" ]]; then
        return 1
    fi
    local -a windows
    # Use tmux directly - extract window index from format
    windows=($(tmux list-windows -t "$session" -F '#{window_index}' 2>/dev/null))
    echo "${windows[@]}"
}

# Helper function to get panes for a session:window (direct tmux call)
_nabi_get_tmux_panes() {
    local session="$1"
    local window="$2"
    if [[ -z "$session" ]] || [[ -z "$window" ]]; then
        return 1
    fi
    local -a panes
    local target="${session}:${window}"
    # Use tmux directly - format as session:window.pane
    panes=($(tmux list-panes -t "$target" -F "${session}:${window}.#{pane_index}" 2>/dev/null))
    echo "${panes[@]}"
}

# Completion for: nabi tmux send-prompt <PANE>
_nabi_tmux_send_prompt_pane() {
    _nabi_debug_log "_nabi_tmux_send_prompt_pane called"
    local -a targets

    # Optimize: Use single tmux command to get all panes across all sessions
    # This is much faster than querying each session separately
    local -a all_panes
    all_panes=($(tmux list-panes -a -s -F "#{session_name}:#{window_index}.#{pane_index}" 2>/dev/null))
    
    if (( ${#all_panes} == 0 )); then
        _message "No tmux sessions found"
        return 1
    fi

    # Extract unique session:window pairs for implicit pane 1 format
    local -A session_windows
    for pane in $all_panes; do
        # Extract session:window from session:window.pane
        if [[ "$pane" =~ ^([^:]+:[^\.]+)\. ]]; then
            session_windows[${match[1]}]=1
        fi
    done
    
    # Add session:window format (implicit pane 1)
    for sw in ${(k)session_windows}; do
        targets+=("$sw")
    done
    
    # Add all panes in session:window.pane format
    targets+=($all_panes)

    # Remove duplicates and sort
    targets=(${(u)targets})
    _nabi_debug_log "Generated ${#targets} targets: ${#targets[@]} total"

    _describe 'pane target' targets
}

# Completion for: nabi tmux list windows <SESSION>
_nabi_tmux_list_windows_session() {
    local -a sessions
    sessions=($(_nabi_get_tmux_sessions))

    if (( ${#sessions} == 0 )); then
        _message "No tmux sessions found"
        return 1
    fi

    _describe 'session' sessions
}

# Completion for: nabi tmux list panes <SESSION>
_nabi_tmux_list_panes_session() {
    local -a sessions
    sessions=($(_nabi_get_tmux_sessions))

    if (( ${#sessions} == 0 )); then
        _message "No tmux sessions found"
        return 1
    fi

    _describe 'session' sessions
}

# Completion for: nabi tmux list panes <SESSION> <WINDOW>
_nabi_tmux_list_panes_window() {
    local session="${words[5]}"

    if [[ -z "$session" ]]; then
        _message "Session required"
        return 1
    fi

    local -a windows
    windows=($(_nabi_get_tmux_windows "$session"))

    if (( ${#windows} == 0 )); then
        _message "No windows found in session '$session'"
        return 1
    fi

    _describe 'window' windows
}

# Note: We don't override _default anymore since we handle everything in _nabi() override
# The _nabi() override catches the pane argument completion before _default is called
