# sv_maker

A command-line tool written in Rust for making precise structural variant modifications in FASTA sequences, including insertions, deletions, inversions, duplications, and copyback operations. Output is pipable. Designed with defective viral genome simulation in mind.

The binary is available at ./target/release/sv_maker

## Build

You may build the binary yourself or just use it directly. The binary is available at `target/release/sv_maker`.

```bash
cargo build --release
```

## Usage

```bash
sv_maker [--output|-o <file>] <input_file> <operation> <args...>
sv_maker [--output|-o <file>] - <operation> <args...>  # read from stdin
```
--output or -o: File path for the optional output .fa file
input_file: File path for the input .fa file (reference sequence to make changes to)


## File Output

By default, output goes to stdout. Use `--output` or `-o` to save to a file:

```bash
sv_maker -o modified.fa input.fa delete 10 20
sv_maker --output result.fa input.fa insert 25 TTTT
```

## Input Requirements

- Single-sequence FASTA files only
- Sequence must contain valid DNA bases (A, T, C, G, N)
- Positions are 1-based and must be within sequence bounds

## Output Format

- Standard FASTA format with 70-character lines
- Headers track all applied operations
- Examples:
  - `>sequence [deleted 5bp at positions 10-14]`
  - `>sequence [duplicated 11bp from positions 10-20 to position 50]`
  - `>sequence [tandem duplicated 11bp at positions 10-20]`
  - `>sequence [5' copyback up to position 50 then reverse complement of position 20 on]`
  - `>sequence [3' copyback (snapback) at position 50 of reference revcomp]`

## Operations

### Delete
Remove bases from a sequence using 1-based, inclusive coordinates.

```bash
sv_maker input.fa delete <start> <end>
```

Example:
```bash
sv_maker sequence.fa delete 10 20  # removes bases 10-20
```

### Insert
Insert a sequence at the specified position (1-based).

```bash
sv_maker input.fa insert <position> <sequence>
```

Example:
```bash
sv_maker sequence.fa insert 15 ATCG  # inserts ATCG at position 15
```

### Invert
Reverse a region of the sequence using 1-based, inclusive coordinates. Add `--complement` to perform reverse complement instead of just reversal.

```bash
sv_maker input.fa invert <start> <end>
sv_maker input.fa invert --complement <start> <end>
```

Examples:
```bash
sv_maker sequence.fa invert 25 35           # inverts bases 25-35
sv_maker sequence.fa invert --complement 25 35  # reverse complements bases 25-35
```

### Duplicate
Duplicate a segment of the sequence and insert it at another position or in tandem.

```bash
sv_maker input.fa duplicate <start> <end> <position>  # regular duplication
sv_maker input.fa duplicate -td <start> <end>         # tandem duplication
```

Examples:
```bash
sv_maker sequence.fa duplicate 10 20 50  # duplicates bases 10-20 and inserts at position 50
sv_maker sequence.fa duplicate -td 10 20 # creates tandem duplication of bases 10-20
```

### Copyback
See copyback or snapback defective viral genomes. Perform copyback operations that retain sequence beginning at one end of the genome up to a breakpoint. Then, a reverse complemented region that is part of the retained sequence is appended from a backstart position (forming a panhandle or hairpin structure). For 3' copybacks, the reference is reverse complemented first. Breakpoint and backstart are relative to the reverse complement for 3' copyback/snapbacks (breakpoint = 50 means all sequence from the start of the 3' end to 50 nucleotides away from it).

```bash
sv_maker input.fa copyback <gend> <breakpoint> <backstart>  # regular copyback
sv_maker input.fa copyback -sb <gend> <breakpoint>          # snapback (backstart = breakpoint)
```

Parameters:
- `gend`: Reference genome end. Either `5` (5' end) or `3` (3' end)
- `breakpoint`: Position where sequence is retained up to
- `backstart`: Start position for reverse complement segment (must be < breakpoint for both 5' and 3')

Examples:
```bash
# 5' copyback: keep positions 1-50, append reverse complement of positions 1-20
sv_maker sequence.fa copyback 5 50 20

# 5' snapback: keep positions 1-50, append reverse complement of positions 1-50
sv_maker sequence.fa copyback -sb 5 50

# 3' copyback: reverse complement genome first, keep positions 1-50, append revcomp of positions 1-20
sv_maker sequence.fa copyback 3 50 20

# 3' snapback: reverse complement genome first, keep positions 1-50, append revcomp of positions 1-50
sv_maker sequence.fa copyback -sb 3 50
```

## Chaining Operations

Operations can be chained using pipes. Use `-` as the input file to read from stdin:

```bash
sv_maker input.fa delete 5 10 | sv_maker - insert 20 GGGG
```

Multiple operations can be chained together:
```bash
sv_maker input.fa delete 1 5 | \
  sv_maker - insert 1 AAA | \
  sv_maker - duplicate -td 10 20 | \
  sv_maker - copyback 5 50 30
```
