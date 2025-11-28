pub fn compute_col_max_len(rows: &Vec<Vec<String>>) -> Vec<usize> {
    let mut col_max_len: Vec<usize> = Vec::new();
    for row in rows.iter() {
        for (colno, column) in row.iter().enumerate() {
            let strlen = column.len();
            if col_max_len.len() <= colno {
                col_max_len.push(strlen);
            } else {
                if strlen > col_max_len[colno] {
                    col_max_len[colno] = strlen;
                }
            }
        }
    }
    col_max_len
}

pub fn print_rows(rows: &Vec<Vec<String>>, col_max_len: &Vec<usize>, header: bool) {
    let mut total = 0;
    for l in col_max_len.iter() {
        total += l;
    }
    total += 3 * col_max_len.len() - 1;
    for (rowno, row) in rows.iter().enumerate() {
        for (colno, column) in row.iter().enumerate() {
            let spacing = col_max_len[colno];
            print_with_spaces(&column, spacing);
            print!(" | ");
        }
        if header && rowno == 0 {
            println!();
            print_times("-", total);
        }
        println!();
    }
}

fn print_times(what: &str, reepat: usize) {
    for _ in 0..reepat {
        print!("{what}");
    }
}

fn print_with_spaces(what: &str, max_len: usize) {
    let strlen = what.len();
    let mut prntstr = what;

    if strlen > max_len {
        prntstr = what.get(0..max_len).unwrap();
        print!("{prntstr}");
    } else {
        let mut len_remains = max_len - strlen;
        let l2 = len_remains >> 1;
        for _ in 0..l2 {
            print!(" ");
        }
        print!("{prntstr}");
        len_remains -= l2;
        for _ in 0..len_remains {
            print!(" ");
        }
    }
}
