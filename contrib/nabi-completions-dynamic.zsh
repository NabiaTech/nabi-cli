#compdef nabi
# Dynamic Completion Enhancement for nabi
#
# This file defines completion functions for dynamic arguments that require runtime queries.
# The justfile post-processes the clap-generated completion file to call these functions.
#
# Supported dynamic completions:
# - `nabi port shift <SERVICE>` — Service names from port registry
# - `nabi tmux send-prompt <PANE>` — Tmux session:window.pane targets
# - `nabi events ack <EVENT_ID>` — Event IDs from event storage
# - `nabi exec <TOOL>` — Registered tool IDs
# - `nabi tool exec <TOOL>` — Registered tool IDs

# XDG-compliant cache directory (matches Rust codebase)
_NABI_CACHE_DIR="${XDG_CACHE_HOME:-${HOME}/.cache}/nabi"
_NABI_STATE_DIR="${XDG_STATE_HOME:-${HOME}/.local/state}/nabi"

# Port registry location
_NABI_PORT_REGISTRY="${_NABI_STATE_DIR}/governance/port-registry.json"

# Debug log file (set _NABI_DEBUG=1 to enable)
_NABI_COMPLETION_DEBUG_LOG="${_NABI_CACHE_DIR}/completion-debug.log"

# Debug logging function (only logs if _NABI_DEBUG is set)
_nabi_debug_log() {
    [[ -n "$_NABI_DEBUG" ]] || return 0
    mkdir -p "${_NABI_CACHE_DIR}" 2>/dev/null
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" >> "$_NABI_COMPLETION_DEBUG_LOG"
}

# ============================================================================
# Completion Function Definitions
# ============================================================================
# These functions are called directly by the post-processed completion file.
# No overriding _nabi needed - just define the functions!

# Completion for: nabi tmux send-prompt <PANE>
_nabi__tmux__send_prompt_pane() {
    _nabi_tmux_send_prompt_pane
}

# Completion for: nabi port shift <SERVICE>
_nabi__port__shift_service() {
    _nabi_port_shift_service
}

# Completion for: nabi exec <TOOL>
_nabi__exec_tool() {
    _nabi_tool_exec_tool_completion
}

# Completion for: nabi tool exec <TOOL>
_nabi__tool__exec_tool() {
    _nabi_tool_exec_tool_completion
}

# Completion for: nabi events ack <EVENT_ID>
_nabi__events__ack_event_id() {
    _nabi_events_ack_event_id
}

# ============================================================================
# Helper Functions
# ============================================================================

# Cache file for tmux data (2 second TTL for fast updates)
# XDG-compliant: uses $XDG_CACHE_HOME/nabi or ~/.cache/nabi
_NABI_TMUX_CACHE="${_NABI_CACHE_DIR}/tmux-completion-cache.txt"
_NABI_TMUX_CACHE_TIME="${_NABI_CACHE_DIR}/tmux-completion-cache-time.txt"

# Check if cache is still valid (2 second TTL for fast updates)
_nabi_cache_valid() {
    [[ -f "$_NABI_TMUX_CACHE_TIME" ]] && \
    [[ -f "$_NABI_TMUX_CACHE" ]] && \
    (( $(date +%s) - $(cat "$_NABI_TMUX_CACHE_TIME" 2>/dev/null || echo 0) < 2 ))
}

