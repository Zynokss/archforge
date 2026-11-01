-- Arch Forge starter. Review shortcuts before applying; log out to change sessions.
hl.monitor({ output = "", mode = "preferred", position = "auto", scale = "auto" })
hl.config({
    general = { gaps_in = 6, gaps_out = 12, border_size = 2, layout = "dwindle",
        col = { active_border = "rgb(@ACCENT@)", inactive_border = "rgb(343a40)" } },
    decoration = { rounding = 10, blur = { enabled = true, size = 4, passes = 2 } },
    input = { kb_layout = "us", follow_mouse = 1, touchpad = { natural_scroll = true } },
    dwindle = { preserve_split = true },
})
hl.on("hyprland.start", function()
    hl.exec_cmd("waybar")
    hl.exec_cmd("/usr/lib/polkit-gnome/polkit-gnome-authentication-agent-1")
end)
hl.bind("SUPER + Return", hl.dsp.exec_cmd("kitty"))
hl.bind("SUPER + D", hl.dsp.exec_cmd("fuzzel"))
hl.bind("SUPER + E", hl.dsp.exec_cmd("thunar"))
hl.bind("SUPER + SHIFT + Q", hl.dsp.window.close())
hl.bind("SUPER + Space", hl.dsp.window.float({ action = "toggle" }))
for _, direction in ipairs({ "left", "right", "up", "down" }) do
    hl.bind("SUPER + " .. direction, hl.dsp.focus({ direction = direction }))
end
for workspace = 1, 9 do
    hl.bind("SUPER + " .. workspace, hl.dsp.focus({ workspace = workspace }))
    hl.bind("SUPER + SHIFT + " .. workspace, hl.dsp.window.move({ workspace = workspace }))
end
hl.bind("SUPER + mouse:272", hl.dsp.window.drag(), { mouse = true })
hl.bind("SUPER + mouse:273", hl.dsp.window.resize(), { mouse = true })
