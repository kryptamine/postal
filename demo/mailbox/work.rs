//! The work Account, at a Gmail-like server: an Inbox with the HTML mails,
//! the Porto Thread with Sam's Replies in Sent Mail, and a few Folders.

use super::{ago, at, attach, html, mail};
use crate::support::fake_imap::FakeImap;

const SENT: &str = "[Gmail]/Sent Mail";

pub fn work(server: &FakeImap) {
    server.add_folder(SENT, &["\\Sent"], Vec::new());
    server.add_folder("[Gmail]/Trash", &["\\Trash"], Vec::new());
    server.add_all_mail("[Gmail]/All Mail");
    older(server);
    porto(server);
    recent(server);
    projects(server);
}

fn older(server: &FakeImap) {
    server.deliver(mail(
        "Ines Duarte",
        "ines@ferro-travel.example",
        "Your Lisbon itinerary",
        &at(9, "08:12"),
        "Hi Sam,\n\nYour trip is confirmed. Flights, the hotel and the train to Porto \
         are all in one place:\n\nhttps://ferro-travel.example/trips/LIS-2291\n\n\
         Check-in opens 24 hours before departure. Reply here if anything changes.\n\n\
         Ines\nFerro Travel",
    ));
    server.deliver(mail(
        "Jonas Berg",
        "jonas@example.com",
        "Code review: the parser rewrite",
        &at(8, "16:40"),
        "Sam,\n\nI went through the branch. The streaming part reads well; two things \
         before it lands:\n\n- the error for an unterminated quote points one column late;\n\
         - the benchmark lost 4% on the large corpus, see the run:\n  \
         https://ci.example.com/runs/88412\n\nNotes inline on the review:\n\
         https://code.example.com/postal-demo/pull/317\n\nJonas",
    ));
    server.deliver(html(
        include_str!("../mail/release-notes.eml"),
        &at(7, "18:05"),
    ));
    server.deliver(mail(
        "Priya Natarajan",
        "priya@example.com",
        "Weekly sync notes",
        &at(6, "11:30"),
        "Notes from today:\n\n1. The search index ships behind a flag next week.\n\
         2. Jonas owns the parser review; Sam answers the open comments.\n\
         3. Porto offsite: Maya sends the plan, everyone replies by Friday.\n\n\
         Next sync Thursday, same room.\n\nPriya",
    ));
    server.deliver(mail(
        "Ops Rota",
        "ops@example.com",
        "Maintenance window: Saturday 02:00 UTC",
        &at(5, "16:00"),
        "The database upgrade runs on Saturday from 02:00 to 03:00 UTC. Search may \
         answer slowly for a few minutes; nothing else should notice.\n\n\
         Status page: https://status.example.com\n\nOps",
    ));
    server.deliver(html(include_str!("../mail/receipt.eml"), &at(5, "09:47")));
}

/// Six Messages in one Thread, two of them Sam's from Sent Mail.
#[expect(clippy::too_many_lines, reason = "mail text, not logic")]
fn porto(server: &FakeImap) {
    let subject = "Offsite in Porto: plan for December";
    let reply = format!("Re: {subject}");
    let maya = |when: &str, subject: &str, body: &str| {
        mail("Maya Chen", "maya@example.com", subject, when, body).in_thread(0)
    };
    server.deliver(maya(
        &at(4, "10:02"),
        subject,
        "Hi all,\n\nHere is the first plan for the offsite, 9 to 11 December in Porto.\n\n\
         Day 1: arrive, dinner by the river.\nDay 2: roadmap for next year, then a long walk.\n\
         Day 3: retro on the launch, back home by the evening.\n\n\
         The house is booked: https://stays.example.com/porto/ribeira-house\n\n\
         Does the agenda work for you? Anything missing?\n\nMaya",
    ));
    server.deliver_to(
        SENT,
        mail(
            "Sam Okafor",
            "me@example.com",
            &reply,
            &at(4, "11:15"),
            "Works for me. Could we keep the afternoon of day 2 for the search \
             demo? Ten minutes is enough.\n\nSam",
        )
        .to("Maya Chen", "maya@example.com")
        .sent_by_me()
        .in_thread(0),
    );
    server.deliver(
        mail(
            "Tomás Reyes",
            "tomas@example.com",
            &reply,
            &at(3, "09:20"),
            "Count me in. I land at 14:30 on the 9th, so I may miss the start of \
             dinner. I can bring the projector.\n\nTomás",
        )
        .in_thread(0),
    );
    server.deliver(
        mail(
            "Priya Natarajan",
            "priya@example.com",
            &reply,
            &at(2, "17:48"),
            "Added the search demo to day 2, 15:00. I also booked a table for \
             eight at Casa do Rio for the first evening.\n\nUpdated agenda:\n\
             https://docs.example.com/offsite-porto-agenda\n\nPriya",
        )
        .in_thread(0),
    );
    server.deliver_to(
        SENT,
        mail(
            "Sam Okafor",
            "me@example.com",
            &reply,
            &at(1, "08:55"),
            "Thanks Priya. I'll have the demo ready by Friday and send the slides \
             before we fly.\n\nSam",
        )
        .to("Priya Natarajan", "priya@example.com")
        .sent_by_me()
        .in_thread(0),
    );
    server.deliver(
        maya(
            &ago(95),
            &reply,
            "Final plan attached to the calendar invite. Train tickets from Lisbon \
             are on me; send me your arrival times by Wednesday.\n\nSee you in Porto!\n\nMaya",
        )
        .unread(),
    );
}

