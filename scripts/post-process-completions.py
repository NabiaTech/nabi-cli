#!/usr/bin/env python3
"""Post-process clap-generated completion file to replace :_default with specific completion functions."""

import re
import sys

def main():
    if len(sys.argv) != 2:
        print("Usage: post-process-completions.py <completion-file>", file=sys.stderr)
        sys.exit(1)
    
    filepath = sys.argv[1]
    
    with open(filepath, 'r') as f:
        lines = f.readlines()
    
    in_tool_section = False
    in_exec = False
    exec_in_tool = False
    
    for i, line in enumerate(lines):
        # Track tool section - starts at (tool) and ends when we see esac followed by ;;
        if re.match(r'^\(tool\)$', line):
            in_tool_section = True
        elif in_tool_section and line.strip() == 'esac':
            # Check if next line is ;; which ends the tool section
            if i + 1 < len(lines) and lines[i + 1].strip() == ';;':
                in_tool_section = False
        
        # Track exec commands
        if re.match(r'^\s*\(exec\)$', line):
            in_exec = True
            exec_in_tool = in_tool_section
        
        # Find tool arg lines and replace based on context
        if ':tool -- Tool ID or command name:_default' in line:
            if in_exec and exec_in_tool:
                lines[i] = line.replace(':_default', ':_nabi__tool__exec_tool')
                in_exec = False
            elif in_exec and not exec_in_tool:
                lines[i] = line.replace(':_default', ':_nabi__exec_tool')
                in_exec = False
    
    with open(filepath, 'w') as f:
        f.writelines(lines)

if __name__ == '__main__':
    main()
