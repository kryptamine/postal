use super::loopback::Outcome;

const TEMPLATE: &str = include_str!("landing.html");

/// The page the browser lands on back from the consent page. Every value
/// filled in is fixed text: nothing from the redirect reaches the HTML.
pub fn page(outcome: &Outcome) -> String {
    let (tone, mark, title, detail) = match outcome {
        Outcome::Code(_) => (
            "ok",
            "M139 108 L147 116 L162 100",
            "You're signed in",
            "You can close this window and return to the terminal.",
        ),
        Outcome::Refused(_) => (
            "err",
            "M142 100 L158 116 M158 100 L142 116",
            "Sign-in did not go through",
            "Return to the terminal to see why and try again.",
        ),
    };

    TEMPLATE
        .replace("{{tone}}", tone)
        .replace("{{mark}}", mark)
        .replace("{{title}}", title)
        .replace("{{detail}}", detail)
}
