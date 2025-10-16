use std::rc::Rc;
use std::{collections::HashMap, fmt};

use crate::{ParserError, Span, Tree, TreeExpr, TreeLambda, tree_apply};

#[derive(Debug)]
pub enum CompiledTree<'src> {
    Tree(Rc<Tree>),
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
            Self::Tree(tree) => write!(f, "{}", &tree)?,
        }
        Ok(())
    }
}

impl<'src> From<Rc<Tree>> for CompiledTree<'src> {
    fn from(value: Rc<Tree>) -> Self {
        Self::Tree(value)
    }
}

impl<'src> From<Vec<CompiledTree<'src>>> for CompiledTree<'src> {
    fn from(value: Vec<CompiledTree<'src>>) -> Self {
        Self::Application(value)
    }
}

pub fn compile_tree_expr<'src>(expr: &TreeExpr<'src>) -> CompiledTree<'src> {
    match expr {
        TreeExpr::Lambda(lambda) => compile_lambda(lambda),
        TreeExpr::Leaf => Tree::leaf().into(),
        TreeExpr::NameRef((name, span)) => CompiledTree::NameRef((name, span.clone())),
        TreeExpr::Application(sub_terms) => {
            assert!(!sub_terms.is_empty(), "empty tree?");
            sub_terms
                .into_iter()
                .map(compile_tree_expr)
                .collect::<Vec<_>>()
                .into()
        }
    }
}

fn compile_lambda<'src>(lambda: &TreeLambda<'src>) -> CompiledTree<'src> {
    let current_result_body = compile_tree_expr(&lambda.body);
    let inputs_in_stack_pop_order = lambda.params.iter().rev();
    inputs_in_stack_pop_order
        .copied()
        .fold(current_result_body, construct_tree_from_out_and_input)
}

fn ignore_input_and_return_tree<'src>(return_tree: CompiledTree<'src>) -> CompiledTree<'src> {
    // △ △ return_tree
    // △ △ return_tree input -> return_tree
    match return_tree {
        CompiledTree::Tree(return_tree) => Tree::fork(Tree::leaf(), return_tree).into(),
        compiled => vec![Tree::stem(Tree::leaf()).into(), compiled].into(),
    }
}

fn identity_func() -> Rc<Tree> {
    // △ (△ △ △) △
    // △ (△ △ △) △ (△) -> △
    // △ (△ △ △) △ (△ x) -> △ x
    // △ (△ △ △) △ (△ x y) -> (△ x) y -> (△ x y)
    let leaf = Tree::leaf();
    Tree::fork(Tree::fork(leaf.clone(), leaf.clone()), leaf)
}

fn partial_stem<'src>(inner: CompiledTree<'src>) -> CompiledTree<'src> {
    vec![Tree::leaf().into(), inner].into()
}

fn partial_fork<'src>(lhs: CompiledTree<'src>, rhs: CompiledTree<'src>) -> CompiledTree<'src> {
    vec![Tree::leaf().into(), lhs, rhs].into()
}

fn construct_tree_from_out_and_input<'src>(
    tree_out: CompiledTree<'src>,
    input_name: &'src str,
) -> CompiledTree<'src> {
    match tree_out {
        CompiledTree::Tree(tree) => ignore_input_and_return_tree(tree.into()).into(),
        CompiledTree::NameRef((ref_name, span)) => {
            if ref_name == input_name {
                CompiledTree::Tree(identity_func())
            } else {
                ignore_input_and_return_tree(CompiledTree::NameRef((ref_name, span)))
            }
        }
        CompiledTree::Application(sub_trees) => sub_trees
            .into_iter()
            .fold(None, |acc_tree, next_tree| {
                Some(match acc_tree {
                    None => construct_tree_from_out_and_input(next_tree, input_name),
                    Some(acc_tree) => partial_fork(
                        partial_stem(acc_tree),
                        construct_tree_from_out_and_input(next_tree, input_name),
                    ),
                })
            })
            .expect("empty tree?"),
    }
}

#[derive(Debug)]
pub struct TreeNamespace<'src> {
    trees: HashMap<&'src str, (Rc<Tree>, Span)>,
}

impl<'src> TreeNamespace<'src> {
    pub fn new() -> Self {
        Self {
            trees: HashMap::with_capacity(128),
        }
    }

    pub fn eval_tree(&self, tree: &CompiledTree<'src>) -> Result<Rc<Tree>, ParserError> {
        match tree {
            CompiledTree::NameRef((name, span)) => self
                .trees
                .get(name)
                .map(|(t, _)| t.clone())
                .ok_or_else(|| (format!("Undefined tree {:?}", name), span.clone())),
            CompiledTree::Tree(tree) => Ok(tree.clone()),
            CompiledTree::Application(sub_trees) => sub_trees
                .into_iter()
                .try_fold(None, |acc_tree, tree| {
                    let res_tree = match acc_tree {
                        None => self.eval_tree(tree)?,
                        Some(acc_tree) => tree_apply(&acc_tree, &self.eval_tree(tree)?),
                    };
                    Ok(Some(res_tree))
                })
                .map(|tree| tree.expect("empty tree?")),
        }
    }

    pub fn define_new_tree(
        &mut self,
        name: &'src str,
        span: Span,
        tree: &CompiledTree<'src>,
    ) -> Result<Rc<Tree>, ParserError> {
        let evaluated = self.eval_tree(tree)?;
        if let Some((_, _other_span)) = self.trees.insert(name, (evaluated.clone(), span.clone())) {
            return Err((format!("Declaration {:?} redefined", name), span));
        };
        Ok(evaluated)
    }

    pub fn get(&self, name: &str) -> Option<&Tree> {
        self.trees.get(name).map(|(t, _)| t.as_ref())
    }
}