# Load cached targets or generate new ones
_nabi_get_cached_targets() {
    if _nabi_cache_valid; then
        # Cache is valid, load from file
        local -a cached
        cached=($(cat "$_NABI_TMUX_CACHE" 2>/dev/null))
        echo "${cached[@]}"
        return 0
    fi

    # Cache invalid or missing, generate fresh data
    local -a all_panes
    all_panes=($(tmux list-panes -a -s -F "#{session_name}:#{window_index}.#{pane_index}" 2>/dev/null))

    if (( ${#all_panes} == 0 )); then
        return 1
    fi

    # Extract unique session:window pairs
    local -A session_windows
    local pane sw
    for pane in $all_panes; do
        sw=${pane%.*}
        session_windows[$sw]=1
    done

    # Build targets array
    local -a targets
    targets+=(${(k)session_windows})
    targets+=($all_panes)
    targets=(${(u)targets})

    # Save to cache (XDG-compliant location)
    mkdir -p "${_NABI_CACHE_DIR}" 2>/dev/null
    echo "${targets[@]}" > "$_NABI_TMUX_CACHE"
    echo $(date +%s) > "$_NABI_TMUX_CACHE_TIME"

    echo "${targets[@]}"
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

    # Use cached targets if available (2 second TTL)
    targets=($(_nabi_get_cached_targets))

    if (( ${#targets} == 0 )); then
        _message "No tmux sessions found"
        return 1
    fi

    _nabi_debug_log "Using ${#targets} targets (cached or fresh)"
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

# Helper function to get services from port registry
_nabi_get_port_services() {
    local -a services

    if [[ ! -f "$_NABI_PORT_REGISTRY" ]]; then
        _nabi_debug_log "Port registry not found at $_NABI_PORT_REGISTRY"
        return 1
    fi

    # Extract service names from standard_allocations and platform_configs
    # Using jq if available, fallback to grep/sed if not
    if command -v jq &> /dev/null; then
        services=($(jq -r '.standard_allocations | keys[]' "$_NABI_PORT_REGISTRY" 2>/dev/null))
    else
        # Fallback: simple pattern matching (less robust but works)
        services=($(grep -o '"[a-z0-9_-]*":[[:space:]]*{' "$_NABI_PORT_REGISTRY" | sed 's/"//g' | sed 's/://' | sort -u))
    fi

    if (( ${#services} == 0 )); then
        _nabi_debug_log "No services found in port registry"
        return 1
    fi

    _nabi_debug_log "Found ${#services} services: ${services[@]}"
    echo "${services[@]}"
}

# Completion for: nabi port shift <SERVICE>
_nabi_port_shift_service() {
    _nabi_debug_log "_nabi_port_shift_service called"
    local -a services

    services=($(_nabi_get_port_services))

    if (( ${#services} == 0 )); then
        _message "No services found in port registry"
        return 1
    fi

    _nabi_debug_log "Using ${#services} services for completion"
    _describe 'service' services
}

# Helper function to get event IDs from event storage
_nabi_get_event_ids() {
    local -a event_ids
    local event_base="${XDG_DATA_HOME:-${HOME}/.local/share}/nabi/events"

    # Check if event directory exists
    if [[ ! -d "$event_base" ]]; then
        return 1
    fi

    # Get event IDs from all .json files in the directory
    for json_file in "$event_base"/*/*.json; do
        if [[ -f "$json_file" ]]; then
            local filename="${json_file##*/}"
            local event_id="${filename%.json}"
            event_ids+=("$event_id")
        fi
    done

    # Return unique event IDs
    if (( ${#event_ids} > 0 )); then
        echo "${event_ids[@]}"
        return 0
    fi

    return 1
}

# Completion for: nabi events ack <EVENT_ID>
_nabi_events_ack_event_id() {
    _nabi_debug_log "_nabi_events_ack_event_id called"
    local -a event_ids

    event_ids=($(_nabi_get_event_ids))

    if (( ${#event_ids} == 0 )); then
        _message "No events found"
        return 1
    fi

    _nabi_debug_log "Found ${#event_ids} event IDs for completion"
    _describe 'event ID' event_ids
}

# Dynamic tool completion function
_nabi_dynamic_tools() {
    local tools
    tools=(${(f)"$(nabi tool list --format=json 2>/dev/null | jq -r '.tools[] | .id + ":" + .description' 2>/dev/null)"})
    _describe 'registered tools' tools
}

# Completion for tool argument of: nabi exec <TOOL> and nabi tool exec <TOOL>
_nabi_tool_exec_tool_completion() {
    local tools
    tools=(${(f)"$(nabi tool list --format=json 2>/dev/null | jq -r '.tools[] | .id + ":" + .description' 2>/dev/null)"})

    if (( ${#tools} == 0 )); then
        _default  # Fall back to default if no tools found
        return 1
    fi

    _describe 'tool' tools
}
