#!/usr/bin/env python3
"""
Port Registry Transformer
Converts TOML port configs to unified JSON registry with conflict detection

Schema-driven transformation following the Aura pattern:
  Config (TOML) → Validate → Transform → Derived State (JSON registry)

Usage:
    python transform_ports.py                    # Transform all configs
    python transform_ports.py --validate-only    # Validate without generating
    python transform_ports.py --check-conflicts  # Only check for port conflicts
    python transform_ports.py --platform macos   # Filter by platform
"""

import os
import sys
import json
from pathlib import Path
from typing import Dict, List, Optional, Set, Tuple, Any
from datetime import datetime
from collections import defaultdict

# Check for tomli (Python 3.11+ has tomllib built-in)
try:
    import tomllib
except ImportError:
    try:
        import tomli as tomllib
    except ImportError:
        print("Error: tomli/tomllib not available. Install: pip install tomli")
        sys.exit(1)


class PortTransformer:
    """Transform TOML port configs to unified JSON registry"""

    def __init__(self, platform: Optional[str] = None):
        self.home = Path.home()
        self.config_dir = self.home / ".config" / "nabi" / "ports"
        self.state_dir = self.home / ".local" / "state" / "nabi" / "ports"
        self.platform = platform or self._detect_platform()

        # Ensure directories exist
        self.state_dir.mkdir(parents=True, exist_ok=True)

    def _detect_platform(self) -> str:
        """Detect current platform from system environment"""
        import platform as sys_platform
        system = sys_platform.system()

        if system == "Darwin":
            return "macos"
        elif system == "Linux":
            # Check if WSL
            with open("/proc/version", "r") as f:
                if "microsoft" in f.read().lower():
                    return "wsl"
            return "linux"
        else:
            return "unknown"

    def load_config(self, config_file: str) -> Dict[str, Any]:
        """Load TOML configuration file"""
        config_path = self.config_dir / config_file

        if not config_path.exists():
            print(f"Warning: Config file not found: {config_path}")
            return {}

        with open(config_path, "rb") as f:
            data = tomllib.load(f)

        return data

    def validate_port_range(self, port: int, min_port: int = 3000, max_port: int = 8499) -> List[str]:
        """Validate port is within acceptable range"""
        errors = []

        if not isinstance(port, int):
            errors.append(f"Port must be integer, got {type(port).__name__}")
        elif port < min_port or port > max_port:
            errors.append(f"Port {port} outside allowed range {min_port}-{max_port}")

        return errors

    def validate_service_config(self, name: str, config: Dict[str, Any]) -> List[str]:
        """Validate service configuration, return list of errors"""
        errors = []

        # Required fields
        if "port" not in config:
            errors.append(f"{name}: Missing required field 'port'")
            return errors

        # Validate port is integer and in range
        port = config["port"]
        errors.extend(self.validate_port_range(port))

        # Container port optional but if specified must be valid
        if "container_port" in config:
            cp = config["container_port"]
            errors.extend(self.validate_port_range(cp))

        # Required descriptions
        if "purpose" not in config:
            errors.append(f"{name}: Missing required field 'purpose'")

        # Validate protocol is valid (supports single and multi-protocol combinations)
        valid_base_protocols = ["tcp", "udp", "http", "https", "websocket", "sse"]
        if "protocol" in config:
            protocol = config["protocol"]
            # Check if it's a single protocol or combination (e.g., "http+sse", "ws+http+sse")
            parts = protocol.replace("ws", "websocket").split("+")
            invalid_parts = [p for p in parts if p not in valid_base_protocols]
            if invalid_parts:
                errors.append(f"{name}: Invalid protocol component(s) '{', '.join(invalid_parts)}' in '{protocol}'")

        # Check platform-specific configuration if filtering by platform
        if self.platform != "unknown":
            platforms_config = config.get("platforms", {})
            if self.platform not in platforms_config and not config.get("enabled", True):
                errors.append(f"{name}: Not configured for platform '{self.platform}' and globally disabled")

        return errors

    def check_port_conflicts(self, all_configs: Dict[str, Dict[str, Any]]) -> Tuple[List[str], Dict[str, List[str]]]:
        """Check for port allocation conflicts within platform"""
        errors = []
        port_map = defaultdict(list)  # port -> [service names]

        for service_name, config in all_configs.items():
            # Skip disabled services
            if not config.get("enabled", True):
                continue

            # Get effective port for current platform
            port = config.get("port")

            # Check if there's a platform-specific port
            platforms = config.get("platforms", {})
            if self.platform in platforms:
                platform_config = platforms[self.platform]
                if "port" in platform_config:
                    port = platform_config["port"]

            if port is None:
                continue

            port_map[port].append(service_name)

        # Find conflicts
        for port, services in port_map.items():
            if len(services) > 1:
                errors.append(f"Port {port} allocated to multiple services: {', '.join(services)}")

        return errors, dict(port_map)

    def transform_service(self, name: str, config: Dict[str, Any]) -> Dict[str, Any]:
        """Transform service config to registry entry"""
        registry_entry = {
            "name": name,
            "port": config.get("port"),
            "protocol": config.get("protocol", "tcp"),
            "purpose": config.get("purpose", ""),
            "enabled": config.get("enabled", True),
            "required": config.get("required", False),
        }

        # Add optional fields
        if "container_port" in config:
            registry_entry["container_port"] = config["container_port"]

        if "container_protocol" in config:
            registry_entry["container_protocol"] = config["container_protocol"]

        # Platform-specific overrides
        if "platforms" in config:
            platforms = {}
            for platform_name, platform_config in config["platforms"].items():
                platforms[platform_name] = {}

                if "enabled" in platform_config:
                    platforms[platform_name]["enabled"] = platform_config["enabled"]

                if "port" in platform_config:
                    platforms[platform_name]["port"] = platform_config["port"]

                if "endpoint" in platform_config:
                    platforms[platform_name]["endpoint"] = platform_config["endpoint"]

                if "container_port" in platform_config:
                    platforms[platform_name]["container_port"] = platform_config["container_port"]

            if platforms:
                registry_entry["platforms"] = platforms

        # Metadata
        registry_entry["notes"] = config.get("notes", "")
        registry_entry["owner"] = config.get("owner", "nabi")

        return registry_entry

    def write_registry(self, services: Dict[str, Dict[str, Any]]) -> Path:
        """Write unified registry JSON file"""
        registry = {
            "generated_at": datetime.now().isoformat(),
            "transformer_version": "1.0.0",
            "platform": self.platform,
            "config_location": str(self.config_dir),
            "services": services,
            "metadata": {
                "total_services": len(services),
                "enabled_services": sum(1 for s in services.values() if s.get("enabled", True)),
                "ranges": {
                    "infrastructure": "3000-3999",
                    "federation_core": "8000-8099",
                    "applications": "8100-8199",
                    "storage": "8200-8299",
                    "communication": "8300-8399",
                    "development": "8400-8499",
                }
            }
        }

        registry_path = self.state_dir / "registry.json"
        with open(registry_path, "w") as f:
            json.dump(registry, f, indent=2)

        return registry_path

    def generate_manifest(self, services: Dict[str, Dict[str, Any]]) -> Path:
        """Generate manifest tracking all transformed configs"""
        manifest = {
            "generated_at": datetime.now().isoformat(),
            "transformer_version": "1.0.0",
            "platform": self.platform,
            "config_location": str(self.config_dir),
            "configs_loaded": [],
            "services": {
                name: {
                    "port": config.get("port"),
                    "enabled": config.get("enabled", True),
                }
                for name, config in services.items()
            }
        }

        # Track which config files were loaded
        for config_file in self.config_dir.glob("*.toml"):
            if config_file.name != "_ranges.toml":  # Skip reference file
                manifest["configs_loaded"].append(config_file.name)

        manifest_path = self.state_dir / "manifest.json"
        with open(manifest_path, "w") as f:
            json.dump(manifest, f, indent=2)

        return manifest_path

    def transform_all(self, validate_only: bool = False) -> bool:
        """Transform all port configurations"""
        print("=" * 80)
        print("Port Registry Transformer")
        print("=" * 80)
        print(f"Config:   {self.config_dir}")
        print(f"Output:   {self.state_dir}")
        print(f"Platform: {self.platform}")
        print()

        # Load all configurations (except _ranges.toml which is reference)
        all_configs = {}
        config_files = [f for f in self.config_dir.glob("*.toml") if f.name != "_ranges.toml"]

        if not config_files:
            config_home = os.getenv('XDG_CONFIG_HOME', os.path.expanduser('~/.config'))
            print(f"Error: No port configuration files found in {config_home}/nabi/ports/")
            return False

        print(f"Loading {len(config_files)} configuration files...")
        for config_file in sorted(config_files):
            config_data = self.load_config(config_file.name)
            if "services" in config_data:
                all_configs.update(config_data["services"])
                print(f"  ✅ {config_file.name} ({len(config_data['services'])} services)")

        if not all_configs:
            print("Error: No services found in configuration files")
            return False

        # Validate all configs
        print(f"\nValidating {len(all_configs)} port allocations...")
        all_errors = []

        for name, config in all_configs.items():
            errors = self.validate_service_config(name, config)
            if errors:
                all_errors.extend(errors)
                for error in errors:
                    print(f"  ❌ {error}")

        # Check for port conflicts
        conflict_errors, port_map = self.check_port_conflicts(all_configs)
        all_errors.extend(conflict_errors)
        for error in conflict_errors:
            print(f"  ⚠️  {error}")

        if all_errors:
            print(f"\n❌ Validation failed with {len(all_errors)} error(s)")
            return False

        print(f"✅ All {len(all_configs)} services validated, no conflicts detected")

        if validate_only:
            print("\n--validate-only mode: No registry generated")
            return True

        # Transform services
        print("\nTransforming to unified registry...")
        transformed_services = {}

        for name, config in all_configs.items():
            try:
                transformed = self.transform_service(name, config)
                transformed_services[name] = transformed
            except Exception as e:
                print(f"  ❌ {name}: {e}")
                all_errors.append(str(e))

        # Write registry
        registry_path = self.write_registry(transformed_services)
        print(f"\n📋 Registry: {registry_path}")
        print(f"   Services:  {len(transformed_services)}")
        print(f"   Platform:  {self.platform}")

        # Write manifest
        manifest_path = self.generate_manifest(transformed_services)
        print(f"📋 Manifest: {manifest_path}")

        # Summary
        print("\n" + "=" * 80)
        print(f"✅ Generated unified port registry with {len(transformed_services)} services")
        print("=" * 80)
        print("\nNext steps:")
        print("  1. Validate registry: nabi port validate")
        print("  2. Check conflicts:   nabi port check-conflicts")
        print("  3. List services:     nabi port list")
        print("  4. View registry:     cat ~/.local/state/nabi/ports/registry.json")
        print()

        return len(all_errors) == 0


def main():
    import argparse

    parser = argparse.ArgumentParser(description="Transform port configs to unified JSON registry")
    parser.add_argument("--validate-only", action="store_true", help="Validate without generating")
    parser.add_argument("--check-conflicts", action="store_true", help="Only check for conflicts")
    parser.add_argument("--platform", help="Override platform detection (macos|wsl|linux|rpi)")

    args = parser.parse_args()

    transformer = PortTransformer(platform=args.platform)

    if args.check_conflicts:
        # Just check conflicts without full transformation
        config_data = transformer.load_config("infrastructure.toml")
        if not config_data.get("services"):
            print("Error: No services found")
            return 1

        all_configs = {}
        for cf in (transformer.config_dir).glob("*.toml"):
            if cf.name != "_ranges.toml":
                data = transformer.load_config(cf.name)
                if "services" in data:
                    all_configs.update(data["services"])

        errors, port_map = transformer.check_port_conflicts(all_configs)
        if errors:
            print("❌ Port conflicts detected:")
            for error in errors:
                print(f"  {error}")
            return 1
        else:
            print("✅ No port conflicts detected")
            return 0

    success = transformer.transform_all(validate_only=args.validate_only)
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()
