# VHS Demo Tape for charmer
# Run with: vhs demo.tape

Output ../images/demo.gif
Output ../images/demo.webm

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

# Show help
Type "../../target/release/charmer --help"
Enter
Sleep 3s

# Clear and start the TUI
Hide
Type "clear"
Enter
Show

# Start charmer with test data
Type "../../target/release/charmer ../../tests/pipelines/simple"
Enter
Sleep 2s

# Navigate through jobs
Sleep 1s
Type "j"
Sleep 500ms
Type "j"
Sleep 500ms
Type "j"
Sleep 500ms

# Show filtering
Type "f"
Sleep 1s
Type "f"
Sleep 1s
Type "f"
Sleep 1s

# Show sorting
Type "s"
Sleep 1s
Type "s"
Sleep 1s

# Show help overlay
Type "?"
Sleep 2s
Type "?"
Sleep 500ms

# Navigate more
Type "G"
Sleep 1s
Type "g"
Sleep 1s

# Quit
Type "q"
Sleep 1s
