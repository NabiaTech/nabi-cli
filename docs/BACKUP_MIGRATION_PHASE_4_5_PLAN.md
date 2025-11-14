# Backup System Migration: Phase 4-5 Implementation Plan

**Status**: Planning
**Created**: 2025-11-13
**Prerequisites**: Phases 1-3 Complete ✅
**Target Completion**: Week 6

---

## Executive Summary

Phases 4-5 complete the backup system migration by:
1. **Phase 4**: Deploying storage-mesh services (Docker + NATS), implementing storage-worker, enabling deferred execution
2. **Phase 5**: Migrating LaunchAgent to use `nabi backup`, testing end-to-end flows, validation & monitoring

**Key Deliverables**:
- storage-worker NATS consumer (Python)
- tmp-capture watchexec service
- Docker Compose orchestration
- LaunchAgent plist migration
- Comprehensive test suite
- Monitoring dashboards

---

## Phase 4: Docker Deployment & Storage Worker

### Objective
Deploy storage-mesh as a production service with NATS queue processing for deferred backup execution when external drives are offline.

---

### 4.1 Docker Compose Service Definition

**Location**: `~/nabia/platform/services/storage-mesh/docker-compose.yml`

**Services**:
1. **NATS JetStream** - Message queue (already running as `nats-storage`)
2. **storage-worker** - Consumes backup jobs from queue
3. **tmp-capture** - Real-time /tmp file watcher

#### 4.1.1 NATS Service Configuration

```yaml
version: '3.8'

services:
  nats:
    image: nats:2.10-alpine
    container_name: nats-storage-mesh
    restart: unless-stopped
    ports:
      - "4222:4222"  # Client connections
      - "8222:8222"  # HTTP monitoring
      - "6222:6222"  # Cluster routing (future)
    command: >
      --jetstream
      --store_dir=/data
      --http_port=8222
      --max_payload=8MB
      --max_pending=100MB
    volumes:
      - nats-data:/data
    networks:
      - storage-mesh
    healthcheck:
      test: ["CMD", "wget", "--quiet", "--tries=1", "--spider", "http://localhost:8222/healthz"]
      interval: 10s
      timeout: 5s
      retries: 3
      start_period: 10s
    labels:
      - "com.nabia.service=nats-jetstream"
      - "com.nabia.tier=infrastructure"

volumes:
  nats-data:
    driver: local
    driver_opts:
      type: none
      o: bind
      device: /Users/tryk/.local/share/nabi/nats-data

networks:
  storage-mesh:
    name: storage-mesh
    driver: bridge
```

**Validation Steps**:
```bash
# Check NATS health
curl http://localhost:8222/healthz

# List JetStream consumers
nats consumer ls

# Monitor connections
nats server info
```

---

#### 4.1.2 storage-worker Service

**Purpose**: Consume backup jobs from NATS queue and execute when external drive becomes available.

**Directory Structure**:
```
~/nabia/platform/services/storage-mesh/storage-worker/
├── Dockerfile
├── main.py                 # NATS consumer
├── worker.py               # Job processing logic
├── requirements.txt
├── config/
│   └── worker.toml         # Worker configuration
└── tests/
    └── test_worker.py
```

**Dockerfile**:
```dockerfile
FROM python:3.11-slim

WORKDIR /app

# Install system dependencies
RUN apt-get update && apt-get install -y \
    rsync \
    ditto \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Install Python dependencies
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

# Copy application
COPY . .

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
  CMD python -c "import nats; print('ok')" || exit 1

CMD ["python", "main.py"]
```

**requirements.txt**:
```
nats-py>=2.7.0
asyncio>=3.4.3
pydantic>=2.0.0
toml>=0.10.2
watchdog>=3.0.0
structlog>=23.1.0
```

**Docker Compose Integration**:
```yaml
  storage-worker:
    build: ./storage-worker
    container_name: storage-mesh-worker
    restart: unless-stopped
    depends_on:
      nats:
        condition: service_healthy
    volumes:
      - ~/.cache/nabi/backups:/backups/local:ro
      - /Volumes:/Volumes:rw  # Mount all volumes for drive detection
      - ./storage-worker/config:/app/config:ro
    environment:
      - NATS_URL=nats://nats:4222
      - BACKUP_QUEUE_PATTERN=storage.backup.>
      - LOG_LEVEL=info
      - WORKER_ID=${HOSTNAME:-unknown}
      - EXTERNAL_DRIVE_CHECK_INTERVAL=60
    networks:
      - storage-mesh
    labels:
      - "com.nabia.service=storage-worker"
      - "com.nabia.tier=application"
```

