// Emoji data for react command
// Custom animated emojis

pub struct Emoji {
    pub id: &'static str,
    pub name: &'static str,
}

pub const EMOJIS: &[Emoji] = &[
    Emoji {
        id: "1384171532955422730",
        name: "UmaruLaugh",
    },
    Emoji {
        id: "1384173218058997881",
        name: "3147bluefire",
    },
    Emoji {
        id: "1384173220743221408",
        name: "58346fire",
    },
    Emoji {
        id: "1384173229584945232",
        name: "61652murderouscat",
    },
    Emoji {
        id: "1384172648854065293",
        name: "AppJedi",
    },
    Emoji {
        id: "1384852089490116698",
        name: "dr_senku_bubble",
    },
    Emoji {
        id: "1384852101611393106",
        name: "jinwoo_sololeveling",
    },
    Emoji {
        id: "1384852105407365150",
        name: "Pepe_King_Animated",
    },
    Emoji {
        id: "1384852115322572881",
        name: "fading_crying_emoji",
    },
    Emoji {
        id: "1384852122847281172",
        name: "Flud_Cat_Cry_Screech",
    },
    Emoji {
        id: "1384852130350764142",
        name: "crown_yellow_gif",
    },
    Emoji {
        id: "1384852136218595418",
        name: "mWhatOwO",
    },
    Emoji {
        id: "1384852148793118791",
        name: "zerotwo_party",
    },
    Emoji {
        id: "1384852152475713650",
        name: "no_anime5",
    },
    Emoji {
        id: "1384852157232058470",
        name: "PaimonTriggerredPing",
    },
    Emoji {
        id: "1384852168552743023",
        name: "cortesdemanga",
    },
    Emoji {
        id: "1384852172776144937",
        name: "JBF_actingSusNotMeOwO",
    },
    Emoji {
        id: "1384852180082888765",
        name: "CryingManAnimated",
    },
    Emoji {
        id: "1384852196134223922",
        name: "saanimegirlshoot",
    },
    Emoji {
        id: "1384852202803429477",
        name: "RaveWeebTC",
    },
    Emoji {
        id: "1384852208746627112",
        name: "hanyaCheer",
    },
];

pub fn get_emoji_by_id(id: &str) -> Option<&'static Emoji> {
    EMOJIS.iter().find(|e| e.id == id)
}
