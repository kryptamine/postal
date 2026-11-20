//! Images in the rendered View of HTML mail: never fetched, each one gone
//! or shown as a label.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::net::TcpListener;

use support::rendered::{DEFAULT, at, frames, html, pane, preview};

// A remote image is a tracking risk: nothing is fetched. One with nothing to
// say takes no room, so a banner leaves no blank rows behind.
#[test]
fn images_are_never_fetched_and_one_without_alt_takes_no_room() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let body = html(&format!(
        "<p>Above</p><img src=\"http://127.0.0.1:{port}/banner.png\" width=\"600\" \
         height=\"300\"><p>Below</p><img src=\"http://127.0.0.1:{port}/logo.png\" alt=\"Acme\">\
         <p>End</p>"
    ));
    let screen = preview(body, &DEFAULT, "Acme");
    let rows = pane(&screen);
    let row = |text: &str| rows.iter().position(|r| r.contains(text)).unwrap();
    // One blank row: the margins of the two paragraphs.
    assert_eq!(row("Below") - row("Above"), 2, "{}", rows.join("\n"));
    assert!(listener.accept().is_err(), "an image was fetched");
}

// An alt is one row, however tall the image, and no wider than it, so the
// layout around it keeps its shape.
#[test]
fn an_alt_is_one_row_and_no_wider_than_its_image() {
    let body = html(
        "<p>Above</p><img src=\"https://cdn.example/banner.png\" width=\"80\" height=\"96\" \
         alt=\"Spring collection banner\"><p>Below</p>",
    );
    let screen = preview(body, &DEFAULT, "Spring");
    let rows = pane(&screen);
    let row = |text: &str| rows.iter().position(|r| r.contains(text)).unwrap();
    assert_eq!(row("Below") - row("Above"), 4, "{}", rows.join("\n"));
    // 80px is ten columns.
    assert_eq!(rows[row("Spring")].trim(), "Spring co…");
}

// Mail reuses one image file with a different alt each time; each keeps
// its own.
#[test]
fn images_from_one_file_keep_their_own_alts() {
    let body = html(
        "<img src=\"https://cdn.example/card.png\" width=\"200\" alt=\"First card\"><br>\
         <img src=\"https://cdn.example/card.png\" width=\"200\" alt=\"Second card\">",
    );
    let screen = preview(body, &DEFAULT, "Second card");
    assert!(screen.contains("First card"), "{}", screen.text());
}

// Icons and alts that repeat their row's text are what doubled words in a
// CI report's table: `macOS macOS`, a cut `an` beside each count.
#[test]
fn an_icon_or_an_alt_beside_the_same_text_is_dropped() {
    let body = html(
        "<table><tr>\
         <td><img src=\"https://cdn.example/ok.png\" width=\"24\" height=\"24\" alt=\"macOS\"></td>\
         <td style=\"padding:16px\">CI / macOS</td>\
         <td><a href=\"https://ci.example/notes\"><img src=\"https://cdn.example/note.png\" \
         width=\"16\" height=\"16\" alt=\"annotations for CI / macOS\"> 3</a></td>\
         </tr></table>\
         <div><img src=\"https://cdn.example/jane.png\" width=\"84\" height=\"84\" alt=\"Jane Doe\">\
         <div>Jane Doe, product manager</div></div>",
    );
    let screen = preview(body, &DEFAULT, "product manager");
    let rows = pane(&screen);
    let row = rows.iter().find(|r| r.contains("CI / macOS")).unwrap();
    assert_eq!(
        row.split_whitespace().collect::<Vec<_>>(),
        ["CI", "/", "macOS", "3"]
    );
    let names = rows.iter().filter(|r| r.contains("Jane Doe")).count();
    assert_eq!(names, 1, "{}", rows.join("\n"));
}

