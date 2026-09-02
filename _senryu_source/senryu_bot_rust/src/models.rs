
#[derive(Clone, Debug)]
pub struct Senryu {
    pub id: i64,
    pub server_id: String,
    pub author_id: String,
    pub kamigo: String,
    pub nakashichi: String,
    pub shimogo: String,
    pub spoiler: bool,
    pub created_at: i64,
}

#[derive(Clone, Debug)]
pub struct RankEntry {
    pub count: i64,
    pub author_id: String,
    pub rank: usize,
}

#[derive(Clone, Debug, Default)]
pub struct ServerStats {
    pub total_senryus: i64,
    pub unique_authors: i64,
}

#[derive(Clone, Debug, Default)]
pub struct DbStats {
    pub senryu_count: i64,
    pub muted_channel_count: i64,
    pub opt_out_count: i64,
    pub connected: bool,
}
