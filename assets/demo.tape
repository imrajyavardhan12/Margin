# The README's hero recording. Run from the repo root:
#   cargo build --release -p margin && vhs assets/demo.tape
#
# The story, in order: read the diff -> stage the good part -> leave a
# note on the questionable part -> check the file off -> hand the notes
# to whoever (or whatever) wrote it. Keep it under ~30s; a hero GIF is an
# invitation, not a manual.
#
# Width/Height are PIXELS, not columns. ~1180px at FontSize 14 lands
# around 110 columns: wide enough for the sidebar (needs 60+), narrow
# enough to stay in unified layout (split auto-engages at 120).
Output assets/demo.gif

Set FontSize 14
Set Width 1180
Set Height 720
Set Padding 14
Set TypingSpeed 55ms

Hide
Type "source assets/demo-env.sh" Enter
Sleep 2s
Show

Type "margin" Sleep 600ms Enter
Sleep 2.5s
Screenshot assets/frames/01-open.png

# Read: move through the hunk, then jump to the next one. The status bar
# tracks `hunk x/y`, the sidebar tracks the file.
Type "jjj" Sleep 1.4s
Type "J" Sleep 1.8s

# Stage just this hunk — the index write, one hunk at a time. The
# sidebar's dot lights up.
Type "s" Sleep 2.2s
Screenshot assets/frames/02-staged.png

# Annotate the part that needs a human answer.
Type "c" Sleep 700ms
Type "cap looks right, but needs a test" Sleep 400ms Enter
Sleep 2.2s
Screenshot assets/frames/03-note.png

# Check the file off: sidebar checkmark, body folds away.
Type "m" Sleep 2s
Screenshot assets/frames/04-viewed.png

# Back to the other file for one more note. `[` not `]`: the file we
# just checked off is the last one, and notes attach to hunks — after
# the fold there is no hunk ahead to annotate.
Type "[" Sleep 800ms
Type "J" Sleep 700ms
Type "c" Sleep 600ms
Type "why 4 attempts?" Sleep 400ms Enter
Sleep 2s

Type "q" Sleep 1s

# The payoff: the notes come back out as Markdown, with path:line
# anchors — paste into a PR, or hand straight to an agent.
Type "margin --notes" Sleep 700ms Enter
Sleep 3.5s
Screenshot assets/frames/05-export.png
