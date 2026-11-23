#!/bin/sh
# The landing page's screenshots: each shot natively in a Ghostty window of
# your own config at the display's scale, and the newsletter as Chrome draws
# it. macOS only; needs tmux, and Screen Recording for the terminal running it.
set -eu
cd "$(dirname "$0")/.."

out=landing/src/assets/media
chrome="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
socket=postal-stills
title=postal-stills
mkdir -p "$out"
cargo build --release --bins --example demo

work=$(mktemp -d)
trap 'tmux -L $socket kill-server 2>/dev/null || true; rm -rf "$work"' EXIT
cat >"$work/tmux.conf" <<'EOF'
set -g status off
set -g escape-time 0
set -g default-terminal "tmux-256color"
set -ga terminal-overrides ",xterm-ghostty:RGB"
EOF
cat >"$work/window.swift" <<'EOF'
import CoreGraphics
let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
for w in windows where (w[kCGWindowOwnerName as String] as? String) == "Ghostty"
    && (w[kCGWindowName as String] as? String) == CommandLine.arguments[1] {
    print(w[kCGWindowNumber as String]!)
}
EOF

# Crops `file` about its centre to the aspect ratio `w`:`h`.
crop() {
	file=$1
	width=$(sips -g pixelWidth "$file" | awk '/pixelWidth/ { print $2 }')
	height=$(sips -g pixelHeight "$file" | awk '/pixelHeight/ { print $2 }')
	if [ $((width * $3)) -gt $((height * $2)) ]; then
		width=$((height * $2 / $3))
	else
		height=$((width * $3 / $2))
	fi
	sips -c "$height" "$width" "$file" >/dev/null
}

# shoot NAME RATIO COLS ROWS DEMO-FLAGS READY KEYS...: opens the demo in a
# window of COLS x ROWS cells, waits for READY on screen, sends each of KEYS
# as one tmux send-keys, and saves the window cropped to RATIO ("16:10").
shoot() {
	name=$1 ratio=$2 cols=$3 rows=$4 flags=$5 ready=$6
	shift 6
	open -na Ghostty --args --title="$title" --window-width="$cols" --window-height="$rows" \
		--window-save-state=never \
		"--initial-command=$(command -v tmux) -L $socket -f $work/tmux.conf new-session -s stills -c $PWD 'target/release/examples/demo target/release/postal $flags'"
	tries=0
	until tmux -L $socket capture-pane -p -t stills 2>/dev/null | grep -q "$ready"; do
		tries=$((tries + 1))
		[ $tries -lt 60 ] || {
			echo "stills: $name never showed $ready" >&2
			exit 1
		}
		sleep 0.5
	done

	sleep 1.5
	for keys in "$@"; do
		# Word splitting is wanted: "j j j" is three keys.
		# shellcheck disable=SC2086
		tmux -L $socket send-keys -t stills $keys
		sleep 0.6
	done

	sleep 1.5
	screencapture -x -o -l "$(swift "$work/window.swift" "$title")" "$out/$name.png"
	tmux -L $socket kill-server
	crop "$out/$name.png" "${ratio%:*}" "${ratio#*:}"
	sleep 1
}

shoot hero 16:10 140 36 "" Brightfold g "j j j"
shoot thread 16:10 140 36 "" Brightfold g j Enter p Enter
shoot rendered 4:3 118 36 "" Brightfold g "j j j" Enter
shoot onboarding 16:9 140 32 --onboarding "Sign in"

# The same newsletter as a browser draws it, beside the rendered one.
python3 -I - demo/mail/newsletter.eml "$work/newsletter.html" <<'EOF'
import email, sys
from email import policy
message = email.message_from_bytes(open(sys.argv[1], "rb").read(), policy=policy.default)
open(sys.argv[2], "w").write(message.get_body(("html",)).get_content())
EOF
"$chrome" --headless --disable-gpu --hide-scrollbars --force-device-scale-factor=2 \
	--window-size=800,600 --screenshot="$PWD/$out/browser.png" "file://$work/newsletter.html" 2>/dev/null
