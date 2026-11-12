use crate::StringId;

/// The English text of one of the game's text keys that films use: the kill
/// feed's and the callouts' lines, medal and player event text, personal
/// score text, map place names and HUD messages, by the key's string id
/// ([`GameEvent::text`](crate::GameEvent::text),
/// [`HudMessage::key`](crate::HudMessage::key)).
///
/// The text is as the game stores it, with its markup: `%pa`, `%pe` and
/// `%ps` are players and `%p` one (the event's player arguments name them),
/// `%s` a string and `%i` a number, `&killfeed_<name>` a kill feed icon,
/// `%c` and a letter opens coloured text that `%` closes, and a leading
/// `%zy` sets the line's style.
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

#[cfg(test)]
mod tests {
    #[test]
    fn the_table_is_in_order() {
        assert!(super::super::text_data::TEXT.windows(2).all(|w| w[0].0 < w[1].0));
    }
}