// An icon that is all its link shows is named by its alt, which cut names
// nothing; where its cell is too narrow, it is cut to fit there.
#[test]
fn a_linked_icon_shows_its_whole_alt_where_it_fits() {
    let icon = "<a href=\"https://social.example\"><img src=\"https://cdn.example/i.png\" \
                width=\"24\" height=\"24\" alt=\"Instagram\"></a>";
    let body = html(&format!(
        "<p>Above</p><table style=\"width:40px\"><tr><td style=\"width:40px;max-width:40px;\
         overflow:hidden\">{icon}</td></tr></table><p>Free</p>{icon}<p>End</p>"
    ));
    let screen = preview(body, &DEFAULT, "Free");
    let rows = pane(&screen);
    let named: Vec<_> = rows.iter().filter(|r| r.contains("Ins")).collect();
    assert_eq!(named.len(), 2, "{}", rows.join("\n"));
    assert!(named[0].contains('…') && !named[0].contains("Instagram"));
    assert!(named[1].contains("Instagram"));
}

// A product's picture and a banner over its button open what the words
// beside them do: `[link]` on each was a column of noise in a shop's mail.
#[test]
fn a_picture_link_whose_page_the_words_open_shows_no_label() {
    let picture = |href: &str| {
        format!(
            "<a href=\"{href}\"><img src=\"https://cdn.example/p.png\" width=\"240\" alt=\"\"></a>"
        )
    };
    let body = html(&format!(
        "{}<p><a href=\"https://shop.example/kettle?from=title\">Kettle</a></p>\
         {}<p><a href=\"https://t.example/2\">See more</a></p>{}<p>End</p>",
        picture("https://shop.example/kettle?from=picture"),
        picture("https://t.example/1"),
        picture("https://elsewhere.example/"),
    ));
    let screen = preview(body, &DEFAULT, "End");
    let labels: usize = pane(&screen).iter().map(|r| r.matches('[').count()).sum();
    assert_eq!(labels, 1, "{}", screen.text());
    assert!(screen.contains("[elsewhere.example]"), "{}", screen.text());
}

// A cell sized for a product photo stood twelve blank rows high once the
// photo was a label one row high.
#[test]
fn a_box_sized_for_a_picture_shrinks_to_its_label() {
    let body = html(
        "<p>Above</p><table><tr><td style=\"height:200px;min-height:200px\">\
         <img src=\"https://cdn.example/p.png\" width=\"240\" alt=\"Kettle photo\">\
         </td></tr></table><p>Below</p>",
    );
    let screen = preview(body, &DEFAULT, "Below");
    let rows = pane(&screen);
    let row = |text: &str| rows.iter().position(|r| r.contains(text)).unwrap();
    // The label's row and the paragraphs' margins round the table.
    assert_eq!(row("Below") - row("Above"), 4, "{}", rows.join("\n"));
}

// A card's picture is named by its title, which the card shows as text
// further on: both read `Dungeon Crawler Carl` twice.
#[test]
fn an_alt_the_mail_shows_as_a_title_is_dropped() {
    let body = html(
        "<table><tr><td><img src=\"https://cdn.example/b.png\" width=\"160\" \
         alt=\"Dungeon Crawler Carl\"></td></tr></table>\
         <table><tr><td>Fallout x Dungeon Crawler Carl</td></tr></table>",
    );
    let screen = preview(body, &DEFAULT, "Fallout");
    let titles = pane(&screen)
        .iter()
        .filter(|r| r.contains("Dungeon Crawler Carl"))
        .count();
    assert_eq!(titles, 1, "{}", screen.text());
}

// A coloured section that held only a picture stood as an empty frame once
// the picture went: its padding framed nothing.
#[test]
fn a_section_a_gone_picture_leaves_empty_goes() {
    let body = html(
        "<p><a href=\"https://shop.example/a\">Shop now</a></p>\
         <table bgcolor=\"#6d28d2\" width=\"100%\"><tr><td style=\"padding:40px\">\
         <a href=\"https://shop.example/a\"><img src=\"https://cdn.example/p.png\" width=\"400\">\
         </a></td></tr></table><p>End</p>",
    );
    let screen = preview(body, &DEFAULT, "End");
    assert_eq!(frames(&screen), 0, "{}", screen.text());
    let rows = pane(&screen);
    let row = |text: &str| rows.iter().position(|r| r.contains(text)).unwrap();
    assert!(row("End") - row("Shop now") <= 3, "{}", rows.join("\n"));
}

