use std::collections::HashMap;
use std::fs::File;
use crate::bitfile::{self, BitFile};
use std::io::{Seek, Write, Read, Result};

/*
fn main() -> std::io::Result<()> {
    let mut input = File::open("input.txt")?;
    let mut output = BitFile::create("output.huffman")?;

    CompressFile(input, output)?;

    Ok(())
}
*/

/*
 * The NODE structure is a node in the Huffman decoding tree. it has a 
 * count, which is its weight in the tree, and the node numbers of its 
 * two children. The saved_count member of the structure is only
 * there for debugging purposes, and can be safely taken out at any 
 * time. It just holds the initial count for each of the symbols, since 
 * the count member is continually being modified as the tree grows.
 */
#[derive(Default, Copy, Clone, Debug)]
struct TreeNode {
    count: u8,
    saved_count: u8,
    child_0: usize,
    child_1: usize
}

/*
 * A Huffman tree is set up for deconding, not encoding. When encoding, 
 * I first walk through the tree and build up a table of codes for 
 * each symbol. The codes are stored in this CODE structure 
 */
#[derive(Default, Copy, Clone)]
struct Code {
    code: u8,
    code_bits: usize,
}

/*
 * CompressFile is the compression routine called by MAIN-C.C. It 
 * looks ofr a single additional argument to be passed to it from 
 * the command line: "-d". if a "-d" is present, it means the 
 * user wants to see the model data dumped out for debugging purposes.
 *
 * This routine works in a fairly straightforward manner. First,
 * it has to allocate storage for three different arrays of data. 
 * Next, it counts all the bytes in the input file. The counts 
 * are all stored in long int, so the next step is to scale them down 
 * to single byte counts in the NODE array. After the counts are 
 * scaled, the HUffman decoding tree is build on top of the NODE 
 * array. Another routine walks through the tree to build a table 
 * of codes, one per symbol. Finally, when the codes are all ready, 
 * compressing the file is a simple matter. After the file is 
 * compressed, the storage is freed up, and the routine returns.
 */
fn CompressFile(mut input: File, mut output: BitFile) -> std::io::Result<()> {
    let mut counts = [0u16; 256];
    let mut nodes: [TreeNode; 514] = [TreeNode::default(); 514];
    let mut codes: [Option<Code>; 257] = [None; 257];

    count_bytes(&mut input, &mut counts)?;
    scale_counts(&mut counts, &mut nodes);
    output_counts(&mut output, &nodes)?;

    let root_node = build_tree(&mut nodes);
    convert_tree_to_code(&nodes, &mut codes, 0, 0, root_node);
    let args: Vec<String> = std::env::args().collect();
    let minus_d = String::from("-d");
    if let Some(minus_d) = args.get(1) {

    }

    if args.len() >= 2  && args[1] == "-d".to_string() {
        print_model(&nodes, &codes);
    }

    compress_data(input, output, codes)?;

    Ok(())
}

/*
 * This routine counts the frequency of occurence of every byte in 
 * the input file. It marks the place in the input stream where it 
 * started, counts up the bytes, then returns to the place where 
 * it started. In most C implementations, the length of a file 
 * cannot exceed an unsighend long, so this routine should always
 * work.
 */
fn count_bytes(input: &mut File, counts: &mut [u16; 256]) -> std::io::Result<()> {
    let input_marker = input.stream_position()?;
    let mut buffer = [0u8;1];

    loop {
        if input.read(&mut buffer)? == 0 {
            break;
        }

        counts[buffer[0] as usize] += 1;
    }
    input.seek(std::io::SeekFrom::Start(input_marker));

    Ok(())
}

/*
 * The special EOS symbol is 256, the first available symbol after all 
 * of the possible bytes. When decoding, reading this symbol
 * indicates that all of the data has been read in.
 */
const END_OF_STREAM: usize = 256;

/*
 * In order to limit the size of my Huffman codes to 16 bits, I scale 
 * my counts down so they fit in an unsigned char, and then store them 
 * all as initial weights in my NODE array. The only thing to be 
 * careful of is to make sure that a node with a non-zero count doesn't 
 * get scaled down to 0. Nodes with values of 0 don't get codes.
 */
