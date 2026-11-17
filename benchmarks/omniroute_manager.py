#!/usr/bin/env python3
"""OmniRoute lifecycle manager.

Controls the background OmniRoute server process (TCP port 20128)
for benchmark harness runs. Strict adherence to zero emojis.
"""

import argparse
import os
import shutil
import socket
import subprocess
import sys
import time
from pathlib import Path


class OmniRouteManager:
    """Manages the lifecycle of the local OmniRoute server."""

    def __init__(
        self,
        host: str = "127.0.0.1",
        port: int = 20128,
        timeout: float = 15.0,
    ) -> None:
        self.host = host
        self.port = port
        self.timeout = timeout
        self.process: subprocess.Popen | None = None
        self.started_by_us: bool = False

    def is_listening(self) -> bool:
        """Check if TCP port is accepting connections on host."""
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.settimeout(1.0)
        try:
            res = sock.connect_ex((self.host, self.port))
            return res == 0
        except OSError:
            return False
        finally:
            sock.close()

    def start(self, timeout: float | None = None) -> bool:
        """Start OmniRoute server in background if not already listening.

        Returns True if listening, raises TimeoutError or RuntimeError on failure.
        """
        if self.is_listening():
            self.started_by_us = False
            return True

        timeout_sec = timeout if timeout is not None else self.timeout

        omniroute_cmd = shutil.which("omniroute") or "omniroute"
        args = [omniroute_cmd, "serve", "--no-open", "--no-tray"]

        use_shell = sys.platform == "win32"
        try:
            self.process = subprocess.Popen(
                args,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                shell=use_shell,
            )
        except Exception as exc:
            raise RuntimeError(f"Failed to spawn OmniRoute process: {exc}") from exc

        self.started_by_us = True

        deadline = time.perf_counter() + timeout_sec
        while time.perf_counter() < deadline:
            if self.is_listening():
                return True
            if self.process.poll() is not None:
                self.started_by_us = False
                exit_code = self.process.poll()
                self.process = None
                raise RuntimeError(
                    f"OmniRoute process exited prematurely with exit code {exit_code}"
                )
            time.sleep(0.25)

        self.stop()
        raise TimeoutError(
            f"OmniRoute failed to listen on {self.host}:{self.port} within {timeout_sec}s"
        )

    def stop(self, force: bool = False) -> bool:
        """Stop OmniRoute server if started by us, or force terminate if requested."""
        if not self.started_by_us and not force:
            return False

        if self.process is not None:
            pid = self.process.pid
            try:
                if sys.platform == "win32":
                    subprocess.run(
                        ["taskkill", "/F", "/T", "/PID", str(pid)],
                        stdout=subprocess.DEVNULL,
                        stderr=subprocess.DEVNULL,
                        check=False,
                    )
                    try:
                        self.process.wait(timeout=3.0)
                    except Exception:
                        pass
                else:
                    self.process.terminate()
                    try:
                        self.process.wait(timeout=3.0)
                    except subprocess.TimeoutExpired:
                        self.process.kill()
            except Exception:
                pass
            finally:
                self.process = None
                self.started_by_us = False

        deadline = time.perf_counter() + 5.0
        while time.perf_counter() < deadline:
            if not self.is_listening():
                break
            time.sleep(0.2)

        return True

    def __enter__(self) -> "OmniRouteManager":
        self.start()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        self.stop()


def main() -> int:
    parser = argparse.ArgumentParser(
        description="OmniRoute lifecycle manager CLI"
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Check if OmniRoute is listening on 127.0.0.1:20128",
    )
    parser.add_argument(
        "--start",
        action="store_true",
        help="Start OmniRoute server if not listening",
    )
    parser.add_argument(
        "--stop",
        action="store_true",
        help="Stop OmniRoute server if running (force termination)",
    )
    parser.add_argument(
        "--port",
        type=int,
        default=20128,
        help="Target port (default: 20128)",
    )
    parser.add_argument(
        "--host",
        type=str,
        default="127.0.0.1",
        help="Target host (default: 127.0.0.1)",
    )
    args = parser.parse_args()

    manager = OmniRouteManager(host=args.host, port=args.port)

    if args.stop:
        if manager.is_listening():
            print(f"[INFO] Stopping OmniRoute on {args.host}:{args.port}...")
            manager.stop(force=True)
            if not manager.is_listening():
                print("[OK] OmniRoute server stopped successfully.")
                return 0
            print("[FAIL] Failed to stop OmniRoute server.")
            return 1
        print(f"[INFO] OmniRoute is not running on {args.host}:{args.port}.")
        return 0

    if args.start:
        if manager.is_listening():
            print(f"[OK] OmniRoute is already listening on {args.host}:{args.port}.")
            return 0
        print(f"[INFO] Starting OmniRoute on {args.host}:{args.port}...")
        try:
            manager.start()
            print(f"[OK] OmniRoute started and listening on {args.host}:{args.port}.")
            return 0
        except Exception as exc:
            print(f"[FAIL] Could not start OmniRoute: {exc}")
            return 1

    if args.check or len(sys.argv) == 1:
        listening = manager.is_listening()
        if listening:
            print(f"[OK] OmniRoute is listening on {args.host}:{args.port}.")
            return 0
        print(f"[FAIL] OmniRoute is not listening on {args.host}:{args.port}.")
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
