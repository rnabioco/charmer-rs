# VHS Quickstart Tape for charmer
# Run from repo root: vhs docs/tapes/quickstart.tape

Output docs/images/quickstart.gif

Set Shell "bash"
Set FontSize 16
Set Width 900
Set Height 500
Set Theme "Catppuccin Mocha"
Set Padding 20

Type "# Quick Start with charmer"
Enter
Sleep 1s

Type "# Start monitoring"
Enter
Sleep 500ms

Type "charmer tests/pipelines/demo"
Enter
Sleep 3s

# Show basic navigation
Type "j"
Sleep 300ms
Type "j"
Sleep 300ms
Type "k"
Sleep 500ms

# Filter to running jobs
Type "f"
Sleep 1s

# Back to all
Type "f"
Type "f"
Type "f"
Type "f"
Sleep 500ms

# Quit
Type "q"
Sleep 1s
