# Scenario of the typing test (fs-release actions/typing-test): defines focus(), which brings a text field of the window
# into focus. The window opens on the task list; "New" opens the form of a new task and its first field ("Title") takes
# the text (the search field is not used: its placeholder is as large as a wide glyph such as "@", which the test's
# "arrived" check compares against). Positions are relative to the window's top left corner (WX0, WY0, set by the
# test; title bar of 28 px included), for the window size of the workflow (`--size 1000x640`).
focus() {
  c "move $((WX0 + 951)) $((WY0 + 108))" 0.4
  c "click $((WX0 + 951)) $((WY0 + 108))" 1.5
  local x=$((WX0 + 810)) y=$((WY0 + 277))
  TT_NEAR="$x,$y"; export TT_NEAR
  c "move $x $y" 0.4
  c "click $x $y" 1
}
