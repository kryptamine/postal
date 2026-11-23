//! The personal Account, at an iCloud-like server: family, the flat, a
//! hike, and a friend who writes in Russian.

use super::{Account, ago, at, attach};
use crate::{home, support::fake_imap::FakeImap};

const PERSONAL: Account = Account(home::PERSONAL);
const MARTIN: (&str, &str) = ("Martin Hale", "martin.hale@example.net");

#[expect(clippy::too_many_lines, reason = "mail text, not logic")]
pub fn personal(server: &FakeImap) {
    server.login_as("sam");
    server.as_icloud();
    server.deliver(PERSONAL.mail(
        ("Crux Climbing", "hello@crux-climbing.example"),
        "Your membership renews on 1 December",
        at(8, "10:00"),
        "Hi Sam,\n\nYour yearly membership renews on 1 December. Nothing to do if \
         you're staying with us; to change your plan, visit\n\n\
         https://crux-climbing.example/membership\n\nSee you on the wall,\nCrux Climbing",
    ));
    server.deliver(PERSONAL.mail(
        ("Riverside Library", "loans@riverside-library.example"),
        "Two of your books are due on Friday",
        at(6, "07:00"),
        "Hello Sam,\n\n\"The Long Way Home\" and \"A Field Guide to Clouds\" are due on Friday. \
         Renew them online:\n\nhttps://riverside-library.example/account/loans\n\n\
         Riverside Library",
    ));
    server.deliver(PERSONAL.mail(
        MARTIN,
        "Flat viewing confirmed for Saturday",
        at(5, "13:24"),
        "Hi Sam,\n\nSaturday at 11:00 works. The flat is on the third floor, no lift, \
         sorry. Ring 3B and I'll come down.\n\nMartin",
    ));
    server.deliver_to(
        "Sent Messages",
        PERSONAL.sent(
            MARTIN,
            "Re: Flat viewing confirmed for Saturday",
            at(5, "14:02"),
            "Perfect, see you at 11. Is there a bike shed?\n\nSam",
        ),
    );
    server.deliver(PERSONAL.mail(
        ("Hannah Weller", "hannah.w@example.net"),
        "Book club: the next pick",
        at(4, "20:15"),
        "The vote is in: \"The Remains of the Day\" it is. We meet on the 12th at \
         mine, 19:30. Bring something to drink and an opinion.\n\nHannah",
    ));
    server.deliver(attach(
        PERSONAL.mail(
            ("Ольга Смирнова", "olga.smirnova@example.ru"),
            "Фотографии с выходных в Карелии",
            at(3, "21:10"),
            "Привет, Сэм!\n\nНаконец разобрала фотографии с озера. Лучшие прикладываю, \
             остальные пришлю ссылкой.\n\nВ следующий раз идём на два дня, а не на один: \
             до водопада так и не дошли.\n\nОбнимаю,\nОля",
        ),
        &[
            ("image", "jpeg", "озеро-на-рассвете.jpg", 412),
            ("image", "jpeg", "привал.jpg", 356),
        ],
    ));
    server.deliver(attach(
        PERSONAL.mail(
            ("Theo Lindqvist", "theo@example.net"),
            "Pictures from the ridge walk",
            at(2, "19:02"),
            "What a day. The one from the summit is my new wallpaper.\n\nSame time next month?\n\nTheo",
        ),
        &[
            ("image", "jpeg", "summit.jpg", 524),
            ("image", "jpeg", "ridge-at-noon.jpg", 488),
            ("image", "heic", "descent.heic", 301),
        ],
    ));
    server.deliver(
        PERSONAL
            .mail(
                ("Grace Okafor", "grace.okafor@example.net"),
                "Dinner on Sunday?",
                ago(140),
                "Hi love,\n\nAre you free on Sunday? Dad is making his jollof and wants an \
                 honest opinion this time 😄\n\nCome around six.\n\nMum",
            )
            .unread(),
    );
}
