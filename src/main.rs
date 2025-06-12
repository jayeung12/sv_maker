use std::fs::File;
use std::io::{self, BufRead, BufReader, Write, stdin};
use std::env;

#[derive(Debug)]
enum Operation {
    Delete { start: usize, end: usize },
    Insert { position: usize, sequence: String },
    Invert { start: usize, end: usize, complement: bool },
    Duplicate { start: usize, end: usize, position: usize },
    TandemDuplicate { start: usize, end: usize },
    Copyback { gend: u8, breakpoint: usize, backstart: usize },
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 3 {
        print_usage(&args[0]);
        std::process::exit(1);
    }

    // Check for --output or -o flag
    let (output_file, remaining_args) = parse_output_option(&args[1..]);
    
    if remaining_args.len() < 2 {
        print_usage(&args[0]);
        std::process::exit(1);
    }

    let input_file = &remaining_args[0];
    let operation = parse_operation(&remaining_args[1..]);
    
    let operation = match operation {
        Ok(op) => op,
        Err(msg) => {
            eprintln!("Error: {}", msg);
            print_usage(&args[0]);
            std::process::exit(1);
        }
    };

    let (header, sequence) = if input_file == "-" {
        read_fasta_from_stdin()?
    } else {
        read_fasta(input_file)?
    };
    let (new_header, new_sequence) = apply_operation(&header, &sequence, operation);
    
    if let Some(output_path) = output_file {
        write_fasta_to_file(&new_header, &new_sequence, &output_path)?;
    } else {
        write_fasta_to_stdout(&new_header, &new_sequence)?;
    }
    
    Ok(())
}

fn print_usage(program_name: &str) {
    eprintln!("Usage:");
    eprintln!("  {} [--output|-o <file>] <input_file> delete <start> <end>", program_name);
    eprintln!("  {} [--output|-o <file>] <input_file> insert <position> <sequence>", program_name);
    eprintln!("  {} [--output|-o <file>] <input_file> invert [--complement] <start> <end>", program_name);
    eprintln!("  {} [--output|-o <file>] <input_file> duplicate <start> <end> <position>", program_name);
    eprintln!("  {} [--output|-o <file>] <input_file> duplicate -td <start> <end>", program_name);
    eprintln!("  {} [--output|-o <file>] <input_file> copyback <gend> <breakpoint> <backstart>", program_name);
    eprintln!("  {} [--output|-o <file>] <input_file> copyback -sb <gend> <breakpoint>", program_name);
    eprintln!("  {} [--output|-o <file>] - <operation> <args...> - Read from stdin", program_name);
    eprintln!("");
    eprintln!("Examples:");
    eprintln!("  {} input.fa delete 10 20                     # Delete bases 10-20", program_name);
    eprintln!("  {} input.fa insert 15 ATCG                   # Insert ATCG at position 15", program_name);
    eprintln!("  {} input.fa invert 25 35                     # Invert bases 25-35", program_name);
    eprintln!("  {} input.fa invert --complement 25 35        # Reverse complement bases 25-35", program_name);
    eprintln!("  {} input.fa duplicate 10 20 50               # Duplicate bases 10-20 to position 50", program_name);
    eprintln!("  {} input.fa duplicate -td 10 20              # Tandem duplicate bases 10-20", program_name);
    eprintln!("  {} input.fa copyback 5 50 20                 # 5' copyback: keep up to pos 50, append revcomp of pos 1-20", program_name);
    eprintln!("  {} input.fa copyback 3 50 80                 # 3' copyback: revcomp genome, keep up to pos 50, append revcomp of pos 1-80", program_name);
    eprintln!("  {} input.fa copyback -sb 5 50                # 5' snapback: keep up to pos 50, append revcomp of pos 1-50", program_name);
    eprintln!("  {} -o output.fa input.fa delete 5 10         # Save result to file", program_name);
    eprintln!("  {} input.fa delete 5 10 | {} - insert 20 GGGG  # Chain operations", program_name, program_name);
    eprintln!("");
    eprintln!("gend: 5 (5' end) or 3 (3' end)");
    eprintln!("For both 5' and 3' end: backstart < breakpoint");
    eprintln!("Without --output, result is written to stdout for piping.");
}

