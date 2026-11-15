# How the paper is written (rules for Claude)

Ilyas writes every word of the paper: title, abstract, body, captions, tables' text. Claude does not write or rewrite
any sentence of it, not even as a "suggested wording" or "for example". When a passage is weak, Claude says what is
wrong and Ilyas rewrites it. If Ilyas asks how, Claude explains the principle in general terms, not by rewriting
the passage.

## What Claude does

1. **Outline.** Proposes the structure and, for each section, the points it must establish and the evidence for
   each. Ilyas decides and words them.
2. **Facts.** Every number and factual claim in the draft is checked against the lab notes
   (`notes/log/`, the `direct` branch), the results folders and `paper/claims.md`. Each check says one of:
   supported (with the source), not supported by anything recorded, or wrong (the right value and its source).
   Claude keeps the evidence column of `claims.md` up to date; the claim's wording is Ilyas's.
3. **Language.** Review comments quote the phrase and name the problem, with one tag each:
   - `[FACT]` a number or claim that the sources do not support, or contradict
   - `[OVERCLAIM]` says more than the evidence or the related work allows (e.g. "first" without the hedge)
   - `[VAGUE]` the reader cannot tell what exactly is meant
   - `[JARGON]` a term the reader has not been given, or project-internal names (rung, leg, high8)
   - `[LONG]` a sentence or paragraph doing too many things
   - `[UNCLEAR]` who or what does the action is missing or ambiguous
   - `[FLOW]` the order of ideas, or a missing link between them
   - `[MISSING]` something a reviewer will expect here and not find
   No replacement text in any comment.
4. **Guidance.** The process, the venue and arXiv rules, what reviewers look for, when a section is ready.

## Files

- The paper source (Ilyas's choice of tool and file): only Ilyas edits it.
- `paper/claims.md`: every number or claim the paper makes, with its evidence and status.
- `paper/reviews/YYYY-MM-DD-<section>.md`: Claude's review of a draft section, in the tagged form above.
- `paper/outline.md`: the agreed structure, in Ilyas's words (the October outline predates E027 and is superseded
  once Ilyas writes the new one).

## The loop, per section

1. Agree on the section's points and their evidence (Claude proposes, Ilyas decides).
2. Ilyas drafts the section.
3. Ilyas asks for a review; Claude writes `paper/reviews/...` (facts first, then language).
4. Ilyas revises; repeat until no `[FACT]` or `[OVERCLAIM]` remains.

Suggested order: the system and the results first, then related work, then the introduction; the abstract and
the title last.
