# AGENTS.md

Guidance for agents working in this repository.

## Comments

Terse, and only what the code cannot say.

- Explain *why*, never *what*. If a name or a line already says it, delete the
  comment.
- One or two lines. A paragraph is a signal the code needs a better name or a
  smaller function.
- Doc comments (`///`) on public items: one sentence, present tense.
- ASCII only. No box-drawing rules, no em dashes.
- No references to the C++ original. State the reason directly instead of
  pointing at another tree: "Depth32Float, because SSAO samples depth and a
  depth-stencil format is not sampleable on wgpu", not "the C++ uses D24S8".
- Tags, so an editor and `rg` can find them:
  - `TODO:` work not done
  - `FIXME:` known broken
  - `SAFETY:` why the `unsafe` block is sound
  - `NOTE:` a constraint that is not obvious from the code

## Commits

One logical change per commit. Conventional prefix (`renderer:`, `adam:`,
`ci:`, `license:`), then a what-and-why subject, then a short body.