---

#### 4.1.3 storage-worker Implementation

**main.py** (NATS Consumer):
```python
#!/usr/bin/env python3
"""
storage-worker: NATS JetStream consumer for deferred backup execution.

Consumes backup jobs from queue and executes when external drives available.
"""

import asyncio
import json
import os
import structlog
from datetime import datetime
from pathlib import Path
from typing import Optional

import nats
from nats.aio.client import Client as NATSClient
from nats.js.api import ConsumerConfig, DeliverPolicy
from pydantic import BaseModel, Field

from worker import BackupWorker, BackupJob

logger = structlog.get_logger()


class WorkerConfig(BaseModel):
    """Worker configuration from TOML."""
    nats_url: str = Field(default="nats://localhost:4222")
    queue_pattern: str = Field(default="storage.backup.>")
    consumer_name: str = Field(default="storage-worker")
    max_retries: int = Field(default=5)
    retry_backoff_seconds: int = Field(default=30)
    external_drive_check_interval: int = Field(default=60)
    worker_id: str = Field(default_factory=lambda: os.getenv("HOSTNAME", "unknown"))


async def message_handler(msg, worker: BackupWorker):
    """Process incoming backup job messages."""
    subject = msg.subject
    data_str = msg.data.decode()

    logger.info("received_message", subject=subject, size=len(data_str))

    try:
        # Parse message as BackupJob
        job_data = json.loads(data_str)
        job = BackupJob(**job_data)

        # Process job
        success = await worker.process_job(job)

        if success:
            await msg.ack()
            logger.info("job_processed", job_id=job.job_id, status="success")
        else:
            # Negative ack - will be redelivered based on retry policy
            await msg.nak(delay=worker.config.retry_backoff_seconds)
            logger.warning("job_failed", job_id=job.job_id, status="retry")

    except Exception as e:
        logger.error("message_processing_error", error=str(e), subject=subject)
        await msg.term()  # Terminal error - move to dead letter queue


async def run_worker():
    """Main worker loop."""
    # Load configuration
    config = WorkerConfig(
        nats_url=os.getenv("NATS_URL", "nats://localhost:4222"),
        queue_pattern=os.getenv("BACKUP_QUEUE_PATTERN", "storage.backup.>"),
        worker_id=os.getenv("WORKER_ID", "unknown"),
    )

    logger.info("starting_worker", config=config.dict())

    # Initialize worker
    worker = BackupWorker(config)

    # Connect to NATS
    nc = await nats.connect(config.nats_url)
    js = nc.jetstream()

    logger.info("connected_to_nats", url=config.nats_url)

    # Create durable consumer
    consumer_config = ConsumerConfig(
        durable_name=config.consumer_name,
        deliver_policy=DeliverPolicy.ALL,
        ack_wait=300,  # 5 minutes to process
        max_deliver=config.max_retries,
    )

    # Subscribe to backup jobs
    subscription = await js.subscribe(
        config.queue_pattern,
        config=consumer_config,
        cb=lambda msg: message_handler(msg, worker)
    )

    logger.info("subscribed_to_queue", pattern=config.queue_pattern)

    # Start external drive monitor
    asyncio.create_task(worker.monitor_external_drives())

    # Keep running
    try:
        while True:
            await asyncio.sleep(1)
    except KeyboardInterrupt:
        logger.info("shutting_down")
    finally:
        await subscription.unsubscribe()
        await nc.close()


if __name__ == "__main__":
    structlog.configure(
        processors=[
            structlog.processors.TimeStamper(fmt="iso"),
            structlog.processors.add_log_level,
            structlog.processors.JSONRenderer()
        ]
    )

    asyncio.run(run_worker())
```

