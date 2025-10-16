use ahash::RandomState;
use std::collections::HashMap;
use std::fmt;
use std::num::NonZero;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TreeIndex(NonZero<u32>);

impl TreeIndex {
    #[inline]
    pub fn idx(&self) -> usize {
        (self.0.get() - 1) as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(align(8))]
struct StoredTree {
    children: Option<(TreeIndex, Option<TreeIndex>)>,
}

impl StoredTree {
    pub fn leaf() -> Self {
        StoredTree { children: None }
    }

    pub fn stem(child: TreeIndex) -> Self {
        StoredTree {
            children: Some((child, None)),
        }
    }

    pub fn fork(lhs: TreeIndex, rhs: TreeIndex) -> Self {
        StoredTree {
            children: Some((lhs, Some(rhs))),
        }
    }

    #[inline]
    fn to_u64(self) -> u64 {
        let (lhs, rhs) = match self.children {
            None => (0, 0),
            Some((lhs, None)) => (lhs.0.get(), 0),
            Some((lhs, Some(rhs))) => ((lhs.0.get()), (rhs.0.get())),
        };

        (lhs as u64) | ((rhs as u64) << 32)
    }
}

impl std::cmp::PartialOrd for StoredTree {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.to_u64().cmp(&other.to_u64()))
    }
}

impl std::cmp::Ord for StoredTree {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.to_u64().cmp(&other.to_u64())
    }
}

impl std::hash::Hash for StoredTree {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.to_u64());
    }
}

impl From<Tree> for StoredTree {
    fn from(value: Tree) -> Self {
        match value {
            Tree::Leaf => Self::leaf(),
            Tree::Stem(value) => Self::stem(value),
            Tree::Fork(lhs, rhs) => Self::fork(lhs, rhs),
        }
    }
}

impl From<StoredTree> for Tree {
    fn from(value: StoredTree) -> Tree {
        match value.children {
            None => Tree::Leaf,
            Some((value, None)) => Tree::Stem(value),
            Some((lhs, Some(rhs))) => Tree::Fork(lhs, rhs),
        }
    }
}

const _ASSERT_STORED_TREE_SIZE: () = const {
    assert!(std::mem::size_of::<StoredTree>() == 8);
};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Tree {
    Leaf,
    Stem(TreeIndex),
    Fork(TreeIndex, TreeIndex),
}

impl Tree {
    pub fn iter_children(&self) -> impl Iterator<Item = TreeIndex> {
        match self {
            Self::Leaf => None.into_iter().chain(None),
            &Self::Stem(a) => Some(a).into_iter().chain(None),
            &Self::Fork(a, b) => Some(a).into_iter().chain(Some(b)),
        }
    }
}

#[derive(Debug)]
struct TreeEvalFrame {
    fork_partial: Option<TreeIndex>,
    duplicate_end: Option<TreeIndex>,
}

impl TreeEvalFrame {
    fn duplicate_middle_eval(y: TreeIndex, b: TreeIndex) -> Self {
        TreeEvalFrame {
            fork_partial: Some(y),
            duplicate_end: Some(b),
        }
    }

    fn duplciate_end_eval(xb: TreeIndex) -> Self {
        TreeEvalFrame {
            fork_partial: None,
            duplicate_end: Some(xb),
        }
    }

    fn fork_partial(v: TreeIndex) -> Self {
        TreeEvalFrame {
            fork_partial: Some(v),
            duplicate_end: None,
        }
    }
}

const INDEX_THRESHOLD: usize = 128;

#[derive(Debug)]
pub struct Trees {
    indexed: HashMap<StoredTree, TreeIndex, RandomState>,
    trees: Vec<StoredTree>,
    eval_stack: Vec<TreeEvalFrame>,
}

impl Trees {
    pub const LEAF: TreeIndex = Self::as_tree_index(0);
    pub const STEM_LEAF: TreeIndex = Self::as_tree_index(1);
    pub const FORK_LEAF_LEAF: TreeIndex = Self::as_tree_index(2);
    pub const IDENTITY: TreeIndex = Self::as_tree_index(3);

    pub fn new(tree_capacity: usize) -> Self {
        let mut trees = Self {
            indexed: HashMap::with_capacity_and_hasher(INDEX_THRESHOLD * 2, Default::default()),
            trees: Vec::with_capacity(tree_capacity),
            eval_stack: Vec::with_capacity(256),
        };

        assert_eq!(trees.push_and_index(Tree::Leaf), Self::LEAF);
        assert_eq!(
            trees.push_and_index(Tree::Stem(Self::LEAF)),
            Self::STEM_LEAF
        );
        assert_eq!(
            trees.push_and_index(Tree::Fork(Self::LEAF, Self::LEAF)),
            Self::FORK_LEAF_LEAF
        );

        // △ (△ △ △) △
        // △ (△ △ △) △ (△) -> △
        // △ (△ △ △) △ (△ x) -> △ x
        // △ (△ △ △) △ (△ x y) -> (△ x) y -> (△ x y)
        assert_eq!(
            trees.push_and_index(Tree::Fork(Self::FORK_LEAF_LEAF, Self::LEAF)),
            Self::IDENTITY
        );

        trees
    }

