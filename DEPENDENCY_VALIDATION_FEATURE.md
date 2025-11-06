# Dependency Validation Feature for `nabi tool register`

## Problem Statement
Python tools registered via `nabi tool register` fail at runtime due to missing dependencies because:
1. Dependencies are not extracted from TOML/requirements.txt
2. Venvs are not validated/created automatically  
3. Dependencies are not installed during registration

## Solution
Add automatic dependency validation and setup during tool registration.

## Implementation

### 1. Add Dependency Validation Function

Insert after line 1225 in `src/main.rs`:

```rust
/// Validate and setup dependencies for Python tools
/// Returns (dependencies_list, validation_messages)
fn validate_tool_dependencies(
    source_path: &Path,
    venv_location: &Option<String>,
    runtime: RuntimeKind,
) -> Result<(Vec<String>, Vec<String>)> {
    let mut dependencies = Vec::new();
    let mut messages = Vec::new();
    
    // Only validate for Python tools
    if !matches!(runtime, RuntimeKind::Python) {
        return Ok((dependencies, messages));
    }
    
    // Look for TOML config with dependencies
    let possible_locations = vec![
        source_path.join("requirements.txt"),
        source_path.parent()
            .and_then(|p| p.parent())
            .and_then(|p| Some(p.join("tools")
                .join(source_path.file_name()?)
                .with_extension("toml"))),
    ];
    
    let mut found_deps = false;
    
    // Check for dependencies
    for toml_path in possible_locations.iter().flatten() {
        if !toml_path.exists() {
            continue;
        }
        
        if toml_path.file_name().and_then(|n| n.to_str()) == Some("requirements.txt") {
            // Parse requirements.txt
            if let Ok(content) = fs::read_to_string(toml_path) {
                dependencies = content
                    .lines()
                    .filter(|line| !line.trim().starts_with('#') && !line.trim().is_empty())
                    .map(|line| line.trim().to_string())
                    .collect();
                found_deps = !dependencies.is_empty();
                messages.push(format!("📦 Found {} dependencies in {}", 
                    dependencies.len(), 
                    toml_path.display()));
                break;
            }
        } else if toml_path.extension().and_then(|e| e.to_str()) == Some("toml") {
            // Parse TOML for [tool.dependencies.python]
            if let Ok(content) = fs::read_to_string(toml_path) {
                if let Ok(toml_value) = toml::from_str::<toml::Value>(&content) {
                    if let Some(tool_deps) = toml_value
                        .get("tool")
                        .and_then(|t| t.get("dependencies"))
                        .and_then(|d| d.get("python"))
                        .and_then(|p| p.as_array())
                    {
                        dependencies = tool_deps
                            .iter()
                            .filter_map(|v| v.as_str())
                            .map(|s| s.to_string())
                            .collect();
                        found_deps = !dependencies.is_empty();
                        messages.push(format!("📦 Found {} dependencies in {}", 
                            dependencies.len(), 
                            toml_path.display()));
                        break;
                    }
                }
            }
        }
    }
    
    if !found_deps {
        messages.push("⚠️  No dependencies found - tool may require manual setup".to_string());
        return Ok((dependencies, messages));
    }
    
    // Validate venv exists if dependencies found
    if let Some(venv_loc) = venv_location {
        let venv_path = expand_home(venv_loc)?;
        let python_bin = venv_path.join("bin").join("python");
        
        if !venv_path.exists() {
            messages.push(format!("🔧 Creating venv at {}...", venv_loc));
            
            // Create venv using uv
            let status = process::Command::new("uv")
                .args(&["venv", venv_path.to_str().unwrap()])
                .status()
                .context("Failed to create venv with uv")?;
            
            if !status.success() {
                anyhow::bail!("Failed to create venv at {}", venv_loc);
            }
            messages.push("✅ Venv created".to_string());
        } else {
            messages.push(format!("✓ Venv exists at {}", venv_loc));
        }
        
        // Install dependencies
        if !dependencies.is_empty() {
            messages.push(format!("📥 Installing {} dependencies...", dependencies.len()));
            
            // Create temporary requirements file
            let temp_req = std::env::temp_dir().join("nabi_temp_requirements.txt");
            fs::write(&temp_req, dependencies.join("\n"))?;
            
            let status = process::Command::new("uv")
                .args(&[
                    "pip", "install",
                    "-r", temp_req.to_str().unwrap(),
                    "--python", python_bin.to_str().unwrap()
                ])
                .status()
                .context("Failed to install dependencies with uv")?;
            
            fs::remove_file(temp_req)?;
            
            if !status.success() {
                messages.push("⚠️  Some dependencies failed to install".to_string());
            } else {
                messages.push(format!("✅ Installed {} packages", dependencies.len()));
            }
        }
    } else {
        messages.push("⚠️  No venv configured - dependencies not installed".to_string());
    }
    
    Ok((dependencies, messages))
}
```

### 2. Integrate into `register_tool()`

Add after line 1105 (after venv_location is determined):

```rust
// Validate and setup dependencies
let (validated_deps, dep_messages) = validate_tool_dependencies(
    &source_path,
    &venv_location,
    runtime
)?;

// Print dependency validation messages
for msg in &dep_messages {
    println!("  {}", msg);
}
```

### 3. Update Manifest Creation

Change line 1153 from:
```rust
dependencies: Vec::new(),
```

To:
```rust
dependencies: validated_deps,
```

## Benefits

1. **Automatic Setup**: Venvs created and dependencies installed automatically
2. **Early Detection**: Dependency issues caught during registration, not runtime
3. **Consistent State**: TOML dependencies always match installed packages
4. **Better UX**: Clear feedback about what was installed

## Testing Plan

```bash
# Test 1: Register tool with dependencies
cd ~/.local/state/nabi/bin
nabi tool register chronology-atomic-backdate.py --force

# Expected output:
# 📦 Found 2 dependencies in ~/.config/nabi/tools/atomic-flow/requirements.txt
# ✓ Venv exists at ~/.nabi/venvs/atomic-flow
# 📥 Installing 2 dependencies...
# ✅ Installed 6 packages

# Test 2: Register tool without dependencies
nabi tool register some-simple-script.py

# Expected output:
# ⚠️  No dependencies found - tool may require manual setup

# Test 3: Register non-Python tool
nabi tool register some-bash-script.sh

# Expected: No dependency validation (skipped for non-Python)
```

## Rollout Plan

1. Review implementation
2. Test with atomic-flow tools
3. Commit to feature branch
4. Build and install test binary
5. Validate with multiple tools
6. Merge to main
7. Update documentation

## Files Modified

- `src/main.rs`: Add validate_tool_dependencies() and integration

## Dependencies Added

None - uses existing:
- `std::process::Command` (already imported)
- `toml` crate (already in dependencies)
- `uv` CLI (already installed)

## Backward Compatibility

✅ Fully backward compatible:
- Existing tools continue to work
- Only validates during NEW registrations
- Gracefully handles missing dependencies
- Non-Python tools unaffected
