# Agent protocol: head study

## Goal

Match `brief.md` and the frozen ranges in `spec.json` by editing `world.txt`. The evaluated mesh is the source of measurements. A passing audit means only that measured ranges pass; it is not a claim of anatomical realism or a complete game character.

## Frozen inputs

Do not edit `brief.md`, `spec.json`, `audit.ts`, `shots.json`, or `poses.json` while iterating on the model. Changes to those files need a separately reviewed spec revision and a new baseline.

## Loop

1. Read the brief, spec, current `world.txt`, latest audit JSON, and visual review.
2. Choose the highest priority failed rule or visual defect. State one hypothesis and the smallest source parameters expected to fix it.
3. Edit only `world.txt`; make one bounded change.
4. Run `bash web/tests/scenes/characters/head_study/run-audit.sh` from `engine/`.
5. If the audit fails, use its measured value, range, and hint. Do not hide, delete, or weaken the rule. If it passes, check every previously passing rule for regression.
6. At a geometry/material milestone, run `bun web/tests/scenes/characters/head_study/capture.mjs` and compare the same named views. Numeric rules do not replace visual review.
7. Stop after 12 iterations or when the measured rules pass and remaining visual issues are clearly reported. If the same issue persists, diagnose a missing representation/tool instead of adding arbitrary offsets.

## Report

Return:

- the exact changed parameters and reason;
- audit pass/fail summary and `target/reports/character-head/audit.json` path;
- remaining unmeasured rules;
- fixed-view image paths and honest visual findings;
- the single next change most likely to improve the result.

Never report a pass without running the audit after the last edit. Never say this head study is a finished, rigged, or game-ready character.
