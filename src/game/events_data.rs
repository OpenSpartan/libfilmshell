// Generated from the game's files. Do not edit by hand.

use super::Vehicle;
use super::events::EventGroup;

/// Each mode group's event files and the ids of its vehicle destroy and hijack responses.
pub(crate) static EVENT_GROUPS: &[EventGroup] = &[
    EventGroup {
        files: &["attritionevents.lua", "eliminationconfig.lua"],
        destroys: &[(0xec, Vehicle::Banshee), (0xed, Vehicle::BruteChopper), (0xee, Vehicle::Falcon), (0xef, Vehicle::Ghost), (0xf0, Vehicle::Mongoose), (0xf1, Vehicle::Mongoose), (0xf2, Vehicle::Phantom), (0xf3, Vehicle::Scorpion), (0xf4, Vehicle::Warthog), (0xf5, Vehicle::Warthog), (0xf6, Vehicle::Warthog), (0xf7, Vehicle::Wasp), (0xf8, Vehicle::Wraith)],
        hijacks: &[(0x102, Vehicle::Banshee), (0x103, Vehicle::BruteChopper), (0x104, Vehicle::Falcon), (0x105, Vehicle::Ghost), (0x106, Vehicle::Mongoose), (0x107, Vehicle::Mongoose), (0x108, Vehicle::Phantom), (0x109, Vehicle::Scorpion), (0x10a, Vehicle::Warthog), (0x10b, Vehicle::Warthog), (0x10c, Vehicle::Warthog), (0x10d, Vehicle::Wasp), (0x10e, Vehicle::Wraith)],
    },
    EventGroup {
        files: &["areacaptureeventstubs.lua", "bastionevents.lua"],
        destroys: &[(0xee, Vehicle::Banshee), (0xef, Vehicle::BruteChopper), (0xf0, Vehicle::Falcon), (0xf1, Vehicle::Ghost), (0xf2, Vehicle::Mongoose), (0xf3, Vehicle::Mongoose), (0xf4, Vehicle::Phantom), (0xf5, Vehicle::Scorpion), (0xf6, Vehicle::Warthog), (0xf7, Vehicle::Warthog), (0xf8, Vehicle::Warthog), (0xf9, Vehicle::Wasp), (0xfa, Vehicle::Wraith)],
        hijacks: &[(0x104, Vehicle::Banshee), (0x105, Vehicle::BruteChopper), (0x106, Vehicle::Falcon), (0x107, Vehicle::Ghost), (0x108, Vehicle::Mongoose), (0x109, Vehicle::Mongoose), (0x10a, Vehicle::Phantom), (0x10b, Vehicle::Scorpion), (0x10c, Vehicle::Warthog), (0x10d, Vehicle::Warthog), (0x10e, Vehicle::Warthog), (0x10f, Vehicle::Wasp), (0x110, Vehicle::Wraith)],
    },
    EventGroup {
        files: &["bigteambattle.lua", "btbslayerevents.lua"],
        destroys: &[(0xe5, Vehicle::Banshee), (0xe6, Vehicle::BruteChopper), (0xe7, Vehicle::Falcon), (0xe8, Vehicle::Ghost), (0xe9, Vehicle::Mongoose), (0xea, Vehicle::Mongoose), (0xeb, Vehicle::Phantom), (0xec, Vehicle::Scorpion), (0xed, Vehicle::Warthog), (0xee, Vehicle::Warthog), (0xef, Vehicle::Warthog), (0xf0, Vehicle::Wasp), (0xf1, Vehicle::Wraith)],
        hijacks: &[(0xfb, Vehicle::Banshee), (0xfc, Vehicle::BruteChopper), (0xfd, Vehicle::Falcon), (0xfe, Vehicle::Ghost), (0xff, Vehicle::Mongoose), (0x100, Vehicle::Mongoose), (0x101, Vehicle::Phantom), (0x102, Vehicle::Scorpion), (0x103, Vehicle::Warthog), (0x104, Vehicle::Warthog), (0x105, Vehicle::Warthog), (0x106, Vehicle::Wasp), (0x107, Vehicle::Wraith)],
    },
    EventGroup {
        files: &["arenaflagevents.lua", "flageventstubs.lua"],
        destroys: &[(0xee, Vehicle::Banshee), (0xef, Vehicle::BruteChopper), (0xf0, Vehicle::Falcon), (0xf1, Vehicle::Ghost), (0xf2, Vehicle::Mongoose), (0xf3, Vehicle::Mongoose), (0xf4, Vehicle::Phantom), (0xf5, Vehicle::Scorpion), (0xf6, Vehicle::Warthog), (0xf7, Vehicle::Warthog), (0xf8, Vehicle::Warthog), (0xf9, Vehicle::Wasp), (0xfa, Vehicle::Wraith)],
        hijacks: &[(0x104, Vehicle::Banshee), (0x105, Vehicle::BruteChopper), (0x106, Vehicle::Falcon), (0x107, Vehicle::Ghost), (0x108, Vehicle::Mongoose), (0x109, Vehicle::Mongoose), (0x10a, Vehicle::Phantom), (0x10b, Vehicle::Scorpion), (0x10c, Vehicle::Warthog), (0x10d, Vehicle::Warthog), (0x10e, Vehicle::Warthog), (0x10f, Vehicle::Wasp), (0x110, Vehicle::Wraith)],
    },
    EventGroup {
        files: &["eevents.lua"],
        destroys: &[(0xee, Vehicle::Banshee), (0xef, Vehicle::BruteChopper), (0xf0, Vehicle::Falcon), (0xf1, Vehicle::Ghost), (0xf2, Vehicle::Mongoose), (0xf3, Vehicle::Mongoose), (0xf4, Vehicle::Phantom), (0xf5, Vehicle::Scorpion), (0xf6, Vehicle::Warthog), (0xf7, Vehicle::Warthog), (0xf8, Vehicle::Warthog), (0xf9, Vehicle::Wasp), (0xfa, Vehicle::Wraith)],
        hijacks: &[(0x104, Vehicle::Banshee), (0x105, Vehicle::BruteChopper), (0x106, Vehicle::Falcon), (0x107, Vehicle::Ghost), (0x108, Vehicle::Mongoose), (0x109, Vehicle::Mongoose), (0x10a, Vehicle::Phantom), (0x10b, Vehicle::Scorpion), (0x10c, Vehicle::Warthog), (0x10d, Vehicle::Warthog), (0x10e, Vehicle::Warthog), (0x10f, Vehicle::Wasp), (0x110, Vehicle::Wraith)],
    },
    EventGroup {
        files: &["areacaptureeventstubs.lua", "fbastionevents.lua", "firefightevents.lua"],
        destroys: &[(0xe1, Vehicle::Banshee), (0xe2, Vehicle::BruteChopper), (0xe3, Vehicle::Falcon), (0xe4, Vehicle::Ghost), (0xe5, Vehicle::Mongoose), (0xe6, Vehicle::Mongoose), (0xe7, Vehicle::Phantom), (0xe8, Vehicle::Scorpion), (0xe9, Vehicle::Warthog), (0xea, Vehicle::Warthog), (0xeb, Vehicle::Warthog), (0xec, Vehicle::Wasp), (0xed, Vehicle::Wraith)],
        hijacks: &[(0xf7, Vehicle::Banshee), (0xf8, Vehicle::BruteChopper), (0xf9, Vehicle::Falcon), (0xfa, Vehicle::Ghost), (0xfb, Vehicle::Mongoose), (0xfc, Vehicle::Mongoose), (0xfd, Vehicle::Phantom), (0xfe, Vehicle::Scorpion), (0xff, Vehicle::Warthog), (0x100, Vehicle::Warthog), (0x101, Vehicle::Warthog), (0x102, Vehicle::Wasp), (0x103, Vehicle::Wraith)],
    },
    EventGroup {
        files: &["gungameevents.lua"],
        destroys: &[(0xee, Vehicle::Banshee), (0xef, Vehicle::BruteChopper), (0xf0, Vehicle::Falcon), (0xf1, Vehicle::Ghost), (0xf2, Vehicle::Mongoose), (0xf3, Vehicle::Mongoose), (0xf4, Vehicle::Phantom), (0xf5, Vehicle::Scorpion), (0xf6, Vehicle::Warthog), (0xf7, Vehicle::Warthog), (0xf8, Vehicle::Warthog), (0xf9, Vehicle::Wasp), (0xfa, Vehicle::Wraith)],
        hijacks: &[(0x104, Vehicle::Banshee), (0x105, Vehicle::BruteChopper), (0x106, Vehicle::Falcon), (0x107, Vehicle::Ghost), (0x108, Vehicle::Mongoose), (0x109, Vehicle::Mongoose), (0x10a, Vehicle::Phantom), (0x10b, Vehicle::Scorpion), (0x10c, Vehicle::Warthog), (0x10d, Vehicle::Warthog), (0x10e, Vehicle::Warthog), (0x10f, Vehicle::Wasp), (0x110, Vehicle::Wraith)],
    },
    EventGroup {
        files: &["oddballevents.lua"],
        destroys: &[(0xee, Vehicle::Banshee), (0xef, Vehicle::BruteChopper), (0xf0, Vehicle::Falcon), (0xf1, Vehicle::Ghost), (0xf2, Vehicle::Mongoose), (0xf3, Vehicle::Mongoose), (0xf4, Vehicle::Phantom), (0xf5, Vehicle::Scorpion), (0xf6, Vehicle::Warthog), (0xf7, Vehicle::Warthog), (0xf8, Vehicle::Warthog), (0xf9, Vehicle::Wasp), (0xfa, Vehicle::Wraith)],
        hijacks: &[(0x104, Vehicle::Banshee), (0x105, Vehicle::BruteChopper), (0x106, Vehicle::Falcon), (0x107, Vehicle::Ghost), (0x108, Vehicle::Mongoose), (0x109, Vehicle::Mongoose), (0x10a, Vehicle::Phantom), (0x10b, Vehicle::Scorpion), (0x10c, Vehicle::Warthog), (0x10d, Vehicle::Warthog), (0x10e, Vehicle::Warthog), (0x10f, Vehicle::Wasp), (0x110, Vehicle::Wraith)],
    },
    EventGroup {
        files: &["slayerevents.lua"],
        destroys: &[(0xee, Vehicle::Banshee), (0xef, Vehicle::BruteChopper), (0xf0, Vehicle::Falcon), (0xf1, Vehicle::Ghost), (0xf2, Vehicle::Mongoose), (0xf3, Vehicle::Mongoose), (0xf4, Vehicle::Phantom), (0xf5, Vehicle::Scorpion), (0xf6, Vehicle::Warthog), (0xf7, Vehicle::Warthog), (0xf8, Vehicle::Warthog), (0xf9, Vehicle::Wasp), (0xfa, Vehicle::Wraith)],
        hijacks: &[(0x104, Vehicle::Banshee), (0x105, Vehicle::BruteChopper), (0x106, Vehicle::Falcon), (0x107, Vehicle::Ghost), (0x108, Vehicle::Mongoose), (0x109, Vehicle::Mongoose), (0x10a, Vehicle::Phantom), (0x10b, Vehicle::Scorpion), (0x10c, Vehicle::Warthog), (0x10d, Vehicle::Warthog), (0x10e, Vehicle::Warthog), (0x10f, Vehicle::Wasp), (0x110, Vehicle::Wraith)],
    },
    EventGroup {
        files: &["bigteambattle.lua", "btbstockpileevents.lua"],
        destroys: &[(0xe5, Vehicle::Banshee), (0xe6, Vehicle::BruteChopper), (0xe7, Vehicle::Falcon), (0xe8, Vehicle::Ghost), (0xe9, Vehicle::Mongoose), (0xea, Vehicle::Mongoose), (0xeb, Vehicle::Phantom), (0xec, Vehicle::Scorpion), (0xed, Vehicle::Warthog), (0xee, Vehicle::Warthog), (0xef, Vehicle::Warthog), (0xf0, Vehicle::Wasp), (0xf1, Vehicle::Wraith)],
        hijacks: &[(0xfb, Vehicle::Banshee), (0xfc, Vehicle::BruteChopper), (0xfd, Vehicle::Falcon), (0xfe, Vehicle::Ghost), (0xff, Vehicle::Mongoose), (0x100, Vehicle::Mongoose), (0x101, Vehicle::Phantom), (0x102, Vehicle::Scorpion), (0x103, Vehicle::Warthog), (0x104, Vehicle::Warthog), (0x105, Vehicle::Warthog), (0x106, Vehicle::Wasp), (0x107, Vehicle::Wraith)],
    },
    EventGroup {
        files: &["areacaptureeventstubs.lua", "sevents.lua"],
        destroys: &[(0xee, Vehicle::Banshee), (0xef, Vehicle::BruteChopper), (0xf0, Vehicle::Falcon), (0xf1, Vehicle::Ghost), (0xf2, Vehicle::Mongoose), (0xf3, Vehicle::Mongoose), (0xf4, Vehicle::Phantom), (0xf5, Vehicle::Scorpion), (0xf6, Vehicle::Warthog), (0xf7, Vehicle::Warthog), (0xf8, Vehicle::Warthog), (0xf9, Vehicle::Wasp), (0xfa, Vehicle::Wraith)],
        hijacks: &[(0x104, Vehicle::Banshee), (0x105, Vehicle::BruteChopper), (0x106, Vehicle::Falcon), (0x107, Vehicle::Ghost), (0x108, Vehicle::Mongoose), (0x109, Vehicle::Mongoose), (0x10a, Vehicle::Phantom), (0x10b, Vehicle::Scorpion), (0x10c, Vehicle::Warthog), (0x10d, Vehicle::Warthog), (0x10e, Vehicle::Warthog), (0x10f, Vehicle::Wasp), (0x110, Vehicle::Wraith)],
    },
];
