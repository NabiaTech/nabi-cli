#compdef nabi
# Dynamic Tmux Completion Enhancement for nabi
#
# This file adds runtime-aware completion to nabi CLI by querying tmux and nabi itself.
# It's loaded after the base _nabi completion to provide:
# - Session names for `nabi tmux send-prompt`
# - Window lists for context-aware selection
# - Pane targets in full session:window.pane format
#
# Install by adding to your .zshrc:
#   source ~/.cache/zsh/completions/nabi-completions-dynamic.zsh

# Completion for: nabi tmux send-prompt <PANE>
_nabi_tmux_send_prompt_pane() {
    local -a sessions windows panes

    # Get all tmux sessions
    sessions=($(nabi list sessions 2>/dev/null))

    if (( ${#sessions} == 0 )); then
        return 1
    fi

    # For each session, get all windows
    local -a targets
    for session in $sessions; do
        windows=($(nabi list windows "$session" 2>/dev/null))

        for window in $windows; do
            # Get panes for this window (in full format: session:window.pane)
            panes=($(nabi list panes "$session" "$window" --format full 2>/dev/null))

            for pane in $panes; do
                targets+=("$pane")
            done
        done
    done

    # Return targets for completion
    _values 'pane target' $targets
}

# Completion for: nabi list sessions
_nabi_list_sessions() {
    local -a sessions
    sessions=($(nabi list sessions 2>/dev/null))
    _values 'session' $sessions
}

# Completion for: nabi list windows <SESSION>
_nabi_list_windows() {
    local session="${words[4]}"

    if [[ -z "$session" ]]; then
        # No session yet, suggest available sessions
        local -a sessions
        sessions=($(nabi list sessions 2>/dev/null))
        _values 'session' $sessions
    else
        # Session specified, list its windows
        local -a windows
        windows=($(nabi list windows "$session" 2>/dev/null))
        _values 'window' $windows
    fi
}

# Completion for: nabi list panes <SESSION> <WINDOW>
_nabi_list_panes() {
    local session="${words[4]}"
    local window="${words[5]}"

    if [[ -z "$session" ]]; then
        # No session yet
        local -a sessions
        sessions=($(nabi list sessions 2>/dev/null))
        _values 'session' $sessions
    elif [[ -z "$window" ]]; then
        # Session specified, list its windows
        local -a windows
        windows=($(nabi list windows "$session" 2>/dev/null))
        _values 'window' $windows
    else
        # Both specified, list panes
        local -a panes
        panes=($(nabi list panes "$session" "$window" 2>/dev/null))
        _values 'pane' $panes
    fi
}

# Wire completions for nabi tmux send-prompt
_nabi_tmux_send_prompt() {
    _arguments -C \
        "1: :_nabi_tmux_send_prompt_pane" \
        "2: :(message)"
}

# Wire completions for nabi list subcommands
_nabi_list() {
    local cmd="$words[3]"

    case "$cmd" in
        sessions)
            _nabi_list_sessions
            ;;
        windows)
            _nabi_list_windows
            ;;
        panes)
            _nabi_list_panes
            ;;
    esac
}

# Register the dynamic completions
compdef _nabi_tmux_send_prompt nabi-tmux-send-prompt
compdef _nabi_list nabi-list
