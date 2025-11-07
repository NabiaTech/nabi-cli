#compdef nabi
# Dynamic Tmux Completion Enhancement for nabi
#
# This file adds runtime-aware completion to nabi CLI by querying tmux and nabi itself.
# It's loaded after the base _nabi completion to provide:
# - Session names for `nabi tmux send-prompt` and `nabi tmux list windows/panes`
# - Window lists for context-aware selection
# - Pane targets in full session:window.pane format
#
# This hooks into the clap-generated completions by overriding specific argument completions.

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
    local -a targets

    # Get all sessions
    local -a sessions
    sessions=($(_nabi_get_tmux_sessions))

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

# Store original _default function
if (( ! $+functions[_nabi_original_default] )); then
    functions[_nabi_original_default]=$functions[_default]
fi

# Override _default to provide dynamic completions for tmux commands
# In zsh completion, $words is an array of words, and $CURRENT is the index of current word
_default() {
    local context="$curcontext"
    
    # Debug: uncomment to see what context we're in
    # echo "DEBUG: context=$context, CURRENT=$CURRENT, words=${words[@]}" >&2

    # Check if we're completing nabi tmux commands
    if [[ "$context" == *"nabi"* ]] && [[ "$context" == *"tmux"* ]]; then
        # Case 1: nabi tmux send-prompt <pane>
        # words[1]=nabi, words[2]=tmux, words[3]=send-prompt, words[4]=pane (being completed)
        if [[ "${words[1]}" == "nabi" ]] && \
           [[ "${words[2]}" == "tmux" ]] && \
           [[ "${words[3]}" == "send-prompt" ]] && \
           [[ $CURRENT -eq 4 ]]; then
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

    # Fall back to original _default for everything else
    _nabi_original_default "$@"
}
