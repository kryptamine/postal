# PROTOTYPE, throwaway. Pulls a varied sample of real mail as .eml for the
# Preview corpus. Read-only: EXAMINE + BODY.PEEK[], no flag ever changes.
# Password comes from the macOS keychain item "postal-prototype".
# Usage: python3 -I fetch.py <gmail-address> <out-dir>
import imaplib
import pathlib
import subprocess
import sys

user, out = sys.argv[1], pathlib.Path(sys.argv[2])
out.mkdir(parents=True, exist_ok=True)
password = subprocess.run(
    ["security", "find-generic-password", "-s", "postal-prototype", "-a", user, "-w"],
    check=True, capture_output=True, text=True,
).stdout.strip()

BUCKETS = [
    ("INBOX", None, 60),
    ("ALL", "category:promotions", 15),
    ("ALL", "category:updates", 15),
    ("ALL", "category:social", 5),
    ("ALL", "receipt OR invoice OR order OR payment", 15),
    ("ALL", "in:inbox \"wrote:\"", 10),
    ("ALL", "from:outlook.com OR from:hotmail.com OR \"Microsoft Outlook\"", 10),
    ("ALL", "has:attachment", 5),
]

imap = imaplib.IMAP4_SSL("imap.gmail.com")
imap.login(user, password)
# "[Gmail]/All Mail" is localised; find it by its \All special-use flag.
typ, boxes = imap.list()
all_mail = next(
    b.decode().rsplit(' "/" ', 1)[1].strip('"') for b in boxes if b"\\All" in b
)
seen = set()
saved = 0
for mailbox, query, n in BUCKETS:
    mailbox = all_mail if mailbox == "ALL" else mailbox
    imap.select(f'"{mailbox}"', readonly=True)
    if query:
        typ, data = imap.uid("SEARCH", "X-GM-RAW", f'"{query.replace(chr(34), chr(92) + chr(34))}"')
    else:
        typ, data = imap.uid("SEARCH", None, "ALL")
    uids = data[0].split()[-n:]
    tag = (query or mailbox).split()[0].replace(":", "-").replace("/", "-").strip('"[]')
    for uid in uids:
        typ, msg = imap.uid("FETCH", uid, "(X-GM-MSGID BODY.PEEK[])")
        meta, raw = msg[0]
        gm = meta.split(b"X-GM-MSGID ")[1].split()[0].decode()
        if gm in seen:
            continue
        seen.add(gm)
        (out / f"{tag}-{gm}.eml").write_bytes(raw)
        saved += 1
    print(f"{mailbox} {query or ''}: {len(uids)}")
imap.logout()
print(f"saved {saved} messages to {out}")