fn scale_counts(counts: &mut [u16; 256], nodes: &mut [TreeNode; 514]) {
    let mut max_count = 0;
    for i in 0..256 {
        if counts[i] > max_count {
            max_count = counts[i];
        }
    }

    if max_count == 0 {
        counts[0] = 1;
        max_count = 1;
    }
    
    max_count = max_count / 255;
    max_count = max_count + 1;

    for i in 0..256 {
        nodes[i].count = (counts[i] / max_count) as u8;

        if nodes[i].count == 0 && counts[i] != 0 {
            nodes[i].count = 1;
        }
    }

    nodes[END_OF_STREAM].count = 1;
}

/*
 * In order for the compressor to build the same model, I have to 
 * store the symbol counts in the compressed file so the expander can
 * read them in. In order to save space, I don't save all 256 symbols
 * unconditionally. The format used to store counts looks like this:
 *
 * start, stop, counts, start, stop, counts, ... 0 
 *
 * This means that I store runs of counts, until all the non-zero
 * counts have been stored. At this time the list is terminated by 
 * storing a start value of 0. Note that at least 1 run of counts has 
 * to be stored, so even if the first start value is 0, I read it in. 
 * It also means that even in an empty file that has no counts, I have 
 * to pass at least one count, which will have a value of 0. 
 *
 * In order to efficiently use this format, I have to identify runs of 
 * non-zero counts. Because of the format used, I don't want to stop a 
 * run because of just one or two zeros in the count stream. So I have 
 * to sit in a loop looking for strings of three or more zero values 
 * in a row.
 *
 * This is simple in concept, but it ends up being one of the most 
 * complicated routines in the whole program. A routine that just 
 * writes out 256 values without attempting to optimize would be much 
 * simpler, but would hurt compression quite a bit on small files.
 */
fn output_counts(output: &mut BitFile, nodes: &[TreeNode; 514]) -> std::io::Result<()> {
    let mut regular_buffer = [0u8; 1];
    let mut first = 0usize;
    while first < 255 && nodes[first].count == 0 {
        first = first + 1;
    }

    /*
     * Each time I hit the start of the loop, I assume that first is the 
     * start of a run of non-zero values. The rest of the loop is 
     * concerned with finding the value for last, which is the end of the 
     * run, and the value of next, which is the start of the next run.
     * At the end of the loop, I assign next to first, so it starts in on the next run.
     */
    let mut last;
    let mut next;
    while first < 256 {
        last = first + 1;
        while last < 256  && nodes[last].count != 0 {
            last += 1;
        }

        last = last - 1;
        next = last + 1;
        while next < 256 && nodes[next].count == 0 {
            next += 1;
        }

        regular_buffer[0] = first as u8;
        output.write(&regular_buffer)?;

        regular_buffer[0] = last as u8;
        output.write(&regular_buffer)?;

        for i in first..=last {
            regular_buffer[0] = nodes[i].count;
            output.write(&regular_buffer)?;
        }

        first = next;
    }

    regular_buffer[0] = 0;
    output.write(&regular_buffer)?;

    Ok(())
}

/*
 * Building the Huffman tree is fairly simple. All of the active nodes 
 * are scanned in order to locate the two nodes with the minimum 
 * weights. These two weights are added together and assigned to a new
 * node. The new node makes the two minimum nodes into its 0 child 
 * and 1 child. The two minimum nodes are then marked as inactive.
 * This process repeats until there is only one node left, which is 
 * the root node. The tree is done, and the root node is passed back 
 * to the calling routine. 
 *
 * Node 513 is used here to arbitratily provide a node with a guaranteed maximum value.
 * it starts off being min_1 and min_2. After all 
 * active nodes have been scanned, I can tell if there is only one 
 * active node left by checking to see if min_1 is still 513.
 */
