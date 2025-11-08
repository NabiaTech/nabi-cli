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

# Helper function to get tmux sessions
_nabi_get_tmux_sessions() {
    local -a sessions
    sessions=($(nabi tmux list sessions --format name 2>/dev/null))
    echo "${sessions[@]}"
}

# Helper function to get windows for a session
_nabi_get_tmux_windows() {
    local session="$1"
    if [[ -z "$session" ]]; then
        return 1
    fi
    local -a windows
    windows=($(nabi tmux list windows "$session" --format id 2>/dev/null))
    echo "${windows[@]}"
}

# Helper function to get panes for a session:window
_nabi_get_tmux_panes() {
    local session="$1"
    local window="$2"
    if [[ -z "$session" ]] || [[ -z "$window" ]]; then
        return 1
    fi
    local -a panes
    # Use full format to get session:window.pane directly
    panes=($(nabi tmux list panes "${session}:${window}" --format full 2>/dev/null))
    echo "${panes[@]}"
}

# Completion for: nabi tmux send-prompt <PANE>
_nabi_tmux_send_prompt_pane() {
    _nabi_debug_log "_nabi_tmux_send_prompt_pane called"
    local -a targets

    # Get all sessions
    local -a sessions
    _nabi_debug_log "Getting sessions..."
    sessions=($(_nabi_get_tmux_sessions))
    _nabi_debug_log "Found ${#sessions} sessions: ${sessions[@]}"

    if (( ${#sessions} == 0 )); then
        _message "No tmux sessions found"
        return 1
    fi

    # For each session, get all windows
    for session in $sessions; do
        local -a windows
        windows=($(_nabi_get_tmux_windows "$session"))

        # Add session:window format (implicit pane 1)
        for window in $windows; do
            targets+=("${session}:${window}")
        done

        # Get all panes in full format (session:window.pane)
        for window in $windows; do
            local -a panes
            panes=($(_nabi_get_tmux_panes "$session" "$window"))
            targets+=($panes)
        done
    done

    # Remove duplicates and sort
    targets=(${(u)targets})
    _nabi_debug_log "Generated ${#targets} targets: ${targets[@]}"

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

# Hook into clap-generated completions by overriding _default completion
# Clap uses :argument:_default for positional arguments, so we override _default
# to check the context and provide dynamic completions

# Store original _default function if it exists
if (( $+functions[_default] )) && (( ! $+functions[_nabi_original_default] )); then
    functions[_nabi_original_default]=$functions[_default]
fi

# Override _default to provide dynamic completions for tmux commands
# In zsh completion, $words is an array of words, and $CURRENT is the index of current word
_default() {
    local context="$curcontext"

    # Debug: see what context we're in
    _nabi_debug_log "_default called: context=$context, CURRENT=$CURRENT, words=${words[@]}"

    # Check if we're completing nabi tmux commands
    if [[ "$context" == *"nabi"* ]] && [[ "$context" == *"tmux"* ]]; then
        _nabi_debug_log "Matched nabi tmux context"
        # Case 1: nabi tmux send-prompt <pane>
        # words[1]=nabi, words[2]=tmux, words[3]=send-prompt, words[4]=pane (being completed)
        if [[ "${words[1]}" == "nabi" ]] && \
           [[ "${words[2]}" == "tmux" ]] && \
           [[ "${words[3]}" == "send-prompt" ]] && \
           [[ $CURRENT -eq 4 ]]; then
            _nabi_debug_log "Calling _nabi_tmux_send_prompt_pane from _default"
            _nabi_tmux_send_prompt_pane
            return
        fi

        # Case 2: nabi tmux list windows <session>
        # words[1]=nabi, words[2]=tmux, words[3]=list, words[4]=windows, words[5]=session (being completed)
        if [[ "${words[1]}" == "nabi" ]] && \
           [[ "${words[2]}" == "tmux" ]] && \
           [[ "${words[3]}" == "list" ]] && \
           [[ "${words[4]}" == "windows" ]] && \
           [[ $CURRENT -eq 5 ]]; then
            _nabi_tmux_list_windows_session
            return
        fi

        # Case 3: nabi tmux list panes <session> <window>
        # First argument: words[5]=session (being completed)
        if [[ "${words[1]}" == "nabi" ]] && \
           [[ "${words[2]}" == "tmux" ]] && \
           [[ "${words[3]}" == "list" ]] && \
           [[ "${words[4]}" == "panes" ]] && \
           [[ $CURRENT -eq 5 ]]; then
            _nabi_tmux_list_panes_session
            return
        fi
        # Second argument: words[6]=window (being completed)
        if [[ "${words[1]}" == "nabi" ]] && \
           [[ "${words[2]}" == "tmux" ]] && \
           [[ "${words[3]}" == "list" ]] && \
           [[ "${words[4]}" == "panes" ]] && \
           [[ $CURRENT -eq 6 ]]; then
            _nabi_tmux_list_panes_window
            return
        fi
    fi

    # Fall back to original _default if it exists, otherwise use built-in file completion
    if (( $+functions[_nabi_original_default] )); then
        _nabi_original_default "$@"
    else
        # No original _default, use built-in file completion
        _files "$@"
    fi
}
