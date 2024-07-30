use libmdbx::Info;

/// Statistics for an individual table in the database.
///
/// A wrapper over MDBX's environment [Stat](libmdbx::Stat).
pub struct TableStat {
    name: &'static str,
    stat: libmdbx::Stat,
}

impl TableStat {
    /// Creates a new TableStat instance
    pub(super) fn new(name: &'static str, stat: libmdbx::Stat) -> Self {
        Self { name, stat }
    }

    /// Size of a database page. This is the same for all databases in the environment.
    #[inline]
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Size of a database page. This is the same for all databases in the environment.
    #[inline]
    pub fn page_size(&self) -> u32 {
        self.stat.page_size()
    }

    /// Depth (height) of the B-tree.
    #[inline]
    pub fn depth(&self) -> u32 {
        self.stat.depth()
    }

    /// Number of internal (non-leaf) pages.
    #[inline]
    pub fn branch_pages(&self) -> usize {
        self.stat.branch_pages()
    }

    /// Number of leaf pages.
    #[inline]
    pub fn leaf_pages(&self) -> usize {
        self.stat.leaf_pages()
    }

    /// Number of overflow pages.
    #[inline]
    pub fn overflow_pages(&self) -> usize {
        self.stat.overflow_pages()
    }

    /// Number of data items.
    #[inline]
    pub fn entries(&self) -> usize {
        self.stat.entries()
    }
}

/// Statistics for the entire MDBX environment.
pub struct Stats {
    /// Statistics for individual tables in the environment
    table_stats: Vec<TableStat>,
    /// Overall environment information
    info: Info,
}

impl Stats {
    /// Creates a new Stats instance
    pub(super) fn new(table_stats: Vec<TableStat>, info: libmdbx::Info) -> Self {
        Self { table_stats, info }
    }

    /// Get statistics for all tables
    pub fn table_stats(&self) -> &[TableStat] {
        &self.table_stats
    }

    /// Get statistics for a specific table
    pub fn table_stat(&self, table_name: &str) -> Option<&TableStat> {
        self.table_stats.iter().find(|s| s.name == table_name)
    }

    /// Get the total number of entries across all tables
    pub fn total_entries(&self) -> usize {
        self.table_stats.iter().map(|stat| stat.entries()).sum()
    }

    /// Get the total number of pages used across all tables
    pub fn total_pages(&self) -> usize {
        self.table_stats
            .iter()
            .map(|stat| stat.branch_pages() + stat.leaf_pages() + stat.overflow_pages())
            .sum()
    }

    /// Get the size of the mapped memory region
    pub fn map_size(&self) -> usize {
        self.info.map_size()
    }

    /// Get the last used page number
    pub fn last_page_number(&self) -> usize {
        self.info.last_pgno()
    }

    /// Get the last transaction ID
    pub fn last_transaction_id(&self) -> usize {
        self.info.last_txnid()
    }

    /// Get the maximum number of reader slots
    pub fn max_readers(&self) -> usize {
        self.info.max_readers()
    }

    /// Get the number of reader slots currently in use
    pub fn current_readers(&self) -> usize {
        self.info.num_readers()
    }
}

impl std::fmt::Debug for TableStat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TableStat")
            .field("name", &self.name)
            .field("page_size", &self.page_size())
            .field("depth", &self.depth())
            .field("branch_pages", &self.branch_pages())
            .field("leaf_pages", &self.leaf_pages())
            .field("overflow_pages", &self.overflow_pages())
            .field("entries", &self.entries())
            .finish()
    }
}

impl std::fmt::Debug for Stats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stats")
            .field("table_stats", &self.table_stats)
            .field("map_size", &self.info.map_size())
            .field("last_page_number", &self.info.last_pgno())
            .field("last_transaction_id", &self.info.last_txnid())
            .field("max_readers", &self.info.max_readers())
            .field("current_readers", &self.info.num_readers())
            .finish()
    }
}
