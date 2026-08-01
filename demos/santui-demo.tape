Output demos/santui-demo.gif
Output demos/santui-demo.mp4

Set WindowBar Colorful
Set FontFamily "MesloLGM Nerd Font"
Set FontSize 14
Set Width 840
Set Height 600
Set Padding 20
Set MarginFill "#0e1116"
Set BorderRadius 6
Set Framerate 30
Set TypingSpeed 40ms
Set LoopOffset 0.2

Env SANTUI_DEV "1"
Type "./target/debug/santui"
Sleep 300ms
Enter

# splash: starfield + logo
Sleep 2s
Right
Sleep 800ms
Enter
Sleep 1.5s

# plugin registry: search radio
Type "/radio"
Sleep 900ms
Enter
Sleep 1s

# launch radio stream player
Enter
Sleep 3s
Down
Sleep 400ms
Down
Sleep 400ms
Down
Sleep 400ms
Down
Sleep 1.5s

# back to splash
Escape
Sleep 1.5s

# command palette
Ctrl+p
Sleep 1.2s
Down
Sleep 400ms
Down
Sleep 800ms
Escape
Sleep 800ms

# quit
Type "q"
Sleep 800ms
