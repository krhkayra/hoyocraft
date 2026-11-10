//! Navigation entries are independent of Slint widgets. Add a game descriptor,
//! artwork and its controller/launch rules before exposing another selectable game.
#[derive(Debug, Clone, Copy)]
pub struct GameDescriptor {
    pub id: &'static str,
    pub title: &'static str,
    pub artwork: &'static [u8],
}

pub const GAMES: &[GameDescriptor] = &[GameDescriptor {
    id: "genshin",
    title: "Genshin Impact",
    artwork: include_bytes!("../assets/genshin-impact.webp"),
}];

#[derive(Debug, Clone, Copy)]
pub struct SocialLink {
    pub title: &'static str,
    pub url: &'static str,
}

// These are the user's accounts, not the official game accounts or repository.
pub const SOCIAL_LINKS: &[SocialLink] = &[
    SocialLink {
        title: "X",
        url: "https://x.com/kayrakrh",
    },
    SocialLink {
        title: "Instagram",
        url: "https://www.instagram.com/kayra.krh/",
    },
    SocialLink {
        title: "GitHub",
        url: "https://github.com/krhkayra",
    },
];