fn build_tree(nodes: &mut [TreeNode; 514]) -> usize {
    nodes[513].count = 0xff;
    let mut next_free = END_OF_STREAM;
    let mut min_1;
    let mut min_2;

    loop {
        min_1 = 513;
        min_2 = 513;
        next_free = next_free + 1;
        for i in 0..next_free {
            if nodes[i].count != 0 {
                if nodes[i].count < nodes[min_1].count {
                    min_2 = min_1;
                    min_1 = i;
                } else if nodes[i].count < nodes[min_2].count {
                    min_2 = i;
                }
            }
        }

        if min_2 == 513 {
            break;
        }

        nodes[next_free].count = nodes[min_1].count + nodes[min_2].count;

        nodes[min_1].saved_count = nodes[min_1].count;
        nodes[min_1].count = 0;

        nodes[min_2].saved_count = nodes[min_2].count;
        nodes[min_2].count = 0;

        nodes[next_free].child_0 = min_1;
        nodes[next_free].child_1 = min_2;
    }
    nodes[next_free].saved_count = nodes[next_free].count;

    next_free
}

/*
 * Since the Huffman tree is built as a decoding tree, there is no 
 * simple way to get the encoding values for each symbol out of 
 * it. This routine recursively walks through the tree, adding the 
 * child bits to each code until it gets to a leaf. When it gets 
 * to a leaf, it stores the code value in the CODE element, and 
 * returns.
 */
fn convert_tree_to_code(
    nodes: &[TreeNode; 514],
    codes: &mut [Option<Code>; 257],
    mut code_so_far: u8,
    mut bits: usize,
    node: usize
    ) {

    if node <= END_OF_STREAM {
        codes[node] = Some(Code {code: code_so_far, code_bits : bits});
    }
    code_so_far <<= 1;
    bits += 1;

    convert_tree_to_code(nodes, codes, code_so_far, bits, nodes[node].child_0);
    convert_tree_to_code(nodes, codes, code_so_far | 1, bits, nodes[node].child_1);

}

/*
 * If the -d command line option is specified, this routine is called 
 * to print out some of the model information after the tree is built.
 * Note that this is the only place that the saveed_count NODE element 
 * is used for anything at all, and in this case it is just for 
 * diagnostic information. By the time I get here, and the tree has 
 * been built, every active element will have 0 in its count.
 */
fn print_model(nodes: &[TreeNode; 514], codes: &[Option<Code>; 257]) -> Result<()> {
    for i in 0..514 {
        if nodes[i].saved_count != 0 {
            //todo put it in debug trait
            print!("node=");
            print_char(i);
            print!(" count={:3}, ", nodes[i].saved_count);
            print!(" child_0=");
            print_char(nodes[i].child_0);
            print!(" child_1=");
            print_char(nodes[i].child_1);

            if i <= END_OF_STREAM {
                if let Some(code) = codes[i] {
                    bitfile::file_print_binary(&mut std::io::stdout(), code.code.into(), code.code_bits)?;
                }
            }

            println!();
        }
    }

    Ok(())
}

/*
 * The print_model routine uses this function to print out node num 
 * bers. The catch is if it is a printable character, it gets printed 
 * out as a character. This makes the debug output a little easier to 
 * read.
 */
fn print_char(c: usize) 
{
    if c < 0x20 && c >= 127 {
        print!("c:3");
        return;
    }

    print!("{}", char::from_u32(c.try_into().unwrap()).unwrap());
}

/*
 * Once th tree gets built, and the CODE table is built, compressing
 * the data is a breeze. Each byte is read in, and its corresponding 
 * Huffman code is sent out.
 */
fn compress_data(mut input: File, mut output: BitFile,  codes: [Option<Code>; 257]) -> Result<()> {
    let mut buffer = [0u8; 1];

    loop {
        match input.read(&mut buffer) {
            Ok(0) => break,
            Ok(_) => {},
            Err(e) => {
                return Err(e);
            }
        }

        let code = codes[buffer[0] as usize].unwrap();
        output.output_bits(code.code as u32, code.code_bits)?;
    }

    let code = codes[END_OF_STREAM].unwrap();
    output.output_bits(code.code as u32, code.code_bits)?;

    Ok(())
} 
