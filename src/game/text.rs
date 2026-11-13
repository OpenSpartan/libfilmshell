use crate::StringId;

/// The English text of a game text key that films use, by its string id.
///
/// The keys are the kill feed's and the callouts' lines, medal and player event
/// text, personal score text, map place names and HUD messages
/// ([`GameEvent::text`](crate::GameEvent::text),
/// [`HudMessage::key`](crate::HudMessage::key)).
///
/// The text is as the game stores it, with its markup: `%p`, `%pa`, `%pe`
/// and `%ps` are players (the event's player arguments name them, in
/// order), `%s` a string and `%i` a number, `&killfeed_<name>` a kill feed
/// icon, `%c` and a letter open coloured text that `%` closes, and a leading
/// `%z` and a letter set the line's style. [`text_parts`] splits it.
///
/// ```
/// use filmshell::StringId;
///
/// assert_eq!(filmshell::game::text(StringId(0x340d4e86)), Some("%s %s"));
/// ```
pub fn text(key: StringId) -> Option<&'static str> {
    let table = super::text_data::TEXT;
    table.binary_search_by_key(&key.0, |t| t.0).ok().map(|i| table[i].1)
}

/// One piece of a game text: words as written, or a place its markup fills in.
///
/// The player, string and number places take the event's arguments in order
/// ([`GameEvent::arguments`](crate::GameEvent::arguments),
/// [`HudMessage::arguments`](crate::HudMessage::arguments)).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TextPart<'a> {
    /// Words, as written.
    Words(&'a str),
    /// A player (`%p`, `%pa`, `%pe`, `%ps`), with the letter after `%p`
    /// when there is one.
    Player(Option<char>),
    /// A string (`%s`): a string id, whose text [`text`] gives.
    String,
    /// A number (`%i`).
    Number,
    /// A kill feed icon (`&killfeed_<name>`), by its whole name
    /// (`killfeed_hydra`).
    Icon(&'a str),
    /// Coloured text starts (`%c` and the colour's letter).
    ColorStart(char),
    /// Coloured text ends (`%`).
    ColorEnd,
    /// The line's style (`%z` and its letter).
    Style(char),
}

/// A game text split into its words and its markup's places ([`text`]
/// describes the markup).
///
/// ```
/// use filmshell::game::{TextPart, text_parts};
///
/// let parts = text_parts("%zy%ps &killfeed_hydra %pe");
/// assert_eq!(
///     parts,
///     [
///         TextPart::Style('y'),
///         TextPart::Player(Some('s')),
///         TextPart::Words(" "),
///         TextPart::Icon("killfeed_hydra"),
///         TextPart::Words(" "),
///         TextPart::Player(Some('e')),
///     ]
/// );
/// ```
pub fn text_parts(text: &str) -> Vec<TextPart<'_>> {
    let mut out = Vec::new();
    let mut words = 0;
    let mut i = 0;
    let b = text.as_bytes();
    let letter = |j: usize| b.get(j).filter(|c| c.is_ascii_lowercase()).map(|&c| c as char);
    while i < b.len() {
        let (part, len) = match b[i] {
            b'%' => match b.get(i + 1) {
                Some(b'p') => match letter(i + 2).filter(|c| matches!(c, 'a' | 'e' | 's')) {
                    Some(k) => (TextPart::Player(Some(k)), 3),
                    None => (TextPart::Player(None), 2),
                },
                Some(b's') => (TextPart::String, 2),
                Some(b'i') => (TextPart::Number, 2),
                Some(b'c') => match letter(i + 2) {
                    Some(k) => (TextPart::ColorStart(k), 3),
                    None => (TextPart::ColorEnd, 1),
                },
                Some(b'z') => match letter(i + 2) {
                    Some(k) => (TextPart::Style(k), 3),
                    None => (TextPart::ColorEnd, 1),
                },
                _ => (TextPart::ColorEnd, 1),
            },
            b'&' => {
                let end = b[i + 1..]
                    .iter()
                    .position(|c| !(c.is_ascii_alphanumeric() || *c == b'_'))
                    .map_or(b.len(), |n| i + 1 + n);
                if end == i + 1 {
                    i += 1;
                    continue;
                }
                (TextPart::Icon(&text[i + 1..end]), end - i)
            }
            _ => {
                i += 1;
                continue;
            }
        };
        if words < i {
            out.push(TextPart::Words(&text[words..i]));
        }
        out.push(part);
        i += len;
        words = i;
    }
    if words < b.len() {
        out.push(TextPart::Words(&text[words..]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_in_order() {
        assert!(super::super::text_data::TEXT.windows(2).all(|w| w[0].0 < w[1].0));
    }

    #[test]
    fn markup_splits_into_its_places() {
        use TextPart::*;
        assert_eq!(text_parts("Killed %pe"), [Words("Killed "), Player(Some('e'))]);
        assert_eq!(text_parts("%p CAPTURED YOUR FLAG"), [Player(None), Words(" CAPTURED YOUR FLAG")]);
        assert_eq!(
            text_parts("%zy%ps &killfeed_callout %caShade Turret%"),
            [
                Style('y'),
                Player(Some('s')),
                Words(" "),
                Icon("killfeed_callout"),
                Words(" "),
                ColorStart('a'),
                Words("Shade Turret"),
                ColorEnd
            ]
        );
        assert_eq!(
            text_parts("You reached [Level %i: &killfeed_sniper ]"),
            [Words("You reached [Level "), Number, Words(": "), Icon("killfeed_sniper"), Words(" ]")]
        );
        assert_eq!(text_parts("%s %s"), [String, Words(" "), String]);
        assert_eq!(text_parts("Plain"), [Words("Plain")]);
        assert_eq!(text_parts(""), []);
    }

    /// Every text in the table splits, and its places put back together
    /// give the text again.
    #[test]
    fn every_text_splits_without_loss() {
        for &(_, t) in super::super::text_data::TEXT {
            let back: std::string::String = text_parts(t)
                .iter()
                .map(|p| match p {
                    TextPart::Words(w) => w.to_string(),
                    TextPart::Player(None) => "%p".into(),
                    TextPart::Player(Some(k)) => format!("%p{k}"),
                    TextPart::String => "%s".into(),
                    TextPart::Number => "%i".into(),
                    TextPart::Icon(n) => format!("&{n}"),
                    TextPart::ColorStart(k) => format!("%c{k}"),
                    TextPart::ColorEnd => "%".into(),
                    TextPart::Style(k) => format!("%z{k}"),
                })
                .collect();
            assert_eq!(back, t);
        }
    }
}
