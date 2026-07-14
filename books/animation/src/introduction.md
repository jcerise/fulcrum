# Introduction

This is the first of the Fulcrum **deep dives** — sub-books that take one system from The
Fulcrum Book (`book/` in the repository) and cover it completely: a code-along tutorial
first, then the full reference. The main book gives animation one chapter; this book gives
it everything the system can do, and everything it deliberately won't.

The deep dives exist because of a conviction about game frameworks: what most of them are
missing isn't features — it's **clear examples and good tutorials**. Every claim in this book
is backed by code that compiles in CI, every tutorial chapter ends with something you can run,
and the finished game ships in the repository for you to diff against.

## Who this is for

You've either read From Zero (the main book's beginner track), read the Grove chapters, or
built games before. This book assumes you know
what a tick is, what the ECS is, and why Fulcrum separates the simulation clock from the
render clock. If "the sim runs at 60 Hz and presentation reads state" doesn't ring a bell,
start with the main book — this one moves faster.

## The one idea this whole book unpacks

Most engines treat animation as a rendering concern: something the drawing code does with
wall-clock time, somewhere after your gameplay ran. Fulcrum takes the opposite position, and
every design in this book flows from it:

> **Animation is simulation state.** Clips advance on the fixed 60 Hz clock, durations are
> measured in ticks, and the current frame of any animation is ordinary game state — readable
> by gameplay systems, identical on every machine, and assertable in a headless test.

The payoff is a sentence most frameworks can't say: *"the sword connects on the exact tick
the attack animation shows its extension frame"* is, in Fulcrum, a testable fact. The
tutorial builds a game around exactly that sentence.

## What you'll build

The **Dojo**: a hero, a training dummy, and a sword. You'll move with WASD, swing with Space,
and learn to stand clear afterward — the dummy wobbles back and bonks you if you're still in
its arc. Small, but every mechanic in it is keyed to animation frames, and by the final
chapter a headless test suite proves the whole fight is deterministic.

The finished game ships at `games/dojo` (`cargo run -p dojo`), and each tutorial chapter has
a runnable checkpoint. Like every Fulcrum tutorial, this is a **code-along**: you'll create
your own crate and type every line. The questions that occur to you mid-keystroke are the
actual curriculum.

## How the book is organized

- **The Tutorial** (five chapters) builds the Dojo from an empty crate: play a clip, switch
  clips by hand until it hurts, replace the pain with a data-driven state machine, key
  gameplay to frames, and prove it all headless.
- **The Reference** (four chapters) is the complete system, organized for lookup: clips and
  players, the Aseprite pipeline, the state-machine file format and its exact evaluation
  semantics, and a recipe collection for the patterns real games need.

Read the tutorial in order; raid the reference whenever.
