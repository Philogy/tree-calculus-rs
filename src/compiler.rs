use crate::fast_tree::{Tree, TreeIndex, Trees};
use crate::{ParserError, Span, TreeExpr, TreeLambda};
use std::{collections::HashMap, fmt};

#[derive(Debug)]
pub enum CompiledTree<'src> {
    Tree(TreeIndex),
    NameRef((&'src str, Span)),
    Application(Vec<CompiledTree<'src>>),
}

impl<'src> fmt::Display for CompiledTree<'src> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NameRef((name, _)) => write!(f, "{}", name)?,
            Self::Application(sub_exprs) => {
                if sub_exprs.len() == 1 {
                    write!(f, "{}", sub_exprs[0])?;
                } else {
                    for (i, sub_expr) in sub_exprs.into_iter().enumerate() {
                        if i > 0 {
                            write!(f, " ")?;
                        }
                        if matches!(sub_expr, Self::Application(_)) {
                            write!(f, "({})", sub_expr)?;
                        } else {
                            write!(f, "{}", sub_expr)?;
                        }
                    }
                }
            }
            Self::Tree(tree) => write!(f, "tree#{}", tree.idx())?,
        }
        Ok(())
    }
}

impl<'src> From<TreeIndex> for CompiledTree<'src> {
    fn from(value: TreeIndex) -> Self {
        Self::Tree(value)
    }
}

impl<'src> From<Vec<CompiledTree<'src>>> for CompiledTree<'src> {
    fn from(value: Vec<CompiledTree<'src>>) -> Self {
        Self::Application(value)
    }
}

pub fn compile_tree_expr<'src>(trees: &mut Trees, expr: &TreeExpr<'src>) -> CompiledTree<'src> {
    match expr {
        TreeExpr::Lambda(lambda) => compile_lambda(trees, lambda),
        TreeExpr::Leaf => Trees::LEAF.into(),
        TreeExpr::NameRef((name, span)) => CompiledTree::NameRef((name, span.clone())),
        TreeExpr::Application(sub_terms) => {
            assert!(!sub_terms.is_empty(), "empty tree?");
            sub_terms
                .into_iter()
                .map(|term| compile_tree_expr(trees, term))
                .collect::<Vec<_>>()
                .into()
        }
    }
}

fn compile_lambda<'src>(trees: &mut Trees, lambda: &TreeLambda<'src>) -> CompiledTree<'src> {
    let current_result_body = compile_tree_expr(trees, &lambda.body);
    let inputs_in_stack_pop_order = lambda.params.iter().rev();
    inputs_in_stack_pop_order
        .copied()
        .fold(current_result_body, |current_result_body, param| {
            construct_tree_from_out_and_input(trees, current_result_body, param)
        })
}

fn ignore_input_and_return_tree<'src>(
    trees: &mut Trees,
    return_tree: CompiledTree<'src>,
) -> CompiledTree<'src> {
    // △ △ return_tree
    // △ △ return_tree input -> return_tree
    match return_tree {
        CompiledTree::Tree(return_tree) => {
            let tree = Tree::Fork(Trees::LEAF, return_tree);
            trees.insert(tree).into()
        }
        compiled => vec![Trees::STEM_LEAF.into(), compiled].into(),
    }
}

fn partial_stem<'src>(inner: CompiledTree<'src>) -> CompiledTree<'src> {
    vec![Trees::LEAF.into(), inner].into()
}

fn partial_fork<'src>(lhs: CompiledTree<'src>, rhs: CompiledTree<'src>) -> CompiledTree<'src> {
    vec![Trees::LEAF.into(), lhs, rhs].into()
}

fn construct_tree_from_out_and_input<'src>(
    trees: &mut Trees,
    tree_out: CompiledTree<'src>,
    input_name: &'src str,
) -> CompiledTree<'src> {
    match tree_out {
        CompiledTree::Tree(tree) => ignore_input_and_return_tree(trees, tree.into()).into(),
        CompiledTree::NameRef((ref_name, span)) => {
            if ref_name == input_name {
                CompiledTree::Tree(Trees::IDENTITY)
            } else {
                ignore_input_and_return_tree(trees, CompiledTree::NameRef((ref_name, span)))
            }
        }
        CompiledTree::Application(sub_trees) => sub_trees
            .into_iter()
            .fold(None, |acc_tree, next_tree| {
                Some(match acc_tree {
                    None => construct_tree_from_out_and_input(trees, next_tree, input_name),
                    Some(acc_tree) => partial_fork(
                        partial_stem(acc_tree),
                        construct_tree_from_out_and_input(trees, next_tree, input_name),
                    ),
                })
            })
            .expect("empty tree?"),
    }
}

#[derive(Debug)]
pub struct TreeNamespace<'src> {
    trees: HashMap<&'src str, (TreeIndex, Span)>,
}

impl<'src> TreeNamespace<'src> {
    pub fn new() -> Self {
        Self {
            trees: HashMap::with_capacity(128),
        }
    }

    pub fn eval_tree(
        &self,
        trees: &mut Trees,
        tree: &CompiledTree<'src>,
    ) -> Result<TreeIndex, ParserError> {
        let mut application_frames = Vec::with_capacity(256);
        application_frames.push((None, std::slice::from_ref(tree)));

        'process_apply_frames: while let Some((mut acc, remaining_trees)) = application_frames.pop()
        {
            for (i, tree) in remaining_trees.into_iter().enumerate() {
                let evaluated = match tree {
                    CompiledTree::NameRef((name, span)) => self
                        .trees
                        .get(name)
                        .map(|&(t, _)| t)
                        .ok_or_else(|| (format!("Undefined tree {:?}", name), span.clone()))?,
                    CompiledTree::Tree(tree) => *tree,
                    CompiledTree::Application(sub_trees) => {
                        application_frames.push((acc, &remaining_trees[i + 1..]));
                        application_frames.push((None, &sub_trees));
                        continue 'process_apply_frames;
                    }
                };
                acc = Some(match acc {
                    None => evaluated,
                    Some(acc) => trees.tree_apply(acc, evaluated),
                });
            }

            let acc = acc.expect("Empty application list?");

            match application_frames.pop() {
                Some((acc_below, remaining_trees_below)) => {
                    let acc = Some(match acc_below {
                        None => acc,
                        Some(acc_below) => trees.tree_apply(acc_below, acc),
                    });
                    application_frames.push((acc, remaining_trees_below));
                }
                None => {
                    return Ok(acc);
                }
            }
        }

        unreachable!("stack ran out?")
    }

    pub fn define_new_tree(
        &mut self,
        trees: &mut Trees,
        name: &'src str,
        span: Span,
        tree: &CompiledTree<'src>,
    ) -> Result<TreeIndex, ParserError> {
        let evaluated = self.eval_tree(trees, tree)?;
        if let Some((_, _other_span)) = self.trees.insert(name, (evaluated, span.clone())) {
            return Err((format!("Declaration {:?} redefined", name), span));
        };
        Ok(evaluated)
    }

    pub fn get(&self, name: &str) -> Option<TreeIndex> {
        self.trees.get(name).map(|&(t, _)| t)
    }
}
