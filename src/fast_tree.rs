use ahash::RandomState;
use std::collections::HashMap;
use std::fmt;
use std::num::NonZero;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TreeIndex(NonZero<u32>);

const MAX_TREE_APPLY_ITERS: usize = 1_000_000_000;

impl TreeIndex {
    #[inline]
    pub const fn idx(&self) -> usize {
        (self.0.get() - 1) as usize
    }

    #[inline]
    const fn from_idx(i: usize) -> Self {
        TreeIndex(unsafe { NonZero::new_unchecked(i as u32 + 1) })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(align(8))]
struct TreeStoreEntry {
    lhs_or_free_entry: Option<TreeIndex>,
    rhs_or_stem: Option<TreeIndex>,
}

impl TreeStoreEntry {
    pub fn leaf() -> Self {
        TreeStoreEntry {
            lhs_or_free_entry: None,
            rhs_or_stem: None,
        }
    }

    pub fn stem(child: TreeIndex) -> Self {
        TreeStoreEntry {
            lhs_or_free_entry: None,
            rhs_or_stem: Some(child),
        }
    }

    pub fn fork(lhs: TreeIndex, rhs: TreeIndex) -> Self {
        TreeStoreEntry {
            lhs_or_free_entry: Some(lhs),
            rhs_or_stem: Some(rhs),
        }
    }

    fn free_entry(next_free: TreeIndex) -> Self {
        TreeStoreEntry {
            lhs_or_free_entry: Some(next_free),
            rhs_or_stem: None,
        }
    }

    fn get_free(&self) -> Option<TreeIndex> {
        match (self.lhs_or_free_entry, self.rhs_or_stem) {
            (Some(next_free), None) => Some(next_free),
            _ => None,
        }
    }

    #[inline]
    fn to_u64(self) -> u64 {
        let lhs = self.lhs_or_free_entry.map_or(0, |x| x.0.get());
        let rhs = self.rhs_or_stem.map_or(0, |x| x.0.get());
        (lhs as u64) | ((rhs as u64) << 32)
    }
}

impl std::cmp::PartialOrd for TreeStoreEntry {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.to_u64().cmp(&other.to_u64()))
    }
}

impl std::cmp::Ord for TreeStoreEntry {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.to_u64().cmp(&other.to_u64())
    }
}

impl std::hash::Hash for TreeStoreEntry {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.to_u64());
    }
}

impl From<Tree> for TreeStoreEntry {
    fn from(value: Tree) -> Self {
        match value {
            Tree::Leaf => Self::leaf(),
            Tree::Stem(value) => Self::stem(value),
            Tree::Fork(lhs, rhs) => Self::fork(lhs, rhs),
        }
    }
}

pub struct IsFreeListEntryError;

impl TryFrom<TreeStoreEntry> for Tree {
    type Error = IsFreeListEntryError;

    fn try_from(value: TreeStoreEntry) -> Result<Self, Self::Error> {
        match (value.lhs_or_free_entry, value.rhs_or_stem) {
            (None, None) => Ok(Tree::Leaf),
            (None, Some(value)) => Ok(Tree::Stem(value)),
            (Some(lhs), Some(rhs)) => Ok(Tree::Fork(lhs, rhs)),
            (Some(_), None) => Err(IsFreeListEntryError),
        }
    }
}

