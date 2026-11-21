use html2text::Handle;

use super::dom::{any_below, attr, has_image};

/// Whether `node` is content a mail client never shows: hidden preheaders,
/// text shrunk or faded to nothing, and Outlook-only parts.
pub fn hidden(node: &Handle) -> bool {
    if attr(node, "hidden").is_some() {
        return true;
    }

    let Some(style) = attr(node, "style") else {
        return false;
    };

    let mut collapsed = false;
    let mut clipped = false;
    for (property, value) in declarations(&style) {
        let hides = match property.as_str() {
            "display" => value == "none",
            "visibility" => value == "hidden",
            "mso-hide" => value == "all",
            "opacity" => zero(&value),
            // MJML zeroes it on wrappers to close inline-block gaps, and sets
            // it again inside; images show whatever the font size.
            "font-size" => zero(&value) && !any_below(node, &sizes_text) && !has_image(node),
            "max-height" => {
                collapsed = zero(&value);
                false
            }
            "overflow" => {
                clipped = value == "hidden";
                false
            }
            _ => false,
        };

        if hides {
            return true;
        }
    }

    collapsed && clipped
}

/// Whether `node` is an image of at most one pixel, as tracking pixels are.
pub fn pixel(node: &Handle) -> bool {
    let tiny = |value: &str| {
        let number = value.trim().trim_end_matches("px");
        number == "0" || number == "1"
    };

    let sized = ["width", "height"]
        .into_iter()
        .any(|key| attr(node, key).is_some_and(|value| tiny(&value)));
    sized
        || attr(node, "style").is_some_and(|style| {
            declarations(&style).any(|(property, value)| {
                matches!(property.as_str(), "width" | "height") && tiny(&value)
            })
        })
}

fn sizes_text(node: &Handle) -> bool {
    attr(node, "style").is_some_and(|style| {
        declarations(&style).any(|(property, value)| property == "font-size" && !zero(&value))
    })
}

/// The `property: value` pairs of an inline style, lowercased, with any
/// `!important` dropped.
fn declarations(style: &str) -> impl Iterator<Item = (String, String)> {
    style.split(';').filter_map(|declaration| {
        let (property, value) = declaration.split_once(':')?;
        let value = value.to_ascii_lowercase();
        let value = value.trim().trim_end_matches("!important").trim_end();
        Some((property.trim().to_ascii_lowercase(), value.to_owned()))
    })
}

/// Whether a CSS length or number is zero, whatever its unit.
fn zero(value: &str) -> bool {
    let number = value.trim_end_matches(|c: char| c.is_ascii_alphabetic() || c == '%');
    number.contains('0') && number.chars().all(|c| c == '0' || c == '.')
}
