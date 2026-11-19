# Creation choices by level

The existing Dwarf presets, ability arrays, backgrounds, level 4 ability increases, equipment themes, subclasses, and previously exposed spell choices are preserved. The additions below are the selected fixed ledger. Coverage and integration readiness are tracked separately in the matrices. A later entry adds to the previous level's selections unless it explicitly moves a spell into the always-prepared list.

## Spell preparation

The five numbers in each cell correspond to levels 1 through 5. Cantrips and spells granted by features are tracked separately from the base allowance. The addition table lists inferred build choices instead of exposing every legal class spell in combat. [Class feature and spellcasting tables, SRD 5.2, pp. 31–82](https://media.dndbeyond.com/compendium-images/srd/5.2/SRD_CC_v5.2.pdf#page=31)

| Preset | Base cantrips | Base prepared spells | Slot ceiling |
|---|---|---|---|
| Bard | 2 / 2 / 2 / 3 / 3 | 4 / 5 / 6 / 7 / 9 | 1 / 1 / 2 / 2 / 3 |
| Cleric | 3 / 3 / 3 / 4 / 4 | 4 / 5 / 6 / 7 / 9 | 1 / 1 / 2 / 2 / 3 |
| Druid | 2 / 2 / 2 / 3 / 3 | 4 / 5 / 6 / 7 / 9 | 1 / 1 / 2 / 2 / 3 |
| Paladin | 0 / 0 / 0 / 0 / 0 | 2 / 3 / 4 / 5 / 6 | 1 / 1 / 1 / 1 / 2 |
| Ranger | 0 / 0 / 0 / 0 / 0 | 2 / 3 / 4 / 5 / 6 | 1 / 1 / 1 / 1 / 2 |
| Sorcerer | 4 / 4 / 4 / 5 / 5 | 2 / 4 / 6 / 7 / 9 | 1 / 1 / 2 / 2 / 3 |
| Warlock | 2 / 2 / 2 / 3 / 3 | 2 / 3 / 4 / 5 / 6 | 1 / 1 / 2 / 2 / 3 (Pact Magic) |
| Wizard | 3 / 3 / 3 / 4 / 4 | 4 / 5 / 6 / 7 / 9 | 1 / 1 / 2 / 2 / 3 |

### Cantrip selections

In the two selection tables below, **bold** marks a fixed cantrip or prepared choice that was unavailable in the previous playable build (`1c33d64`). A subclass can grant the same spell independently. The spellbook-only and other fixed grants are listed separately below.

| Preset | Level 1 | Level 4 |
|---|---|---|
| Bard | Vicious Mockery, Starry Wisp | **True Strike** |
| Cleric | Sacred Flame, **Guidance**, **Resistance** | **Light** |
| Druid | Starry Wisp, Shillelagh | **Resistance** |
| Sorcerer | Fire Bolt, Ray of Frost, **Shocking Grasp**, **Light** | **True Strike** |
| Warlock | Eldritch Blast, **Poison Spray** | **True Strike** |
| Wizard | Fire Bolt, Ray of Frost, **Shocking Grasp** | **True Strike** |

Guide's Magic Initiate (Druid) grants the existing Druidcraft, Mending and Speak with Animals picks. Druidic additionally grants Speak with Animals to the Druid. Where the same spell is granted from two sources, retain separate source and payment records and expose a single identifiable spell with its legal payments.

### Base prepared progression

| Preset | Level 1 | Level 2 | Level 3 | Level 4 | Level 5 |
|---|---|---|---|---|---|
| Bard | Dissonant Whispers, Healing Word, Cure Wounds, **Thunderwave** | +**Command** | +Shatter | +**Hold Person** | +Hypnotic Pattern, **Dispel Magic** |
| Cleric | Guiding Bolt, Healing Word, Cure Wounds, Bless | +**Shield of Faith** | Cure Wounds and Bless become Life grants. +Spiritual Weapon, **Command**, **Hold Person** | +**Sanctuary** | +Spirit Guardians, **Dispel Magic** |
| Druid | Thunderwave, Healing Word, Cure Wounds, **Longstrider** | +**Entangle** | +Moonbeam | +**Hold Person** | +**Protection from Energy**, **Dispel Magic** |
| Paladin | Divine Favor, Divine Smite | Divine Smite becomes a class grant. +Bless, Cure Wounds | +**Command** | +**Heroism** | +**Magic Weapon** |
| Ranger | Cure Wounds, **Longstrider** | +**Entangle** | +**Jump** | +**Ensnaring Strike** | +**Magic Weapon** |
| Sorcerer | Chromatic Orb, Shield | +Magic Missile, **Thunderwave** | Chromatic Orb becomes a Draconic grant. +Scorching Ray, **Burning Hands**, **Hold Person** | +**Shatter** | +Fireball, **Counterspell** |
| Warlock | Hex, **Bane** | +**Expeditious Retreat** | +**Hold Person** | +**Misty Step** | +**Counterspell** |
| Wizard | Magic Missile, Burning Hands, Shield, **Mage Armor** | +**Thunderwave** | +Scorching Ray | +**Shatter** | +Fireball, **Misty Step** |

No selectable pick on this table displaces a previously exposed option. Burning Hands and Scorching Ray stay available for the Fiend via fixed grants. Fireball stays available for the Land Druid via the Arid grant. Each addition links to tested combat behavior or a documented unreachable trigger in the [spell coverage matrix](magic.tsv).


### Always prepared and book-only entries

- Life Cleric gains Aid, Bless, Cure Wounds and Lesser Restoration at level 3, then Mass Healing Word and Revivify at 5.
- Arid Druid gains Blur, Burning Hands and Fire Bolt at level 3, then Fireball at 5.
- Paladin gains Divine Smite and its one free long-rest use at 2. Devotion gains Protection from Evil and Good and Shield of Faith at 3, then Aid and Zone of Truth at 5. Faithful Steed separately grants Find Steed and its free long-rest use at 5.
- Ranger gains Hunter's Mark at 1, with its free casts separate from spell-slot payments.
- Draconic Sorcerer gains Alter Self, Chromatic Orb, Command and Dragon's Breath at 3, then Fear and Fly at 5.
- Fiend Warlock gains Burning Hands, Command, Scorching Ray and Suggestion at 3, then Fireball and Stinking Cloud at 5.
- Wizard spellbook at 1 holds the four prepared spells plus Thunderwave and Find Familiar. At 2 add Grease and Expeditious Retreat. At 3 add Scorching Ray and Misty Step, plus Shatter and Gust of Wind from Evocation Savant. At 4 add Blur and Mirror Image. At 5 add Fireball and Counterspell, plus Lightning Bolt from Evocation Savant. Book sizes are 6, 8, 12, 14 and 17. Memorize Spell at 5 swaps preparation after a short rest. Ritual Adept allows rituals directly from the book, subject to their casting time and components. A familiar present at the first turn requires explicit encounter preparation, or a casting window that actually permits Find Familiar. [Evoker and Wizard rules, SRD 5.2, pp. 77–82](https://media.dndbeyond.com/compendium-images/srd/5.2/SRD_CC_v5.2.pdf#page=77)

## Other fixed creation decisions

- Warlock invocations. Add Eldritch Mind at level 1, preserve Agonizing Blast for Eldritch Blast and Fiendish Vigor at 2, preserve Repelling Blast for Eldritch Blast and add Eldritch Spear for Eldritch Blast at 5. Those five choices satisfy the 1/3/3/3/5 quotas. Eldritch Spear and Repelling Blast have the relevant damaging-cantrip prerequisites. [Invocations, SRD 5.2, pp. 71–75](https://media.dndbeyond.com/compendium-images/srd/5.2/SRD_CC_v5.2.pdf#page=71)
- Druid known forms. Preserve Wolf and add the other SRD suggestions, Rat, Riding Horse and Spider at level 2. At level 4 add Black Bear and Giant Badger. Eligible limits are four known forms of CR ≤¼ at level 2 and six of CR ≤½ at 4, each without a fly speed. These fix the learned roster. The [Wild Shape evidence](wild-shape.tsv) covers equipment, actions, movement, senses and reversion for these forms. [Wild Shape and Beast Shapes, SRD 5.2, pp. 42–43](https://media.dndbeyond.com/compendium-images/srd/5.2/SRD_CC_v5.2.pdf#page=42)
- Keep Protector Cleric, Warden Druid, Defense Fighter, Defense Paladin, Archery Ranger, Colossus Slayer Hunter, and Quickened and Heightened Sorcerer. Open Hand's Addle, Push and Topple are options per eligible hit, not separate build choices. There are distinct per-use tactical choices for metamagic casts, rider spending and shared-resource payment.
- Mastery choice counts through this tier are Barbarian 2/2/2/3/3, Fighter 3/3/3/4/4, and Paladin, Ranger and Rogue 2 at every level. Preserve known masteries for Greataxe, Longsword, Shortsword, Longbow, Rapier and Shortbow. Barbarian also learns Longsword at 1 and Rapier at 4. Fighter also learns Greataxe and Shortbow at 1 and Longbow at 4. Paladin also learns Rapier at 1. Learning mastery grants no equipment. The extra choices can apply if the character actually takes possession of that weapon during combat.
- Background feat, skill, language, tool, class proficiencies, exact equipment and costs remain tracked in the equipment and origin matrix. The combat audit accounts for any effects these selections make available. [Creation and equipment, SRD 5.2, pp. 20–27 and 83–103](https://media.dndbeyond.com/compendium-images/srd/5.2/SRD_CC_v5.2.pdf#page=20)

## Skill, tool and language selections

Class and background proficiency choices come from the 2024 rules. The listed Expertise starts at the grant level and remains through level 5. [Background proficiencies](https://www.dndbeyond.com/backgrounds?filter-partnered-content=f&filter-source=145) · [Class traits, SRD 5.2, pp. 28–82](https://media.dndbeyond.com/compendium-images/srd/5.2/SRD_CC_v5.2.pdf#page=28) · [Executable manifest](../../src/arena2/origins.rs)

| Preset | Base class skills | Background skills | Additional skills and Expertise |
|---|---|---|---|
| Barbarian | Athletics, Perception | Investigation, Persuasion | Nature at 3 |
| Bard | Perception, Stealth, Insight | Acrobatics, Performance | Expertise in Perception and Stealth at 2. Athletics, Deception and Investigation at 3 |
| Cleric | Insight, Religion | Stealth, Survival | — |
| Druid | Perception, Nature | Stealth, Survival | — |
| Fighter | Athletics, Perception | Investigation, Persuasion | — |
| Monk | Acrobatics, Athletics | Stealth, Survival | — |
| Paladin | Athletics, Insight | History, Persuasion | Skilled selects Perception, Stealth and Medicine |
| Ranger | Perception, Investigation, Animal Handling | Stealth, Survival | Expertise in Perception at 2 |
| Rogue | Acrobatics, Perception, Stealth, Sleight of Hand | Investigation, Persuasion | Expertise in Stealth and Sleight of Hand at 1 |
| Sorcerer | Arcana, Insight | Deception, Sleight of Hand | Skilled selects Acrobatics, Athletics and Perception |
| Warlock | Arcana, Investigation | Deception, Sleight of Hand | Skilled selects Acrobatics, Athletics and Perception |
| Wizard | Arcana, Nature | Investigation, Perception | Skilled selects Acrobatics, Stealth and Athletics. Scholar gives Expertise in Arcana at 2 |

- Everyone speaks Common, Dwarvish and Elvish. Druid adds Druidic, Rogue adds Thieves' Cant and Goblin, and Ranger at level 2 adds Draconic and Halfling.
- Artisan selects smith's tools. Its Crafter feat adds leatherworker's tools, woodcarver's tools and tinker's tools. Entertainer selects lute. Bard's class tools are bagpipes, drum and flute, and Musician adds horn, lyre and viol. Guide grants cartographer's tools. Charlatan grants a forgery kit. Noble chooses dragonchess. Scribe grants calligrapher's supplies.
- Druid receives the herbalism kit proficiency. Monk chooses cook's utensils. Rogue receives thieves' tools proficiency from the class. The level 5 Poison option requires a carried poisoner's kit without granting proficiency. Supplied equipment can include additional items and consumables without granting new proficiencies. [Tools and feats, SRD 5.2, pp. 85–99](https://media.dndbeyond.com/compendium-images/srd/5.2/SRD_CC_v5.2.pdf#page=85)