const _ASSERT_STORED_TREE_SIZE: () = const {
    assert!(std::mem::size_of::<TreeStoreEntry>() == 8);
};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Tree {
    Leaf,
    Stem(TreeIndex),
    Fork(TreeIndex, TreeIndex),
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum TreeFunc {
    // (△) x = △ x
    Leaf,
    // (△ a) x = △ a x
    Stem(TreeIndex),
    // (△ △ a) b => a
    IgnoreThen(TreeIndex),
    // △ (△ x) y z = x z (y z)
    DoubleChain(TreeIndex, TreeIndex),
    // △ (△ x y) z w => (x | y w.stem | z w.lhs w.rhs)
    Match(TreeIndex, TreeIndex, TreeIndex),
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

type EvalInput = (TreeIndex, TreeIndex);

#[derive(Debug)]
struct TreeEvalFrame {
    input_to_cache: EvalInput,
    fork_partial: Option<TreeIndex>,
    duplicate_end: Option<TreeIndex>,
}

impl TreeEvalFrame {
    fn just_cache(input_to_cache: EvalInput) -> Self {
        Self {
            input_to_cache,
            fork_partial: None,
            duplicate_end: None,
        }
    }

    fn duplicate_middle_eval(input_to_cache: EvalInput, y: TreeIndex, b: TreeIndex) -> Self {
        TreeEvalFrame {
            input_to_cache,
            fork_partial: Some(y),
            duplicate_end: Some(b),
        }
    }

    fn duplciate_end_eval(input_to_cache: EvalInput, xb: TreeIndex) -> Self {
        TreeEvalFrame {
            input_to_cache,
            fork_partial: None,
            duplicate_end: Some(xb),
        }
    }

    fn fork_partial(input_to_cache: EvalInput, v: TreeIndex) -> Self {
        TreeEvalFrame {
            input_to_cache,
            fork_partial: Some(v),
            duplicate_end: None,
        }
    }
}

#[derive(Debug)]
pub struct Trees {
    indexed_trees: HashMap<TreeStoreEntry, TreeIndex, RandomState>,
    cached_applications: HashMap<(TreeIndex, TreeIndex), TreeIndex, RandomState>,
    trees: Vec<TreeStoreEntry>,
    next_free: usize,
    eval_stack: Vec<TreeEvalFrame>,
    iters: usize,
}

impl Trees {
    pub const LEAF: TreeIndex = TreeIndex::from_idx(0);
    pub const STEM_LEAF: TreeIndex = TreeIndex::from_idx(1);
    pub const FORK_LEAF_LEAF: TreeIndex = TreeIndex::from_idx(2);
    pub const IDENTITY: TreeIndex = TreeIndex::from_idx(3);

    pub fn new(tree_capacity: usize, application_cache_size: usize) -> Self {
        let mut trees = Self {
            indexed_trees: HashMap::with_capacity_and_hasher(tree_capacity, Default::default()),
            cached_applications: HashMap::with_capacity_and_hasher(
                application_cache_size,
                Default::default(),
            ),
            next_free: 0,
            trees: Vec::with_capacity(tree_capacity),
            eval_stack: Vec::with_capacity(256),
            iters: 0,
        };

        assert_eq!(trees.insert(Tree::Leaf), Self::LEAF);
        assert_eq!(trees.insert(Tree::Stem(Self::LEAF)), Self::STEM_LEAF);
        assert_eq!(
            trees.insert(Tree::Fork(Self::LEAF, Self::LEAF)),
            Self::FORK_LEAF_LEAF
        );

        // △ (△ △ △) △
        // △ (△ △ △) △ (△) -> △
        // △ (△ △ △) △ (△ x) -> △ x
        // △ (△ △ △) △ (△ x y) -> (△ x) y -> (△ x y)
        assert_eq!(
            trees.insert(Tree::Fork(Self::FORK_LEAF_LEAF, Self::LEAF)),
            Self::IDENTITY
        );

        trees
    }

    pub fn ignore_then(&mut self, then: TreeIndex) -> TreeIndex {
        self.insert(Tree::Fork(Trees::LEAF, then))
    }

    fn get_debug_checked(&self, idx: usize) -> TreeStoreEntry {
        debug_assert!(idx < self.trees.len(), "index out of bounds");
        *unsafe { self.trees.get_unchecked(idx) }
    }

    fn push_and_index(&mut self, tree: Tree) -> TreeIndex {
        let tree = tree.into();
        let idx = TreeIndex::from_idx(self.trees.len());
        if self.next_free == self.trees.len() {
            self.trees.push(tree);
            self.next_free += 1;
        } else {
            let free_list_node = self.get_debug_checked(self.next_free);
            let next_free = free_list_node.get_free();
            debug_assert!(next_free.is_some(), "Free entry not free");
            self.trees[self.next_free] = tree;
            self.next_free = unsafe { next_free.unwrap_unchecked() }.idx();
        }
        self.indexed_trees.insert(tree, idx);
        idx
    }

    pub fn insert(&mut self, tree: Tree) -> TreeIndex {
        match self.indexed_trees.get(&tree.into()) {
            Some(idx) => *idx,
            None => self.push_and_index(tree),
        }
    }

    pub fn index(&self, idx: TreeIndex) -> Tree {
        let stored = self.get_debug_checked(idx.idx());
        let tree_res = stored.try_into();
        debug_assert!(tree_res.is_ok(), "querying free index?");
        unsafe { tree_res.unwrap_unchecked() }
    }

    pub fn index_func(&self, idx: TreeIndex) -> TreeFunc {
        match self.index(idx) {
            Tree::Leaf => TreeFunc::Leaf,
            Tree::Stem(inner) => TreeFunc::Stem(inner),
            Tree::Fork(lhs, rhs) => match self.index(lhs) {
                Tree::Leaf => TreeFunc::IgnoreThen(rhs),
                Tree::Stem(x) => TreeFunc::DoubleChain(x, rhs),
                Tree::Fork(x, y) => TreeFunc::Match(x, y, rhs),
            },
        }
    }

    pub fn insert_func(&mut self, func: TreeFunc) -> TreeIndex {
        match func {
            TreeFunc::Leaf => Self::LEAF,
            TreeFunc::Stem(inner) => self.insert(Tree::Stem(inner)),
            TreeFunc::IgnoreThen(then) => self.ignore_then(then),
            TreeFunc::DoubleChain(x, y) => {
                let stem_x = self.insert(Tree::Stem(x));
                self.insert(Tree::Fork(stem_x, y))
            }
            TreeFunc::Match(x, y, z) => {
                let xy = self.insert(Tree::Fork(x, y));
                self.insert(Tree::Fork(xy, z))
            }
        }
    }

    pub fn count_non_garbage(&self, alive: impl Iterator<Item = TreeIndex>) -> usize {
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

        tree_alive.into_iter().filter(|b| *b).count()
    }

    pub fn parse_stem_nat(&self, start: TreeIndex) -> Result<u64, TreeIndex> {
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

    pub fn parse_fork_nat(&self, start: TreeIndex) -> Result<u64, TreeIndex> {
        let mut tree = start;
        let mut x = 0;
        loop {
            match self.index(tree) {
                Tree::Leaf => return Ok(x),
                Tree::Fork(Trees::LEAF, inner) => {
                    x += 1;
                    debug_assert!(tree > inner);
                    tree = inner;
                }
                _ => return Err(start),
            }
        }
    }

    pub fn tree_apply(&mut self, a: TreeIndex, b: TreeIndex) -> TreeIndex {
        self.iters = 0;
        // println!("tree_apply");
        // println!("    ===== a =====\n    {}", self.as_ref(self.index(a)));
        // println!("    ===== b =====\n    {}", self.as_ref(self.index(b)));
        let idx = self.tree_apply_inner(a, b);
        if self.iters > 100_000 {
            println!("eval took {} steps", self.iters);
        }
        self.cached_applications.clear();
        idx
    }

    fn tree_apply_inner(&mut self, mut a: TreeIndex, mut b: TreeIndex) -> TreeIndex {
        self.eval_stack.clear();

        'apply: loop {
            self.iters += 1;
            // if iters % 100_000 == 0 {
            //     println!("tree_apply_inner {}", iters);
            // }

            if self.iters >= MAX_TREE_APPLY_ITERS {
                let slice_start = self.eval_stack.len().saturating_sub(30);
                for f in &self.eval_stack[slice_start..] {
                    eprintln!("  {:?}", f);
                }
                panic!(
                    "potentially infinite execution (exceeded {} iterations)",
                    self.iters,
                );
            }

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
                        let Some(&result) = self.cached_applications.get(&(a, b)) else {
                            let frame = TreeEvalFrame::duplicate_middle_eval((a, b), y, b);
                            self.eval_stack.push(frame);
                            a = x; // Evaluate `x b`, push `(_ (y b))`.
                            continue;
                        };
                        result
                    }
                    // △ (△ x y) z w => (x | y w.stem | z w.lhs w.rhs)
                    (Tree::Fork(a1, a2), a3) => match self.index(b) {
                        Tree::Leaf => a1,
                        Tree::Stem(u) => {
                            let Some(&result) = self.cached_applications.get(&(a, b)) else {
                                self.eval_stack.push(TreeEvalFrame::just_cache((a, b)));
                                (a, b) = (a2, u); // Evaluate `a2 u`.
                                continue;
                            };
                            result
                        }
                        Tree::Fork(u, v) => {
                            let Some(&result) = self.cached_applications.get(&(a, b)) else {
                                self.eval_stack.push(TreeEvalFrame::fork_partial((a, b), v));
                                (a, b) = (a3, u); // Evaluate `a3 u`, push `_ v`
                                continue;
                            };
                            result
                        }
                    },
                },
            };

            'unwind_stack: loop {
                match self.eval_stack.pop() {
                    None => {
                        // println!("self.eval_stack.len(): {:?}", self.eval_stack.len());
                        return result;
                    }
                    Some(TreeEvalFrame {
                        input_to_cache,
                        fork_partial: Some(v),
                        duplicate_end: None,
                    }) => {
                        self.eval_stack
                            .push(TreeEvalFrame::just_cache(input_to_cache));
                        (a, b) = (result, v); // Complete `(a3 u) v`.
                        continue 'apply;
                    }
                    Some(TreeEvalFrame {
                        input_to_cache,
                        fork_partial: Some(y),
                        duplicate_end: Some(stack_b),
                    }) => {
                        (a, b) = (y, stack_b); // Eval `y b`, push `(_ (y b))`
                        self.eval_stack
                            .push(TreeEvalFrame::duplciate_end_eval(input_to_cache, result));
                        continue 'apply;
                    }
                    Some(TreeEvalFrame {
                        input_to_cache,
                        fork_partial: None,
                        duplicate_end: Some(xb),
                    }) => {
                        self.eval_stack
                            .push(TreeEvalFrame::just_cache(input_to_cache));
                        (a, b) = (xb, result); // Evaluated `y b`, evaluate `x b (y b)`.
                        continue 'apply;
                    }
                    Some(TreeEvalFrame {
                        input_to_cache,
                        fork_partial: None,
                        duplicate_end: None,
                    }) => {
                        // let (a, b) = input_to_cache;
                        // println!(
                        //     "({}) ({}) => {}",
                        //     self.as_ref(self.index(a)),
                        //     self.as_ref(self.index(b)),
                        //     self.as_ref(self.index(result))
                        // );
                        self.cached_applications.insert(input_to_cache, result);
                        continue 'unwind_stack;
                    }
                }
            }
        }
    }

    pub fn as_ref<'a>(&'a self, tree: Tree) -> TreeRef<'a> {
        TreeRef { trees: self, tree }
    }

    pub fn report_final_usage(&self) {
        println!("self.trees.len(): {:?}", self.trees.len());
        println!("self.indexed_trees.len(): {:?}", self.indexed_trees.len());
        println!(
            "self.cached_applications.capacity(): {:?}",
            self.cached_applications.capacity()
        );
        println!(
            "self.eval_stack.capacity(): {:?}",
            self.eval_stack.capacity()
        );
    }

    pub fn total_trees_stored(&self) -> usize {
        self.trees.len()
    }

    pub fn count_nodes(&self, tree: TreeIndex) -> usize {
        let mut trees_stack = Vec::with_capacity(256);
        trees_stack.push(tree);
        let mut total = 0;
        while let Some(tree) = trees_stack.pop() {
            total += 1;
            match self.index(tree) {
                Tree::Leaf => {}
                Tree::Stem(inner) => trees_stack.push(inner),
                Tree::Fork(lhs, rhs) => trees_stack.extend_from_slice(&[lhs, rhs]),
            }
        }
        total
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