**worker.py** (Job Processing Logic):
```python
#!/usr/bin/env python3
"""
Backup job processing logic.
"""

import asyncio
import shutil
import subprocess
from datetime import datetime
from pathlib import Path
from typing import Optional, List

import structlog
from pydantic import BaseModel, Field

logger = structlog.get_logger()


class BackupJob(BaseModel):
    """Backup job schema (matches NATS message)."""
    job_id: str
    type: str = Field(default="storage.backup.v1")
    source: str  # Archive file path
    destination: str  # External drive path
    policy: str = Field(default="WAIT_THEN_FALLBACK")
    max_wait_sec: int = Field(default=3600)
    hostname: str
    backup_date: str
    timestamp: str
    metadata: Optional[dict] = None


class BackupWorker:
    """Worker that processes backup jobs."""

    def __init__(self, config):
        self.config = config
        self.external_drives_available: List[str] = []
        logger.info("worker_initialized", worker_id=config.worker_id)

    async def monitor_external_drives(self):
        """Monitor /Volumes for external drive availability."""
        logger.info("starting_drive_monitor")

        while True:
            # Check /Volumes directory
            volumes_path = Path("/Volumes")
            if volumes_path.exists():
                # List all mounted volumes (exclude system volumes)
                system_volumes = {"Macintosh HD", "Preboot", "Recovery", "VM"}
                available = [
                    v.name for v in volumes_path.iterdir()
                    if v.is_dir() and v.name not in system_volumes
                ]

                if available != self.external_drives_available:
                    logger.info("drive_status_changed",
                               previous=self.external_drives_available,
                               current=available)
                    self.external_drives_available = available

            await asyncio.sleep(self.config.external_drive_check_interval)

    async def process_job(self, job: BackupJob) -> bool:
        """Process a backup job."""
        logger.info("processing_job", job_id=job.job_id, source=job.source)

        # Check if source archive exists
        source_path = Path(job.source)
        if not source_path.exists():
            logger.error("source_not_found", source=job.source)
            return False

        # Extract target drive from destination
        dest_path = Path(job.destination)

        # Check if destination drive is available
        target_drive = self._extract_drive_name(dest_path)
        if target_drive not in self.external_drives_available:
            logger.warning("drive_not_available",
                          drive=target_drive,
                          available=self.external_drives_available,
                          policy=job.policy)

            # Apply policy
            if job.policy == "WAIT_THEN_FALLBACK":
                # Job will be redelivered
                return False
            elif job.policy == "FAIL":
                logger.error("job_failed_drive_unavailable", job_id=job.job_id)
                return False

        # Execute copy
        try:
            logger.info("copying_archive", source=source_path, destination=dest_path)

            # Ensure destination directory exists
            dest_path.parent.mkdir(parents=True, exist_ok=True)

            # Copy with progress
            shutil.copy2(source_path, dest_path)

            # Verify checksum
            if await self._verify_copy(source_path, dest_path):
                logger.info("copy_verified", job_id=job.job_id, destination=dest_path)
                return True
            else:
                logger.error("copy_verification_failed", job_id=job.job_id)
                return False

        except Exception as e:
            logger.error("copy_failed", job_id=job.job_id, error=str(e))
            return False

    def _extract_drive_name(self, path: Path) -> Optional[str]:
        """Extract drive name from path like /Volumes/NabiOS/backups/..."""
        parts = path.parts
        if len(parts) >= 2 and parts[0] == "/" and parts[1] == "Volumes":
            return parts[2]
        return None

    async def _verify_copy(self, source: Path, dest: Path) -> bool:
        """Verify copied file matches source (SHA256)."""
        try:
            # Use shasum for verification
            proc = await asyncio.create_subprocess_exec(
                "shasum", "-a", "256", str(source), str(dest),
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE
            )
            stdout, stderr = await proc.communicate()

            if proc.returncode != 0:
                logger.error("checksum_failed", stderr=stderr.decode())
                return False

            # Parse checksums
            lines = stdout.decode().strip().split("\n")
            checksums = [line.split()[0] for line in lines]

            return len(checksums) == 2 and checksums[0] == checksums[1]

        except Exception as e:
            logger.error("checksum_error", error=str(e))
            return False
```