// A CI report's status icons repeat each job's name and go: the column
// stood empty under its header in every row.
#[test]
fn a_column_every_picture_left_empty_goes() {
    let row = |job: &str| {
        format!(
            "<tr><td style=\"padding:16px\"><img src=\"https://cdn.example/ok.png\" \
             width=\"24\" height=\"24\" alt=\"{job}\"></td><td>{job} passed</td></tr>"
        )
    };
    let body = html(&format!(
        "<table><thead><tr><th>Status</th><th>Job</th></tr></thead><tbody>{}{}</tbody></table>",
        row("Lint"),
        row("Tests")
    ));
    let screen = preview(body, &DEFAULT, "Tests passed");
    let rows = pane(&screen);
    assert!(
        !rows.iter().any(|r| r.contains("Status")),
        "{}",
        rows.join("\n")
    );
    assert_eq!(at(&screen, "Job").1, at(&screen, "Lint passed").1);
}

// A picture link names nothing, so `[link]` left the reader guessing; its
// address does: the words of its page, or the site a redirect opens.
#[test]
fn a_bare_picture_link_is_named_by_its_address() {
    let picture = |href: &str| {
        format!(
            "<p><a href=\"{href}\"><img src=\"https://cdn.example/p.png\" width=\"400\"></a></p>"
        )
    };
    let body = html(&format!(
        "<p>Above</p>{}{}<p>End</p>",
        picture("https://www.events.example/e/ignite-watch-party-tickets-2004779926728?ref=mail"),
        picture("https://click.bank.example/f/a/Yv9Qtu~~/AAD4-xA~/pEerNWDa"),
    ));
    let screen = preview(body, &DEFAULT, "End");
    assert!(
        screen.contains("[ignite watch party tickets]"),
        "{}",
        screen.text()
    );
    assert!(screen.contains("[bank.example]"), "{}", screen.text());
}

// An alt that ends by calling itself a picture says so about every one:
// the noun goes, and an alt that names nothing else goes whole.
#[test]
fn an_alt_loses_the_noun_that_calls_it_a_picture() {
    let body = html(
        "<p>Above</p><a href=\"https://social.example/inbox\"><img src=\"https://cdn.example/m.png\" \
         width=\"24\" height=\"24\" alt=\"Messaging icon\"></a>\
         <p><img src=\"https://cdn.example/hero.png\" width=\"400\" alt=\"Some Image\"></p>\
         <p><img src=\"https://cdn.example/brand.png\" width=\"400\" alt=\"Acme Hotels Logo\"></p>\
         <p>Below</p>",
    );
    let screen = preview(body, &DEFAULT, "Below");
    let rows = pane(&screen).join("\n");
    assert!(
        rows.contains("Messaging") && rows.contains("Acme Hotels"),
        "{rows}"
    );
    for noun in ["icon", "Image", "Some", "Logo"] {
        assert!(!rows.contains(noun), "{noun} in\n{rows}");
    }
}

// A store badge is all its link shows: cut to the badge's width its alt
// named no store.
#[test]
fn a_badge_shows_its_whole_alt_where_it_fits() {
    let body = html(
        "<a href=\"https://store.example/app\"><img src=\"https://cdn.example/badge.png\" \
         width=\"120\" height=\"40\" alt=\"Download on the App Store\"></a>",
    );
    let screen = preview(body, &DEFAULT, "Download");
    let rows = pane(&screen).join("\n");
    assert!(rows.contains("Download on the App Store"), "{rows}");
}

// A card's picture over its caption said the caption twice, in the next
// row of the table, where the text of its own row could not show it.
#[test]
fn an_alt_that_is_the_caption_beside_it_is_dropped() {
    let body = html(
        "<table><tr><td><a href=\"https://shop.example/dev\">\
         <img src=\"https://cdn.example/dev.png\" width=\"200\" alt=\"Development\"></a></td></tr>\
         <tr><td>Development</td></tr></table><p>End</p>",
    );
    let screen = preview(body, &DEFAULT, "End");
    let rows = pane(&screen).join("\n");
    assert_eq!(rows.matches("Development").count(), 1, "{rows}");
}

