use std::fmt;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub enum Tree {
    Leaf,
    Stem(Rc<Tree>),
    Fork(Rc<Tree>, Rc<Tree>),
}

impl Tree {
    pub fn leaf() -> Rc<Self> {
        Rc::new(Self::Leaf)
    }

    pub fn fork(lhs: Rc<Tree>, rhs: Rc<Tree>) -> Rc<Self> {
        Rc::new(Self::Fork(lhs, rhs))
    }

    pub fn stem(value: Rc<Tree>) -> Rc<Self> {
        Rc::new(Self::Stem(value))
    }

    pub fn iter_children(&self) -> impl Iterator<Item = &Tree> {
        match self {
            Self::Leaf => None.into_iter().chain(None),
            Self::Stem(a) => Some(a.as_ref()).into_iter().chain(None),
            Self::Fork(a, b) => Some(a.as_ref()).into_iter().chain(Some(b.as_ref())),
        }
    }
}

impl fmt::Display for Tree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "△")?;
        for child in self.iter_children() {
            match child {
                Self::Leaf => write!(f, " △")?,
                child => write!(f, " ({})", child)?,
            }
        }
        Ok(())
    }
}

pub fn rec_tree_apply(a: &Rc<Tree>, b: &Rc<Tree>) -> Rc<Tree> {
    match a.as_ref() {
        // △ x = △ x
        Tree::Leaf => Rc::new(Tree::Stem(b.clone())),
        // △ a x = △ a x
        Tree::Stem(a) => Rc::new(Tree::Fork(a.clone(), b.clone())),
        Tree::Fork(lhs, rhs) => match (lhs.as_ref(), rhs) {
            // △ △ a b = a
            (Tree::Leaf, a) => a.clone(),
            // △ (△ x) y z = x z (y z)
            (Tree::Stem(a1), a2) => rec_tree_apply(&rec_tree_apply(a1, b), &rec_tree_apply(a2, b)),
            // △ (△ x y) z w => (x | y w.stem | z w.lhs w.rhs)
            (Tree::Fork(a1, a2), a3) => match b.as_ref() {
                Tree::Leaf => a1.clone(),
                Tree::Stem(u) => rec_tree_apply(a2, u),
                Tree::Fork(u, v) => rec_tree_apply(&rec_tree_apply(a3, u), v),
            },
        },
    }
}

enum TreeEvalStage {
    ForkPartialPatternMatch(Rc<Tree>),
    DuplicatePendingSecondEval(Rc<Tree>, Rc<Tree>),
    DuplicatePendingFinalEval(Rc<Tree>),
}

pub fn tree_apply(mut a: Rc<Tree>, mut b: Rc<Tree>) -> Rc<Tree> {
    let mut eval_stack = Vec::with_capacity(1024);

    loop {
        let result = match a.as_ref() {
            // △ b = △ b
            Tree::Leaf => Tree::stem(b.clone()),
            // △ a b = △ a b
            Tree::Stem(a) => Tree::fork(a.clone(), b.clone()),
            Tree::Fork(lhs, rhs) => match (lhs.as_ref(), rhs) {
                // △ △ a b = a
                (Tree::Leaf, a) => a.clone(),
                // △ (△ x) y b = x b (y b)
                (Tree::Stem(x), y) => {
                    eval_stack.push(TreeEvalStage::DuplicatePendingSecondEval(
                        y.clone(),
                        b.clone(),
                    ));
                    a = x.clone(); // Evaluate `x b`, push `(_ (y b))`.
                    continue;
                }
                // △ (△ x y) z w => (x | y w.stem | z w.lhs w.rhs)
                (Tree::Fork(a1, a2), a3) => match b.as_ref() {
                    Tree::Leaf => a1.clone(),
                    Tree::Stem(u) => {
                        (a, b) = (a2.clone(), u.clone()); // Evaluate `a2 u`.
                        continue;
                    }
                    Tree::Fork(u, v) => {
                        eval_stack.push(TreeEvalStage::ForkPartialPatternMatch(v.clone()));
                        (a, b) = (a3.clone(), u.clone()); // Evaluate `a3 u`, push `_ v`
                        continue;
                    }
                },
            },
        };

        match eval_stack.pop() {
            None => return result,
            Some(TreeEvalStage::ForkPartialPatternMatch(v)) => {
                (a, b) = (result, v); // Complete `(a3 u) v`.
                continue;
            }
            Some(TreeEvalStage::DuplicatePendingSecondEval(y, stack_b)) => {
                (a, b) = (y, stack_b); // Eval `y b`, push `(_ (y b))`
                eval_stack.push(TreeEvalStage::DuplicatePendingFinalEval(result));
                continue;
            }
            Some(TreeEvalStage::DuplicatePendingFinalEval(xb)) => {
                (a, b) = (xb, result);
                continue;
            }
        }
    }
}