fn recent(server: &FakeImap) {
    server.deliver(mail(
        "Priya Natarajan",
        "priya@example.com",
        "Standup moves to 10:30 from Monday",
        &at(3, "12:05"),
        "Two of us start later on Mondays now, so standup moves to 10:30 for \
         everyone. Same link, same fifteen minutes.\n\nPriya",
    ));
    server.deliver(html(
        include_str!("../mail/invitation.eml"),
        &at(2, "12:30"),
    ));
    server.deliver(
        attach(
            mail(
                "Daniel Weiss",
                "daniel@example.com",
                "Q4 budget, second draft",
                &at(1, "15:10"),
                "Sam,\n\nThe second draft is attached, with the forecast split by team. \
             Travel is up because of Porto; everything else is flat.\n\n\
             Could you check the engineering lines before Thursday?\n\nDaniel",
            ),
            &[
                ("application", "pdf", "q4-budget-draft-2.pdf", 182),
                (
                    "application",
                    "vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                    "forecast-by-team.xlsx",
                    46,
                ),
            ],
        )
        .unread(),
    );
    server.deliver(
        mail(
            "Lena Park",
            "lena@example.com",
            "🚀 The new onboarding is live",
            &at(1, "18:22"),
            "It's out! 🎉\n\nEvery new account now goes through the three-step setup. \
             Sign-ups finishing it went from 61% to 78% in the beta. 📈\n\n\
             Thanks to everyone who tested it 🙏 Drinks on Friday are on me 🍻\n\nLena",
        )
        .unread(),
    );
    server.deliver(html(include_str!("../mail/newsletter.eml"), &ago(170)));
    server.deliver(
        mail(
            "Ana Ruiz",
            "ana@example.com",
            "Lunch on Thursday?",
            &ago(24),
            "The new ramen place by the station finally opened. Thursday at 12:30? \
             Jonas is in too.\n\nAna",
        )
        .unread(),
    );
}

fn projects(server: &FakeImap) {
    server.add_folder(
        "Projects",
        &[],
        vec![
            mail(
                "Jonas Berg",
                "jonas@example.com",
                "Search index: rollout checklist",
                &at(12, "14:00"),
                "Checklist for the rollout:\n\n- reindex the staging corpus\n\
                 - compare the top ten results for the fifty saved queries\n\
                 - flip the flag for the team first\n\nJonas",
            ),
            mail(
                "Priya Natarajan",
                "priya@example.com",
                "Talk proposal for the spring conference",
                &at(10, "09:05"),
                "Draft abstract below; tell me what you'd cut.\n\n\
                 \"Reading mail at the speed of typing\": how a terminal client keeps \
                 every keystroke under a frame.\n\nPriya",
            ),
        ],
    );
    server.add_folder(
        "Receipts",
        &[],
        vec![attach(
            mail(
                "Atelier Nord",
                "billing@atelier-nord.example",
                "Invoice 2026-114",
                &at(14, "08:00"),
                "Hello Sam,\n\nPlease find the invoice for the November workshop attached.\n\n\
             Atelier Nord",
            ),
            &[("application", "pdf", "invoice-2026-114.pdf", 64)],
        )],
    );
}
