#!/usr/bin/env python3
"""Exercise CLI terminal lifecycle on private PTYs; never opens a desktop window."""
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import signal
import struct
import subprocess
import tempfile
import termios
import time

ROOT = Path(__file__).resolve().parent.parent


def build_probe():
    result = subprocess.run(
        ["cargo", "test", "--offline", "--no-default-features", "--bin",
         "archforge-cli", "--no-run", "--message-format=json"],
        cwd=ROOT, text=True, stdout=subprocess.PIPE, check=True,
    )
    for line in result.stdout.splitlines():
        item = json.loads(line)
        if item.get("reason") == "compiler-artifact" and item.get("executable"):
            return item["executable"]
    raise RuntimeError("test executable was not produced")


def check(executable, mode):
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 110, 0, 0))

    def session():
        os.setsid()
        fcntl.ioctl(0, termios.TIOCSCTTY, 0)

    with tempfile.TemporaryDirectory(prefix="archforge-cli-lifecycle-") as temp:
        marker = Path(temp) / "ready"
        env = dict(os.environ, ARCHFORGE_TEST_READY=str(marker))
        if mode == "reader-hangup":
            env["ARCHFORGE_TEST_EVENT_READER"] = "1"
        error_log = Path(temp) / "stderr"
        with error_log.open("w") as errors:
            child = subprocess.Popen(
                [executable, "--ignored", "--exact", "terminal_regression::session", "--nocapture"],
                stdin=slave, stdout=slave, stderr=errors, env=env, preexec_fn=session,
            )
        os.close(slave)

        def drain():
            if master is not None and select.select([master], [], [], 0)[0]:
                try:
                    os.read(master, 65536)
                except OSError:
                    pass

        try:
            deadline = time.monotonic() + 10
            while not marker.exists():
                drain()
                if child.poll() is not None or time.monotonic() > deadline:
                    raise AssertionError(f"{mode}: probe did not start")
                time.sleep(0.02)
            # Let the reader block in poll, draining terminal output as needed.
            for _ in range(25):
                drain()
                time.sleep(0.02)
            assert child.poll() is None, f"{mode}: exited before lifecycle trigger"
            if mode in ("hangup", "reader-hangup"):
                os.close(master)
                master = None
            elif mode == "quit":
                os.write(master, b"q")
            else:
                child.send_signal({"sigterm": signal.SIGTERM, "sighup": signal.SIGHUP}[mode])
            deadline = time.monotonic() + 3
            while child.poll() is None and time.monotonic() < deadline:
                drain()
                time.sleep(0.02)
            assert child.poll() == 0, (
                f"{mode}: failed or stuck (status={child.poll()}): {error_log.read_text()}"
            )
            print(f"PASS {mode}: exited promptly")
        finally:
            if child.poll() is None:
                child.kill()
                child.wait()
            if master is not None:
                os.close(master)


if __name__ == "__main__":
    executable = build_probe()
    for mode in ("reader-hangup", "hangup", "sigterm", "sighup", "quit"):
        check(executable, mode)