    fn push_and_index(&mut self, tree: Tree) -> TreeIndex {
        let tree = tree.into();
        let idx = Self::as_tree_index(self.trees.len());
        self.trees.push(tree);
        if self.indexed.len() < INDEX_THRESHOLD {
            self.indexed.insert(tree, idx);
        }
        idx
    }

    const fn as_tree_index(i: usize) -> TreeIndex {
        TreeIndex(NonZero::new(i as u32 + 1).unwrap())
    }

    pub fn insert(&mut self, tree: Tree) -> TreeIndex {
        match self.indexed.get(&tree.into()) {
            Some(idx) => *idx,
            None => self.push_and_index(tree),
        }
    }

    pub fn index(&self, idx: TreeIndex) -> Tree {
        let idx = idx.idx();
        debug_assert!(idx < self.trees.len(), "index out of bounds");
        let stored = *unsafe { self.trees.get_unchecked(idx) };
        stored.into()
    }

    pub fn count_non_garbage(&self, alive: impl Iterator<Item = TreeIndex>) -> u32 {
        let mut tree_alive = vec![false; self.trees.len()];
        let mut todo_stack: Vec<_> = alive.into_iter().collect();

        while let Some(idx) = todo_stack.pop() {
            let iidx = idx.idx();
            if tree_alive[iidx] {
                continue;
            }
            tree_alive[iidx] = true;
            for child in self.index(idx).iter_children() {
                todo_stack.push(child);
            }
        }

        tree_alive.into_iter().filter(|b| *b).count() as u32
    }

    pub fn parse_nat(&self, start: TreeIndex) -> Result<u64, TreeIndex> {
        let mut tree = start;
        let mut x = 0;
        loop {
            match self.index(tree) {
                Tree::Leaf => return Ok(x),
                Tree::Stem(inner) => {
                    x += 1;
                    debug_assert!(tree > inner);
                    tree = inner;
                }
                Tree::Fork(_, _) => return Err(start),
            }
        }
    }

    pub fn tree_apply(&mut self, mut a: TreeIndex, mut b: TreeIndex) -> TreeIndex {
        self.eval_stack.clear();

        loop {
            let result = match self.index(a) {
                // △ b = △ b
                Tree::Leaf => self.insert(Tree::Stem(b)),
                // △ a b = △ a b
                Tree::Stem(a) => self.insert(Tree::Fork(a, b)),
                Tree::Fork(lhs, rhs) => match (self.index(lhs), rhs) {
                    // △ △ a b = a
                    (Tree::Leaf, a) => a,
                    // △ (△ x) y b = x b (y b)
                    (Tree::Stem(x), y) => {
                        let frame = TreeEvalFrame::duplicate_middle_eval(y, b);
                        self.eval_stack.push(frame);
                        a = x; // Evaluate `x b`, push `(_ (y b))`.
                        continue;
                    }
                    // △ (△ x y) z w => (x | y w.stem | z w.lhs w.rhs)
                    (Tree::Fork(a1, a2), a3) => match self.index(b) {
                        Tree::Leaf => a1,
                        Tree::Stem(u) => {
                            (a, b) = (a2, u); // Evaluate `a2 u`.
                            continue;
                        }
                        Tree::Fork(u, v) => {
                            self.eval_stack.push(TreeEvalFrame::fork_partial(v));
                            (a, b) = (a3, u); // Evaluate `a3 u`, push `_ v`
                            continue;
                        }
                    },
                },
            };

            match self.eval_stack.pop() {
                None => return result,
                Some(TreeEvalFrame {
                    fork_partial: Some(v),
                    duplicate_end: None,
                }) => {
                    (a, b) = (result, v); // Complete `(a3 u) v`.
                    continue;
                }
                Some(TreeEvalFrame {
                    fork_partial: Some(y),
                    duplicate_end: Some(stack_b),
                }) => {
                    (a, b) = (y, stack_b); // Eval `y b`, push `(_ (y b))`
                    self.eval_stack
                        .push(TreeEvalFrame::duplciate_end_eval(result));
                    continue;
                }
                Some(TreeEvalFrame {
                    fork_partial: None,
                    duplicate_end: Some(xb),
                }) => {
                    (a, b) = (xb, result); // Evaluated `y b`, evaluate `x b (y b)`.
                }
                Some(invalid_frame) => unreachable!("invalid frame: {:?}", invalid_frame),
            }
        }
    }

    pub fn as_ref<'a>(&'a self, tree: Tree) -> TreeRef<'a> {
        TreeRef { trees: self, tree }
    }

    pub fn report_final_usage(&self) {
        println!("self.trees.len(): {:?}", self.trees.len());
        println!(
            "self.eval_stack.capacity(): {:?}",
            self.eval_stack.capacity()
        );
        println!("self.indexed.len(): {:?}", self.indexed.len());
    }
}

#[derive(Debug, Clone)]
pub struct TreeRef<'trees> {
    trees: &'trees Trees,
    tree: Tree,
}

impl<'trees> fmt::Display for TreeRef<'trees> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "△")?;
        for child_idx in self.tree.iter_children() {
            match self.trees.index(child_idx) {
                Tree::Leaf => write!(f, " △")?,
                child => write!(f, " ({})", self.trees.as_ref(child))?,
            }
        }
        Ok(())
    }
}
