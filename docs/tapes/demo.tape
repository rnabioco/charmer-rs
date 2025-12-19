# VHS Demo Tape for charmer
# Run from repo root: vhs docs/tapes/demo.tape

Output docs/images/demo.gif

Set Shell "bash"
Set FontSize 14
Set Width 1200
Set Height 700
Set Theme "Catppuccin Mocha"
Set Padding 20

# Title screen
Type "# charmer - Snakemake Pipeline Monitor"
Enter
Sleep 1s

# Show help briefly
Type "charmer --help"
Enter
Sleep 2s

# Clear and start the TUI
Hide
Type "clear"
Enter
Show

# Start charmer with demo pipeline
Type "charmer tests/pipelines/demo"
Enter

# Watch jobs accumulate
Sleep 3s

# Navigate down to see dependency chain highlighting
Type "j"
Sleep 400ms
Type "j"
Sleep 400ms
Type "j"
Sleep 400ms
Type "j"
Sleep 400ms
Type "j"
Sleep 1s

# Watch more jobs complete
Sleep 2s

# Go to bottom to see more jobs
Type "G"
Sleep 1.5s

# Back to top
Type "g"
Sleep 1s

# Switch to Rules view
Type@100ms "Tab"
Sleep 2s

# Navigate rules
Type "j"
Sleep 500ms
Type "j"
Sleep 500ms
Type "j"
Sleep 1s

# Back to Jobs view
Type@100ms "Tab"
Sleep 1s

# Filter to running jobs only
Type "f"
Sleep 1.5s

# Filter to completed
Type "f"
Sleep 1.5s

# Back to all jobs
Type "f"
Sleep 500ms
Type "f"
Sleep 1s

# Change sort order
Type "s"
Sleep 1s
Type "s"
Sleep 1s
Type "s"
Sleep 1s

# Show help overlay
Type "?"
Sleep 2s
Type "?"
Sleep 500ms

# Final view of accumulating jobs
Sleep 2s

# Quit
Type "q"
Sleep 500ms
