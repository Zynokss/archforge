#!/usr/bin/env python3
"""Open an unfocused Arch Forge preview beside VS Code on Hyprland.

No global rules, workspace switches, focus dispatches, or cursor movement.
Refuse to open when VS Code's monitor cannot be identified.
"""
import argparse
import json
from pathlib import Path
import shlex
import subprocess

ROOT = Path(__file__).resolve().parent.parent


def target(clients, monitors):
    editors = [w for w in clients if w.get("class", "").lower() in ("code", "code-oss", "com.microsoft.vscode", "vscodium")]
    editors.sort(key=lambda w: ROOT.name.lower() not in w.get("title", "").lower())
    for editor in editors:
        monitor = next((m for m in monitors if m.get("id") == editor.get("monitor")), None)
        workspace = editor.get("workspace", {}).get("id", 0)
        if monitor and workspace > 0:
            return monitor, workspace
    raise ValueError("No VS Code window on a known monitor. Preview was not opened.")


def launch_expression(monitor, workspace):
    scale = monitor.get("scale", 1) or 1
    logical_width, logical_height = int(monitor["width"] / scale), int(monitor["height"] / scale)
    width = min(1640, logical_width - 100)
    height = min(950, logical_height - 100)
    command = "exec env ARCH_FORGE_PREVIEW=1 ARCH_FORGE_THEME=crimson " + shlex.quote(str(ROOT / "target/debug/archforge"))
    rules = '{monitor = %s, workspace = %s, float = true, size = {%d, %d}, move = {%d, %d}, no_initial_focus = true, no_follow_mouse = true, focus_on_activate = false, suppress_event = "activate activatefocus", opacity = "1.0 override 1.0 override"}' % (
        json.dumps(monitor["name"] + " silent"), json.dumps(str(workspace) + " silent"), width, height,
        (logical_width - width) // 2, max(50, (logical_height - height) // 2),
    )
    return "hl.dsp.exec_cmd(" + json.dumps(command, ensure_ascii=False) + ", " + rules + ")"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    try:
        read = lambda kind: json.loads(subprocess.check_output(["hyprctl", kind, "-j"], text=True))
        clients = read("clients")
        monitor, workspace = target(clients, read("monitors"))
        expression = launch_expression(monitor, workspace)
        print(f'Preview target: {monitor["name"]}, workspace {workspace}; initial focus disabled.')
        if args.dry_run:
            print(expression)
            return
        if any(w.get("class") == "io.github.archforge.Preview" for w in clients):
            print("A preview is already open. It was left in place without changing focus.")
            return
        if not (ROOT / "target/debug/archforge").is_file():
            raise ValueError("Build the application with cargo build before opening a preview.")
        result = subprocess.run(["hyprctl", "dispatch", expression], text=True, capture_output=True, check=True)
        if "error" in (result.stdout + result.stderr).lower():
            raise ValueError(result.stdout + result.stderr)
        print(result.stdout.strip())
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        raise SystemExit(str(error)) from error


if __name__ == "__main__":
    main()
