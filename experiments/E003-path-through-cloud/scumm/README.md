# scumm: E003's machinery in Rust

Decision D7 moves E003's heavy machinery from Python to Rust, in layers. The Python modules stay the
reference. A layer replaces them only once it reproduces their output byte for byte.

| crate | ports | state |
|---|---|---|
| `ir` (scumm-ir) | ir.py, facts.py's key scheme, everything model.Game computes from the game files alone | done: 0 mismatches |
| `machine` (scumm-machine) | model.py's Machine, row by row | rows 0-2 done: 0 differences; rows 3-5 next (machine/PORT-NOTES.md) |
| `reader` | reader.py | after |

A later crate depends on `ir` with `scumm-ir.workspace = true`.

## Run the comparison

From this folder:

    cargo test --release

`ir/tests/oracle.rs` does three things:

1. It runs `../dump_ir.py` with `ROOT/.venv/bin/python` if the Python dumps in `ROOT/out/scumm-ir/` are
   missing or stale. Stale means a source changed since the dump: ir.py, facts.py, model.py,
   dump_ir.py or the two fixtures. The hashes are in `ir-python.sources.json`. The run takes about 20 s.
2. It builds the same lines in Rust.
3. It compares the text byte for byte. A line that differs is reported by its unit and field paths.

Three dumps are compared:

- **full**: tables, Game fields, every unit with its analyses, and facts.py over `ir/tests/fact_states.json`;
- **noblocks**: the units read without the raw blocks, so the text rules are checked too;
- **synthetic**: `ir/tests/synthetic.json`, covering malformed statements, every note class, every end and
  yield rule, and every loop kind, in a Game built by hand.

`ir/tests/facts.rs` checks that the 4,178 fact keys in `results/` parse and print back unchanged.

To compare by hand:

    ../../../.venv/bin/python ../dump_ir.py      # writes ir-python*.jsonl.gz
    cargo run --release --bin dump_ir            # writes ir-rust*.jsonl.gz
    cd ../../../out/scumm-ir
    for v in "" -noblocks -synthetic; do cmp <(zcat ir-python$v.jsonl.gz) <(zcat ir-rust$v.jsonl.gz) && echo same $v; done

`dump_ir.py`'s docstring defines the JSON format. The repository root is found from this folder;
`FAST_MI_ROOT` overrides it.

## Speed

    cargo run --release --bin bench_load

Measured on the laptop on 2026-10-24, with other jobs running:

| load the whole game and every static analysis | time | peak RSS |
|---|---|---|
| Python: Game.load, then every loop, wait point and time exit | 15.2 s (3.3 s + 11.9 s) | 71 MB |
| Rust: Game::load (both const_polls settings) | 0.18 s | 22 MB |

The machine's comparison runs through `machine-run` and `golden.py compare`; machine/PORT-NOTES.md says how.

## What the machine port builds on

- `Game` (derefs to `Script`) owns every unit. It is immutable and computes all analyses in `Game::new`.
  So add every unit before building it. test_model.py adds units to a Game after it exists; Python
  allows that because it computes analyses lazily, but Rust does not.
- `Script::add_unit` is ir.py's `_read` (and `_entries`) on text. `Game::new(script, GameData)` is
  model.py's `Game(...)` constructor. Tests build units by hand with these two.
- Instructions are `game.ins(unit)[pc]`. A statement is a `Copy` `Stmt`. Expressions are `ExprId`s into
  `game.ast`. Names are `Sym`s, and the op and getter names the machine dispatches on are constants in
  `known`.
- Facts are `Fact`, a 16-byte `Copy` key that prints as facts.py's string. A fact map is `FactMap`.
- Per-point tables are indexed by `Script::point(unit, idx)`: wait points, time loops and join points.
