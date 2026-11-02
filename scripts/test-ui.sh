#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
# Separate display and D-Bus session. Windows never appear on either monitor.
ui_config_dir=$(mktemp -d /tmp/archforge-ui-config.XXXXXX)
trap 'rm -rf -- "$ui_config_dir"' EXIT
export XDG_CONFIG_HOME="$ui_config_dir"
export ARCH_FORGE_THEME=crimson GDK_BACKEND=x11 GSK_RENDERER=cairo GTK_A11Y=none
if (( $# == 0 )); then
  set -- cargo test native_ui_workflows -- --ignored --test-threads=1
fi
xvfb-run -a -s '-screen 0 1740x1060x24 -extension RANDR' dbus-run-session -- env -u WAYLAND_DISPLAY XDG_CURRENT_DESKTOP=XFCE "$@"
