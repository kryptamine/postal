# Shared by every tape: the palette and font of the stills, and the demo
# started off camera. `make media` runs each tape from the repository root.
Set Shell bash
Set FontFamily "JetBrainsMono Nerd Font"
Set FontSize 18
Set LineHeight 1.25
Set Width 1600
Set Height 1000
Set Padding 24
Set TypingSpeed 70ms
Set Theme { "name": "gruber-darker-renaissance", "background": "#000000", "foreground": "#d2d2cc", "cursor": "#efd45b", "selection": "#484848", "black": "#1e1e1e", "red": "#d64c4b", "green": "#8ac568", "yellow": "#efd45b", "blue": "#9fb2d6", "magenta": "#9e95c7", "cyan": "#96a29c", "white": "#d2d2cc", "brightBlack": "#8c8c8c", "brightRed": "#f26063", "brightGreen": "#8ac568", "brightYellow": "#efd45b", "brightBlue": "#bdd0f6", "brightMagenta": "#9e95c7", "brightCyan": "#96a29c", "brightWhite": "#ebebe6" }

Hide
# `open` stands in for the desktop's opener, so a link or an attachment
# opened on camera starts nothing outside the recording.
Type "stub=$(mktemp -d) && printf '#!/bin/sh\n' > $stub/open && chmod +x $stub/open && clear" Enter
Type "PATH=$stub:$PATH target/release/examples/demo target/release/postal" Enter
Wait+Screen@30s /Brightfold/
Sleep 2s
