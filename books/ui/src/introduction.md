# Introduction

This is a Fulcrum deep dive: one engine system, taken all the way down. The main book gives
the UI one chapter; this book gives it everything the system can do — and everything it
deliberately won't. Fulcrum's position is that most engines aren't missing features, they're
missing **clear examples and good tutorials**. So every claim in this book is backed by code
that compiles in CI, and the tutorial builds a real, shipped, tested game.

## Who this is for

You've read the main book, or at least: you can put a window on screen, you know a sim
system from a frame system, and "determinism" makes you nod rather than squint. If
`add_frame_system` doesn't ring a bell, start with the main book — this one leans on the
sim/frame split hard, because the UI is where that split earns its keep.

## The one idea this whole book unpacks

Most engines treat UI as a second program living inside your game — retained widget objects
you construct, mutate, and clean up, with their own event system, styling language, and
lifecycle. Fulcrum's UI is smaller and stranger:

> **The UI is a projection of the simulation.** The tree is data in a file. The styling is
> fields on that data. Every frame, your game re-drives every label and every visibility
> flag from sim state, and every click travels back as a replayable command. Nothing in the
> UI remembers anything.

The payoff is a sentence most frameworks can't say: *a whole day of shopkeeping — every
button press included — replays from a seed and a command log, and your menus can be tested
without a window.* Skins fall out for free: when styling is just data, restyling is just
writing different data.

## What you'll build

The Bazaar: a market stall where customers queue up wanting figs, silk, and lamps, and you
serve them by clicking the right ware before their patience runs out. The whole game is
played through its UI — HUD, shop counter, pause menu, day summary — and the entire thing
wears interchangeable skins: sun-bleached wood by day, lacquer and lamplight by night. The
finished game ships in this repository as `games/bazaar` (`cargo run -p bazaar`).

You'll build it as your own crate, and you'll type every line. The questions that occur to
you mid-keystroke are the actual curriculum.

## How the book is organized

- **The Tutorial** (5 chapters): put a panel on screen and learn the layout model; build the
  full readout HUD and drive it from a sim; open the shop — buttons, the pause menu, and the
  click path across the sim boundary; then dress everything in nine-slice skins and a
  swappable theme; finally, prove the whole thing headless, pointer and all.
- **The Reference**: exact semantics for nodes and layout, widgets and interaction, the
  `.ui.ron` file format, and a recipes chapter for the patterns the tutorial didn't need.

Read the tutorial in order; raid the reference whenever.
