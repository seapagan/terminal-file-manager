//! Shared policy explanations for choice menus and contextual Help.

pub(super) fn text(action: crate::input::Action) -> &'static [&'static str] {
    use crate::input::Action;
    match action {
        Action::Sort => SORT,
        Action::Group => GROUP,
        Action::Hidden => HIDDEN,
        Action::Ignore => IGNORE,
        _ => &[],
    }
}

const NAVIGATION: &str = "Choose with `Up` / `Down` or the mouse wheel; `Home` / `End` selects the first or last. Press `Enter` or `Space`, or click a choice, to apply it. Press `Esc` or click outside to cancel. Press another popup shortcut to switch menus, or the current menu's shortcut to close it. Both discard the pending choice. Press `F1` or click ‘[?]’ for Help; `Esc` in Help returns to the same highlighted choice.";

const SORT: &[&str] = &[
    "Choose the order within each directory group. ‘Natural’ compares digit runs as numbers, so file2 comes before file10; ASCII letter case does not affect the order. ‘Name’ compares names without letter case, then uses the original name to break ties.",
    "‘Extension’ compares the final suffix with case-sensitive ordering, with extensionless names first. ‘Size’ puts the largest entries first. ‘Modified’ and ‘Created’ put the newest timestamps first; entries with unavailable sizes or timestamps come last.",
    "‘Kind’ orders entry types: directories, files, symbolic links, junctions, other reparse points, sockets, FIFOs, devices, then unknown types. ‘Version’ uses version-aware ordering for names such as release2 and release10; non-Unicode names use ‘Name’ ordering.",
    "‘Unsorted’ keeps the filesystem's directory-listing order within each group. That order may change after a refresh.",
    "Equal sort values use ‘Name’ ordering to break ties, except in ‘Unsorted’ mode. ‘Reverse’ flips the chosen order and its name tie-breaks, including the placement of unavailable metadata. Directory grouping stays ‘First’, ‘Last’ or ‘Mixed’; ‘Reverse’ has no effect on ‘Unsorted’.",
    NAVIGATION,
];

const GROUP: &[&str] = &[
    "‘First’ puts enterable directories before other entries. ‘Last’ puts them after other entries. ‘Mixed’ sorts both together. Directory links you can enter belong to the directory group.",
    "Your ‘Sort’ choice applies within each group. ‘Reverse’ changes that order while keeping ‘First’ or ‘Last’ in place. With ‘Unsorted’, each group keeps its directory-listing order.",
    NAVIGATION,
];

const HIDDEN: &[&str] = &[
    "‘Hide’ excludes hidden entries. ‘Show’ includes them, subject to the active name filter and ‘Ignore’ policy. On Unix, hidden names start with a dot; on Windows, the native Hidden attribute identifies hidden entries.",
    "Showing hidden entries does not reveal entries excluded by ‘Ignore: Hide’ or the name filter. Hidden entries keep their marks, but file operations use only the visible selection.",
    NAVIGATION,
];

const IGNORE: &[&str] = &[
    "‘Dim’ shows Git-ignored entries with subdued names. ‘Hide’ excludes them. ‘Off’ shows them with normal name styling. Hidden-entry settings and the active name filter still apply in all three modes.",
    "These settings change how tfm displays Git-ignored entries; they do not edit ignore files. Excluded entries keep their marks, but file operations use only the visible selection.",
    NAVIGATION,
];

pub(super) fn title(action: crate::input::Action) -> &'static str {
    use crate::input::Action;
    match action {
        Action::Sort => "Sort Help",
        Action::Group => "Group Help",
        Action::Hidden => "Hidden Help",
        Action::Ignore => "Ignore Help",
        _ => "Policy Help",
    }
}
