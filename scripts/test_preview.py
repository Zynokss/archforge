import unittest
from preview import launch_expression, target, ROOT


class PreviewPlacementTests(unittest.TestCase):
    def test_editor_monitor_is_used_instead_of_focused_game_monitor(self):
        monitors = [{"id": 0, "name": "game", "focused": True}, {"id": 1, "name": "editor", "focused": False}]
        clients = [{"class": "com.microsoft.VSCode", "title": str(ROOT), "monitor": 1, "workspace": {"id": 2}}]
        monitor, workspace = target(clients, monitors)
        self.assertEqual((monitor["name"], workspace), ("editor", 2))

    def test_no_editor_fails_closed(self):
        with self.assertRaises(ValueError):
            target([], [{"id": 0, "name": "game"}])

    def test_rules_use_silent_workspace_and_disable_initial_focus(self):
        code = launch_expression({"name": "HDMI-A-1", "width": 1920, "height": 1080}, 2)
        for rule in ['monitor = "HDMI-A-1 silent"', 'workspace = "2 silent"', "no_initial_focus = true", "focus_on_activate = false", "ARCH_FORGE_PREVIEW=1"]:
            self.assertIn(rule, code)
        self.assertNotIn("hl.dsp.focus", code)

    def test_scaled_monitor_uses_logical_dimensions(self):
        code = launch_expression({"name": "scaled", "width": 3840, "height": 2160, "scale": 2}, 2)
        self.assertIn("size = {1640, 950}", code)
        self.assertIn("move = {140, 65}", code)


if __name__ == "__main__":
    unittest.main()
