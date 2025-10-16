use tree_calculus::{
    Declaration, Lexer, Span, TreeNamespace, compile_tree_expr,
    fast_tree::{Tree, TreeIndex, Trees},
    parse_declarations,
};

fn pretty_err<'src>(source: &'src str, reason: String, span: Span, with_line_number: bool) {
    let newlines: Vec<_> = source
        .char_indices()
        .filter_map(|(i, c)| (c == '\n').then_some(i))
        .collect();
    use std::iter::once;
    for (line, (start, end)) in once(0)
        .chain(newlines.iter().copied())
        .zip(newlines.iter().copied().chain(once(source.len())))
        .enumerate()
    {
        let line = line + 1;
        if start <= span.start && span.start < end {
            let start = if line > 1 { start + 1 } else { start };
            let size = if with_line_number {
                eprintln!("{} | {}", line, &source[start..end]);
                4 + line.checked_ilog10().unwrap_or(0)
            } else {
                eprintln!("{}", &source[start..end]);
                0
            };
            for _ in 0..size {
                eprint!(" ");
            }
            for j in start..end {
                if j < span.start {
                    eprint!(" ");
                } else if j < span.end {
                    eprint!("^")
                }
            }
        }
    }
    eprintln!("");
    eprintln!("{}", reason);
}

fn visualize_tree(buf: &mut String, trees: &Trees, name: &str, tree: TreeIndex) {
    use std::fmt::Write;
    buf.clear();

    fn visualize_tree_inner(buf: &mut String, trees: &Trees, tree: TreeIndex, id: u64) -> u64 {
        match trees.index(tree) {
            Tree::Leaf => id + 1,
            Tree::Stem(value) => {
                let next_id = id + 1;
                write!(buf, "n{}->n{};", id, next_id).unwrap();
                visualize_tree_inner(buf, trees, value, next_id)
            }
            Tree::Fork(lhs, rhs) => {
                let next_id = id + 1;
                write!(buf, "n{}->n{};", id, next_id).unwrap();
                let next_id = visualize_tree_inner(buf, trees, lhs, next_id);
                write!(buf, "n{}->n{};", id, next_id).unwrap();
                let next_id = visualize_tree_inner(buf, trees, rhs, next_id);
                next_id
            }
        }
    }

    write!(
        buf,
        r#"digraph T {{
            label="{}"
            node [label="", shape=circle];
        "#,
        name
    )
    .unwrap();
    visualize_tree_inner(buf, trees, tree, 0);
    write!(buf, "}}").unwrap();
}

fn main() {
    let path = std::env::args().nth(1).expect("missing path");
    let source = std::fs::read_to_string(path).expect("reading file failed");

    let mut lexer = Lexer::new(&source);
    let decls = match parse_declarations(&mut lexer) {
        Ok(decls) => decls,
        Err((reason, span)) => {
            pretty_err(&source, reason, span, true);
            std::process::exit(1);
        }
    };

    let mut namespace = TreeNamespace::new();

    let mut buf = String::with_capacity(4096 * 8);
    let mut trees = Trees::new(1_000_000, 2_000_000);

    for decl in decls.iter() {
        match decl {
            Declaration::TreeDecl(decl) => {
                let compiled = compile_tree_expr(&mut trees, &decl.expr);
                if let Err((reason, span)) =
                    namespace.define_new_tree(&mut trees, decl.name, decl.span.clone(), &compiled)
                {
                    pretty_err(&source, reason, span, true);
                    std::process::exit(1);
                }
            }
            Declaration::NumEval((name, span))
            | Declaration::Show((name, span))
            | Declaration::Visualize((name, span)) => {
                if let None = namespace.get(name) {
                    pretty_err(&source, format!("Undefined {:?}", name), span.clone(), true);
                    std::process::exit(1);
                }
            }
        }
    }

    // dbg!(&trees);

    for decl in decls.iter() {
        match decl {
            Declaration::TreeDecl(_) => {}
            Declaration::Visualize((name, _)) => {
                let tree = namespace.get(name).expect("errors processed");
                visualize_tree(&mut buf, &trees, name, tree);

                std::fs::write(format!("trees/tree-{}.dot", name), &buf).unwrap();
            }
            Declaration::NumEval((name, _)) => {
                let tree_idx = namespace.get(name).expect("errors processed?");
                match trees.parse_nat(tree_idx) {
                    Ok(x) => println!("{name} => {x}"),
                    Err(tree_idx) => {
                        println!(
                            "{name} => (not nat: {})",
                            trees.as_ref(trees.index(tree_idx))
                        )
                    }
                }
            }
            Declaration::Show((name, _)) => {
                let tree_idx = namespace.get(name).expect("errors processed?");
                let tree = trees.index(tree_idx);
                println!("{} = {}", name, trees.as_ref(tree));
            }
        }
    }

    println!("\n==== TREE STATS ====");
    trees.report_final_usage();
    let non_garbage = trees.count_non_garbage(namespace.iter_trees());
    let total_trees = trees.total_trees_stored();
    println!(
        "non garbage: {} ({:.2}%)",
        non_garbage,
        (non_garbage as f32) / (total_trees as f32) * 100.0
    );
    println!(
        "garbage: {} ({:.2}%)",
        total_trees - non_garbage,
        (total_trees - non_garbage) as f32 / (total_trees as f32) * 100.0
    );
}
