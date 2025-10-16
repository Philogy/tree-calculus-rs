use tree_calculus::{Lexer, Span, Tree, TreeNamespace, compile_tree_expr, parse_declarations};

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

fn visualize_tree(buf: &mut String, name: &str, tree: &Tree) {
    use std::fmt::Write;
    buf.clear();

    fn visualize_tree_inner(buf: &mut String, tree: &Tree, id: u64) -> u64 {
        match tree {
            Tree::Leaf => id + 1,
            Tree::Stem(value) => {
                let next_id = id + 1;
                write!(buf, "n{}->n{};", id, next_id).unwrap();
                visualize_tree_inner(buf, value, next_id)
            }
            Tree::Fork(lhs, rhs) => {
                let next_id = id + 1;
                write!(buf, "n{}->n{};", id, next_id).unwrap();
                let next_id = visualize_tree_inner(buf, lhs, next_id);
                write!(buf, "n{}->n{};", id, next_id).unwrap();
                let next_id = visualize_tree_inner(buf, rhs, next_id);
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
    visualize_tree_inner(buf, tree, 0);
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

    for decl in decls.iter() {
        let compiled = compile_tree_expr(&decl.expr);
        let tree = match namespace.define_new_tree(decl.name, decl.span.clone(), &compiled) {
            Ok(tree) => tree,
            Err((reason, span)) => {
                pretty_err(&source, reason, span, true);
                std::process::exit(1);
            }
        };

        println!("{} = {}", decl.name, tree);
    }

    for decl in decls.iter() {
        let tree = namespace.get(decl.name).expect("errors processed");
        visualize_tree(&mut buf, &decl.name, tree);

        std::fs::write(format!("trees/tree-{}.dot", decl.name), &buf).unwrap();
    }
}