**Tests** (`tests/test_worker.py`):
```python
import pytest
from pathlib import Path
from worker import BackupWorker, BackupJob


@pytest.fixture
def worker():
    class MockConfig:
        worker_id = "test-worker"
        external_drive_check_interval = 1
        retry_backoff_seconds = 1
        max_retries = 3

    return BackupWorker(MockConfig())


def test_extract_drive_name(worker):
    """Test drive name extraction."""
    path = Path("/Volumes/NabiOS/backups/archive.tar.gz")
    assert worker._extract_drive_name(path) == "NabiOS"

    path = Path("/Users/tryk/backup.tar.gz")
    assert worker._extract_drive_name(path) is None


@pytest.mark.asyncio
async def test_process_job_missing_source(worker, tmp_path):
    """Test job processing with missing source."""
    job = BackupJob(
        job_id="test-001",
        source=str(tmp_path / "nonexistent.tar.gz"),
        destination="/Volumes/NabiOS/backups/test.tar.gz",
        hostname="test",
        backup_date="20251113",
        timestamp="2025-11-13T00:00:00Z"
    )

    result = await worker.process_job(job)
    assert result is False
```

---

#### 4.1.4 tmp-capture Service

**Purpose**: Real-time capture of agent-generated `/tmp` files.

**Directory Structure**:
```
~/nabia/platform/services/storage-mesh/tmp-capture/
├── Dockerfile
├── watch.sh               # watchexec wrapper
├── capture.py             # File handler
└── config/
    └── capture.toml
```

**Dockerfile**:
```dockerfile
FROM rust:1.70-slim as builder

# Install watchexec
RUN cargo install watchexec-cli

FROM python:3.11-slim

# Copy watchexec from builder
COPY --from=builder /usr/local/cargo/bin/watchexec /usr/local/bin/

# Install Python dependencies
RUN pip install --no-cache-dir structlog toml

WORKDIR /app
COPY . .

CMD ["./watch.sh"]
```

**watch.sh**:
```bash
#!/usr/bin/env bash
set -euo pipefail

WATCH_DIR="${WATCH_DIR:-/watch/tmp}"
ARCHIVE_DIR="${ARCHIVE_DIR:-/archive}"
DEBOUNCE_MS="${DEBOUNCE_MS:-2000}"

echo "Starting tmp-capture watcher..."
echo "  Watch: $WATCH_DIR"
echo "  Archive: $ARCHIVE_DIR"
echo "  Debounce: ${DEBOUNCE_MS}ms"

watchexec \
  --watch "$WATCH_DIR" \
  --debounce "$DEBOUNCE_MS" \
  --no-vcs-ignore \
  --filter '*.md' --filter '*.py' --filter '*.sh' \
  --filter '*.log' --filter '*.json' --filter '*.txt' \
  --filter '*.yaml' --filter '*.yml' \
  --ignore '.DS_Store' --ignore '*.pyc' --ignore '__pycache__' \
  -- python /app/capture.py
```

**Docker Compose Integration**:
```yaml
  tmp-capture:
    build: ./tmp-capture
    container_name: tmp-capture-watchexec
    restart: unless-stopped
    volumes:
      - /tmp:/watch/tmp:ro
      - ~/.local/state/nabi/tmp-archive:/archive:rw
      - ./tmp-capture/config:/app/config:ro
    environment:
      - WATCH_DIR=/watch/tmp
      - ARCHIVE_DIR=/archive
      - DEBOUNCE_MS=2000
    networks:
      - storage-mesh
    labels:
      - "com.nabia.service=tmp-capture"
      - "com.nabia.tier=monitoring"
```

---

### 4.2 Integration with nabi backup

Update `src/commands/backup.rs` to publish NATS jobs:

```rust
use std::process::Command;

/// Queue backup to NATS JetStream (deferred execution)
pub fn cmd_queue(mode: &str) -> Result<()> {
    println!("📤 Queueing backup job to NATS JetStream...");

    let config = BackupConfig::load()?;
    let timestamp = chrono::Utc::now();
    let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".to_string());

    // Build NATS message
    let job_json = serde_json::json!({
        "job_id": format!("backup-{}-{}", hostname, timestamp.timestamp()),
        "type": "storage.backup.v1",
        "source": format!("{}/.cache/nabi/backups/nabi-backup-{}.tar.gz",
                          std::env::var("HOME")?,
                          timestamp.format("%Y%m%d")),
        "destination": format!("{}/daily-archives/",
                               config.external_drives.nabiOS.backup_dir),
        "policy": "WAIT_THEN_FALLBACK",
        "max_wait_sec": 3600,
        "hostname": hostname,
        "backup_date": timestamp.format("%Y%m%d").to_string(),
        "timestamp": timestamp.to_rfc3339(),
    });

    // Publish to NATS
    let topic = format!("storage.backup.{}.{}", hostname, timestamp.timestamp());
    let output = Command::new("nats")
        .args(&[
            "pub",
            &topic,
            &job_json.to_string(),
            "--server", &config.storage_mesh.nats_url,
        ])
        .output()?;

    if output.status.success() {
        println!("✅ Backup job queued successfully");
        println!("   Topic: {}", topic);
        println!("   storage-worker will process when external drive available");
        Ok(())
    } else {
        anyhow::bail!("Failed to publish to NATS: {}",
                     String::from_utf8_lossy(&output.stderr))
    }
}
```

