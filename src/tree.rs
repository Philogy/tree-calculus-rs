use std::fmt;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub enum Tree {
    Leaf,
    Stem(Rc<Tree>),
    Fork(Rc<Tree>, Rc<Tree>),
}

impl Tree {
    fn iter_children(&self) -> impl Iterator<Item = &Tree> {
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

pub fn rec_apply(a: &Rc<Tree>, b: &Rc<Tree>) -> Rc<Tree> {
    match a.as_ref() {
        // △ x = △ x
        Tree::Leaf => Rc::new(Tree::Stem(b.clone())),
        // △ a x = △ a x
        Tree::Stem(a) => Rc::new(Tree::Fork(a.clone(), b.clone())),
        Tree::Fork(lhs, rhs) => match (lhs.as_ref(), rhs) {
            // △ △ a b = a
            (Tree::Leaf, a) => a.clone(),
            // △ (△ x) y z = x z (y z)
            (Tree::Stem(a1), a2) => rec_apply(&rec_apply(a1, b), &rec_apply(a2, b)),
            // △ (△ x y) z w => (x | y w.stem | z w.lhs w.rhs)
            (Tree::Fork(a1, a2), a3) => match b.as_ref() {
                Tree::Leaf => a1.clone(),
                Tree::Stem(u) => rec_apply(a2, u),
                Tree::Fork(u, v) => rec_apply(&rec_apply(a3, u), v),
            },
        },
    }
}