fn parse_output_option(args: &[String]) -> (Option<String>, Vec<String>) {
    let mut output_file = None;
    let mut remaining_args = Vec::new();
    let mut i = 0;
    
    while i < args.len() {
        match args[i].as_str() {
            "--output" | "-o" => {
                if i + 1 < args.len() {
                    output_file = Some(args[i + 1].clone());
                    i += 2; // Skip both the flag and the filename
                } else {
                    eprintln!("Error: --output requires a filename");
                    std::process::exit(1);
                }
            },
            _ => {
                remaining_args.push(args[i].clone());
                i += 1;
            }
        }
    }
    
    (output_file, remaining_args)
}

fn complement_base(base: char) -> char {
    match base.to_ascii_uppercase() {
        'A' => 'T',
        'T' => 'A',
        'C' => 'G',
        'G' => 'C',
        'N' => 'N',
        _ => base, // Keep any other characters as-is
    }
}

fn parse_operation(args: &[String]) -> Result<Operation, String> {
    if args.is_empty() {
        return Err("No operation specified".to_string());
    }
    
    match args[0].as_str() {
        "delete" => {
            if args.len() != 3 {
                return Err("Delete operation requires start and end positions".to_string());
            }
            let start: usize = args[1].parse().map_err(|_| "Start position must be a number")?;
            let end: usize = args[2].parse().map_err(|_| "End position must be a number")?;
            
            if start == 0 || end == 0 {
                return Err("Positions must be 1-based (starting from 1)".to_string());
            }
            if start > end {
                return Err("Start position must be <= end position".to_string());
            }
            
            Ok(Operation::Delete { start, end })
        },
        "insert" => {
            if args.len() != 3 {
                return Err("Insert operation requires position and sequence".to_string());
            }
            let position: usize = args[1].parse().map_err(|_| "Position must be a number")?;
            let sequence = args[2].clone();
            
            if position == 0 {
                return Err("Position must be 1-based (starting from 1)".to_string());
            }
            
            // Validate sequence contains only valid DNA bases
            if !sequence.chars().all(|c| matches!(c.to_ascii_uppercase(), 'A' | 'T' | 'C' | 'G' | 'N')) {
                return Err("Sequence must contain only valid DNA bases (A, T, C, G, N)".to_string());
            }
            
            Ok(Operation::Insert { position, sequence: sequence.to_uppercase() })
        },
        "invert" => {
            let mut complement = false;
            let mut pos_args = Vec::new();
            
            // Parse arguments, looking for --complement flag
            for arg in &args[1..] {
                if arg == "--complement" {
                    complement = true;
                } else {
                    pos_args.push(arg);
                }
            }
            
            if pos_args.len() != 2 {
                return Err("Invert operation requires start and end positions".to_string());
            }
            
            let start: usize = pos_args[0].parse().map_err(|_| "Start position must be a number")?;
            let end: usize = pos_args[1].parse().map_err(|_| "End position must be a number")?;
            
            if start == 0 || end == 0 {
                return Err("Positions must be 1-based (starting from 1)".to_string());
            }
            if start > end {
                return Err("Start position must be <= end position".to_string());
            }
            
            Ok(Operation::Invert { start, end, complement })
        },
        "duplicate" => {
            let mut tandem = false;
            let mut pos_args = Vec::new();
            
            // Parse arguments, looking for -td flag
            for arg in &args[1..] {
                if arg == "-td" {
                    tandem = true;
                } else {
                    pos_args.push(arg);
                }
            }
            
            if tandem {
                // Tandem duplication: duplicate <start> <end>
                if pos_args.len() != 2 {
                    return Err("Tandem duplicate operation requires start and end positions".to_string());
                }
                
                let start: usize = pos_args[0].parse().map_err(|_| "Start position must be a number")?;
                let end: usize = pos_args[1].parse().map_err(|_| "End position must be a number")?;
                
                if start == 0 || end == 0 {
                    return Err("Positions must be 1-based (starting from 1)".to_string());
                }
                if start > end {
                    return Err("Start position must be <= end position".to_string());
                }
                
                Ok(Operation::TandemDuplicate { start, end })
            } else {
                // Regular duplication: duplicate <start> <end> <position>
                if pos_args.len() != 3 {
                    return Err("Duplicate operation requires start, end, and insert positions".to_string());
                }
                
                let start: usize = pos_args[0].parse().map_err(|_| "Start position must be a number")?;
                let end: usize = pos_args[1].parse().map_err(|_| "End position must be a number")?;
                let position: usize = pos_args[2].parse().map_err(|_| "Insert position must be a number")?;
                
                if start == 0 || end == 0 || position == 0 {
                    return Err("Positions must be 1-based (starting from 1)".to_string());
                }
                if start > end {
                    return Err("Start position must be <= end position".to_string());
                }
                
                Ok(Operation::Duplicate { start, end, position })
            }
        },
        "copyback" => {
            let mut snapback = false;
            let mut pos_args = Vec::new();
            
            // Parse arguments, looking for -sb flag
            for arg in &args[1..] {
                if arg == "-sb" {
                    snapback = true;
                } else {
                    pos_args.push(arg);
                }
            }
            
            if snapback {
                // Snapback mode: copyback <gend> <breakpoint>
                if pos_args.len() != 2 {
                    return Err("Copyback with -sb flag requires gend and breakpoint".to_string());
                }
                
                let gend_str = &pos_args[0];
                let gend: u8 = match gend_str.as_str() {
                    "5" => 5,
                    "3" => 3,
                    _ => return Err("gend must be either 5 or 3".to_string())
                };
                
                let breakpoint: usize = pos_args[1].parse().map_err(|_| "Breakpoint must be a number")?;
                
                if breakpoint == 0 {
                    return Err("Breakpoint must be 1-based (starting from 1)".to_string());
                }
                
                Ok(Operation::Copyback { gend, breakpoint, backstart: breakpoint })
            } else {
                // Regular copyback: copyback <gend> <breakpoint> <backstart>
                if pos_args.len() != 3 {
                    return Err("Copyback operation requires gend, breakpoint, and backstart".to_string());
                }
                
                let gend_str = &pos_args[0];
                let gend: u8 = match gend_str.as_str() {
                    "5" => 5,
                    "3" => 3,
                    _ => return Err("gend must be either 5 or 3".to_string())
                };
                
                let breakpoint: usize = pos_args[1].parse().map_err(|_| "Breakpoint must be a number")?;
                let backstart: usize = pos_args[2].parse().map_err(|_| "Backstart must be a number")?;
                
                if breakpoint == 0 || backstart == 0 {
                    return Err("Positions must be 1-based (starting from 1)".to_string());
                }
                
                // Validate backstart position relative to breakpoint based on gend
                if gend == 5 && backstart >= breakpoint {
                    return Err("For 5' end, backstart must be less than breakpoint".to_string());
                }
                if gend == 3 && backstart >= breakpoint {
                    return Err("For 3' end, backstart must be less than breakpoint".to_string());
                }
                
                Ok(Operation::Copyback { gend, breakpoint, backstart })
            }
        },
        _ => Err(format!("Unknown operation '{}'. Use 'delete', 'insert', 'invert', 'duplicate', or 'copyback'", args[0]))
    }
}

