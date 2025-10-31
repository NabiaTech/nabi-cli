#!/usr/bin/env python3
"""
Port Registry Validator
Validates port allocations against actual running services

Schema-driven validation completing the Aura pattern:
  Registry (JSON) → Validate → Reality Check → Drift Report

Usage:
    python validate_ports.py                     # Full validation
    python validate_ports.py --check-drift       # Only check for drift
    python validate_ports.py --show-running      # Show all currently running ports
    python validate_ports.py --fix-drift         # Attempt to fix common drift issues
"""

import os
import sys
import json
import subprocess
from pathlib import Path
from typing import Dict, List, Set, Tuple, Optional
from datetime import datetime
from collections import defaultdict


class PortValidator:
    """Validate port registry against actual system state"""

    def __init__(self):
        self.home = Path.home()
        self.state_dir = self.home / ".local" / "state" / "nabi" / "ports"
        self.registry_path = self.state_dir / "registry.json"
        self.platform = self._detect_platform()

        # Load registry
        self.registry = self._load_registry()

    def _detect_platform(self) -> str:
        """Detect current platform"""
        import platform as sys_platform
        system = sys_platform.system()

        if system == "Darwin":
            return "macos"
        elif system == "Linux":
            with open("/proc/version", "r") as f:
                if "microsoft" in f.read().lower():
                    return "wsl"
            return "linux"
        else:
            return "unknown"

    def _load_registry(self) -> Dict:
        """Load port registry JSON"""
        if not self.registry_path.exists():
            print(f"Error: Port registry not found at {self.registry_path}")
            print("Run: python transform_ports.py")
            sys.exit(1)

        with open(self.registry_path, "r") as f:
            return json.load(f)

    def get_running_ports_lsof(self) -> Dict[int, Dict[str, str]]:
        """Get currently listening ports using lsof (macOS/Linux)"""
        running = {}

        try:
            result = subprocess.run(
                ["lsof", "-i", "-P", "-n"],
                capture_output=True,
                text=True,
                timeout=5
            )

            if result.returncode != 0:
                return running

            lines = result.stdout.strip().split("\n")[1:]  # Skip header
            for line in lines:
                parts = line.split()
                if len(parts) >= 10:
                    try:
                        name = parts[0]
                        state = parts[9] if len(parts) > 9 else ""

                        # Extract port from address (e.g., "*:8000" or "127.0.0.1:8000")
                        addr_part = parts[8] if len(parts) > 8 else ""
                        if ":" in addr_part:
                            port_str = addr_part.split(":")[-1]
                            if port_str.isdigit():
                                port = int(port_str)
                                if state == "LISTEN":
                                    running[port] = {
                                        "process": name,
                                        "state": state,
                                        "address": addr_part
                                    }
                    except (ValueError, IndexError):
                        continue

        except FileNotFoundError:
            print("Warning: lsof not found, skipping lsof port check")
        except subprocess.TimeoutExpired:
            print("Warning: lsof timeout")

        return running

    def get_running_ports_docker(self) -> Dict[int, Dict[str, str]]:
        """Get ports from running Docker containers"""
        running = {}

        try:
            result = subprocess.run(
                ["docker", "ps", "--format", "{{.ID}}\t{{.Image}}\t{{.Ports}}"],
                capture_output=True,
                text=True,
                timeout=5
            )

            if result.returncode != 0:
                return running

            lines = result.stdout.strip().split("\n")
            for line in lines:
                if not line:
                    continue

                parts = line.split("\t")
                if len(parts) >= 3:
                    container_id = parts[0][:12]
                    image = parts[1]
                    ports_str = parts[2]

                    # Parse ports (e.g., "0.0.0.0:8000->8000/tcp")
                    for port_mapping in ports_str.split(","):
                        port_mapping = port_mapping.strip()
                        if "->" in port_mapping:
                            external = port_mapping.split("->")[0]
                            if ":" in external:
                                port_str = external.split(":")[-1]
                                if port_str.isdigit():
                                    port = int(port_str)
                                    running[port] = {
                                        "container": container_id,
                                        "image": image,
                                        "mapping": port_mapping
                                    }

        except FileNotFoundError:
            pass  # Docker not installed, skip
        except subprocess.TimeoutExpired:
            print("Warning: docker timeout")

        return running

    def get_declared_ports(self) -> Dict[int, str]:
        """Get all declared ports from registry for current platform"""
        declared = {}

        for name, service in self.registry.get("services", {}).items():
            # Check if service is enabled for current platform
            platforms_config = service.get("platforms", {})

            # Default: use port from service config
            port = service.get("port")

            # Override if platform-specific port exists
            if self.platform in platforms_config:
                platform_config = platforms_config[self.platform]
                if not platform_config.get("enabled", True):
                    continue  # Service disabled on this platform
                if "port" in platform_config:
                    port = platform_config["port"]

            # Skip if service is globally disabled
            if not service.get("enabled", True):
                continue

            if port:
                declared[port] = name

        return declared

    def check_drift(self) -> Tuple[List[str], List[str], List[str]]:
        """
        Check for drift between declared and actual ports
        Returns: (errors, warnings, info)
        """
        errors = []
        warnings = []
        info = []

        running_lsof = self.get_running_ports_lsof()
        running_docker = self.get_running_ports_docker()
        running_all = {**running_lsof, **running_docker}
        declared = self.get_declared_ports()

        # Find declared ports that aren't running
        for port, service_name in declared.items():
            service = self.registry["services"][service_name]
            required = service.get("required", False)

            if port not in running_all:
                if required:
                    errors.append(f"Required service '{service_name}' not running on port {port}")
                else:
                    warnings.append(f"Optional service '{service_name}' not running on port {port}")

        # Find running ports that aren't declared
        for port, info_dict in running_all.items():
            if port not in declared:
                process_name = info_dict.get("process") or info_dict.get("image", "unknown")
                info.append(f"Undeclared service '{process_name}' running on port {port}")

        return errors, warnings, info

    def validate_all(self) -> bool:
        """Run full validation"""
        print("=" * 80)
        print("Port Registry Validator")
        print("=" * 80)
        print(f"Registry: {self.registry_path}")
        print(f"Platform: {self.platform}")
        print()

        # Load registry validation
        print("Validating registry structure...")
        if "services" not in self.registry or not self.registry["services"]:
            print("❌ Registry missing services")
            return False

        total_services = len(self.registry["services"])
        enabled_services = sum(1 for s in self.registry["services"].values() if s.get("enabled", True))

        print(f"✅ Registry valid: {total_services} total, {enabled_services} enabled")
        print()

        # Check drift
        print("Checking for port drift...")
        errors, warnings, infos = self.check_drift()

        if errors:
            print("\n❌ Errors (services that should be running but aren't):")
            for error in errors:
                print(f"  • {error}")

        if warnings:
            print("\n⚠️  Warnings (optional services not running):")
            for warning in warnings:
                print(f"  • {warning}")

        if infos:
            print("\n❓ Info (services running but not declared):")
            for info_msg in infos[:5]:  # Show first 5
                print(f"  • {info_msg}")
            if len(infos) > 5:
                print(f"  ... and {len(infos) - 5} more")

        # Summary
        print("\n" + "=" * 80)
        if errors:
            print(f"❌ Validation failed: {len(errors)} critical drift issue(s)")
            print("=" * 80)
            return False
        else:
            print(f"✅ Validation passed")
            if warnings:
                print(f"   {len(warnings)} warning(s), {len(infos)} info message(s)")
            print("=" * 80)
            return True

    def show_running(self) -> bool:
        """Show all currently running ports"""
        print("=" * 80)
        print("Currently Running Ports")
        print("=" * 80)
        print(f"Platform: {self.platform}")
        print()

        running_lsof = self.get_running_ports_lsof()
        running_docker = self.get_running_ports_docker()

        if running_lsof:
            print("Via lsof (system processes):")
            for port in sorted(running_lsof.keys()):
                info = running_lsof[port]
                print(f"  {port:5d} - {info['process']:20s} ({info['address']})")

        if running_docker:
            print("\nVia Docker:")
            for port in sorted(running_docker.keys()):
                info = running_docker[port]
                print(f"  {port:5d} - {info['image']:30s} ({info['mapping']})")

        print("\n" + "=" * 80)
        return True

    def generate_report(self) -> Path:
        """Generate validation report"""
        errors, warnings, infos = self.check_drift()
        running_lsof = self.get_running_ports_lsof()
        running_docker = self.get_running_ports_docker()
        declared = self.get_declared_ports()

        report = {
            "generated_at": datetime.now().isoformat(),
            "platform": self.platform,
            "summary": {
                "total_declared": len(declared),
                "total_running": len({**running_lsof, **running_docker}),
                "errors": len(errors),
                "warnings": len(warnings),
                "undeclared_services": len(infos),
            },
            "errors": errors,
            "warnings": warnings,
            "undeclared": infos,
            "running_ports": {
                "lsof": running_lsof,
                "docker": running_docker,
            },
            "declared_ports": declared,
        }

        report_path = self.state_dir / f"validation_report_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"
        with open(report_path, "w") as f:
            json.dump(report, f, indent=2)

        return report_path


def main():
    import argparse

    parser = argparse.ArgumentParser(description="Validate port registry against actual system state")
    parser.add_argument("--check-drift", action="store_true", help="Only check for drift")
    parser.add_argument("--show-running", action="store_true", help="Show all running ports")
    parser.add_argument("--report", action="store_true", help="Generate validation report")

    args = parser.parse_args()

    validator = PortValidator()

    if args.show_running:
        success = validator.show_running()
    elif args.check_drift:
        errors, warnings, infos = validator.check_drift()
        print(f"Errors: {len(errors)}, Warnings: {len(warnings)}, Undeclared: {len(infos)}")
        success = len(errors) == 0
    elif args.report:
        report_path = validator.generate_report()
        print(f"Report generated: {report_path}")
        success = True
    else:
        success = validator.validate_all()
        if args.report:
            report_path = validator.generate_report()
            print(f"Report: {report_path}")

    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()