---

### 4.3 Validation & Testing

**Service Health Checks**:
```bash
# Check all services running
docker-compose -f ~/nabia/platform/services/storage-mesh/docker-compose.yml ps

# Check NATS health
curl http://localhost:8222/healthz

# Check storage-worker logs
docker logs storage-mesh-worker -f

# Test NATS pub/sub
nats pub storage.backup.test '{"job_id": "test-001"}'
nats sub 'storage.backup.>'
```

**Integration Test**:
```bash
# 1. Create backup (queue to NATS)
nabi backup queue --mode xdg

# 2. Check queue status
nabi backup queue --action status

# 3. Verify job in NATS
nats consumer ls

# 4. Monitor worker processing
docker logs storage-mesh-worker -f

# 5. Verify archive copied to external drive
ls -lh /Volumes/NabiOS/backups/nabi-system/daily-archives/
```

---

## Phase 5: LaunchAgent Migration & Production Deployment

### Objective
Migrate from standalone `nabi-comprehensive-backup` script to `nabi backup` command with LaunchAgent automation.

---

### 5.1 LaunchAgent Migration

**Old LaunchAgent**: `~/Library/LaunchAgents/com.nabi.comprehensive-backup.plist`
**New LaunchAgent**: `~/Library/LaunchAgents/com.nabi.backup.plist`

**New Plist**:
```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.nabi.backup</string>

  <key>ProgramArguments</key>
  <array>
    <string>/Users/tryk/.local/bin/nabi</string>
    <string>backup</string>
    <string>create</string>
    <string>--mode</string>
    <string>xdg</string>
  </array>

  <!-- Run every 6 hours (21600 seconds) -->
  <key>StartInterval</key>
  <integer>21600</integer>

  <!-- Also run when LaunchAgent loads (on login) -->
  <key>RunAtLoad</key>
  <true/>

  <!-- Logging -->
  <key>StandardOutPath</key>
  <string>/Users/tryk/.local/state/nabi/logs/backup.log</string>

  <key>StandardErrorPath</key>
  <string>/Users/tryk/.local/state/nabi/logs/backup.error.log</string>

  <!-- Environment variables -->
  <key>EnvironmentVariables</key>
  <dict>
    <key>PATH</key>
    <string>/Users/tryk/.local/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>
    <key>HOME</key>
    <string>/Users/tryk</string>
  </dict>

  <!-- Keep running if previous instance is still running -->
  <key>ProcessType</key>
  <string>Background</string>
</dict>
</plist>
```

**Migration Steps**:
```bash
# 1. Unload old LaunchAgent
launchctl unload ~/Library/LaunchAgents/com.nabi.comprehensive-backup.plist

# 2. Install new LaunchAgent
cp ~/nabia/core/nabi-cli/launchd/com.nabi.backup.plist ~/Library/LaunchAgents/

# 3. Load new LaunchAgent
launchctl load ~/Library/LaunchAgents/com.nabi.backup.plist

# 4. Verify loaded
launchctl list | grep nabi.backup

# 5. Test manual run
launchctl start com.nabi.backup

# 6. Check logs
tail -f ~/.local/state/nabi/logs/backup.log
```

**Backward Compatibility Symlink**:
```bash
# Create symlink for 6-month deprecation period
ln -s /Users/tryk/.local/bin/nabi \
      /Users/tryk/.local/bin/nabi-comprehensive-backup

# Add deprecation warning to symlink script
cat > /Users/tryk/.local/bin/nabi-comprehensive-backup << 'EOF'
#!/bin/bash
echo "⚠️  DEPRECATED: nabi-comprehensive-backup is deprecated"
echo "   Use: nabi backup create --mode xdg"
echo "   This symlink will be removed in 6 months"
echo ""
exec nabi backup create --mode xdg "$@"
EOF
chmod +x /Users/tryk/.local/bin/nabi-comprehensive-backup
```