fn read_fasta(filename: &str) -> std::io::Result<(String, String)> {
    let file = File::open(filename)?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    
    // Read header
    let header = match lines.next() {
        Some(line) => {
            let line = line?;
            if !line.starts_with('>') {
                eprintln!("Error: File does not appear to be a valid FASTA file (no header starting with '>')");
                std::process::exit(1);
            }
            line
        },
        None => {
            eprintln!("Error: File is empty");
            std::process::exit(1);
        }
    };
    
    // Read sequence
    let mut sequence = String::new();
    let mut sequence_count = 0;
    
    for line in lines {
        let line = line?;
        if line.starts_with('>') {
            sequence_count += 1;
            if sequence_count > 0 {
                eprintln!("Error: FASTA file contains multiple sequences. Only single-sequence files are supported.");
                std::process::exit(1);
            }
        } else {
            // Remove whitespace and convert to uppercase
            sequence.push_str(&line.trim().to_uppercase());
        }
    }
    
    if sequence.is_empty() {
        eprintln!("Error: No sequence found in FASTA file");
        std::process::exit(1);
    }
    
    Ok((header, sequence))
}

fn read_fasta_from_stdin() -> std::io::Result<(String, String)> {
    let stdin = stdin();
    let reader = stdin.lock();
    let mut lines = reader.lines();
    
    // Read header
    let header = match lines.next() {
        Some(line) => {
            let line = line?;
            if !line.starts_with('>') {
                eprintln!("Error: Input does not appear to be a valid FASTA file (no header starting with '>')");
                std::process::exit(1);
            }
            line
        },
        None => {
            eprintln!("Error: No input provided");
            std::process::exit(1);
        }
    };
    
    // Read sequence
    let mut sequence = String::new();
    let mut sequence_count = 0;
    
    for line in lines {
        let line = line?;
        if line.starts_with('>') {
            sequence_count += 1;
            if sequence_count > 0 {
                eprintln!("Error: Input contains multiple sequences. Only single-sequence files are supported.");
                std::process::exit(1);
            }
        } else {
            // Remove whitespace and convert to uppercase
            sequence.push_str(&line.trim().to_uppercase());
        }
    }
    
    if sequence.is_empty() {
        eprintln!("Error: No sequence found in input");
        std::process::exit(1);
    }
    
    Ok((header, sequence))
}

