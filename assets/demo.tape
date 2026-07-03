# Renders assets/demo.gif for the README. Run from the repo root:
#   cargo build --release -p margin && vhs assets/demo.tape
Output assets/demo.gif

Set FontSize 15
Set Width 1440
Set Height 780
Set Padding 12
Set TypingSpeed 60ms

Hide
Type "source assets/demo-env.sh" Enter
Sleep 2s
Show

Type "margin diff" Sleep 800ms Enter
Sleep 2.5s

# Scroll through the first hunk.
Type "jjjj" Sleep 1.2s
Type "J" Sleep 1.5s

# Search: incremental regex with live match count.
Type "/" Sleep 600ms
Type "retry" Sleep 1.2s Enter
Type "n" Sleep 1.2s

# Jump between files with the fuzzy picker.
Type "f" Sleep 800ms
Type "conf" Sleep 1s Enter
Sleep 1.5s

# Toggle unified <-> side-by-side, then wrap the long line.
Type "v" Sleep 1.8s
Type "G" Sleep 1s
Type "w" Sleep 2.2s

# The cheat sheet, then out.
Type "?" Sleep 2s
Type "q" Sleep 600ms
Type "q" Sleep 1s