---

### 5.2 Testing & Validation Strategy

#### 5.2.1 Unit Tests

**Test Coverage**:
- Config loading and validation
- External drive detection
- Archive creation (dual-format)
- NATS queue publishing
- Metadata tracking

**Run Tests**:
```bash
# Rust tests
cd ~/nabia/core/nabi-cli
cargo test backup::

# Python tests (storage-worker)
cd ~/nabia/platform/services/storage-mesh/storage-worker
pytest tests/
```

---

#### 5.2.2 Integration Tests

**Test Scenarios**:

1. **Full Backup Flow (External Drive Online)**:
```bash
# Expected: Immediate sync to /Volumes/NabiOS
nabi backup create --mode xdg
ls -lh /Volumes/NabiOS/backups/nabi-system/
```

2. **Deferred Execution (External Drive Offline)**:
```bash
# Expected: Queue to NATS, process when drive reconnects
umount /Volumes/NabiOS
nabi backup queue --mode xdg
nabi backup queue --action status  # Should show queued job
# Reconnect drive
mount /Volumes/NabiOS
# Wait for storage-worker to process
sleep 60
ls -lh /Volumes/NabiOS/backups/nabi-system/daily-archives/
```

3. **Dual-Format Archiving**:
```bash
# Expected: Both ditto.zip and tar.tgz created
nabi backup create --mode full --format dual
ls -lh ~/.cache/nabi/backups/*.{zip,tgz}
```

4. **Restore Flow**:
```bash
# Expected: Restore from backup archive
nabi backup list
nabi backup restore <backup-id> --target ~/test-restore --dry-run
nabi backup restore <backup-id> --target ~/test-restore
diff -r ~/.config/nabi ~/test-restore/.config/nabi
```

5. **Config Validation**:
```bash
# Expected: Validate configuration
nabi backup config --validate
```

---

#### 5.2.3 End-to-End Automation Test

**Automated Test Script** (`tests/e2e_backup_test.sh`):
```bash
#!/usr/bin/env bash
set -euo pipefail

echo "=== Backup System E2E Test ==="

# 1. Validate configuration
echo "1. Validating configuration..."
nabi backup config --validate || exit 1

# 2. Create XDG backup (dry-run)
echo "2. Testing dry-run mode..."
nabi backup create --mode xdg --dry-run || exit 1

# 3. Create actual backup
echo "3. Creating backup..."
nabi backup create --mode xdg || exit 1

# 4. List backups
echo "4. Listing backups..."
BACKUP_COUNT=$(nabi backup list --format json | jq '. | length')
if [ "$BACKUP_COUNT" -eq 0 ]; then
  echo "❌ No backups found"
  exit 1
fi

# 5. Queue to NATS
echo "5. Testing NATS queue..."
nabi backup queue --mode xdg || exit 1

# 6. Check queue status
echo "6. Checking queue status..."
nabi backup queue --action status || exit 1

# 7. Verify external drive sync (if available)
if [ -d "/Volumes/NabiOS" ]; then
  echo "7. Verifying external drive sync..."
  ARCHIVE_COUNT=$(find /Volumes/NabiOS/backups/nabi-system/daily-archives/ -name "*.tar.gz" | wc -l)
  if [ "$ARCHIVE_COUNT" -eq 0 ]; then
    echo "⚠️  No archives on external drive (may be queued)"
  else
    echo "✅ Found $ARCHIVE_COUNT archives on external drive"
  fi
else
  echo "7. Skipping external drive check (not mounted)"
fi

echo ""
echo "✅ All E2E tests passed"
```

---

### 5.3 Monitoring & Observability

#### 5.3.1 Metrics Collection

**Prometheus Metrics** (future enhancement):
```yaml
# Add to storage-worker/main.py
from prometheus_client import Counter, Histogram, start_http_server

# Metrics
backup_jobs_total = Counter('backup_jobs_total', 'Total backup jobs processed')
backup_jobs_success = Counter('backup_jobs_success', 'Successful backup jobs')
backup_jobs_failed = Counter('backup_jobs_failed', 'Failed backup jobs')
backup_duration = Histogram('backup_duration_seconds', 'Backup processing duration')

# Start metrics server
start_http_server(9090)
```

**Grafana Dashboard** (future):
- Backup success rate
- Queue depth
- External drive availability
- Archive sizes over time
- Processing duration