fn apply_operation(header: &str, sequence: &str, operation: Operation) -> (String, String) {
    match operation {
        Operation::Delete { start, end } => {
            // Convert to 0-based indexing
            let start_idx = start - 1;
            let end_idx = end; // end is inclusive in 1-based, so end_idx is exclusive in 0-based
            
            if end_idx > sequence.len() {
                eprintln!("Error: End position {} is beyond sequence length {}", end, sequence.len());
                std::process::exit(1);
            }
            
            let new_sequence = format!("{}{}", &sequence[..start_idx], &sequence[end_idx..]);
            let deleted_length = end_idx - start_idx;
            let new_header = format!("{} [deleted {}bp at positions {}-{}]", header, deleted_length, start, end);
            
            (new_header, new_sequence)
        },
        Operation::Insert { position, sequence: insert_seq } => {
            // Convert to 0-based indexing
            let insert_idx = position - 1;
            
            if insert_idx > sequence.len() {
                eprintln!("Error: Insert position {} is beyond sequence length {}", position, sequence.len());
                std::process::exit(1);
            }
            
            let new_sequence = format!("{}{}{}", &sequence[..insert_idx], &insert_seq, &sequence[insert_idx..]);
            let new_header = format!("{} [inserted {}bp '{}' at position {}]", header, insert_seq.len(), insert_seq, position);
            
            (new_header, new_sequence)
        },
        Operation::Invert { start, end, complement } => {
            // Convert to 0-based indexing
            let start_idx = start - 1;
            let end_idx = end; // end is inclusive in 1-based, so end_idx is exclusive in 0-based
            
            if end_idx > sequence.len() {
                eprintln!("Error: End position {} is beyond sequence length {}", end, sequence.len());
                std::process::exit(1);
            }
            
            // Extract the region to invert
            let before = &sequence[..start_idx];
            let to_invert = &sequence[start_idx..end_idx];
            let after = &sequence[end_idx..];
            
            // Process the region based on complement flag
            let processed: String = if complement {
                // Reverse complement: reverse and complement each base
                to_invert.chars().rev().map(complement_base).collect()
            } else {
                // Just reverse
                to_invert.chars().rev().collect()
            };
            
            let new_sequence = format!("{}{}{}", before, processed, after);
            let inverted_length = end_idx - start_idx;
            let operation_desc = if complement {
                "reverse complemented"
            } else {
                "inverted"
            };
            let new_header = format!("{} [{} {}bp at positions {}-{}]", header, operation_desc, inverted_length, start, end);
            
            (new_header, new_sequence)
        },
        Operation::Duplicate { start, end, position } => {
            // Convert to 0-based indexing
            let start_idx = start - 1;
            let end_idx = end; // end is inclusive in 1-based, so end_idx is exclusive in 0-based
            let insert_idx = position - 1;
            
            if end_idx > sequence.len() {
                eprintln!("Error: End position {} is beyond sequence length {}", end, sequence.len());
                std::process::exit(1);
            }
            
            if insert_idx > sequence.len() {
                eprintln!("Error: Insert position {} is beyond sequence length {}", position, sequence.len());
                std::process::exit(1);
            }
            
            // Extract the segment to duplicate
            let segment = &sequence[start_idx..end_idx];
            
            // Insert the duplicated segment at the specified position
            let new_sequence = format!("{}{}{}", &sequence[..insert_idx], segment, &sequence[insert_idx..]);
            let duplicated_length = end_idx - start_idx;
            let new_header = format!("{} [duplicated {}bp from positions {}-{} to position {}]", header, duplicated_length, start, end, position);
            
            (new_header, new_sequence)
        },
        Operation::TandemDuplicate { start, end } => {
            // Convert to 0-based indexing
            let start_idx = start - 1;
            let end_idx = end; // end is inclusive in 1-based, so end_idx is exclusive in 0-based
            
            if end_idx > sequence.len() {
                eprintln!("Error: End position {} is beyond sequence length {}", end, sequence.len());
                std::process::exit(1);
            }
            
            // Extract the segment to duplicate
            let segment = &sequence[start_idx..end_idx];
            
            // Insert the duplicated segment directly after the original segment
            let new_sequence = format!("{}{}{}{}", &sequence[..start_idx], segment, segment, &sequence[end_idx..]);
            let duplicated_length = end_idx - start_idx;
            let new_header = format!("{} [tandem duplicated {}bp at positions {}-{}]", header, duplicated_length, start, end);
            
            (new_header, new_sequence)
        },
        Operation::Copyback { gend, breakpoint, backstart } => {
            // Convert to 0-based indexing
            let breakpoint_idx = breakpoint - 1;
            let backstart_idx = backstart - 1;
            
            if breakpoint > sequence.len() {
                eprintln!("Error: Breakpoint {} is beyond sequence length {}", breakpoint, sequence.len());
                std::process::exit(1);
            }
            
            if backstart > sequence.len() {
                eprintln!("Error: Backstart {} is beyond sequence length {}", backstart, sequence.len());
                std::process::exit(1);
            }
            
            let new_sequence = if gend == 5 {
                // 5' end processing
                // Keep sequence up to breakpoint
                let kept_part = &sequence[..breakpoint_idx + 1];
                
                // Get reverse complement from backstart to beginning (5' end)
                let copyback_part = &sequence[..backstart_idx + 1];
                let reverse_complement: String = copyback_part.chars().rev().map(complement_base).collect();
                
                format!("{}{}", kept_part, reverse_complement)
            } else {
                // 3' end processing
                // First reverse complement the entire sequence
                let rev_comp_sequence: String = sequence.chars().rev().map(complement_base).collect();
                
                // Now apply same logic as 5' end to the reverse complemented sequence
                // Keep sequence up to breakpoint
                let kept_part = &rev_comp_sequence[..breakpoint_idx + 1];
                
                // Get reverse complement from backstart to beginning
                let copyback_part = &rev_comp_sequence[..backstart_idx + 1];
                let reverse_complement: String = copyback_part.chars().rev().map(complement_base).collect();
                
                format!("{}{}", kept_part, reverse_complement)
            };
            
            let operation_desc = if gend == 5 {
                if backstart == breakpoint {
                    format!("5' copyback (snapback) at position {}", breakpoint)
                } else {
                    format!("5' copyback up to position {} then reverse complement of position {} on", breakpoint, backstart)
                }
            } else {
                if backstart == breakpoint {
                    format!("3' copyback (snapback) at position {} of reference revcomp", breakpoint)
                } else {
                    format!("3' copyback up to position {} of reference revcomp then reverse complement of position {} on", breakpoint, backstart)
                }
            };
            
            let new_header = format!("{} [{}]", header, operation_desc);
            
            (new_header, new_sequence)
        }
    }
}

fn write_fasta_to_stdout(header: &str, sequence: &str) -> std::io::Result<()> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    
    writeln!(handle, "{}", header)?;
    
    // Write sequence in 70-character lines (standard FASTA format)
    for chunk in sequence.as_bytes().chunks(70) {
        writeln!(handle, "{}", std::str::from_utf8(chunk).unwrap())?;
    }
    
    Ok(())
}

fn write_fasta_to_file(header: &str, sequence: &str, filename: &str) -> std::io::Result<()> {
    let mut file = File::create(filename)?;
    
    writeln!(file, "{}", header)?;
    
    // Write sequence in 70-character lines (standard FASTA format)
    for chunk in sequence.as_bytes().chunks(70) {
        writeln!(file, "{}", std::str::from_utf8(chunk).unwrap())?;
    }
    
    Ok(())
}
