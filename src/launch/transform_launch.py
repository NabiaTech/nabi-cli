#!/usr/bin/env python3
"""
Launch Configuration Transformer
Converts TOML configs to macOS LaunchAgent plists

Schema-driven transformation following the Aura pattern:
  Config (TOML) → Validate → Transform → Derived State (plist)

Usage:
    python transform_launch.py                    # Transform all configs
    python transform_launch.py --validate-only    # Validate without generating
    python transform_launch.py --service memchain # Transform specific service
"""

import os
import sys
import json
import plistlib
from pathlib import Path
from typing import Dict, List, Optional, Any
from datetime import datetime

# Check for tomli (Python 3.11+ has tomllib built-in)
try:
    import tomllib
except ImportError:
    try:
        import tomli as tomllib
    except ImportError:
        print("Error: tomli/tomllib not available. Install: pip install tomli")
        sys.exit(1)


class LaunchTransformer:
    """Transform TOML launch configs to LaunchAgent plists"""

    def __init__(self):
        self.home = Path.home()
        self.config_dir = self.home / ".config" / "nabi" / "launch"
        self.state_dir = self.home / ".local" / "state" / "nabi" / "launch"
        self.launchagent_dir = self.home / "Library" / "LaunchAgents"

        # Ensure directories exist
        self.state_dir.mkdir(parents=True, exist_ok=True)
        self.launchagent_dir.mkdir(parents=True, exist_ok=True)
        (self.home / ".local" / "state" / "nabi" / "logs").mkdir(parents=True, exist_ok=True)

    def expand_path(self, path: str) -> str:
        """Expand ~ and environment variables in paths"""
        if path.startswith("~/"):
            return str(self.home / path[2:])
        return os.path.expandvars(path)

    def load_config(self, config_file: str) -> Dict[str, Any]:
        """Load TOML configuration file and flatten nested environment_variables"""
        config_path = self.config_dir / config_file

        if not config_path.exists():
            print(f"Warning: Config file not found: {config_path}")
            return {}

        with open(config_path, "rb") as f:
            data = tomllib.load(f)

        # Flatten nested environment_variables tables
        # TOML: [services.name.environment_variables]
        # becomes: services.name.environment_variables = {...}
        for category in ["services", "schedules"]:
            if category in data:
                for name, config in data[category].items():
                    if "environment_variables" in config and isinstance(config["environment_variables"], dict):
                        # Already inline dict, no change needed
                        continue

        return data

    def validate_service_config(self, name: str, config: Dict[str, Any]) -> List[str]:
        """Validate service configuration, return list of errors"""
        errors = []

        # Required fields
        if "program" not in config:
            errors.append(f"{name}: Missing required field 'program'")

        if "description" not in config:
            errors.append(f"{name}: Missing required field 'description'")

        # Validate program exists
        if "program" in config:
            program_path = self.expand_path(config["program"])
            if not os.path.exists(program_path):
                errors.append(f"{name}: Program not found: {program_path}")

        # Validate mutually exclusive scheduling options
        if "start_interval" in config and "start_calendar_interval" in config:
            errors.append(f"{name}: Cannot specify both start_interval and start_calendar_interval")

        return errors

    def transform_service(self, name: str, config: Dict[str, Any]) -> Dict[str, Any]:
        """Transform service config to LaunchAgent plist dictionary"""
        plist = {
            "Label": f"com.nabi.{name}",
            "ProgramArguments": [self.expand_path(config["program"])],
        }

        # Add program arguments if specified
        if "program_arguments" in config:
            plist["ProgramArguments"].extend([
                self.expand_path(arg) for arg in config["program_arguments"]
            ])

        # Run at load
        if config.get("run_at_load", False):
            plist["RunAtLoad"] = True

        # Keep alive (auto-restart)
        if config.get("keep_alive", False):
            plist["KeepAlive"] = True

        # Standard output
        if "standard_out_path" in config:
            plist["StandardOutPath"] = self.expand_path(config["standard_out_path"])

        # Standard error
        if "standard_error_path" in config:
            plist["StandardErrorPath"] = self.expand_path(config["standard_error_path"])

        # Working directory
        if "working_directory" in config:
            plist["WorkingDirectory"] = self.expand_path(config["working_directory"])

        # Environment variables
        if "environment_variables" in config:
            plist["EnvironmentVariables"] = {
                k: self.expand_path(v) for k, v in config["environment_variables"].items()
            }

        # Throttle interval (prevents crash loops)
        if "throttle_interval" in config:
            plist["ThrottleInterval"] = config["throttle_interval"]

        # Scheduling: start_interval (simple)
        if "start_interval" in config:
            plist["StartInterval"] = config["start_interval"]

        # Scheduling: start_calendar_interval (cron-style)
        if "start_calendar_interval" in config:
            plist["StartCalendarInterval"] = config["start_calendar_interval"]

        return plist

    def write_plist(self, name: str, plist_data: Dict[str, Any]) -> Path:
        """Write plist to LaunchAgents directory"""
        plist_path = self.launchagent_dir / f"com.nabi.{name}.plist"

        with open(plist_path, "wb") as f:
            plistlib.dump(plist_data, f)

        return plist_path

    def generate_manifest(self, services: Dict[str, Path]) -> Path:
        """Generate manifest tracking all generated plists"""
        manifest = {
            "generated_at": datetime.now().isoformat(),
            "transformer_version": "1.0.0",
            "config_location": str(self.config_dir),
            "services": {
                name: {
                    "plist_path": str(path),
                    "config_file": "services.toml" if name not in ["nabi_doctor", "link_mapper_audit", "manifest_regeneration", "federation_state_backup", "cleanup_old_logs"] else "schedules.toml",
                    "label": f"com.nabi.{name}"
                }
                for name, path in services.items()
            }
        }

        manifest_path = self.state_dir / "manifest.json"
        with open(manifest_path, "w") as f:
            json.dump(manifest, f, indent=2)

        return manifest_path

    def transform_all(self, validate_only: bool = False) -> bool:
        """Transform all configurations"""
        print("=" * 80)
        print("Launch Configuration Transformer")
        print("=" * 80)
        print(f"Config: {self.config_dir}")
        print(f"Output: {self.launchagent_dir}")
        print(f"State:  {self.state_dir}")
        print()

        # Load configurations
        services_config = self.load_config("services.toml")
        schedules_config = self.load_config("schedules.toml")

        all_configs = {}

        # Merge services
        if "services" in services_config:
            all_configs.update(services_config["services"])

        # Merge schedules
        if "schedules" in schedules_config:
            all_configs.update(schedules_config["schedules"])

        if not all_configs:
            print("Error: No services or schedules found in configuration files")
            return False

        # Validate all configs
        print("Validating configurations...")
        all_errors = []
        for name, config in all_configs.items():
            # Skip disabled services during validation
            if config.get("enabled") == False:
                print(f"  ⏭️  {name} (disabled, skipping validation)")
                continue

            errors = self.validate_service_config(name, config)
            all_errors.extend(errors)

        if all_errors:
            print("\n❌ Validation Errors:")
            for error in all_errors:
                print(f"  - {error}")
            return False

        print(f"✅ All {len(all_configs)} configurations valid")

        if validate_only:
            print("\n--validate-only mode: No plists generated")
            return True

        # Transform and write plists
        print("\nGenerating LaunchAgent plists...")
        generated_services = {}

        for name, config in all_configs.items():
            # Skip disabled services
            if config.get("enabled") == False:
                print(f"  ⏭️  {name} (disabled in config)")
                continue

            try:
                plist_data = self.transform_service(name, config)
                plist_path = self.write_plist(name, plist_data)
                generated_services[name] = plist_path
                print(f"  ✅ {name} → {plist_path.name}")
            except Exception as e:
                print(f"  ❌ {name}: {e}")
                all_errors.append(str(e))

        # Generate manifest
        manifest_path = self.generate_manifest(generated_services)
        print(f"\n📋 Manifest: {manifest_path}")

        # Summary
        print("\n" + "=" * 80)
        print(f"✅ Generated {len(generated_services)} LaunchAgent plists")
        print("=" * 80)
        print("\nNext steps:")
        print("  1. Load services: nabi launch enable <service>")
        print("  2. Check status:  nabi launch status")
        print("  3. View logs:     tail -f ~/.local/state/nabi/logs/<service>.log")
        print()

        return len(all_errors) == 0


def main():
    import argparse

    parser = argparse.ArgumentParser(description="Transform launch configs to LaunchAgent plists")
    parser.add_argument("--validate-only", action="store_true", help="Validate without generating")
    parser.add_argument("--service", help="Transform specific service only")

    args = parser.parse_args()

    transformer = LaunchTransformer()

    if args.service:
        print(f"Error: --service not yet implemented")
        sys.exit(1)

    success = transformer.transform_all(validate_only=args.validate_only)
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()