// Six icons named `Acme on …` had no room for six names, and cut to their
// own width each read `Acm…`: the words they share go, and the table sized
// for the icons widens to the names.
#[test]
fn a_row_of_icons_drops_the_words_their_names_share() {
    let icon = |name: &str| {
        format!(
            "<td><a href=\"https://{name}.example\"><img src=\"https://cdn.example/{name}.png\" \
             width=\"32\" height=\"32\" alt=\"Acme on {name}\"></a></td>"
        )
    };

    let icons: String = ["Facebook", "Instagram", "LinkedIn", "YouTube"]
        .into_iter()
        .map(icon)
        .collect();
    let body = html(&format!(
        "<p>Follow us</p><table width=\"140\" style=\"display:inline-block\"><tr>{icons}</tr></table>\
         <p>Thanks</p>"
    ));
    let screen = preview(body, &DEFAULT, "YouTube");
    let rows = pane(&screen);
    let row = rows.iter().find(|r| r.contains("YouTube")).unwrap();
    assert!(
        row.contains("Facebook") && row.contains("LinkedIn"),
        "{}",
        rows.join("\n")
    );
    assert!(
        !screen.contains("Acme on") && !row.contains('…'),
        "{}",
        rows.join("\n")
    );
}

// A link or a cell sized for its picture cut the picture's name to that
// size: `Тел…` in 32 px, `[revolut.co…` in 100 px.
#[test]
fn a_box_sized_for_a_picture_widens_to_its_name() {
    let body = html(
        "<p>Follow us</p>\
         <a href=\"https://t.example\" style=\"display:inline-block;width:32px;height:32px\">\
         <img src=\"https://cdn.example/t.png\" width=\"32\" height=\"32\" alt=\"Telegram\"></a>\
         <table><tr><td style=\"width:80px\"><a href=\"https://www.bank.example/home\">\
         <img src=\"https://cdn.example/logo.png\" style=\"width:100%\"></a></td></tr></table>\
         <p>End</p>",
    );
    let screen = preview(body, &DEFAULT, "End");
    assert!(screen.contains("Telegram"), "{}", screen.text());
    assert!(screen.contains("[bank.example]"), "{}", screen.text());
}

// `dots`, `car`, `flag`: an alt of one lowercase word is the picture's
// file name, and beside the text it decorates it read as a stray word.
#[test]
fn an_alt_that_is_a_file_name_is_dropped() {
    let body = html(
        "<table><tr><td><img src=\"https://cdn.example/dots.png\" alt=\"dots\"></td>\
         <td>From the station</td></tr></table>\
         <a href=\"https://social.example\"><img src=\"https://cdn.example/f.png\" \
         width=\"32\" height=\"32\" alt=\"facebook\"></a><p>End</p>",
    );
    let screen = preview(body, &DEFAULT, "End");
    assert!(!screen.contains("dots"), "{}", screen.text());
    // All its link shows, the word still names the link.
    assert!(screen.contains("facebook"), "{}", screen.text());
}

// An alt that describes the picture, written for a screen reader, is a
// sentence about a photo the reader cannot see.
#[test]
fn an_alt_that_describes_its_picture_is_dropped() {
    let body = html(
        "<p>Above</p><img src=\"https://cdn.example/hero.png\" width=\"400\" \
         alt=\"An audience sits in a dark auditorium watching the stage\"><p>Below</p>",
    );
    let screen = preview(body, &DEFAULT, "Below");
    assert!(!screen.contains("audience"), "{}", screen.text());
}

// The sender's logo over the first words or under the last says only who
// sent the mail, which the header says already.
#[test]
fn the_logo_at_either_end_of_the_mail_is_dropped() {
    let body = html(
        "<a href=\"https://acme.example\"><img src=\"https://cdn.example/logo.png\" \
         width=\"120\" height=\"40\" alt=\"Acme\"></a><p>Hello</p>\
         <img src=\"https://cdn.example/x.png\" width=\"120\" height=\"40\" alt=\"Spring sale\">\
         <p>Bye</p><img src=\"https://cdn.example/logo.png\" width=\"120\" alt=\"Acme Corp\">",
    );
    let screen = preview(body, &DEFAULT, "Bye");
    assert!(!screen.contains("Acme"), "{}", screen.text());
    assert!(screen.contains("Spring sale"), "{}", screen.text());
}
