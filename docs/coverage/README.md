# Coverage audit

The master catalog records 121 level 1–5 class and subclass grants, 30 individual always-prepared spell grants, 49 shared combat and construction rules, and 12 creation presets. A named grant can contain several decisions with different triggers, costs and targets. The domain matrices record these decisions separately. A test for one branch does not verify the parent grant.

The frozen mechanics revision `70408a3` passes the source-and-test audit across 550 rows in 18 matrices. Of those, 483 have executed test evidence, 37 record fixed character choices, and 30 describe an unreachable trigger with its reason. The 200 master entries include their linked domain records. [Master catalog](catalog.tsv) · [Per-level build choices](creation-ledger.md) · [Master-to-mechanic links](../../coverage/links.tsv)

From the repository root, run the native and Python suites first, then verify every cited test against that run.

```sh
make check > /tmp/duel-check.log 2>&1 &&
  python3 coverage/check_creation.py &&
  python3 coverage/check_matrix.py --strict --test-log /tmp/duel-check.log &&
  python3 coverage/audit_progress.py --strict --test-log /tmp/duel-check.log
```

The audited revision passed 392 Rust and 25 Python tests. An additional deterministic run played 45,600 bouts using only uniformly sampled legal choices at every decision, including off-turn choices and mixed-level opponents. It checked termination, labels, rejected actions after termination, and that each nonterminal mask offered a move. Reproduce it with the same Cargo toolchain used by Make.

```sh
DUEL_AUDIT_REPETITIONS=380 cargo test --locked --offline --release --lib legal_choices_always
```

The terminal/Gym parity tests feed the same choices from each seat into scripted scenarios and compare the actual actor, mask, observation and outcome. The native hidden-information tests also vary the unobserved fighter's true position independently of the visible position and objects. [Terminal and Gym scenarios](../../python/tests/test_arena_terminal.py) · [Native visibility tests](../../src/arena2/bridge.rs) · [Legal-choice stress test](../../src/arena2/interface_tests.rs)

The creation check verifies cantrip and prepared spell counts on all 40 spellcasting class/level profiles. It separates choices promoted into an always-prepared grant and does not stand in for checking individual spell effects.

A verified implementation row needs a repository implementation reference and at least one discoverable executable test with the format `src/arena2/example.rs#test_name`. Without an execution log, the checker verifies discoverable test declarations. Pass a completed Cargo/unittest log with --test-log to require that each verified reference actually passed. The progress report uses the same option to identify failing or unobserved test evidence. To claim rule coverage, review what the linked assertions exercise, including unavailable levels, optional alternatives and resource spending.

Use `pending`, `partial`, `missing` or `unverified` to retain open items. Use `creation_choice` for fixed build choices with a concrete selection and a repository reference. Use `not_applicable` when the trigger is impossible under the documented arena contract and explain the reason in the choices column. The strict commands fail while any coverage row or master dependency remains unresolved. The master dependency map names the responsible domain, shows open mechanics separately from implemented rows awaiting evidence, and identifies missing or invalid evidence anchors. Reviewable means the child rows are closed and the master still needs an independent scope check. The counts refer to audit rows, whose breadth varies.

## Auditing changes to the fixed builds

- Track every learned or prepared cantrip and spell at each level. Record additions, replacements, legal class lists and quota, with separate columns or rows for fixed grants that do not count towards the quota. Wizard additionally needs a legal spellbook and prepared subset. Evocation Savant grants spells to the book at levels 3 and 5.
- Record the Warlock invocation progression at levels 1, 2 and 5. The legal total is 1, 3 and 5 respectively. Tie Agonizing Blast and Repelling Blast to the selected cantrip and show that each choice meets its level prerequisite.
- Record four known Wild Shape forms when the Druid gains the feature at level 2. Respect the maximum challenge rating and fly restriction. Starting forms and tactical form selection need separate evidence.
- Choose a permanent starting style or order per preset and list its alternatives. A recurring choice such as Circle of the Land terrain or Hunter's Prey after a rest is fixed for an arena start with no intervening rest. A per-trigger choice such as Open Hand Technique is offered during combat.
- Record class and background skill, tool and language selections with their required choice counts. Identify which effects the arena can actually trigger. Equipment provenance should distinguish legal starting gear, option-B purchases and supplied arena equipment, including focus, required component, draw/stow and occupied-hand rules.
- Record level-specific weapon mastery choices and test each selected mastery on its actual weapon. Weapon access, proficiency and the combat decision to draw or throw a weapon are separate checks.
- Document the arena boundaries for social actions, light, terrain, spectators, rests and incapacitation. A source rule can be inapplicable only if its trigger stays unavailable after the relevant domain upgrades.

The 12 fixed presets and initial ability choices come from the existing arena catalog. Rule identification uses the official class and combat chapters. For mandatory spells use the parent subclass table before applying the prepared-spell rules. [Arena rules](../arena-rules.md) · [SRD 5.2](https://media.dndbeyond.com/compendium-images/srd/5.2/SRD_CC_v5.2.pdf)
