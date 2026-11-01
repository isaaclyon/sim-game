//! Display names derived from the world seed and IDs (ADR 0022). Nothing is stored or drawn
//! from a gameplay RNG, so naming cannot change the simulation. Changing these lists or the
//! mixing below renames everyone; that is acceptable because names are display only.

const GIVEN: [&str; 64] = [
    "Ada", "Amir", "Ana", "Arjun", "Beatriz", "Ben", "Bilal", "Carmen", "Chen", "Chloe", "Dara",
    "David", "Diego", "Elena", "Emeka", "Emma", "Farah", "Felix", "Grace", "Hana", "Hugo", "Ines",
    "Isaac", "Ivan", "Jae", "Jamal", "Jonas", "Julia", "Kai", "Kemal", "Lars", "Leila", "Lena",
    "Luis", "Maya", "Mei", "Mia", "Nadia", "Noah", "Nora", "Omar", "Oscar", "Priya", "Rafael",
    "Rosa", "Ruth", "Sam", "Sara", "Sofia", "Tariq", "Tess", "Theo", "Tomas", "Uma", "Vera",
    "Victor", "Wen", "Yara", "Yusuf", "Zara", "Zoe", "Ezra", "Iris", "Malik",
];

const SURNAMES: [&str; 64] = [
    "Abbott",
    "Adeyemi",
    "Alvarez",
    "Andersen",
    "Bauer",
    "Becker",
    "Bose",
    "Brennan",
    "Castro",
    "Chowdhury",
    "Costa",
    "Dimitrov",
    "Dubois",
    "Eriksen",
    "Fischer",
    "Flores",
    "Garcia",
    "Haddad",
    "Hart",
    "Horvat",
    "Ibrahim",
    "Ito",
    "Jensen",
    "Kapoor",
    "Kim",
    "Kowalski",
    "Larsen",
    "Lee",
    "Lopez",
    "Mendes",
    "Moreau",
    "Morgan",
    "Nakamura",
    "Nguyen",
    "Novak",
    "Okafor",
    "Olsen",
    "Ortiz",
    "Park",
    "Patel",
    "Petrov",
    "Quinn",
    "Reyes",
    "Rossi",
    "Sato",
    "Schmidt",
    "Silva",
    "Singh",
    "Sokolov",
    "Suzuki",
    "Tanaka",
    "Torres",
    "Turner",
    "Varga",
    "Vogel",
    "Wagner",
    "Walsh",
    "Wang",
    "Weber",
    "Yilmaz",
    "Young",
    "Zhang",
    "Ziegler",
    "Zubiri",
];

/// SplitMix64 finaliser: a well-mixed index from `seed`, a per-list salt and an ID.
fn mix(seed: u64, salt: u64, id: u32) -> usize {
    crate::rng::mix(seed ^ salt, u64::from(id)) as usize
}

/// Given name from the citizen, surname from the household. A shared surname is a display
/// convention for people who live together, not a family relationship.
pub(super) fn display_name(seed: u64, citizen: u32, household: u32) -> String {
    let given = GIVEN[mix(seed, 0x6769_7665, citizen) % GIVEN.len()];
    let surname = SURNAMES[mix(seed, 0x7375_726e, household) % SURNAMES.len()];
    format!("{given} {surname}")
}