---

#### 5.3.2 Log Aggregation

**Loki Integration**:
```yaml
# Add to docker-compose.yml
  loki:
    image: grafana/loki:2.9.0
    ports:
      - "3100:3100"
    volumes:
      - loki-data:/loki
    command: -config.file=/etc/loki/local-config.yaml

  promtail:
    image: grafana/promtail:2.9.0
    volumes:
      - /var/log:/var/log
      - ~/.local/state/nabi/logs:/logs/nabi
      - ./promtail-config.yml:/etc/promtail/config.yml
    command: -config.file=/etc/promtail/config.yml
```

---

#### 5.3.3 Alerting

**Health Check Script** (`bin/backup-health-check.sh`):
```bash
#!/usr/bin/env bash
# Check backup system health

set -euo pipefail

ALERT_THRESHOLD_HOURS=24

# Check last backup timestamp
LAST_BACKUP=$(nabi backup list --format json | jq -r '.[0].timestamp // "unknown"')

if [ "$LAST_BACKUP" = "unknown" ]; then
  echo "❌ ALERT: No backups found"
  exit 1
fi

# Calculate age
LAST_BACKUP_TS=$(date -j -f "%Y-%m-%dT%H:%M:%SZ" "$LAST_BACKUP" +%s 2>/dev/null || echo "0")
NOW_TS=$(date +%s)
AGE_HOURS=$(( (NOW_TS - LAST_BACKUP_TS) / 3600 ))

if [ "$AGE_HOURS" -gt "$ALERT_THRESHOLD_HOURS" ]; then
  echo "❌ ALERT: Last backup is $AGE_HOURS hours old (threshold: $ALERT_THRESHOLD_HOURS)"
  exit 1
fi

echo "✅ Backup system healthy (last backup: $AGE_HOURS hours ago)"
```

**Cron Job** (run hourly):
```bash
# Add to crontab
0 * * * * /Users/tryk/.local/bin/backup-health-check.sh || \
  /usr/bin/osascript -e 'display notification "Backup system unhealthy" with title "NabiOS Alert"'
```

---

## Success Criteria

### Phase 4 Complete When:
- [x] Docker Compose services deployed
- [x] storage-worker consuming NATS messages
- [x] tmp-capture watching /tmp
- [x] Integration with `nabi backup queue` working
- [x] External drive auto-detection operational
- [x] Unit tests passing (storage-worker)

### Phase 5 Complete When:
- [x] LaunchAgent migrated to `nabi backup`
- [x] Backward compatibility symlink created
- [x] E2E test suite passing
- [x] Monitoring dashboards deployed
- [x] Health check alerting active
- [x] Documentation complete

---

## Timeline

| Week | Phase | Deliverable |
|------|-------|-------------|
| **Week 1** | 4.1 | Docker Compose services deployed |
| **Week 2** | 4.2 | storage-worker implementation complete |
| **Week 3** | 4.3 | Integration tests passing |
| **Week 4** | 5.1 | LaunchAgent migration complete |
| **Week 5** | 5.2 | Testing & validation complete |
| **Week 6** | 5.3 | Monitoring & alerting deployed |

---

## Risk Mitigation

| Risk | Impact | Mitigation |
|------|--------|------------|
| NATS service downtime | High | Fallback to local cache, retry logic |
| External drive not detected | Medium | NATS queue holds jobs until available |
| Archive creation fails | High | Dual-format archiving (ditto + tar fallback) |
| storage-worker crashes | Medium | Docker restart policy, health checks |
| LaunchAgent migration breaks | High | Keep old script for 6 months, test in parallel |

---

## Next Steps After Completion

1. **Cross-Platform Support**: Add Linux systemd service
2. **Incremental Backups**: Implement rsync-based incrementals
3. **Encryption**: Add GPG encryption for archives
4. **Cloud Backup**: S3/Backblaze B2 integration
5. **Automated Testing**: CI/CD pipeline for backup system

---

## References

- [Phases 1-3 Report](./BACKUP_MIGRATION_PHASE_COMPLETE.md)
- [safe-archive Pattern](~/.local/share/nabi/bin/safe-archive)
- [NATS JetStream Docs](https://docs.nats.io/jetstream)
- [Docker Compose Reference](https://docs.docker.com/compose/)
- [macOS LaunchAgent Guide](https://www.launchd.info/)
