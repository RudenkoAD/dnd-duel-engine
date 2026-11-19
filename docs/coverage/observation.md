# Interface for a single arena decision

The batched Rust observation and legal moves belong to the side currently choosing. This can be the defender, a familiar's owner or the player releasing a readied action. A fixed-seat Gym environment returns to that player at each of their decisions. The text interface requests the same native mask and action labels. When the game waits for the other player, an explicit seat projection returns an empty mask and omits the other player's pending parameters.

ABI 2002 has 128 phase-local action IDs and 1,024 float features. The first 32 main actions preserve their ABI 1003 IDs. Within a parameter choice, IDs identify the options of that particular phase. A row, a spell-slot rank and a combatant ID can therefore use the same number at different decision boundaries.

A creature target is numbered from the current decision maker's seat, including on a reaction or a consent prompt.

| Target ID | Creature |
|---:|---|
| 0 | Your duelist |
| 1 | The opposing duelist |
| 2 | Your companion |
| 3 | The opponent’s companion |

A missing or ineligible target leaves its slot masked. For Magic Missile, 0–3 assign one dart, and 16–19 assign all remaining darts to the same respective creature. A spell recipient menu uses 0–3 for a single creature, or `32 + bits` for a group, with bit `1 << target_id` for each creature. For example, selecting oneself and one’s companion uses 37. An empty set uses 32 wherever that choice is legal. Object pairs use the same own-issuer/opposing-issuer order, even when an object is carried by the other side. Turn owner, winner and the actor returned by the batch remain physical seat IDs. Coordinates are measured on the same board for both players.

A shared policy computes 128 scores for every choice. Class, level, resources and phase are inputs. Rust masks unavailable actions before selection. Scores at the same index serve different purposes across phases: for example, an ID representing a movement direction on the main turn can designate a creature during targeting. The terminal shows 1-based ordinals for the currently displayed options. [Policy](../../python/arena/policy.py) · [Public Rust decision IDs](../../src/arena2/target_ids.rs)

| Float indices (inclusive) | Information |
|---|---|
| 0–31 | Both public class identities, observable health, known distance, sight, turn and actor movement |
| 32–83 | Each combatant's public build and observable condition; private resources and reaction availability for the viewer |
| 84–107 | Original class-feature channels; the viewer gets their own feature details |
| 108–115 | Private roster subclass state. The class and subclass determine each channel's interpretation. |
| 116–123 | Subclass decision phase, one-hot remainder `(phase - 140) % 8` |
| 124–131 | Subclass decision phase, one-hot integer quotient of `(phase - 140) / 8` |
| 132–159 | Layout, legacy phase flags, own turn budget, pending hit or aim, known displacement |
| 160–223 | Eight relative movement choices, or legal area previews at the current coordinate step. The ninth column uses 146–153 |
| 224–255 | First 32 legal bits for the prior network input adapter |
| 256–329 | Decision phase, one-hot, IDs 0–73 |
| 330–369 | Location availability, altitude, prone/grappled, equipment and object interaction |
| 370–432 | Public 9 × 7 terrain, in row order |
| 433–496 | Summoned creatures in viewer-relative order, including limited contacts from Divine Sense |
| 497–520 | Spell and metamagic choice context |
| 521–544 | Common action, hidden status, ready declaration, current maneuver and pending checks |
| 545–551 | Per-projectile selection, remaining count, prior target, bounce origin and current dart reaction |
| 552–567 | Martial choice context, including declared on-hit options and Hunter's Lore |
| 568–575 | Traversal and fall context |
| 576–583 | Armor weight, worn state and don/doff progress for observable fighters |
| 584–596 | Component supplies, weapon identity, aimed dagger cell and Dwarf senses |
| 597–598 | Mending target and progress |
| 599 | Additional layout ID offset, 1 for the dark bastion |
| 600–615 | Visible dropped weapons and items the viewer took from the other fighter |
| 616–631 | Additional phase IDs 74–89 |
| 632–639 | Finesse, Light and improvised weapon intent |
| 640–767 | All 128 legal action bits, also provided as a separate boolean mask |
| 768–815 | Visible dropped objects, including location, HP, AC and burning status |
| 816–863 | Persistent magic effects. A visible fixed field projects its geometry even if its caster is out of sight. |
| 864–895 | Additional phase IDs 90–121, including area choices, weapon maneuvers, reverting or adopting a form and reacting to a failed save |
| 896–907 | Observable worn equipment, shield and armor training, private transferable inventory |
| 908–915 | Pact rite progress, visible companion temporary HP, and private temporary HP source for members of the viewer’s side |
| 916–919 | Own variant 1 and 2 flags, then opposing variant 1 and 2 flags. A canonical variant leaves both of its flags clear. |
| 920–951 | Additional visible ground items, eight values per item |
| 952–975 | Occupied footprint, creature size, squeezing, Starry Wisp, Guiding Bolt and Ray of Frost for each observable creature |
| 976–1015 | Identified ongoing effects, one flag per identity |
| 1016–1023 | Private response context for Bardic Inspiration, Portent, Restore Balance, Psychic Spells and Protective Field |

A combatant can know an opponent's location without seeing them. Audible movement and Stonecunning can establish a location. A successful Hide removes the opponent's cell until a rule locates them. The encoding withholds delta, distance, path, and area-hit predictions for an unlocated opponent. The actor may still target an eligible point and commit the attack. Outcome resolution uses the real state after the choice. Terminal snapshots omit the opponent's private spell slots, reaction budget and class-resource counts.

The native result reason distinguishes a defeat, double knockout, turn limit and decision limit. Terminal states carry no legal actions. [Native projection](../../src/arena2/bridge.rs) · [Gym and fixed-seat opponent interface](../../python/arena/gym_env.py)

The roster contains 36 level-five builds in class order. A build ID is `3 * class_id + variant`. Native `Batch.reset_builds` loads pairs of these IDs and accepts an optional layout ID for each fixture. Gym uses the roster through `DndArena-v0`. Historical class-and-level fixtures use `Batch.reset` or `DndArenaLegacy-v0`. [Python native interface](../../python/arena/native.py)
