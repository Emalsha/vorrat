//! The household plan: what a household writes down before a crisis.

/// The household plan's parts: (settings key, heading, hint). Each is free
/// text the household writes, kept as `hplan.<key>` in the settings.
pub const PLAN_FIELDS: &[(&str, &str, &str)] = &[
    (
        "meet_near",
        "Meeting point near home",
        "If you can't get home or have to leave it, for example a neighbour's door or a street corner.",
    ),
    (
        "meet_far",
        "Meeting point outside the area",
        "If the whole area has to be left, for example a relative's home in another town.",
    ),
    (
        "contacts",
        "Phone numbers",
        "One per line: family, work, school, doctor, the vet. Phones may be flat: keep this on paper.",
    ),
    (
        "relay",
        "Contact outside the area",
        "Someone far away everyone calls or texts to say they are safe, when local lines are busy.",
    ),
    (
        "water",
        "Where to turn off the water",
        "Where the main tap is, and what you need to turn it.",
    ),
    (
        "gas",
        "Where to turn off the gas",
        "Only if your home has gas. Leave turning it back on to a professional.",
    ),
    (
        "power",
        "Where the fuse box and main switch are",
        "And where the torch is that you will need to find them in the dark.",
    ),
    (
        "help",
        "Who needs help, and who helps them",
        "People who can't leave on their own, need medicine or care, or have to be picked up from school.",
    ),
    (
        "neighbours",
        "Neighbours",
        "Who to check on, and who could help you. Some guides suggest sharing stock and a radio.",
    ),
    (
        "notes",
        "Other notes",
        "The radio station for official news, the alert app your country uses, where the grab bags are.",
    ),
];
