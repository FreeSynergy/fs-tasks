# Scenario of the typing test (fs-release actions/typing-test): defines focus(), which brings a text field of the window
# into focus. The window opens on the task list with the search field at the top; clicking it is all there is to do.
# Positions are relative to the window's top left corner (WX0, WY0, set by the test; title bar included), for the
# window size of the workflow (`--size 1000x640`).
focus() {
  local x=$((WX0 + 380)) y=$((WY0 + 80))
  TT_NEAR="$x,$y"; export TT_NEAR
  c "move $x $y" 0.4
  c "click $x $y" 1
}
