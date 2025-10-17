use crate::{Tree, TreeFunc, TreeIndex, Trees};

pub fn try_simplify_tree(trees: &mut Trees, tree: TreeIndex) -> TreeIndex {
    // println!("try_simplify_tree {}", trees.as_ref(trees.index(tree)));
    return match trees.index_func(tree) {
        TreeFunc::Leaf | TreeFunc::Stem(_) | TreeFunc::IgnoreThen(_) => tree,
        TreeFunc::Match(x, y, z) => {
            let y = try_simplify_tree(trees, y);
            let z = try_simplify_tree(trees, z);
            trees.insert_func(TreeFunc::Match(x, y, z))
        }
        TreeFunc::DoubleChain(x, y) => {
            let x = try_simplify_tree(trees, x);
            let y = try_simplify_tree(trees, y);
            match (trees.index_func(x), trees.index_func(y)) {
                (TreeFunc::IgnoreThen(then_x), TreeFunc::IgnoreThen(then_y)) => {
                    // Result of `then_x then_y`
                    match trees.index_func(then_x) {
                        TreeFunc::Leaf => {
                            let then_y = try_simplify_tree(trees, then_y);
                            let new_then = trees.insert(Tree::Stem(then_y));
                            trees.ignore_then(new_then)
                        }
                        TreeFunc::Stem(lhs) => {
                            let then_y = try_simplify_tree(trees, then_y);
                            let new_then = trees.insert(Tree::Fork(lhs, then_y));
                            trees.ignore_then(new_then)
                        }
                        TreeFunc::IgnoreThen(then) => trees.ignore_then(then),
                        // TreeFunc::Match(x, _, _) => match trees.index(then_y) {
                        //     Tree::Leaf => trees.ignore_then(x),
                        //     Tree::Stem(_) | Tree::Fork(_, _) => {
                        //         trees.insert_func(TreeFunc::DoubleChain(x, y))
                        //     }
                        // },
                        _ => trees.insert_func(TreeFunc::DoubleChain(x, y)),
                    }
                }
                (TreeFunc::IgnoreThen(then_x), _) if y == Trees::IDENTITY => {
                    try_simplify_tree(trees, then_x)
                }
                _ => {
                    let y = try_simplify_tree(trees, y);
                    trees.insert_func(TreeFunc::DoubleChain(x, y))
                }
            }
        }
    };
}
