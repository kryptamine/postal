# An Avatar is initials on a hue the address picks, never a picture

A desktop client shows a picture or a letter tile beside each mail, so a
sender is known before the name is read (issue #267). Each card in the
Message List starts with an Avatar: the sender's initials on a tint, four
columns wide. It is on by default; `avatars = "hide"` in `config.toml`, a
switch on the Icons tab of Settings, turns it off at once.

**Never a picture.** A BIMI logo, a Gravatar or a favicon was weighed and
left out. In a card three rows tall a picture is a dozen pixels high and
reads as a smudge; the terminals that draw one disagree on the protocol,
and tmux breaks most of them; and fetching one per sender tells that
sender, or a third party, that the mail was looked at. A glyph per service
(a Nerd Font logo for GitHub, Slack and the like) was tried in a prototype
and dropped too: a short hand-kept list, and a tile unlike the rest.

**The same on every card and every run.** The hue is one of nine spread
round the wheel at one brightness, chosen by FNV-1a over the address
without case. A fixed hash, not `RandomState`, so a sender keeps a colour
across runs, which is what makes it recognisable. The initials come from
the display name, the first letters of its first two words, or the first
two letters of a single word; without a name, from the address's local
part. A sender that changes display name keeps the colour.

**On the terminal's background.** The tile mixes 28% of the hue into the
terminal's background, and the initials take the hue moved until they
read on it (`MailColours::text`), so the tile sits behind the text on a
light terminal or a dark one. Half blocks above and below the row of
initials make it two rows tall to the eye, the letters centred. Under
`NO_COLOR` the tile is reversed text.

**Room.** The Avatar costs seven columns of a card: the marks, the tile
and a gap. A card narrower than 34 columns, the list beside the Preview in
a terminal of 100, keeps the text-only card. Beside an Avatar the Unread
dot moves to the tile's middle row, level with the letters, and the
attachment mark to the end of the subject's row; a Nerd Font envelope
drawn on the senders' row stood above the tile and looked out of place.

A frame draws the Avatars without allocating (ADR 0001): the initials are
formatted on the stack.
